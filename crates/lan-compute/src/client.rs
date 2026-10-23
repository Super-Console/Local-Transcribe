//! LAN client: discovery plus task operations against a remote node.
//!
//! The client speaks the `/api/v1` HTTP protocol described in
//! [`crate::protocol`]. Discovery uses mDNS/DNS-SD and never touches the
//! network beyond the local segment.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::protocol::{
    Capabilities, CollectReport, NodeInfo, RemoteModel, StorageInfo, Task, TaskResult,
    UploadProgress, MDNS_SERVICE_TYPE, TXT_COMPUTE, TXT_COMPUTE_ON, TXT_MODEL, TXT_NAME, TXT_ROLE,
    TXT_VER,
};

/// Failure reported by [`LanClient`]. Never carries credentials.
#[derive(Debug, thiserror::Error)]
pub enum LanError {
    #[error("network error: {0}")]
    Network(String),
    #[error("node returned status {status}: {message}")]
    Api { status: u16, message: String },
    #[error("io error: {0}")]
    Io(String),
}

/// Install the bundled rustls (ring) crypto provider exactly once.
///
/// Only relevant with the `tls` feature: reqwest is then built with
/// `rustls-no-provider`, so a provider must be installed before any
/// `reqwest::Client` is constructed. [`LanClient::new`] does this for you.
/// Without the `tls` feature this is a no-op (the LAN protocol is plain
/// HTTP and no TLS backend is compiled in).
pub fn ensure_crypto_provider() {
    #[cfg(feature = "tls")]
    {
        static INSTALL: std::sync::Once = std::sync::Once::new();
        INSTALL.call_once(|| {
            let provider = rustls::crypto::ring::default_provider();
            if provider.install_default().is_err() {
                // Another provider was already installed process-wide; that
                // one will be used instead and is equally fine.
            }
        });
    }
    #[cfg(not(feature = "tls"))]
    {
        // Nothing to install: no TLS backend is compiled into this build.
    }
}

/// Direct HTTP client for a user-selected LAN node.
pub struct LanClient {
    base_url: String,
    http: reqwest::Client,
}

