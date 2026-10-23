//! macOS opt-in sharing. No startup listener or auto-start preference.
use std::{net::Ipv4Addr, path::Path, sync::Mutex};

use serde::Serialize;
use voice_to_text_core::LocalModel;
use voice_to_text_lan::node::{NodeConfig, NodeServer, RunningNode};

#[derive(Clone, Serialize)]
pub struct LanInterface {
    pub name: String,
    pub address: Ipv4Addr,
    pub mask: Ipv4Addr,
}

#[derive(Clone, Serialize)]
pub struct HostStatus {
    pub supported: bool,
    pub running: bool,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub model: String,
}

impl Default for HostStatus {
    fn default() -> Self {
        Self {
            supported: cfg!(target_os = "macos"),
            running: false,
            name: String::new(),
            address: String::new(),
            port: 0,
            model: String::new(),
        }
    }
}

struct HostedNode {
    node: RunningNode,
    status: HostStatus,
    token: String,
}

#[derive(Default)]
pub struct HostState(Mutex<Option<HostedNode>>);

impl HostState {
    pub fn status(&self) -> Result<HostStatus, String> {
        Ok(self
            .0
            .lock()
            .map_err(|_| "Node state unavailable.")?
            .as_ref()
            .map(|host| host.status.clone())
            .unwrap_or_default())
    }

    pub fn token(&self) -> Result<String, String> {
        self.0
            .lock()
            .map_err(|_| "Node state unavailable.")?
            .as_ref()
            .map(|host| host.token.clone())
            .ok_or_else(|| "Start sharing first.".into())
    }

    pub fn start(
        &self,
        root: &Path,
        name: String,
        address: String,
        port: u16,
        model: LocalModel,
    ) -> Result<HostStatus, String> {
        let mut current = self.0.lock().map_err(|_| "Node state unavailable.")?;
        if current.is_some() {
            return Err("Sharing is already running. Stop it before changing settings.".into());
        }
        if name.trim().is_empty() || name.len() > 80 || port == 0 {
            return Err("Enter a node name (1–80 bytes) and a port (1–65535).".into());
        }
        let interface = interfaces()?
            .into_iter()
            .find(|item| item.address.to_string() == address)
            .ok_or("Choose a currently connected private LAN interface.")?;
        let data_dir = root.join("lan-host");
        std::fs::create_dir_all(&data_dir).map_err(|_| "Cannot prepare node storage.")?;
        let token = pairing_token(&data_dir)?;
        let node = NodeServer::start_on_lan(
            NodeConfig {
                name: name.trim().into(),
                data_dir,
                listen: (interface.address, port).into(),
                token: token.clone(),
                compute_enabled: true,
                role: "cpu".into(),
                model_dir: Some(model.directory.clone()),
                media_root: None,
            },
            interface.mask,
        )
        .map_err(|error| error.to_string())?;
        let status = HostStatus {
            supported: true,
            running: true,
            name: name.trim().into(),
            address,
            port: node.port(),
            model: model.display_name,
        };
        *current = Some(HostedNode {
            node,
            status: status.clone(),
            token,
        });
        Ok(status)
    }

    pub fn stop(&self) -> Result<HostStatus, String> {
        // Keep the lock until the socket and worker have stopped; no racing restart.
        let mut current = self.0.lock().map_err(|_| "Node state unavailable.")?;
        if let Some(host) = current.take() {
            host.node.shutdown();
            host.node.wait();
        }
        Ok(HostStatus::default())
    }
}

impl Drop for HostState {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(target_os = "macos")]
fn pairing_token(directory: &Path) -> Result<String, String> {
    use std::{
        io::Write,
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
    };
    let path = directory.join("pairing-token");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
    {
        Ok(mut file) => {
            let token = format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            );
            file.write_all(token.as_bytes())
                .map_err(|_| "Cannot save pairing token.")?;
            file.sync_all().map_err(|_| "Cannot save pairing token.")?;
            Ok(token)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|_| "Cannot inspect pairing token.")?;
            if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
                return Err("Pairing token must be an owner-only regular file.".into());
            }
            let token = std::fs::read_to_string(path).map_err(|_| "Cannot read pairing token.")?;
            if token.len() != 64 || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("Invalid saved pairing token.".into());
            }
            Ok(token)
        }
        Err(_) => Err("Cannot create pairing token.".into()),
    }
}

