//! 任务书 §8 测试 12 的端到端部分：`lan stop` 读到不存在的 pid 时
//! 退出码 1、stdout 只有 JSON、stderr 不打印东西。
//!
//! 不拉起真实服务：在数据目录里放一个 Unix socket，按服务协议应答一次，
//! 这样断言的是 CLI 自己的连接、输出与退出码行为。

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::Command;

use voice_to_text_service::{Request, SOCKET_FILE};

fn scratch() -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // 放在 /tmp 下：Unix socket 的 sun_path 上限是 104 字节，
    // /var/folders/... 的临时目录拼上 service.sock 会超长。
    let dir = PathBuf::from("/tmp").join(format!("lt-cli-exit-{}-{now}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn lan_stop_reports_not_running_with_exit_code_one_and_json_on_stdout() {
    let dir = scratch();
    // macOS 的 sun_path 上限 104 字节；temp dir 拼上 service.sock 后仍足够短。
    let listener = UnixListener::bind(dir.join(SOCKET_FILE)).unwrap();

    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let request: Request = serde_json::from_str(line.trim()).unwrap();
        assert!(matches!(request, Request::LanStopLocal));
        let mut stream = stream;
        let payload = serde_json::json!({
            "ok": false,
            "error": { "code": "not_running", "message": "节点未在运行" },
        });
        writeln!(stream, "{payload}").unwrap();
        stream.flush().unwrap();
    });

    let output = Command::new(env!("CARGO_BIN_EXE_local-transcribe"))
        .arg("lan")
        .arg("stop")
        .arg("--data-dir")
        .arg(&dir)
        .output()
        .unwrap();
    server.join().unwrap();

    assert_eq!(output.status.code(), Some(1), "a failed LAN stop exits 1");
    assert!(
        output.stderr.is_empty(),
        "stderr must stay empty: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let payload: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout is not JSON ({error}): {stdout}"));
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["code"], "not_running");
    assert_eq!(payload["error"]["message"], "节点未在运行");

    let _ = std::fs::remove_dir_all(dir);
}