impl std::fmt::Debug for LanClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanClient")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl LanClient {
    /// Build a client for the node at `host:port`.
    ///
    /// LAN node traffic must go direct: ambient `HTTP(S)_PROXY` / `ALL_PROXY`
    /// (Clash, corporate proxies, …) routinely swallow private and tailnet
    /// addresses (100.x / 192.168.x). Nodes are only ever reached on the local
    /// network, so environment proxies are unconditionally bypassed; see the
    /// `proxy_policy` integration test.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        ensure_crypto_provider();
        Self {
            base_url: format!("http://{}:{}", host.into(), port),
            http: reqwest::Client::builder()
                .no_proxy()
                .connect_timeout(std::time::Duration::from_secs(3))
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    /// `GET /api/v1/health`: true when the node answers `status == "ok"`.
    pub async fn health(&self) -> Result<bool, LanError> {
        let body: serde_json::Value = self.get_json("/api/v1/health").await?;
        Ok(body.get("status").and_then(|v| v.as_str()) == Some("ok"))
    }

    /// `GET /api/v1/capabilities`.
    pub async fn capabilities(&self) -> Result<Capabilities, LanError> {
        self.get_json("/api/v1/capabilities").await
    }

    pub async fn models(&self) -> Result<Vec<RemoteModel>, LanError> {
        self.get_json("/api/v1/models").await
    }

    /// `POST /api/v1/tasks`: register a task in `file_transferring` state.
    pub async fn create_task(
        &self,
        file_name: &str,
        file_size: u64,
        chunks_total: u32,
        model_id: Option<&str>,
    ) -> Result<Task, LanError> {
        let body = serde_json::json!({
            "file_name": file_name,
            "file_size": file_size,
            "chunks_total": chunks_total,
            "model_id": model_id,
        });
        let request = self
            .http
            .post(format!("{}/api/v1/tasks", self.base_url))
            .json(&body);
        let response = request.send().await.map_err(network_error)?;
        parse_json(response).await
    }

    /// `PUT /api/v1/tasks/{id}/files/content` in chunks, resuming from the
    /// bytes the node already holds. `progress` is called with
    /// `(sent_bytes, total_bytes)` after every accepted chunk.
    pub async fn upload_file(
        &self,
        task_id: &str,
        path: &Path,
        chunk_bytes: u64,
        progress: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<Task, LanError> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};

        let total = tokio::fs::metadata(path)
            .await
            .map_err(|error| LanError::Io(error.to_string()))?
            .len();
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|error| LanError::Io(error.to_string()))?;
        let chunk_size = chunk_bytes.max(1) as usize;
        let mut buffer = vec![0u8; chunk_size];

        // Resume: start from what the node already received.
        let mut offset = self.file_progress(task_id).await?.received_bytes;
        while offset < total {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|error| LanError::Io(error.to_string()))?;
            let mut filled = 0usize;
            while filled < chunk_size {
                let read = file
                    .read(&mut buffer[filled..])
                    .await
                    .map_err(|error| LanError::Io(error.to_string()))?;
                if read == 0 {
                    break;
                }
                filled += read;
            }
            if filled == 0 {
                break;
            }

            let response = self
                .http
                .put(format!(
                    "{}/api/v1/tasks/{}/files/content",
                    self.base_url, task_id
                ))
                .query(&[("offset", offset)])
                .body(buffer[..filled].to_vec())
                .send()
                .await
                .map_err(network_error)?;
            let status = response.status();
            if status.as_u16() == 409 {
                // The node holds more bytes than we assumed; resync and retry.
                offset = self.file_progress(task_id).await?.received_bytes;
                continue;
            }
            let progress_body: UploadProgress = parse_json(response).await?;
            offset = progress_body.received_bytes;
            progress(offset, total);
        }
        self.task(task_id).await
    }

    /// `GET /api/v1/tasks/{id}`.
    pub async fn task(&self, task_id: &str) -> Result<Task, LanError> {
        self.get_json(&format!("/api/v1/tasks/{task_id}")).await
    }

    /// `GET /api/v1/tasks`, newest first.
    pub async fn tasks(&self) -> Result<Vec<Task>, LanError> {
        self.get_json("/api/v1/tasks").await
    }

    /// `POST /api/v1/tasks/{id}/cancel`.
    pub async fn cancel(&self, task_id: &str) -> Result<Task, LanError> {
        let request = self
            .http
            .post(format!("{}/api/v1/tasks/{}/cancel", self.base_url, task_id));
        let response = request.send().await.map_err(network_error)?;
        parse_json(response).await
    }

    /// `GET /api/v1/tasks/{id}/result`; 409 until the task completed.
    pub async fn result(&self, task_id: &str) -> Result<TaskResult, LanError> {
        self.get_json(&format!("/api/v1/tasks/{task_id}/result"))
            .await
    }

    /// `POST /api/v1/collect`: export finished tasks into `output_dir`.
    pub async fn collect(&self, output_dir: &str) -> Result<CollectReport, LanError> {
        let body = serde_json::json!({ "output_dir": output_dir });
        let request = self
            .http
            .post(format!("{}/api/v1/collect", self.base_url))
            .json(&body);
        let response = request.send().await.map_err(network_error)?;
        parse_json(response).await
    }

    /// `GET /api/v1/storage`.
    pub async fn storage(&self) -> Result<StorageInfo, LanError> {
        self.get_json("/api/v1/storage").await
    }

    /// `GET /api/v1/tasks/{id}/file`: how much of the payload arrived.
    pub async fn file_progress(&self, task_id: &str) -> Result<UploadProgress, LanError> {
        self.get_json(&format!("/api/v1/tasks/{}/file", task_id))
            .await
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, LanError> {
        let request = self.http.get(format!("{}{}", self.base_url, path));
        let response = request.send().await.map_err(network_error)?;
        parse_json(response).await
    }
}

async fn parse_json<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, LanError> {
    let status = response.status();
    if !status.is_success() {
        let message = response.text().await.unwrap_or_default();
        return Err(LanError::Api {
            status: status.as_u16(),
            message: truncate(message, 512),
        });
    }
    response
        .json::<T>()
        .await
        .map_err(|error| LanError::Network(error.to_string()))
}

fn network_error(error: reqwest::Error) -> LanError {
    LanError::Network(error.to_string())
}

fn truncate(text: String, max: usize) -> String {
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max).collect();
    format!("{cut}…")
}

/// Browse `_localtranscribe._tcp.local.` for `timeout` and report every
/// node that answers. Discovery failures degrade to an empty list.
pub fn discover_blocking(timeout: Duration) -> Vec<NodeInfo> {
    let daemon = match mdns_sd::ServiceDaemon::new() {
        Ok(daemon) => daemon,
        Err(error) => {
            eprintln!("[lan-client] mDNS unavailable: {error}");
            return Vec::new();
        }
    };
    let mut found = Vec::new();
    match daemon.browse(MDNS_SERVICE_TYPE) {
        Ok(receiver) => {
            let deadline = std::time::Instant::now() + timeout;
            loop {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match receiver.recv_timeout(remaining) {
                    Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) => {
                        let node = node_info_from_resolved(&info);
                        if !found.iter().any(|known: &NodeInfo| {
                            known.host == node.host && known.port == node.port
                        }) {
                            found.push(node);
                        }
                    }
                    Ok(_) => {}
                    Err(mdns_sd::RecvTimeoutError::Timeout) => break,
                    Err(mdns_sd::RecvTimeoutError::Disconnected) => break,
                }
            }
        }
        Err(error) => eprintln!("[lan-client] cannot browse services: {error}"),
    }
    let _ = daemon.shutdown();
    found
}

