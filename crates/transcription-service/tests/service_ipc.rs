use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use voice_to_text_service::{Request, ServiceClient};

fn scratch(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Unix socket paths are limited (SUN_LEN, 104 bytes on macOS) and the
    // default temp folder is already long, so Unix keeps the short /tmp;
    // Windows uses the normal temp directory (loopback TCP, no path limit).
    #[cfg(unix)]
    let base = PathBuf::from("/tmp");
    #[cfg(windows)]
    let base = std::env::temp_dir();
    let path = base.join(format!(
        "voice-to-text-{name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn server_bin() -> &'static str {
    env!("CARGO_BIN_EXE_voice-to-text-service")
}

fn spawn_server(data_dir: &Path, roots: &[&Path]) -> Child {
    let mut command = Command::new(server_bin());
    command
        .arg("serve")
        .arg("--data-dir")
        .arg(data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for root in roots {
        command.arg("--allow-root").arg(root);
    }
    command.spawn().expect("spawn service process")
}

/// Spawn the service and wait until its socket exists; on failure the child's
/// output is included so the real startup error is visible.
fn spawn_ready_server(data_dir: &Path, roots: &[&Path]) -> Child {
    let mut child = spawn_server(data_dir, roots);
    if wait_for_socket(data_dir, Duration::from_secs(10)) {
        return child;
    }
    let _ = child.kill();
    let detail = match child.wait_with_output() {
        Ok(output) => output_text(&output),
        Err(error) => format!("wait failed: {error}"),
    };
    panic!("service socket never appeared; child output: {detail}");
}

fn wait_for_socket(data_dir: &Path, timeout: Duration) -> bool {
    let socket = voice_to_text_service::endpoint_file(data_dir);
    let started = Instant::now();
    while started.elapsed() < timeout {
        if socket.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

fn stop(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn service_is_single_instance_and_its_socket_is_owner_only() {
    let data = scratch("service-single-instance");
    let first = spawn_ready_server(&data, &[]);

    // Only the owner may connect: socket file permissions must be 0600.
    // (Windows relies on the per-user data folder ACL instead of a mode.)
    #[cfg(unix)]
    {
        let mode = fs::metadata(data.join("service.sock"))
            .expect("socket metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "socket must be owner-read/write only");
    }

    let mut client = ServiceClient::connect(&data).expect("connect to the running service");
    let ping = client.request(&Request::Ping).expect("ping response");
    assert!(ping.ok);
    let service = ping.service.expect("service info");
    assert_eq!(service.protocol_version, 1);

    // A second instance must refuse to start and report a distinct exit code.
    let second: Output = Command::new(server_bin())
        .arg("serve")
        .arg("--data-dir")
        .arg(&data)
        .output()
        .expect("spawn second service");
    assert_eq!(
        second.status.code(),
        Some(3),
        "second instance must exit 3: {}",
        output_text(&second)
    );
    let text = output_text(&second);
    assert!(
        text.contains("already running"),
        "second instance must say why: {text}"
    );

    stop(first);
}

#[test]
fn submit_fails_closed_without_an_authorized_root_and_outside_one() {
    // Case 1: no --allow-root configured at all.
    let data = scratch("service-fail-closed");
    let media_dir = scratch("service-fail-closed-media");
    let media = media_dir.join("sample.wav");
    fs::write(&media, b"RIFF-not-really-checked-at-authorize-time").unwrap();
    let server = spawn_ready_server(&data, &[]);
    {
        let mut client = ServiceClient::connect(&data).expect("connect");
        let response = client
            .request(&Request::Submit {
                path: media.to_string_lossy().into_owned(),
                model_dir: None,
                model_id: None,
                owner: Some("cli".to_owned()),
            })
            .expect("submit response");
        assert!(!response.ok, "no roots must mean deny");
        let error = response.error.expect("error body");
        assert_eq!(error.code, "unauthorized");
        assert!(error.message.contains("--allow-root"), "{}", error.message);
    }
    stop(server);

    // Case 2: an authorized root exists, but the file lives outside it.
    let data = scratch("service-outside-root");
    let allowed = scratch("service-allowed-root");
    let server = spawn_ready_server(&data, &[&allowed]);
    {
        let mut client = ServiceClient::connect(&data).expect("connect");
        let response = client
            .request(&Request::Submit {
                path: media.to_string_lossy().into_owned(),
                model_dir: None,
                model_id: None,
                owner: Some("cli".to_owned()),
            })
            .expect("submit response");
        assert!(!response.ok, "outside-root must be denied");
        assert_eq!(response.error.expect("error body").code, "unauthorized");
    }
    stop(server);
}

/// P0 gate: several clients submit concurrently; the service must run exactly
/// one inference at a time, keep every job visible, and finish all of them.
///
/// Requires:
///   VOICE_TO_TEXT_TEST_MODEL_DIR  an approved model directory
///   VOICE_TO_TEXT_TEST_MEDIA      an absolute path to a media fixture
#[test]
#[ignore = "P0 gate: requires an approved model directory and a local media fixture"]
fn concurrent_clients_share_exactly_one_inference_worker() {
    let model_dir = PathBuf::from(
        std::env::var_os("VOICE_TO_TEXT_TEST_MODEL_DIR").expect("set model directory"),
    );
    let media = PathBuf::from(std::env::var_os("VOICE_TO_TEXT_TEST_MEDIA").expect("set media"));
    let media = fs::canonicalize(&media).expect("canonical media path");
    let root = media
        .parent()
        .expect("media has a parent directory")
        .to_path_buf();

    let data = scratch("service-concurrent");
    let server = spawn_ready_server(&data, &[&root]);

    // Three clients submit concurrently, each from its own connection.
    let mut handles = Vec::new();
    for _ in 0..3 {
        let data = data.clone();
        let media = media.clone();
        let model_dir = model_dir.clone();
        handles.push(thread::spawn(move || {
            let mut client = ServiceClient::connect(&data).expect("connect");
            let response = client
                .request(&Request::Submit {
                    path: media.to_string_lossy().into_owned(),
                    model_dir: Some(model_dir.to_string_lossy().into_owned()),
                    model_id: None,
                    owner: Some("cli".to_owned()),
                })
                .expect("submit response");
            assert!(
                response.ok,
                "submit should be accepted: {:?}",
                response.error
            );
            response.job.expect("job reply").job_id
        }));
    }
    let job_ids: Vec<String> = handles
        .into_iter()
        .map(|handle| handle.join().expect("submission thread"))
        .collect();
    assert_eq!(job_ids.len(), 3);

    let mut client = ServiceClient::connect(&data).expect("connect");
    let mut observed_states = std::collections::HashSet::new();
    let deadline = Instant::now() + Duration::from_secs(240);
    loop {
        let mut all_terminal = true;
        for job_id in &job_ids {
            let response = client
                .request(&Request::Status {
                    job_id: job_id.clone(),
                })
                .expect("status response");
            assert!(response.ok, "status must resolve: {:?}", response.error);
            let job = response.job.expect("job reply");
            observed_states.insert(job.state.clone());
            if job.state != "completed" {
                all_terminal = false;
                assert_ne!(
                    job.state, "failed",
                    "job {} failed: {:?}",
                    job_id, job.error
                );
            }
        }
        if all_terminal {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "jobs did not finish in time; observed states: {observed_states:?}"
        );
        thread::sleep(Duration::from_millis(50));
    }

    // The waiting state must have been visible at some point (queued jobs).
    assert!(
        observed_states.contains("queued"),
        "waiting jobs must be visible as queued: {observed_states:?}"
    );

    // Execution history: three runs, each completed, never overlapping.
    let runs = client
        .request(&Request::Runs)
        .expect("runs response")
        .runs
        .expect("runs list");
    assert_eq!(runs.len(), 3, "exactly one run per submitted job: {runs:?}");
    let mut ordered = runs;
    ordered.sort_by_key(|run| (run.started_unix_ms, run.finished_unix_ms));
    for pair in ordered.windows(2) {
        assert!(
            pair[0].finished_unix_ms <= pair[1].started_unix_ms,
            "inference runs must never overlap: {ordered:?}"
        );
    }
    let mut run_ids: Vec<String> = ordered.iter().map(|run| run.job_id.clone()).collect();
    run_ids.sort();
    let mut expected = job_ids.clone();
    expected.sort();
    assert_eq!(run_ids, expected, "every submitted job ran exactly once");

    stop(server);
}
