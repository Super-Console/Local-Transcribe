//! Unit tests for the persistent task store: persistence, crash recovery
//! and state-machine enforcement.

use std::path::PathBuf;

use crate::protocol::{Segment, TaskError, TaskResult, TaskState};
use crate::store::{StoreError, TaskStore};

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lan-store-test-{}-{}", name, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn drive_to(store: &TaskStore, id: &str, target: TaskState) {
    let mut state = store.get(id).expect("task").state;
    while state != target {
        let next = match state {
            TaskState::Init => TaskState::FileTransferring,
            TaskState::FileTransferring => TaskState::Queued,
            TaskState::Queued => TaskState::Transcribing,
            TaskState::Transcribing => TaskState::PostProcessing,
            other => panic!("cannot drive further from {other:?}"),
        };
        state = store.transition(id, next).expect("legal transition").state;
    }
}

fn sample_result() -> TaskResult {
    TaskResult {
        transcript: "hello".to_owned(),
        segments: vec![Segment {
            start_ms: 0,
            end_ms: 100,
            text: "hello".to_owned(),
        }],
        duration_ms: 100,
    }
}

#[test]
fn created_tasks_are_visible_and_listed_newest_first() {
    let dir = scratch("create");
    let store = TaskStore::new(dir.clone());
    let first = store.create_task("a.mp3", 10, 1, None);
    let second = store.create_task("b.mp3", 20, 2, Some("model-x"));
    assert_eq!(first.state, TaskState::Init);
    assert_eq!(first.received_bytes, 0);
    assert_eq!(second.model_id.as_deref(), Some("model-x"));

    let listed = store.list();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, second.id, "newest task must come first");
    assert_eq!(listed[1].id, first.id);
    assert_eq!(store.get(&first.id).expect("first").file_name, "a.mp3");
    assert!(store.get("no-such-task").is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn transition_accepts_legal_and_rejects_illegal_moves() {
    let dir = scratch("transition");
    let store = TaskStore::new(dir.clone());
    let task = store.create_task("a.mp3", 10, 1, None);
    let id = task.id.as_str();

    let moved = store
        .transition(id, TaskState::FileTransferring)
        .expect("legal");
    assert_eq!(moved.state, TaskState::FileTransferring);
    store.transition(id, TaskState::Queued).expect("legal");

    // Illegal: Transcribing without the engine, straight from Queued is fine,
    // but Completed must still pass PostProcessing.
    store
        .transition(id, TaskState::Transcribing)
        .expect("legal");
    let err = store
        .transition(id, TaskState::Completed)
        .expect_err("transcribing -> completed is illegal");
    assert!(matches!(err, StoreError::IllegalTransition { .. }));
    assert_eq!(
        store.get(id).expect("task").state,
        TaskState::Transcribing,
        "rejected transition must not change the stored state"
    );

    let err = store
        .transition(id, TaskState::Queued)
        .expect_err("moving backwards is illegal");
    assert!(matches!(err, StoreError::IllegalTransition { .. }));

    let err = store
        .transition("missing-task", TaskState::Queued)
        .expect_err("unknown task");
    assert!(matches!(err, StoreError::NotFound(_)));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn set_received_updates_counters_and_persists() {
    let dir = scratch("received");
    let store = TaskStore::new(dir.clone());
    let task = store.create_task("a.mp3", 100, 4, None);
    let id = task.id.as_str();
    store
        .transition(id, TaskState::FileTransferring)
        .expect("legal");
    store.set_received(id, 25, 1).expect("update");
    store.set_received(id, 50, 2).expect("update");
    let task = store.get(id).expect("task");
    assert_eq!(task.received_bytes, 50);
    assert_eq!(task.chunks_received, 2);

    drop(store);
    let reopened = TaskStore::new(dir.clone());
    let task = reopened.get(id).expect("task after reopen");
    assert_eq!(task.received_bytes, 50);
    assert_eq!(task.chunks_received, 2);
    // A reopen is a restart: the in-flight task is reported as interrupted,
    // but its transfer counters survive for a possible resubmission.
    assert_eq!(task.state, TaskState::Failed);
    assert_eq!(task.error.expect("error").code, "interrupted");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn finish_ok_records_result_and_finish_err_records_error() {
    let dir = scratch("finish");
    let store = TaskStore::new(dir.clone());
    let ok = store.create_task("a.mp3", 10, 1, None);
    drive_to(&store, &ok.id, TaskState::PostProcessing);
    let done = store.finish_ok(&ok.id, sample_result()).expect("finish");
    assert_eq!(done.state, TaskState::Completed);
    assert_eq!(done.result, Some(sample_result()));

    let bad = store.create_task("b.mp3", 10, 1, None);
    drive_to(&store, &bad.id, TaskState::Queued);
    let failed = store
        .finish_err(
            &bad.id,
            TaskError {
                code: "model_missing".to_owned(),
                message: "model directory is not usable".to_owned(),
            },
        )
        .expect("finish");
    assert_eq!(failed.state, TaskState::Failed);
    assert_eq!(failed.error.as_ref().expect("error").code, "model_missing");

    drop(store);
    let reopened = TaskStore::new(dir.clone());
    assert_eq!(
        reopened.get(&ok.id).expect("task").result,
        Some(sample_result())
    );
    assert_eq!(
        reopened.get(&bad.id).expect("task").state,
        TaskState::Failed
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn request_cancel_moves_cancellable_tasks_only() {
    let dir = scratch("cancel");
    let store = TaskStore::new(dir.clone());
    let task = store.create_task("a.mp3", 10, 1, None);
    let id = task.id.as_str();
    store
        .transition(id, TaskState::FileTransferring)
        .expect("legal");
    store.transition(id, TaskState::Queued).expect("legal");
    let cancelled = store.request_cancel(id).expect("cancel");
    assert_eq!(cancelled.state, TaskState::CancelRequested);

    let done = store.create_task("b.mp3", 10, 1, None);
    drive_to(&store, &done.id, TaskState::PostProcessing);
    store.finish_ok(&done.id, sample_result()).expect("finish");
    let err = store
        .request_cancel(&done.id)
        .expect_err("completed is terminal");
    assert!(matches!(err, StoreError::IllegalTransition { .. }));
    assert_eq!(
        store.get(&done.id).expect("task").state,
        TaskState::Completed
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn restart_keeps_finished_tasks_untouched() {
    let dir = scratch("restart-finished");
    let store = TaskStore::new(dir.clone());
    let completed = store.create_task("done.mp3", 10, 1, None);
    drive_to(&store, &completed.id, TaskState::PostProcessing);
    store
        .finish_ok(&completed.id, sample_result())
        .expect("finish");

    let failed = store.create_task("bad.mp3", 10, 1, None);
    drive_to(&store, &failed.id, TaskState::Queued);
    store
        .finish_err(
            &failed.id,
            TaskError {
                code: "decode_failed".to_owned(),
                message: "cannot decode media".to_owned(),
            },
        )
        .expect("finish");

    let cancelled = store.create_task("stop.mp3", 10, 1, None);
    drive_to(&store, &cancelled.id, TaskState::Queued);
    store.request_cancel(&cancelled.id).expect("cancel");
    store
        .transition(&cancelled.id, TaskState::Cancelled)
        .expect("legal");

    drop(store);
    let reopened = TaskStore::new(dir.clone());
    let completed = reopened.get(&completed.id).expect("task");
    assert_eq!(completed.state, TaskState::Completed);
    assert_eq!(completed.result, Some(sample_result()));
    assert!(completed.error.is_none());

    let failed = reopened.get(&failed.id).expect("task");
    assert_eq!(failed.state, TaskState::Failed);
    assert_eq!(
        failed.error.as_ref().expect("error").code,
        "decode_failed",
        "the original error must survive the restart"
    );

    let cancelled = reopened.get(&cancelled.id).expect("task");
    assert_eq!(cancelled.state, TaskState::Cancelled);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn restart_marks_in_flight_tasks_as_interrupted() {
    use TaskState::*;
    let dir = scratch("restart-recovery");
    let store = TaskStore::new(dir.clone());

    let mut ids = Vec::new();
    for target in [FileTransferring, Queued, Transcribing, PostProcessing] {
        let task = store.create_task("wip.mp3", 10, 1, None);
        drive_to(&store, &task.id, target);
        ids.push(task.id);
    }
    let cancelling = store.create_task("wip.mp3", 10, 1, None);
    drive_to(&store, &cancelling.id, Queued);
    store.request_cancel(&cancelling.id).expect("cancel");
    assert_eq!(
        store.get(&cancelling.id).expect("task").state,
        CancelRequested
    );
    ids.push(cancelling.id);

    drop(store);
    let reopened = TaskStore::new(dir.clone());
    for id in &ids {
        let task = reopened.get(id).expect("task");
        assert_eq!(task.state, Failed, "task {id} must be marked failed");
        let error = task.error.expect("interrupted error");
        assert_eq!(error.code, "interrupted");
        assert_eq!(error.message, "服务中断，任务未完成");
    }
    std::fs::remove_dir_all(&dir).ok();
}
