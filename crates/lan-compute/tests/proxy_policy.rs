//! LAN compute must reach nodes directly.
//!
//! Regression test: an ambient `HTTP(S)_PROXY` / `ALL_PROXY` (Clash, corporate
//! proxies, etc.) must never carry node traffic. Proxies routinely swallow
//! private and tailnet addresses (100.x / 192.168.x), which showed up as a
//! node always reporting 离线 in the GUI while `curl --noproxy '*'` worked.
//!
//! This test lives in its own integration-test binary so the process-global
//! proxy environment variables cannot race other tests.

use std::net::SocketAddr;

/// One-shot HTTP server answering `GET /api/v1/health` with `{"status":"ok"}`.
async fn health_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 2048];
            let _ = socket.read(&mut buf).await;
            let body = r#"{"status":"ok","proto_version":1}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    (addr, handle)
}

#[tokio::test(flavor = "multi_thread")]
async fn lan_client_bypasses_ambient_proxy_env() {
    // A dead local proxy: if the client honors these, the request cannot
    // succeed. NO_PROXY deliberately does not cover the server address,
    // matching the real-world failure (tailnet/private IPs, not loopback).
    // Edition 2021: std::env::set_var is a safe fn here.
    std::env::set_var("HTTP_PROXY", "http://127.0.0.1:9");
    std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9");
    std::env::set_var("ALL_PROXY", "socks5://127.0.0.1:9");
    std::env::set_var("NO_PROXY", "example.invalid");
    std::env::set_var("no_proxy", "example.invalid");

    let (addr, handle) = health_server().await;
    let client =
        voice_to_text_lan::client::LanClient::new(addr.ip().to_string(), addr.port(), String::new());
    // A hanging proxy connect must not hang the test; treat it as failure.
    let result = match tokio::time::timeout(std::time::Duration::from_secs(5), client.health()).await
    {
        Ok(result) => result,
        Err(_) => panic!("LAN client request timed out instead of reaching the node directly"),
    };
    // The failure path never connects, so the one-shot server would block on
    // accept() forever; abort instead of awaiting it.
    handle.abort();

    assert!(
        matches!(result, Ok(true)),
        "LAN client must reach the node directly even with an ambient proxy configured; got {result:?}"
    );
}