/// Async wrapper around [`discover_blocking`] so GUI code can await it.
pub async fn discover(timeout: Duration) -> Vec<NodeInfo> {
    tokio::task::spawn_blocking(move || discover_blocking(timeout))
        .await
        .unwrap_or_default()
}

/// Whether this process may use the local network.
///
/// macOS gates local network access per app; until it is granted, packets to
/// the local segment (mDNS included) fail with `EHOSTUNREACH`/`EPERM`. The
/// probe lets the UI say so instead of silently showing an empty list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalNetworkAccess {
    Granted,
    Denied,
    NoNetwork,
}

/// Classify a failed local-network send.
///
/// `HostUnreachable` is how macOS reports both "permission not granted" and
/// "no route to that host"; the UI only uses this to show a hint, so treating
/// it as denied is the useful reading.
pub fn classify_local_network_error(error: &std::io::Error) -> LocalNetworkAccess {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::PermissionDenied | ErrorKind::HostUnreachable => LocalNetworkAccess::Denied,
        _ => LocalNetworkAccess::NoNetwork,
    }
}

/// Send one throwaway multicast datagram to see whether the local network is
/// reachable from this process. The packet carries no protocol meaning.
pub fn local_network_access() -> LocalNetworkAccess {
    local_network_access_detail().0
}

/// Same probe, with the raw OS error so the troubleshooting UI can say
/// whether this is a permission decision or a network/multicast problem.
pub fn local_network_access_detail() -> (LocalNetworkAccess, Option<i32>) {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
        return (LocalNetworkAccess::NoNetwork, None);
    };
    let _ = socket.set_multicast_ttl_v4(1);
    let _ = socket.set_multicast_loop_v4(false);
    match socket.send_to(&[0u8; 1], (Ipv4Addr::new(224, 0, 0, 251), 5353)) {
        Ok(_) => (LocalNetworkAccess::Granted, None),
        Err(error) => (classify_local_network_error(&error), error.raw_os_error()),
    }
}

/// Port a node listens on unless the user picked another one.
pub const DEFAULT_NODE_PORT: u16 = 7878;

/// Discovery with a TCP fallback.
///
/// mDNS is tried first because it carries the node name, port and model in one
/// answer. Many networks — and some hosts whose network stack blocks multicast
/// for ordinary processes — never deliver those packets, so the remaining time
/// is spent probing the local /24 over TCP. The TCP path only speaks
/// `/api/v1/capabilities`, so it works with any node the protocol supports and
/// needs no platform-specific API.
pub fn discover_with_scan(timeout: Duration, extra_ports: &[u16]) -> Vec<NodeInfo> {
    let started = Instant::now();
    let mut ports = vec![DEFAULT_NODE_PORT];
    for port in extra_ports {
        if !ports.contains(port) {
            ports.push(*port);
        }
    }

    let scanned = match local_scan_hosts() {
        Some(hosts) => scan_blocking(
            &hosts,
            &ports,
            Duration::from_millis(250),
            Duration::from_millis(800),
        ),
        None => Vec::new(),
    };

    let remaining = timeout.saturating_sub(started.elapsed());
    let mdns = if remaining >= Duration::from_millis(300) {
        discover_blocking(remaining)
    } else {
        Vec::new()
    };

    merge_discovered(mdns, scanned)
}

/// mDNS results win; TCP results only fill in hosts mDNS did not report.
pub fn merge_discovered(mdns: Vec<NodeInfo>, scanned: Vec<NodeInfo>) -> Vec<NodeInfo> {
    let mut merged = mdns;
    for node in scanned {
        if !merged
            .iter()
            .any(|known| known.host == node.host && known.port == node.port)
        {
            merged.push(node);
        }
    }
    merged
}

/// Hosts of the local /24, excluding this machine.
///
/// The prefix length is not read here: a UDP `connect` only asks the routing
/// table which source address would be used, which keeps this cross-platform
/// and dependency free. /24 matches every home and small-office LAN the app
/// targets; wider networks simply get a partial scan.
pub fn local_scan_hosts() -> Option<Vec<Ipv4Addr>> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(1, 1, 1, 1), 53)).ok()?;
    let local = match socket.local_addr().ok()? {
        SocketAddr::V4(address) => *address.ip(),
        SocketAddr::V6(_) => return None,
    };
    if !local.is_private() {
        return None;
    }
    let octets = local.octets();
    let mut hosts = Vec::with_capacity(255);
    for last in 1u8..=254 {
        let host = Ipv4Addr::new(octets[0], octets[1], octets[2], last);
        if host != local {
            hosts.push(host);
        }
    }
    Some(hosts)
}

