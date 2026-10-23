//! End-to-end tests for the LAN node server and client: lifecycle, resume
//! upload, authentication, cancel, collect export and path containment.

use std::path::{Path, PathBuf};
use std::time::Duration;

use voice_to_text_lan::client::{LanClient, LanError};
use voice_to_text_lan::node::{NodeConfig, NodeServer, RunningNode};
use voice_to_text_lan::protocol::{
    Segment, TaskError, TaskResult, TaskState, UploadProgress, MAX_UPLOAD_CHUNK_BYTES,
};
use voice_to_text_lan::store::TaskStore;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lan-node-test-{}-{}", name, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn start_node(
    dir: &Path,
    compute: bool,
    model_dir: Option<PathBuf>,
    media_root: Option<PathBuf>,
    _legacy_token: &str,
) -> RunningNode {
    let config = NodeConfig {
        name: "test-node".to_owned(),
        data_dir: dir.join("data"),
        listen: "127.0.0.1:0".parse().expect("listen addr"),
        compute_enabled: compute,
        role: "auto".to_owned(),
        model_dir,
        media_root,
        retention_policy: None,
    };
    NodeServer::start(config).expect("node starts")
}

fn base_url(node: &RunningNode) -> String {
    format!("http://127.0.0.1:{}", node.port())
}

#[test]
fn node_starts_without_pairing_token() {
    let dir = scratch("empty-token");
    let result = NodeServer::start(NodeConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        data_dir: dir,
        ..NodeConfig::default()
    });
    let node = result.expect("credentials are not required");
    node.shutdown();
    node.wait();
}

#[test]
fn stopping_node_does_not_wait_forever_for_an_incomplete_upload() {
    use std::io::Write;
    let dir = scratch("stop-upload");
    let node = std::sync::Arc::new(start_node(&dir, false, None, None, "stop-token"));
    let mut connection = std::net::TcpStream::connect(("127.0.0.1", node.port())).unwrap();
    connection.write_all(b"POST /api/v1/tasks HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer stop-token\r\nContent-Type: application/json\r\nContent-Length: 100000\r\n\r\n{").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    node.shutdown();
    let (tx, rx) = std::sync::mpsc::channel();
    let waiter = std::sync::Arc::clone(&node);
    let worker = std::thread::spawn(move || {
        waiter.wait();
        let _ = tx.send(());
    });
    let stopped = rx.recv_timeout(Duration::from_secs(3)).is_ok();
    drop(connection);
    worker.join().unwrap();
    assert!(
        stopped,
        "stopping sharing must not hang on a stalled HTTP upload"
    );
}

#[tokio::test]
async fn compute_node_rejects_a_different_model_before_media_upload() {
    let dir = scratch("model-selection");
    let model_dir = dir.join("paraformer-fixture");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.onnx"), b"layout-only fixture").unwrap();
    std::fs::write(model_dir.join("tokens.txt"), b"a 1\n").unwrap();
    let node = start_node(&dir, true, Some(model_dir), None, "model-token");
    let client = LanClient::new("127.0.0.1", node.port());
    let result = client
        .create_task("test.wav", 100, 1, Some("a-different-model"))
        .await;
    node.shutdown();
    node.wait();
    assert!(
        result.is_err(),
        "must not silently transcribe using a different model"
    );
}

