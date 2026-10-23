//! macOS opt-in sharing. The GUI supervises a `local-transcribe-node` child
//! process; it never hosts the node in-process.
use std::{net::Ipv4Addr, path::Path, sync::Mutex};

use serde::Serialize;
use voice_to_text_lan::node::SubnetFilter;

use crate::node_supervisor::NodeSupervisor;

#[derive(Debug, Clone, Serialize)]
pub struct LanInterface {
    pub name: String,
    pub address: Ipv4Addr,
    pub mask: Ipv4Addr,
    /// `lan`, `vpn`, `tailscale` or `other`; shown next to the address so the
    /// user can tell a Wi-Fi/Ethernet interface from a tunnel.
    pub kind: String,
}

#[derive(Clone, Serialize)]
pub struct HostStatus {
    pub supported: bool,
    pub running: bool,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub model: String,
    /// Mask actually in effect, e.g. `255.255.255.0` or `255.255.255.255`.
    pub mask: String,
    /// `same_subnet` (/1–/30) or `none` (/31, /32, 0.0.0.0).
    pub subnet_filter: String,
    /// Non-fatal notice, e.g. the saved interface disappeared at launch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

// 手动实现是有意的：`supported` 在 macOS 上必须为 true，而 `#[derive(Default)]`
// 会把所有平台都置为 false。Windows 目标上 clippy 会把 cfg! 求值成 false 后
// 误判为可派生（tauri CI 的 -D warnings 会拦），故显式豁免该 lint。
#[allow(clippy::derivable_impls)]
impl Default for HostStatus {
    fn default() -> Self {
        Self {
            supported: cfg!(target_os = "macos"),
            running: false,
            name: String::new(),
            address: String::new(),
            port: 0,
            model: String::new(),
            mask: String::new(),
            subnet_filter: SubnetFilter::None.as_str().to_owned(),
            notice: None,
        }
    }
}

#[derive(Default)]
pub struct HostState {
    supervisor: Mutex<Option<NodeSupervisor>>,
    notice: Mutex<Option<String>>,
}

impl HostState {
    pub fn status(&self) -> Result<HostStatus, String> {
        let mut guard = self
            .supervisor
            .lock()
            .map_err(|_| "Node state unavailable.")?;
        let mut status = match guard.as_mut() {
            Some(supervisor) => supervisor.status()?,
            None => HostStatus::default(),
        };
        status.notice = self.notice.lock().ok().and_then(|notice| notice.clone());
        Ok(status)
    }

    pub fn set_notice(&self, notice: Option<String>) {
        if let Ok(mut current) = self.notice.lock() {
            *current = notice;
        }
    }

    /// Validate settings, then spawn the sidecar. Models are scanned by the
    /// child itself; the GUI no longer assembles a model list.
    pub fn start(
        &self,
        root: &Path,
        log_dir: &Path,
        name: String,
        address: String,
        port: u16,
        idle_unload: u64,
    ) -> Result<HostStatus, String> {
        let mut guard = self
            .supervisor
            .lock()
            .map_err(|_| "Node state unavailable.")?;
        if guard.is_some() {
            return Err("Sharing is already running. Stop it before changing settings.".into());
        }
        if name.trim().is_empty() || name.len() > 80 || port == 0 {
            return Err("Enter a node name (1–80 bytes) and a port (1–65535).".into());
        }
        let mask = resolve_mask(&address)?;
        let mut supervisor = NodeSupervisor::start(
            root,
            log_dir,
            name.trim(),
            &address,
            port,
            mask,
            idle_unload,
        )?;
        let status = supervisor.status()?;
        *guard = Some(supervisor);
        self.set_notice(None);
        Ok(status)
    }

