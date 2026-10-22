use std::{
    fs,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use voice_to_text_core::{JobState, JobStatus, JobStore, Transcript};

static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

fn unique_root(label: &str) -> std::path::PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "voice-to-text-{label}-{timestamp}-{}",
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn running(job_id: &str) -> JobStatus {
    JobStatus {
        job_id: job_id.to_owned(),
        state: JobState::Running,
        phase: "Starting".to_owned(),
        completed_chunks: 0,
        total_chunks: None,
        transcript_available: false,
        result_saved: false,
        error: None,
    }
}

fn completed(job_id: &str) -> JobStatus {
    JobStatus {
        job_id: job_id.to_owned(),
        state: JobState::Completed,
        phase: "Completed".to_owned(),
        completed_chunks: 1,
        total_chunks: Some(1),
        transcript_available: true,
        result_saved: true,
        error: None,
    }
}

#[test]
fn job_status_round_trips_and_active_job_is_shared_between_store_instances() {
    let root = unique_root("shared-jobs");
    let first = JobStore::new(&root);
    let second = JobStore::new(&root);
    let mut lease = first.begin_job(running("job-1")).unwrap();

    assert_eq!(second.job_status("job-1").unwrap(), Some(running("job-1")));
    assert!(second.begin_job(running("job-2")).is_err());

    lease.save_status(completed("job-1")).unwrap();
    drop(lease);
    let next = second.begin_job(running("job-2")).unwrap();
    assert_eq!(next.job_id(), "job-2");
}

#[test]
fn cancellation_request_is_visible_across_store_instances_and_terminal_jobs_are_not_cancelled() {
    let root = unique_root("job-cancel");
    let first = JobStore::new(&root);
    let second = JobStore::new(&root);
    let mut lease = first.begin_job(running("job-cancel")).unwrap();

    assert!(second.request_cancel("job-cancel").unwrap());
    assert!(lease.cancellation_requested().unwrap());

    lease
        .save_status(JobStatus {
            job_id: "job-cancel".to_owned(),
            state: JobState::Cancelled,
            phase: "Cancelled".to_owned(),
            completed_chunks: 0,
            total_chunks: None,
            transcript_available: false,
            result_saved: false,
            error: None,
        })
        .unwrap();
    drop(lease);
    assert!(!second.request_cancel("job-cancel").unwrap());
}

#[test]
fn dropped_worker_lease_is_recovered_as_interrupted_before_next_job() {
    let root = unique_root("stale-job");
    let store = JobStore::new(&root);
    let lease = store.begin_job(running("job-stale")).unwrap();
    drop(lease);

    let next = store.begin_job(running("job-next")).unwrap();
    let stale = store.job_status("job-stale").unwrap().unwrap();

    assert_eq!(stale.state, JobState::Failed);
    assert!(stale
        .error
        .as_deref()
        .is_some_and(|message| message.contains("interrupted")));
    drop(next);
}

#[test]
fn child_process_holds_job_lease_until_parent_releases_it() {
    let Ok(root) = std::env::var("VOICE_TO_TEXT_JOB_STORE_CHILD_ROOT") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let store = JobStore::new(&root);
    let lease = store.begin_job(running("job-cross-process")).unwrap();
    fs::write(root.join("child-ready"), b"ready").unwrap();
    let release = root.join("parent-release");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !release.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        release.exists(),
        "parent did not release the child task lease"
    );
    drop(lease);
}

#[test]
fn job_lease_and_status_are_shared_across_processes() {
    let root = unique_root("cross-process");
    fs::create_dir_all(&root).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_process_holds_job_lease_until_parent_releases_it",
        ])
        .env("VOICE_TO_TEXT_JOB_STORE_CHILD_ROOT", &root)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let ready = root.join("child-ready");
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ready.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if !ready.exists() {
        fs::write(root.join("parent-release"), b"release").unwrap();
        let output = child.wait_with_output().unwrap();
        panic!(
            "child did not acquire its task lease: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let store = JobStore::new(&root);
    let active = store.active_job_status();
    let second_start = store.begin_job(running("job-parent"));
    fs::write(root.join("parent-release"), b"release").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "child process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(active.unwrap(), Some(running("job-cross-process")));
    assert!(
        second_start.is_err(),
        "a second process must not start another active job"
    );
    assert_eq!(
        store
            .job_status("job-cross-process")
            .unwrap()
            .unwrap()
            .state,
        JobState::Failed,
        "an exited worker must be recovered as interrupted"
    );
}

#[test]
fn saved_transcripts_are_available_to_an_independent_store_instance() {
    let root = unique_root("job-results");
    let first = JobStore::new(&root);
    let second = JobStore::new(&root);
    let transcript = Transcript {
        schema_version: "1".to_owned(),
        job_id: "job-result".to_owned(),
        source_file_name: "meeting.wav".to_owned(),
        model_id: "sensevoice-demo".to_owned(),
        model_version: None,
        engine: "sherpa-onnx/test".to_owned(),
        language: None,
        duration_ms: Some(1_500),
        text: "hello from the shared result store".to_owned(),
        segments: None,
    };

    first.save_transcript(&transcript).unwrap();
    assert_eq!(second.transcript("job-result").unwrap(), transcript);
    assert!(second.transcript("../outside").is_err());
}