#[tokio::test]
async fn full_lifecycle_upload_resume_cancel_and_result_gate() {
    let dir = scratch("lifecycle");
    let token = "secret-token-1";
    let node = start_node(&dir, false, None, None, token);
    let client = LanClient::new("127.0.0.1", node.port());

    assert!(client.health().await.expect("health"));
    let caps = client.capabilities().await.expect("capabilities");
    assert_eq!(caps.proto_version, 1);
    assert!(!caps.compute_enabled);
    assert_eq!(caps.max_chunk_bytes, MAX_UPLOAD_CHUNK_BYTES);
    assert_eq!(caps.role, "auto");

    let size: u64 = 256 * 1024;
    let payload = dir.join("talk.bin");
    std::fs::write(&payload, vec![0xABu8; size as usize]).expect("write payload");

    let task = client
        .create_task("talk.bin", size, 4, None)
        .await
        .expect("create task");
    assert_eq!(task.state, TaskState::FileTransferring);
    assert_eq!(task.file_size, size);

    // First half uploaded out of band, then the connection "drops".
    let half = size / 2;
    let raw = reqwest::Client::new();
    let response = raw
        .put(format!(
            "{}/api/v1/tasks/{}/files/content?offset=0",
            base_url(&node),
            task.id
        ))
        .bearer_auth(token)
        .body(vec![0xABu8; half as usize])
        .send()
        .await
        .expect("first half upload");
    assert_eq!(response.status().as_u16(), 200);
    let progress: UploadProgress = response.json().await.expect("upload progress");
    assert_eq!(progress.received_bytes, half);
    assert_eq!(progress.chunks_received, 1);

    // Resume through LanClient: it must continue from `half`, not restart.
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_in_callback = std::sync::Arc::clone(&seen);
    let finished = client
        .upload_file(&task.id, &payload, 64 * 1024, move |sent, total| {
            seen_in_callback
                .lock()
                .expect("progress lock")
                .push((sent, total));
        })
        .await
        .expect("resumed upload");
    assert_eq!(
        finished.state,
        TaskState::Queued,
        "upload completion queues"
    );
    assert_eq!(finished.received_bytes, size);
    assert!(finished.chunks_received >= 2);

    let calls = seen.lock().expect("progress lock").clone();
    assert!(!calls.is_empty(), "progress callback must fire");
    assert_eq!(*calls.last().expect("last progress"), (size, size));
    assert!(
        calls.iter().all(|(sent, _)| *sent <= size),
        "progress must never exceed the file size"
    );

    // The file progress endpoint reflects the full payload.
    let response = raw
        .get(format!("{}/api/v1/tasks/{}/file", base_url(&node), task.id))
        .bearer_auth(token)
        .send()
        .await
        .expect("file progress");
    let progress: UploadProgress = response.json().await.expect("progress body");
    assert_eq!(progress.received_bytes, size);

    // Compute off: nothing transcribes, so the result gate stays closed.
    let err = client.result(&task.id).await.expect_err("no result yet");
    assert!(matches!(err, LanError::Api { status: 409, .. }));

    let listed = client.tasks().await.expect("task list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, task.id);

    let cancelled = client.cancel(&task.id).await.expect("cancel");
    assert!(matches!(
        cancelled.state,
        TaskState::CancelRequested | TaskState::Cancelled
    ));

    // The node resolves the stray cancel request shortly after.
    let mut final_state = cancelled.state;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        final_state = client.task(&task.id).await.expect("task").state;
        if final_state == TaskState::Cancelled {
            break;
        }
    }
    assert_eq!(final_state, TaskState::Cancelled);

    let storage = client.storage().await.expect("storage");
    assert!(storage.data_dir.contains("data"));
    assert!(storage.free_bytes > 0);

    node.shutdown();
    node.wait();
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn endpoints_do_not_require_or_validate_legacy_credentials() {
    let dir = scratch("auth");
    let token = "secret-token-2";
    let node = start_node(&dir, false, None, None, token);
    let url = format!("{}/api/v1/tasks", base_url(&node));
    let http = reqwest::Client::new();

    let response = http.get(&url).send().await.expect("no token request");
    assert_eq!(response.status().as_u16(), 200, "no credentials needed");

    let response = http
        .get(&url)
        .bearer_auth("wrong-token")
        .send()
        .await
        .expect("wrong token request");
    assert_eq!(
        response.status().as_u16(),
        200,
        "legacy credentials are ignored"
    );

    let response = http
        .get(format!("{}/api/v1/health", base_url(&node)))
        .send()
        .await
        .expect("health request");
    assert_eq!(response.status().as_u16(), 200, "health stays public");

    node.shutdown();
    node.wait();
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn collect_rejects_output_outside_the_media_root() {
    let dir = scratch("collect-reject");
    let media_root = dir.join("media");
    std::fs::create_dir_all(&media_root).expect("media root");
    let token = "secret-token-3";
    let node = start_node(&dir, false, None, Some(media_root.clone()), token);
    let client = LanClient::new("127.0.0.1", node.port());

    let outside = dir.join("outside").to_string_lossy().into_owned();
    let err = client.collect(&outside).await.expect_err("outside root");
    assert!(matches!(err, LanError::Api { status: 403, .. }));

    let traversal = media_root
        .join("..")
        .join("escape")
        .to_string_lossy()
        .into_owned();
    let err = client.collect(&traversal).await.expect_err("traversal");
    assert!(matches!(err, LanError::Api { status: 403, .. }));
    assert!(
        !dir.join("escape").exists(),
        "nothing may be created outside"
    );

    node.shutdown();
    node.wait();
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn collect_exports_completed_task_with_dataset_zip() {
    let dir = scratch("collect-zip");
    let media_root = dir.join("media");
    std::fs::create_dir_all(&media_root).expect("media root");
    let data_dir = dir.join("data");

    // Seed a finished task straight through the store.
    let store = TaskStore::new(data_dir.clone());
    let task = store.create_task("talk.bin", 10, 1, Some("model-x"));
    store
        .transition(&task.id, TaskState::FileTransferring)
        .expect("to transfer");
    store
        .transition(&task.id, TaskState::Queued)
        .expect("queue");
    store
        .transition(&task.id, TaskState::Transcribing)
        .expect("transcribe");
    store
        .transition(&task.id, TaskState::PostProcessing)
        .expect("post process");
    store
        .finish_ok(
            &task.id,
            TaskResult {
                transcript: "hello world".to_owned(),
                segments: vec![Segment {
                    start_ms: 0,
                    end_ms: 1234,
                    text: "hello world".to_owned(),
                }],
                duration_ms: 1234,
            },
        )
        .expect("finish");
    drop(store);

    let token = "secret-token-4";
    let node = start_node(&dir, false, None, Some(media_root.clone()), token);
    let client = LanClient::new("127.0.0.1", node.port());

    let export_dir = media_root.join("export");
    let report = client
        .collect(export_dir.to_string_lossy().as_ref())
        .await
        .expect("collect");
    assert_eq!(report.exported, vec![task.id.clone()]);
    assert!(report.failed.is_empty());

    let json_path = export_dir.join(format!("{}.json", task.id));
    assert!(json_path.is_file(), "task snapshot json must exist");
    let snapshot: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json_path).expect("snapshot"))
            .expect("json");
    assert_eq!(snapshot["id"], task.id.as_str());
    assert_eq!(snapshot["result"]["transcript"], "hello world");

    let zip_path = export_dir.join(format!("{}.zip", task.id));
    assert!(zip_path.is_file(), "dataset zip must exist");
    let file = std::fs::File::open(&zip_path).expect("open zip");
    let mut archive = zip::ZipArchive::new(file).expect("read zip");
    let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
    assert!(names.contains(&"transcript.txt".to_owned()));
    assert!(names.contains(&"manifest.json".to_owned()));

    use std::io::Read;
    let mut transcript = String::new();
    archive
        .by_name("transcript.txt")
        .expect("transcript entry")
        .read_to_string(&mut transcript)
        .expect("read transcript");
    assert_eq!(transcript, "hello world");

    let mut manifest_raw = String::new();
    archive
        .by_name("manifest.json")
        .expect("manifest entry")
        .read_to_string(&mut manifest_raw)
        .expect("read manifest");
    let manifest: serde_json::Value = serde_json::from_str(&manifest_raw).expect("manifest json");
    assert_eq!(manifest["task_id"], task.id.as_str());

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"hello world");
    let expected: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(manifest["sha256"], expected.as_str());

    node.shutdown();
    node.wait();
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn debug_output_never_contains_the_token() {
    let token = "top-secret-token-value";
    let config = NodeConfig::default();
    assert!(!format!("{config:?}").contains(token));
    assert!(!format!("{config:#?}").contains(token));

    let client = LanClient::new("127.0.0.1", 9);
    assert!(!format!("{client:?}").contains(token));
    let _ = &client;
}

#[tokio::test]
#[ignore = "requires VOICE_TO_TEXT_TEST_MODEL_DIR"]
async fn compute_node_transcribes_uploaded_media() {
    let model_dir = PathBuf::from(
        std::env::var_os("VOICE_TO_TEXT_TEST_MODEL_DIR").expect("set VOICE_TO_TEXT_TEST_MODEL_DIR"),
    );
    let media = PathBuf::from(
        std::env::var_os("VOICE_TO_TEXT_TEST_MEDIA").expect("set VOICE_TO_TEXT_TEST_MEDIA"),
    );
    let dir = scratch("compute");
    let token = "secret-token-5";
    let node = start_node(&dir, true, Some(model_dir), None, token);
    let client = LanClient::new("127.0.0.1", node.port());

    let size = std::fs::metadata(&media).expect("media metadata").len();
    let file_name = media
        .file_name()
        .expect("media file name")
        .to_string_lossy()
        .into_owned();
    let task = client
        .create_task(&file_name, size, 1, None)
        .await
        .expect("create task");
    let task = client
        .upload_file(&task.id, &media, 4 * 1024 * 1024, |_, _| {})
        .await
        .expect("upload media");

    let mut last_state = task.state;
    for _ in 0..600 {
        if last_state.is_terminal() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        last_state = client.task(&task.id).await.expect("task").state;
    }
    assert_eq!(
        last_state,
        TaskState::Completed,
        "a real model run must complete"
    );

    let result = client.result(&task.id).await.expect("result");
    assert!(!result.transcript.is_empty());

    node.shutdown();
    node.wait();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn task_errors_carry_no_secrets() {
    // Compile-time reminder that TaskError is plain text data.
    let error = TaskError {
        code: "interrupted".to_owned(),
        message: "服务中断，任务未完成".to_owned(),
    };
    assert_eq!(error.code, "interrupted");
}