#[cfg(not(target_os = "macos"))]
fn pairing_token(_directory: &Path) -> Result<String, String> {
    Err("Desktop sharing is currently available on macOS only.".into())
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
                // en*: physical Ethernet / Wi-Fi, not VPN/tunnel/bridge listeners.
                if name.starts_with("en") && address.is_private() {
                    result.push(LanInterface {
                        name,
                        address,
                        mask,
                    });
                }
            }
            cursor = entry.ifa_next;
        }
        libc::freeifaddrs(head);
        Ok(result)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn interfaces() -> Result<Vec<LanInterface>, String> {
    Ok(Vec::new())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn sharing_is_off_by_default_and_stop_is_idempotent() {
        let state = HostState::default();
        assert!(!state.status().unwrap().running);
        assert!(state.token().is_err());
        assert!(!state.stop().unwrap().running);
        assert!(!state.stop().unwrap().running);
    }

    #[test]
    fn token_is_private_and_stable_across_restarts() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("lan-token-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let first = pairing_token(&root).unwrap();
        assert_eq!(first.len(), 64);
        assert_eq!(first, pairing_token(&root).unwrap());
        let metadata = std::fs::metadata(root.join("pairing-token")).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        std::fs::set_permissions(
            root.join("pairing-token"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(pairing_token(&root).is_err());
    }

    #[test]
    #[ignore = "opens a short-lived listener on a real Mac LAN interface; no real inference"]
    fn mac_host_start_auth_stop_restart_and_port_conflict() {
        use voice_to_text_lan::client::LanClient;
        let interface = interfaces()
            .unwrap()
            .into_iter()
            .next()
            .expect("a private Wi-Fi/Ethernet interface");
        let root = std::env::temp_dir().join(format!("lan-host-{}", uuid::Uuid::new_v4()));
        let model_dir = root.join("paraformer-fixture");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(
            model_dir.join("model.onnx"),
            b"layout-only fixture, not a real model",
        )
        .unwrap();
        std::fs::write(model_dir.join("tokens.txt"), b"a 1\n").unwrap();
        let model = voice_to_text_core::inspect_model_directory(&model_dir).unwrap();
        let port = {
            let reservation = std::net::TcpListener::bind((interface.address, 0)).unwrap();
            reservation.local_addr().unwrap().port()
        };
        let state = HostState::default();
        assert!(
            state
                .start(&root, "test".into(), "0.0.0.0".into(), port, model.clone())
                .is_err()
        );
        let status = state
            .start(
                &root,
                "test".into(),
                interface.address.to_string(),
                port,
                model.clone(),
            )
            .unwrap();
        assert!(status.running);
        std::net::TcpStream::connect_timeout(
            &(interface.address, port).into(),
            std::time::Duration::from_secs(3),
        )
        .expect("the newly bound private-address socket must accept a TCP connection");
        let token = state.token().unwrap();
        assert!(
            state
                .start(
                    &root,
                    "test".into(),
                    interface.address.to_string(),
                    port,
                    model.clone()
                )
                .is_err()
        );
        let client = LanClient::new(interface.address.to_string(), port, token.clone());
        tauri::async_runtime::block_on(async {
            assert!(client.health().await.unwrap());
            assert!(client.capabilities().await.unwrap().compute_enabled);
            assert!(
                LanClient::new(interface.address.to_string(), port, "wrong".into())
                    .capabilities()
                    .await
                    .is_err()
            );
        });
        assert!(!state.stop().unwrap().running);
        assert!(
            std::net::TcpStream::connect_timeout(
                &(interface.address, port).into(),
                std::time::Duration::from_secs(1)
            )
            .is_err()
        );
        let occupied = std::net::TcpListener::bind((interface.address, port)).unwrap();
        assert!(
            state
                .start(
                    &root,
                    "test".into(),
                    interface.address.to_string(),
                    port,
                    model.clone()
                )
                .is_err()
        );
        assert!(!state.status().unwrap().running);
        drop(occupied);
        assert!(
            state
                .start(
                    &root,
                    "test".into(),
                    interface.address.to_string(),
                    port,
                    model
                )
                .unwrap()
                .running
        );
        assert_eq!(state.token().unwrap(), token);
        state.stop().unwrap();
    }

    #[test]
    #[ignore = "real Mac LAN interface, installed model and approved local media fixture required"]
    fn mac_host_transcribes_an_upload_with_a_real_model() {
        use voice_to_text_lan::{client::LanClient, protocol::TaskState};
        let interface = interfaces()
            .unwrap()
            .into_iter()
            .next()
            .expect("a private LAN interface");
        let model = voice_to_text_core::inspect_model_directory(
            std::env::var_os("VOICE_TO_TEXT_TEST_MODEL_DIR").expect("model fixture"),
        )
        .unwrap();
        let media = std::path::PathBuf::from(
            std::env::var_os("VOICE_TO_TEXT_TEST_MEDIA").expect("media fixture"),
        );
        let original = std::fs::read(&media).unwrap();
        let root = std::env::temp_dir().join(format!("lan-host-real-{}", uuid::Uuid::new_v4()));
        let port = std::net::TcpListener::bind((interface.address, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let state = HostState::default();
        state
            .start(
                &root,
                "local-validation".into(),
                interface.address.to_string(),
                port,
                model,
            )
            .unwrap();
        let client = LanClient::new(interface.address.to_string(), port, state.token().unwrap());
        tauri::async_runtime::block_on(async {
            let task = client
                .create_task("fixture.wav", original.len() as u64, 1, None)
                .await
                .unwrap();
            client
                .upload_file(&task.id, &media, 4 * 1024 * 1024, |_, _| {})
                .await
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            loop {
                let current = client.task(&task.id).await.unwrap();
                if current.state.is_terminal() {
                    assert_eq!(current.state, TaskState::Completed);
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "transcription timed out"
                );
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            let result = client.result(&task.id).await.unwrap();
            assert!(!result.transcript.trim().is_empty());
            assert!(result.duration_ms > 0);
        });
        assert_eq!(
            original,
            std::fs::read(media).unwrap(),
            "source media stays unchanged"
        );
        state.stop().unwrap();
        assert!(!state.status().unwrap().running);
    }
}
