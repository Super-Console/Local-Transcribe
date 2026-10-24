//! 任务书 §8 测试 5：两个进程同时改同一份 LAN 设置，两条节点都要落盘。
//!
//! 子进程必须用**节点二进制**（`CARGO_BIN_EXE_local-transcribe-node`）：
//! 它的 `main` 会先调用 `settings_test_helper()`，写完一次就退出。
//! 不能用 `current_exe()`——在测试里那是测试二进制本身，子进程会重跑整个
//! 测试套件、再各自起两个子进程，形成指数级自我复制。

use std::path::PathBuf;
use std::process::{Command, Stdio};

use voice_to_text_lan::settings::{find_node, read, write, LanSettings, SETTINGS_FILE};

fn scratch() -> (PathBuf, PathBuf) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "lt-lan-settings-concurrency-{now}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(SETTINGS_FILE);
    (dir, path)
}

#[test]
fn two_processes_editing_different_nodes_lose_nothing() {
    let (dir, path) = scratch();
    write(&path, &LanSettings::default()).unwrap();

    let node_binary = env!("CARGO_BIN_EXE_local-transcribe-node");
    let mut children = Vec::new();
    for index in 0..2 {
        // 先 spawn 再读 stderr：这里要并发，不能等一个读完再起下一个。
        let mut child = Command::new(node_binary)
            .env("VOICE_TO_TEXT_LAN_SETTINGS_HOLDER", &dir)
            .env("VOICE_TO_TEXT_LAN_SETTINGS_INDEX", index.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut text = String::new();
        use std::io::Read as _;
        let _ = child.stderr.take().unwrap().read_to_string(&mut text);
        children.push((index, child, text));
    }
    for (index, mut child, stderr) in children {
        let status = child.wait().unwrap();
        assert!(
            status.success(),
            "helper {index} failed: {status}\n{stderr}"
        );
    }

    let saved = read(&path);
    let names: Vec<_> = saved.nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(names.len(), 2, "both nodes must survive, got {names:?}");
    let first = find_node(&saved, "child-0").expect("child-0 was saved");
    let second = find_node(&saved, "child-1").expect("child-1 was saved");
    assert_ne!(first.id, second.id, "each node gets its own id");
    assert!(!first.id.is_empty() && !second.id.is_empty());

    let _ = std::fs::remove_dir_all(dir);
}