/// Probe `hosts` × `ports` in parallel and keep whoever answers as a node.
pub fn scan_blocking(
    hosts: &[Ipv4Addr],
    ports: &[u16],
    connect_timeout: Duration,
    response_timeout: Duration,
) -> Vec<NodeInfo> {
    let targets: Vec<(Ipv4Addr, u16)> = hosts
        .iter()
        .flat_map(|host| ports.iter().map(move |port| (*host, *port)))
        .collect();
    if targets.is_empty() {
        return Vec::new();
    }

    let workers = targets.len().min(64);
    let queue = std::sync::Mutex::new(targets);
    let found = std::sync::Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let next = queue.lock().ok().and_then(|mut queue| queue.pop());
                let Some((host, port)) = next else { break };
                if let Some(node) = probe_node(host, port, connect_timeout, response_timeout) {
                    if let Ok(mut found) = found.lock() {
                        found.push(node);
                    }
                }
            });
        }
    });

    let mut found = found.into_inner().unwrap_or_default();
    found.sort_by_key(|node| (node.host.clone(), node.port));
    found
}

fn probe_node(
    host: Ipv4Addr,
    port: u16,
    connect_timeout: Duration,
    response_timeout: Duration,
) -> Option<NodeInfo> {
    let address = SocketAddr::from((host, port));
    let mut stream = TcpStream::connect_timeout(&address, connect_timeout).ok()?;
    stream.set_read_timeout(Some(response_timeout)).ok()?;
    stream.set_write_timeout(Some(response_timeout)).ok()?;
    let request = format!(
        "GET /api/v1/capabilities HTTP/1.1\r\nHost: {host}:{port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut response = Vec::new();
    (&mut stream)
        .take(64 * 1024)
        .read_to_end(&mut response)
        .ok()?;
    let text = String::from_utf8_lossy(&response);
    let (head, body) = text.split_once("\r\n\r\n")?;
    if !head.starts_with("HTTP/1.") || !head.lines().next()?.contains(" 200") {
        return None;
    }
    let start = body.find('{')?;
    let json = &body[start..];
    let end = json.rfind('}')?;
    let capabilities: Capabilities = serde_json::from_str(&json[..=end]).ok()?;
    let name = if capabilities.name.trim().is_empty() {
        host.to_string()
    } else {
        capabilities.name
    };
    Some(NodeInfo {
        name,
        host: host.to_string(),
        port,
        proto_version: capabilities.proto_version,
        compute_enabled: capabilities.compute_enabled,
        role: capabilities.role,
        model_id: capabilities.model_id,
    })
}

fn node_info_from_resolved(info: &mdns_sd::ResolvedService) -> NodeInfo {
    let properties = info.get_properties();
    let name = properties
        .get_property_val_str(TXT_NAME)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            info.get_fullname()
                .split('.')
                .next()
                .unwrap_or("unknown")
                .to_owned()
        });
    let proto_version = properties
        .get_property_val_str(TXT_VER)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1);
    let compute_enabled = properties.get_property_val_str(TXT_COMPUTE) == Some(TXT_COMPUTE_ON);
    let role = properties
        .get_property_val_str(TXT_ROLE)
        .unwrap_or("auto")
        .to_owned();
    let model_id = properties
        .get_property_val_str(TXT_MODEL)
        .filter(|value| *value != "none")
        .map(str::to_owned);
    let host = info
        .get_addresses_v4()
        .iter()
        .next()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| info.get_hostname().trim_end_matches('.').to_owned());
    NodeInfo {
        name,
        host,
        port: info.get_port(),
        proto_version,
        compute_enabled,
        role,
        model_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_service_txt_records_map_to_node_info() {
        let mut properties = std::collections::HashMap::new();
        properties.insert(TXT_VER.to_owned(), "1".to_owned());
        properties.insert(TXT_NAME.to_owned(), "study-room".to_owned());
        properties.insert(TXT_COMPUTE.to_owned(), "on".to_owned());
        properties.insert(TXT_ROLE.to_owned(), "gpu".to_owned());
        properties.insert(TXT_MODEL.to_owned(), "sensevoice-small".to_owned());
        let info = mdns_sd::ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            "study-room",
            "study-room.local.",
            "192.0.2.10",
            7878,
            properties,
        )
        .expect("service info");
        let resolved = info.as_resolved_service();
        let node = node_info_from_resolved(&resolved);
        assert_eq!(node.name, "study-room");
        // Use the resolved address, not a second system-resolver lookup of a
        // synthetic hostname (which can collide between two default-named Macs).
        assert_eq!(node.host, "192.0.2.10");
        assert_eq!(node.port, 7878);
        assert_eq!(node.proto_version, 1);
        assert!(node.compute_enabled);
        assert_eq!(node.role, "gpu");
        assert_eq!(node.model_id.as_deref(), Some("sensevoice-small"));
    }
}
