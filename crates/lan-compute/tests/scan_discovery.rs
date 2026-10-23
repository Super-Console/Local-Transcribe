//! TCP fallback discovery: locate nodes without mDNS.
//!
//! Some networks (and some macOS network stacks) block multicast for ordinary
//! processes while TCP still works. Discovery therefore has a second, purely
//! cross-platform path: probe candidate hosts over TCP and confirm each
//! responder through `GET /api/v1/capabilities`.
use std::net::Ipv4Addr;
use std::time::Duration;

use voice_to_text_lan::client::{merge_discovered, scan_blocking};
use voice_to_text_lan::node::{NodeConfig, NodeServer};
use voice_to_text_lan::protocol::NodeInfo;

fn node_info(name: &str, host: &str, port: u16) -> NodeInfo {
    NodeInfo {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
        proto_version: 1,
        compute_enabled: true,
        role: "cpu".to_owned(),
        model_id: None,
    }
}

fn start_scan_fixture(name: &str) -> (NodeServerFixture, u16) {
    let root = std::env::temp_dir().join(format!("scan-discovery-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let node = NodeServer::start(NodeConfig {
        name: name.to_owned(),
        listen: "127.0.0.1:0".parse().unwrap(),
        data_dir: root.join("tasks"),
        compute_enabled: false,
        ..NodeConfig::default()
    })
    .expect("fixture node must start");
    let port = node.port();
    (NodeServerFixture { node }, port)
}

struct NodeServerFixture {
    node: voice_to_text_lan::node::RunningNode,
}

impl Drop for NodeServerFixture {
    fn drop(&mut self) {
        self.node.shutdown();
        self.node.wait();
    }
}

#[test]
fn tcp_scan_finds_node_by_capabilities_and_reports_its_name() {
    let (_fixture, port) = start_scan_fixture("scan-node");
    let found = scan_blocking(
        &[Ipv4Addr::LOCALHOST],
        &[port],
        Duration::from_millis(300),
        Duration::from_secs(3),
    );

    let node = found
        .iter()
        .find(|node| node.port == port)
        .expect("TCP scan must find the node listening on loopback");
    assert_eq!(
        node.name, "scan-node",
        "capabilities must expose the display name"
    );
    assert_eq!(node.host, "127.0.0.1");
    assert_eq!(node.proto_version, 1);
}

#[test]
fn tcp_scan_ignores_ports_that_are_not_nodes() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let found = scan_blocking(
        &[Ipv4Addr::LOCALHOST],
        &[port],
        Duration::from_millis(300),
        Duration::from_millis(400),
    );
    assert!(
        found.is_empty(),
        "a bare listener must not be reported as a node"
    );
}

#[test]
fn merge_prefers_mdns_results_and_dedupes_by_host_and_port() {
    let mdns = vec![node_info("from-mdns", "192.168.1.243", 7878)];
    let scanned = vec![
        node_info("from-scan", "192.168.1.243", 7878),
        node_info("from-scan", "192.168.1.184", 7878),
    ];

    let merged = merge_discovered(mdns, scanned);

    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].name, "from-mdns");
    assert_eq!(merged[1].host, "192.168.1.184");
}

#[test]
fn blocked_local_network_errors_are_classified_as_denied() {
    use std::io;
    use voice_to_text_lan::client::{classify_local_network_error, LocalNetworkAccess};

    let denied = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
    assert_eq!(
        classify_local_network_error(&denied),
        LocalNetworkAccess::Denied
    );

    // macOS reports EHOSTUNREACH while local network access is not granted.
    let unreachable = io::Error::new(io::ErrorKind::HostUnreachable, "no route");
    assert_eq!(
        classify_local_network_error(&unreachable),
        LocalNetworkAccess::Denied
    );

    let no_network = io::Error::new(io::ErrorKind::NetworkUnreachable, "no interface");
    assert_eq!(
        classify_local_network_error(&no_network),
        LocalNetworkAccess::NoNetwork
    );
}
