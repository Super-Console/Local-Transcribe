//! Optional cleanup for media received by the LAN node.
//!
//! Only terminal tasks are touched, deletions go to the macOS Trash, and the
//! default is off. Other platforms report "unsupported" so the UI can disable
//! the switch instead of silently using `rm`.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use serde::Deserialize;

use crate::protocol::Task;
use voice_to_text_core::move_to_trash;

const GIB: u64 = 1_073_741_824;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanRetentionPolicy {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_age")]
    pub age_days: u64,
    #[serde(default = "default_bytes")]
    pub max_bytes: u64,
}

fn default_mode() -> String {
    "off".to_owned()
}

fn default_age() -> u64 {
    7
}

fn default_bytes() -> u64 {
    GIB
}

impl Default for LanRetentionPolicy {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            age_days: default_age(),
            max_bytes: default_bytes(),
        }
    }
}

pub fn read_policy(path: &Path) -> Option<LanRetentionPolicy> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

struct Candidate {
    json: PathBuf,
    directory: PathBuf,
    bytes: u64,
    modified: SystemTime,
}

/// Remove terminal task media according to `policy`. Returns how many task
/// payloads were moved to the Trash.
pub fn apply(data_dir: &Path, policy: &LanRetentionPolicy) -> Result<usize, String> {
    if policy.mode == "off" {
        return Ok(0);
    }
    if !cfg!(target_os = "macos") {
        return Err("Automatic node cleanup is only implemented on macOS.".to_owned());
    }
    let tasks_dir = data_dir.join("tasks");
    let Ok(entries) = fs::read_dir(&tasks_dir) else {
        return Ok(0);
    };
    let now = SystemTime::now();
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(task) = serde_json::from_slice::<Task>(&bytes) else {
            continue;
        };
        if !task.state.is_terminal() {
            continue;
        }
        let directory = tasks_dir.join(&task.id);
        let payload = directory.join("payload.bin");
        let bytes = fs::metadata(&payload).map(|meta| meta.len()).unwrap_or(0);
        let modified = fs::metadata(&payload)
            .and_then(|meta| meta.modified())
            .or_else(|_| fs::metadata(&directory).and_then(|meta| meta.modified()))
            .unwrap_or(now);
        candidates.push(Candidate {
            json: path,
            directory,
            bytes,
            modified,
        });
    }

    let mut removed = 0usize;
    match policy.mode.as_str() {
        "age" => {
            let limit = Duration::from_secs(policy.age_days.max(1) * 24 * 3600);
            for candidate in candidates {
                if now.duration_since(candidate.modified).unwrap_or_default() >= limit {
                    trash_candidate(candidate)?;
                    removed += 1;
                }
            }
        }
        "size" => {
            let total: u64 = candidates.iter().map(|candidate| candidate.bytes).sum();
            if total <= policy.max_bytes {
                return Ok(0);
            }
            candidates.sort_by_key(|candidate| candidate.modified);
            let mut remaining = total;
            for candidate in candidates {
                if remaining <= policy.max_bytes {
                    break;
                }
                remaining = remaining.saturating_sub(candidate.bytes);
                trash_candidate(candidate)?;
                removed += 1;
            }
        }
        _ => {}
    }
    Ok(removed)
}

fn trash_candidate(candidate: Candidate) -> Result<(), String> {
    if candidate.directory.exists() {
        move_to_trash(&candidate.directory)?;
    }
    if candidate.json.exists() {
        move_to_trash(&candidate.json)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{apply, LanRetentionPolicy};
    use crate::protocol::{Task, TaskState};
    use std::path::Path;

    fn policy(mode: &str) -> LanRetentionPolicy {
        LanRetentionPolicy {
            mode: mode.to_owned(),
            age_days: 1,
            max_bytes: 10,
        }
    }

    fn task(root: &Path, id: &str, state: TaskState, payload: &[u8]) -> Task {
        let tasks = root.join("tasks");
        std::fs::create_dir_all(tasks.join(id)).unwrap();
        std::fs::write(tasks.join(id).join("payload.bin"), payload).unwrap();
        let task = Task {
            id: id.to_owned(),
            state,
            file_name: "a.wav".into(),
            file_size: payload.len() as u64,
            received_bytes: payload.len() as u64,
            chunks_total: 1,
            chunks_received: 1,
            model_id: None,
            created_at_unix_ms: 1,
            updated_at_unix_ms: 1,
            error: None,
            result: None,
        };
        std::fs::write(
            tasks.join(format!("{id}.json")),
            serde_json::to_vec(&task).unwrap(),
        )
        .unwrap();
        task
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "lan-retention-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn off_touches_nothing() {
        let root = scratch("off");
        task(&root, "done", TaskState::Completed, b"xxxxxxxx");
        assert_eq!(apply(&root, &policy("off")).unwrap(), 0);
        assert!(root.join("tasks/done.json").exists());
    }

    #[test]
    fn size_cleanup_skips_non_terminal_tasks() {
        let root = scratch("size");
        task(&root, "running", TaskState::Transcribing, b"0123456789");
        let result = apply(&root, &policy("size"));
        if cfg!(target_os = "macos") {
            assert_eq!(result.unwrap(), 0);
            assert!(root.join("tasks/running.json").exists());
        }
    }

    #[test]
    fn size_cleanup_removes_terminal_payloads_oldest_first() {
        let root = scratch("size-terminal");
        task(&root, "one", TaskState::Completed, b"0123456789");
        task(&root, "two", TaskState::Completed, b"0123456789");
        if !cfg!(target_os = "macos") {
            return;
        }
        let removed = apply(&root, &policy("size")).unwrap();
        assert_eq!(removed, 1);
        assert!(!root.join("tasks/one.json").exists());
        assert!(root.join("tasks/two.json").exists());
    }
}