    pub fn stop(&self) -> Result<HostStatus, String> {
        let mut guard = self
            .supervisor
            .lock()
            .map_err(|_| "Node state unavailable.")?;
        if let Some(supervisor) = guard.as_mut() {
            supervisor.stop()?;
        }
        *guard = None;
        Ok(HostStatus::default())
    }
}

impl Drop for HostState {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// The mask comes from the selected interface. `0.0.0.0` is the explicit
/// "all interfaces" choice and carries no scope; anything else must match a
/// connected interface, so a vanished adapter can never be silently replaced.
fn resolve_mask(address: &str) -> Result<Ipv4Addr, String> {
    if address == "0.0.0.0" {
        return Ok(Ipv4Addr::UNSPECIFIED);
    }
    // Test-only: loopback never appears in `interfaces()` and cannot be picked
    // in the UI, but supervisor tests bind it to avoid touching the LAN.
    if address == "127.0.0.1" {
        return Ok(Ipv4Addr::new(255, 255, 255, 255));
    }
    interfaces()?
        .into_iter()
        .find(|item| item.address.to_string() == address)
        .map(|item| item.mask)
        .ok_or_else(|| "Choose a currently connected network interface.".to_owned())
}

/// Classify one IPv4 interface. Link-local and loopback addresses are not
/// usable for sharing and are dropped here.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn classify(name: &str, address: Ipv4Addr) -> Option<&'static str> {
    if address.is_loopback() || address.is_unspecified() || address.is_link_local() {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("utun") && is_tailscale_cgnat(address) {
        return Some("tailscale");
    }
    if lower.starts_with("utun")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("ppp")
        || lower.starts_with("ipsec")
    {
        return Some("vpn");
    }
    if lower.starts_with("en") || lower.starts_with("eth") {
        return Some("lan");
    }
    Some("other")
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn is_tailscale_cgnat(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

#[cfg(target_os = "macos")]
pub fn interfaces() -> Result<Vec<LanInterface>, String> {
    use std::{ffi::CStr, ptr};
    let mut head = ptr::null_mut();
    // SAFETY: getifaddrs owns a linked list until freeifaddrs; all pointers are
    // checked and sockaddr_in is read only for AF_INET entries.
    unsafe {
        if libc::getifaddrs(&mut head) != 0 {
            return Err("Cannot read local network interfaces.".into());
        }
        let mut cursor = head;
        let mut result = Vec::new();
        while !cursor.is_null() {
            let entry = &*cursor;
            if !entry.ifa_addr.is_null()
                && !entry.ifa_netmask.is_null()
                && !entry.ifa_name.is_null()
                && (*entry.ifa_addr).sa_family as i32 == libc::AF_INET
                && entry.ifa_flags & libc::IFF_UP as u32 != 0
                && entry.ifa_flags & libc::IFF_LOOPBACK as u32 == 0
            {
                let name = CStr::from_ptr(entry.ifa_name)
                    .to_string_lossy()
                    .into_owned();
                let address = Ipv4Addr::from(
                    (*(entry.ifa_addr as *const libc::sockaddr_in))
                        .sin_addr
                        .s_addr
                        .to_ne_bytes(),
                );
                let mask = Ipv4Addr::from(
                    (*(entry.ifa_netmask as *const libc::sockaddr_in))
                        .sin_addr
                        .s_addr
                        .to_ne_bytes(),
                );
                if let Some(kind) = classify(&name, address) {
                    result.push(LanInterface {
                        name,
                        address,
                        mask,
                        kind: kind.to_owned(),
                    });
                }
            }
            cursor = entry.ifa_next;
        }
        libc::freeifaddrs(head);
        result.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(result)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn interfaces() -> Result<Vec<LanInterface>, String> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::{HostStatus, classify, resolve_mask};

    #[test]
    fn tailscale_and_lan_interfaces_are_listed_and_loopback_is_not() {
        assert_eq!(
            classify("utun4", "100.64.0.1".parse().unwrap()),
            Some("tailscale")
        );
        assert_eq!(
            classify("en0", "192.168.1.20".parse().unwrap()),
            Some("lan")
        );
        assert_eq!(classify("utun0", "10.8.0.2".parse().unwrap()), Some("vpn"));
        assert_eq!(classify("lo0", "127.0.0.1".parse().unwrap()), None);
        assert_eq!(classify("en5", "169.254.10.2".parse().unwrap()), None);
    }

    #[test]
    fn status_defaults_to_stopped_without_filtering_text() {
        let status = HostStatus::default();
        assert!(!status.running);
        assert_eq!(status.subnet_filter, "none");
    }

    #[test]
    fn wildcard_address_needs_no_interface() {
        assert_eq!(resolve_mask("0.0.0.0").unwrap().to_string(), "0.0.0.0");
    }
}
