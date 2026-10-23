//! Trusted-LAN contract: no pairing, and remote model IDs belong to the host.
use voice_to_text_lan::node::{NodeConfig, NodeServer};

#[tokio::test]
async fn trusted_lan_needs_no_credentials_and_lists_models_without_paths() {
    let root = std::env::temp_dir().join(format!("trusted-lan-{}", uuid::Uuid::new_v4()));
    let model = root.join("remote-paraformer");
    std::fs::create_dir_all(&model).unwrap();
    std::fs::write(model.join("model.onnx"), b"layout-only fixture").unwrap();
    std::fs::write(model.join("tokens.txt"), b"a 1\n").unwrap();
    let node = NodeServer::start(NodeConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        data_dir: root.join("tasks"),
        compute_enabled: true,
        model_dir: Some(model),
        ..NodeConfig::default()
    })
    .expect("sharing must start without generating or requiring credentials");
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let base = format!("http://127.0.0.1:{}/api/v1", node.port());
    for route in ["capabilities", "tasks", "storage", "models"] {
        let response = http.get(format!("{base}/{route}")).send().await.unwrap();
        assert!(
            response.status().is_success(),
            "{route} must work without authentication"
        );
    }
    let models: serde_json::Value = http
        .get(format!("{base}/models"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(models.as_array().unwrap().len(), 1);
    assert_eq!(models[0]["id"], "paraformer-remote-paraformer");
    assert!(models[0].get("directory").is_none());
    let task: serde_json::Value = http.post(format!("{base}/tasks"))
        .json(&serde_json::json!({"file_name":"sample.wav", "file_size":100, "chunks_total":1, "model_id": models[0]["id"]}))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    assert_eq!(task["model_id"], models[0]["id"]);
    node.shutdown();
    node.wait();
}

#[tokio::test]
async fn client_can_select_any_host_model_and_unknown_ids_are_rejected() {
    use voice_to_text_lan::client::LanClient;
    let root = std::env::temp_dir().join(format!("multi-model-lan-{}", uuid::Uuid::new_v4()));
    let models = ["paraformer-first", "paraformer-second"]
        .into_iter()
        .map(|name| {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("model.onnx"),
                b"layout fixture, not inference weights",
            )
            .unwrap();
            std::fs::write(dir.join("tokens.txt"), b"a 1\n").unwrap();
            voice_to_text_core::inspect_model_directory(dir).unwrap()
        })
        .collect::<Vec<_>>();
    let node = NodeServer::start_with_models(
        NodeConfig {
            listen: "127.0.0.1:0".parse().unwrap(),
            data_dir: root.join("tasks"),
            compute_enabled: true,
            ..NodeConfig::default()
        },
        models.clone(),
    )
    .unwrap();
    let client = LanClient::new("127.0.0.1", node.port());
    let catalog = client.models().await.unwrap();
    assert_eq!(catalog.len(), 2);
    for model in &models {
        let task = client
            .create_task("test.wav", 100, 1, Some(&model.id))
            .await
            .unwrap();
        assert_eq!(task.model_id.as_deref(), Some(model.id.as_str()));
    }
    let default = client.create_task("test.wav", 100, 1, None).await.unwrap();
    assert_eq!(default.model_id.as_deref(), Some(models[0].id.as_str()));
    assert!(client
        .create_task("test.wav", 100, 1, Some("client-local-model-not-on-host"))
        .await
        .is_err());
    // No weights are loaded: these tasks have not uploaded media or entered the queue.
    assert!(client
        .tasks()
        .await
        .unwrap()
        .iter()
        .all(|task| task.state == voice_to_text_lan::protocol::TaskState::FileTransferring));
    node.shutdown();
    node.wait();
}
