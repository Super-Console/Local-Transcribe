//! Persistent task store with an in-memory index.
//!
//! Each task lives in `<data_dir>/tasks/<id>.json`, written atomically
//! (temp file + rename). Tasks that were still in flight when the process
//! died are marked `Failed` on the next start.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::protocol::{Task, TaskError, TaskResult, TaskState};

#[cfg(test)]
mod tests;

/// Failure raised by task store operations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("task not found: {0}")]
    NotFound(String),
    #[error("illegal task state transition: {from} -> {to}")]
    IllegalTransition { from: TaskState, to: TaskState },
    #[error("task store io error: {0}")]
    Io(String),
}

struct StoreInner {
    tasks: BTreeMap<String, Task>,
    /// Monotonic timestamp source so two tasks created in the same
    /// millisecond still sort deterministically.
    last_unix_ms: u64,
}

impl StoreInner {
    fn next_unix_ms(&mut self) -> u64 {
        let now = unix_now_ms();
        let value = now.max(self.last_unix_ms.saturating_add(1));
        self.last_unix_ms = value;
        value
    }
}

/// Thread-safe task store: one JSON file per task plus a memory index.
pub struct TaskStore {
    tasks_dir: PathBuf,
    inner: Mutex<StoreInner>,
}

impl TaskStore {
    /// Open (or create) the store under `data_dir` and load every persisted
    /// task. Tasks interrupted mid-flight are marked `Failed`.
    pub fn new(data_dir: PathBuf) -> Self {
        let tasks_dir = data_dir.join("tasks");
        if let Err(error) = fs::create_dir_all(&tasks_dir) {
            eprintln!("[lan-store] cannot create {}: {error}", tasks_dir.display());
        }
        let mut tasks = BTreeMap::new();
        let mut last_unix_ms = 0u64;
        let mut interrupted = Vec::new();
        if let Ok(entries) = fs::read_dir(&tasks_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let Ok(bytes) = fs::read(&path) else {
                    continue;
                };
                let Ok(mut task) = serde_json::from_slice::<Task>(&bytes) else {
                    eprintln!(
                        "[lan-store] skipping unreadable task file {}",
                        path.display()
                    );
                    continue;
                };
                last_unix_ms = last_unix_ms
                    .max(task.created_at_unix_ms)
                    .max(task.updated_at_unix_ms);
                if matches!(
                    task.state,
                    TaskState::FileTransferring
                        | TaskState::Queued
                        | TaskState::Transcribing
                        | TaskState::PostProcessing
                        | TaskState::CancelRequested
                ) {
                    task.state = TaskState::Failed;
                    task.error = Some(TaskError {
                        code: "interrupted".to_owned(),
                        message: "服务中断，任务未完成".to_owned(),
                    });
                    interrupted.push(task.clone());
                }
                tasks.insert(task.id.clone(), task);
            }
        }
        for task in &interrupted {
            if let Err(error) = persist_task(&tasks_dir, task) {
                eprintln!(
                    "[lan-store] cannot persist recovered task {}: {error}",
                    task.id
                );
            }
        }
        Self {
            tasks_dir,
            inner: Mutex::new(StoreInner {
                tasks,
                last_unix_ms,
            }),
        }
    }

    /// Directory holding the per-task JSON files.
    pub fn data_dir(&self) -> &Path {
        self.tasks_dir.parent().unwrap_or(self.tasks_dir.as_path())
    }

    /// Register a new task in `Init` state.
    pub fn create_task(
        &self,
        file_name: &str,
        file_size: u64,
        chunks_total: u32,
        model_id: Option<&str>,
    ) -> Task {
        let mut inner = lock(&self.inner);
        let now = inner.next_unix_ms();
        let task = Task::new(
            uuid::Uuid::new_v4().to_string(),
            file_name.to_owned(),
            file_size,
            chunks_total,
            model_id.map(str::to_owned),
            now,
        );
        if let Err(error) = persist_task(&self.tasks_dir, &task) {
            eprintln!("[lan-store] cannot persist task {}: {error}", task.id);
        }
        inner.tasks.insert(task.id.clone(), task.clone());
        task
    }

    /// Look one task up by id.
    pub fn get(&self, id: &str) -> Option<Task> {
        if !is_safe_id(id) {
            return None;
        }
        lock(&self.inner).tasks.get(id).cloned()
    }

    /// All tasks, newest first.
    pub fn list(&self) -> Vec<Task> {
        let mut tasks: Vec<Task> = lock(&self.inner).tasks.values().cloned().collect();
        tasks.sort_by(|a, b| {
            b.created_at_unix_ms
                .cmp(&a.created_at_unix_ms)
                .then_with(|| b.id.cmp(&a.id))
        });
        tasks
    }

    /// Move a task to `next`, rejecting any move the state machine forbids.
    pub fn transition(&self, id: &str, next: TaskState) -> Result<Task, StoreError> {
        let mut inner = lock(&self.inner);
        let now = inner.next_unix_ms();
        let mut updated = lookup(&inner, id)?;
        if !updated.state.can_transition_to(&next) {
            return Err(StoreError::IllegalTransition {
                from: updated.state,
                to: next,
            });
        }
        updated.state = next;
        updated.updated_at_unix_ms = now;
        persist_task(&self.tasks_dir, &updated)?;
        inner.tasks.insert(updated.id.clone(), updated.clone());
        Ok(updated)
    }

    /// Record how many bytes of the payload arrived so far.
    pub fn set_received(
        &self,
        id: &str,
        received_bytes: u64,
        chunks_received: u32,
    ) -> Result<Task, StoreError> {
        let mut inner = lock(&self.inner);
        let now = inner.next_unix_ms();
        let mut updated = lookup(&inner, id)?;
        updated.received_bytes = received_bytes;
        updated.chunks_received = chunks_received;
        updated.updated_at_unix_ms = now;
        persist_task(&self.tasks_dir, &updated)?;
        inner.tasks.insert(updated.id.clone(), updated.clone());
        Ok(updated)
    }

    /// Store the transcription result and move the task to `Completed`.
    pub fn finish_ok(&self, id: &str, result: TaskResult) -> Result<Task, StoreError> {
        let mut inner = lock(&self.inner);
        let now = inner.next_unix_ms();
        let mut updated = lookup(&inner, id)?;
        if !updated.state.can_transition_to(&TaskState::Completed) {
            return Err(StoreError::IllegalTransition {
                from: updated.state,
                to: TaskState::Completed,
            });
        }
        updated.state = TaskState::Completed;
        updated.result = Some(result);
        updated.error = None;
        updated.updated_at_unix_ms = now;
        persist_task(&self.tasks_dir, &updated)?;
        inner.tasks.insert(updated.id.clone(), updated.clone());
        Ok(updated)
    }

    /// Record the failure and move the task to `Failed`.
    pub fn finish_err(&self, id: &str, error: TaskError) -> Result<Task, StoreError> {
        let mut inner = lock(&self.inner);
        let now = inner.next_unix_ms();
        let mut updated = lookup(&inner, id)?;
        if !updated.state.can_transition_to(&TaskState::Failed) {
            return Err(StoreError::IllegalTransition {
                from: updated.state,
                to: TaskState::Failed,
            });
        }
        updated.state = TaskState::Failed;
        updated.error = Some(error);
        updated.result = None;
        updated.updated_at_unix_ms = now;
        persist_task(&self.tasks_dir, &updated)?;
        inner.tasks.insert(updated.id.clone(), updated.clone());
        Ok(updated)
    }

    /// Ask a still-cancellable task to stop.
    pub fn request_cancel(&self, id: &str) -> Result<Task, StoreError> {
        self.transition(id, TaskState::CancelRequested)
    }
}

fn lookup(inner: &StoreInner, id: &str) -> Result<Task, StoreError> {
    if !is_safe_id(id) {
        return Err(StoreError::NotFound(id.to_owned()));
    }
    inner
        .tasks
        .get(id)
        .cloned()
        .ok_or_else(|| StoreError::NotFound(id.to_owned()))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Task ids are UUID strings; anything else could escape `tasks/`.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Write one task atomically: temp file in the same directory, then rename.
fn persist_task(tasks_dir: &Path, task: &Task) -> Result<(), StoreError> {
    let bytes =
        serde_json::to_vec_pretty(task).map_err(|error| StoreError::Io(error.to_string()))?;
    let final_path = tasks_dir.join(format!("{}.json", task.id));
    let temp_path = tasks_dir.join(format!("{}.json.{}.tmp", task.id, uuid::Uuid::new_v4()));
    fs::write(&temp_path, &bytes).map_err(|error| StoreError::Io(error.to_string()))?;
    fs::rename(&temp_path, &final_path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        StoreError::Io(error.to_string())
    })
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}
