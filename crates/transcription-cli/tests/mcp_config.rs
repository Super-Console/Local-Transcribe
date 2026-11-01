//! `local-transcribe mcp-config` 的输出契约：stdout 必须是可直接粘贴的合法
//! JSON。服务按请求读取调用者给出的绝对路径，因此配置里不再携带目录参数。

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn scratch() -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("lt-mcp-config-{}-{now}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(dir: &Path) -> (Value, String) {
    let fake_mcp = dir.join(if cfg!(windows) {
        "voice-to-text-mcp.exe"
    } else {
        "voice-to-text-mcp"
    });
    std::fs::write(&fake_mcp, b"#!/bin/sh\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_local-transcribe"))
        .arg("mcp-config")
        .env("LOCAL_TRANSCRIBE_MCP", &fake_mcp)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "mcp-config must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Value = serde_json::from_str(&stdout).expect("stdout must be pure JSON");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        !stderr.contains("--allow-root"),
        "the note must not mention a directory allow-list"
    );
    (parsed, fake_mcp.to_string_lossy().into_owned())
}

#[test]
fn mcp_config_prints_pasteable_json_without_arguments() {
    let dir = scratch();
    let (parsed, command) = run(&dir);
    let server = &parsed["mcpServers"]["local-transcribe"];
    assert_eq!(server["command"].as_str().unwrap(), command);
    assert_eq!(server["args"].as_array().unwrap().len(), 0);
}
