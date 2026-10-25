//! M1 baseline: cache thrashing before the node moves into its own process.
//!
//! Runs the node server and a local transcription inside one process, so both
//! paths share `RECOGNIZER_CACHE`. Alternating two models therefore pays a
//! full model load on every switch; two consecutive tasks on the same model
//! show the reuse time. Real models and a real audio file are required.
//!
//! ```sh
//! VOICE_TO_TEXT_TEST_MODEL_A=<big model dir> \
//! VOICE_TO_TEXT_TEST_MODEL_B=<small model dir> \
//! VOICE_TO_TEXT_TEST_MEDIA=<wav> \
//! cargo run --release -p voice-to-text-lan --example overlap_baseline
//! ```

use std::{
    env,
    path::PathBuf,
    time::{Duration, Instant},
};

use voice_to_text_core::{inspect_model_directory, transcribe_file_cancellable};
use voice_to_text_lan::{
    client::LanClient,
    node::{NodeConfig, NodeServer},
    protocol::TaskState,
};

fn rss_kb() -> u64 {
    let pid = std::process::id().to_string();
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output();
    output
        .ok()
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse::<u64>()
                .ok()
        })
        .unwrap_or(0)
}

fn main() {
    let model_a_dir = PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MODEL_A").expect("model A"));
    let model_b_dir = PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MODEL_B").expect("model B"));
    let media = PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MEDIA").expect("media"));
    let model_a = inspect_model_directory(&model_a_dir).expect("model A layout");
    let model_b = inspect_model_directory(&model_b_dir).expect("model B layout");

    let data_dir = std::env::temp_dir().join(format!("m1-node-{}", std::process::id()));
    let node = NodeServer::start_with_models(
        NodeConfig {
            name: "m1".into(),
            data_dir,
            listen: "127.0.0.1:0".parse().unwrap(),
            compute_enabled: true,
            role: "cpu".into(),
            model_dir: None,
            media_root: None,
            retention_policy: None,
            llm_models_dir: None,
            llm_idle_unload_seconds: None,
        },
        vec![model_b.clone()],
    )
    .expect("node starts");
    let port = node.port();
    let client = LanClient::new("127.0.0.1", port);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");

    println!("model_a={} model_b={}", model_a.id, model_b.id);
    println!("media={} rss_at_start_kb={}", media.display(), rss_kb());

    let local = |label: &str| {
        let started = Instant::now();
        let transcript = transcribe_file_cancellable(&media, &model_a, || false, |_| {})
            .expect("local transcription");
        println!(
            "step={label} ms={} chars={} rss_kb={}",
            started.elapsed().as_millis(),
            transcript.text.chars().count(),
            rss_kb()
        );
    };
    local("local_a_1");
    local("local_a_2");

    let remote = |label: &str| {
        let started = Instant::now();
        let size = std::fs::metadata(&media).expect("media metadata").len();
        runtime.block_on(async {
            let task = client
                .create_task("m1.wav", size, 1, Some(&model_b.id))
                .await
                .expect("create task");
            client
                .upload_file(&task.id, &media, 4 * 1024 * 1024, |_, _| {})
                .await
                .expect("upload");
            let deadline = Instant::now() + Duration::from_secs(300);
            loop {
                let current = client.task(&task.id).await.expect("task status");
                if current.state.is_terminal() {
                    assert_eq!(current.state, TaskState::Completed);
                    break;
                }
                assert!(Instant::now() < deadline, "node task timed out");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        println!(
            "step={label} ms={} rss_kb={}",
            started.elapsed().as_millis(),
            rss_kb()
        );
    };
    remote("node_b_1");
    remote("node_b_2");

    local("local_a_3_after_switch");
    local("local_a_4");

    node.shutdown();
    node.wait();
}
