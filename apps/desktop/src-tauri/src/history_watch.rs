//! 监视转写结果目录，让界面在 CLI/MCP 写完结果后自动刷新。
//!
//! 应用自己发起的任务会通过 `JobStatus` 事件更新界面；但 CLI 与 MCP 是独立
//! 进程，它们把结果直接写进 `transcripts/`。这里用一次轻量的目录签名轮询
//! （文件名数量 + 最大 mtime + 总大小）代替文件系统通知库，避免引入新依赖，
//! 也避免在 macOS 上额外占用 FSEvents 句柄。

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};

/// 历史列表发生变化时向前端广播的事件名。
pub const HISTORY_EVENT: &str = "local-transcribe:history-changed";

/// 轮询间隔。1.5 秒足够让“用 CLI 转写完再看界面”感觉是即时的。
const POLL_INTERVAL: Duration = Duration::from_millis(1500);

/// 目录签名：`(最新修改时间纳秒, 文件数, 总字节数)`。
pub fn signature(dir: &Path) -> Option<(u128, usize, u64)> {
    let entries = fs::read_dir(dir).ok()?;
    let mut newest: u128 = 0;
    let mut count = 0usize;
    let mut bytes = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        count += 1;
        bytes += metadata.len();
        if let Ok(modified) = metadata.modified()
            && let Ok(since) = modified.duration_since(UNIX_EPOCH)
        {
            newest = newest.max(since.as_nanos());
        }
    }
    Some((newest, count, bytes))
}

/// 启动后台轮询；目录首次出现也算一次变化，界面会自己重读。
pub fn spawn(app: AppHandle, dir: PathBuf) {
    let spawned = thread::Builder::new()
        .name("history-watch".to_owned())
        .spawn(move || {
            let mut last = signature(&dir);
            loop {
                thread::sleep(POLL_INTERVAL);
                let next = signature(&dir);
                if next != last {
                    last = next;
                    let _ = app.emit(HISTORY_EVENT, ());
                }
            }
        });
    if let Err(error) = spawned {
        eprintln!("could not start the history watcher: {error}");
    }
}

/// 测试辅助：写入一个占位结果文件。
#[cfg(test)]
fn touch(dir: &Path, name: &str, contents: &[u8]) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(name), contents).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lt-history-watch-{}-{}-{name}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_missing_directory_has_no_signature() {
        assert!(signature(&scratch("missing").join("nope")).is_none());
    }

    #[test]
    fn a_new_result_changes_the_signature() {
        let dir = scratch("new-result");
        let empty = signature(&dir).expect("signature");
        touch(&dir, "job-1.json", b"{}");
        let with_one = signature(&dir).expect("signature");
        assert_ne!(empty, with_one, "writing a result must be visible");
        assert_eq!(with_one.1, 1);
    }

    #[test]
    fn rewriting_an_existing_result_changes_the_signature() {
        let dir = scratch("rewrite");
        touch(&dir, "job-1.json", b"{}");
        let first = signature(&dir).expect("signature");
        touch(&dir, "job-1.json", b"{\"text\":\"longer\"}");
        let second = signature(&dir).expect("signature");
        assert_ne!(first, second, "a longer result must be visible");
        assert_eq!(second.1, 1, "still one file");
    }

    #[test]
    fn non_json_files_are_ignored() {
        let dir = scratch("ignore");
        touch(&dir, "notes.txt", b"hello");
        let only_txt = signature(&dir).expect("signature");
        assert_eq!(only_txt.1, 0);
        touch(&dir, "job.json", b"{}");
        let with_json = signature(&dir).expect("signature");
        assert_ne!(only_txt, with_json);
    }
}
