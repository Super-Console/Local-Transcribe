//! Local MCP server (stdio only).
//!
//! Every tool is a thin client of the Local Transcribe service: submitting a
//! job returns an ID immediately, and no tool ever loads a model inside the
//! MCP process. File access stays restricted to explicitly configured roots.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use std::{env, fs, process};

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerConfig},
    schemars::JsonSchema,
    tool, tool_handler, tool_router, Json, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use voice_to_text_service::{default_data_dir, Request, Response, ServiceClient};

const SERVICE_BINARY: &str = "voice-to-text-service";
const PROTOCOL_NOTE: &str = "Tools talk to the shared local service over a Unix socket; inference never runs inside this MCP process.";

#[derive(Debug, Clone)]
struct VoiceToTextServer {
    tool_router: ToolRouter<Self>,
    data_dir: PathBuf,
    allowed_roots: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// Tool arguments
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct TranscribeArgs {
    #[schemars(
        description = "Absolute path of a local audio/video file under a configured --allow-root"
    )]
    path: String,
    #[serde(default)]
    #[schemars(
        description = "Optional model id from list_models; omitted uses the selected model"
    )]
    model_id: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional saved LAN node id from list_nodes; omitted runs locally")]
    node_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct JobIdArgs {
    #[schemars(description = "Stable task ID returned by transcribe_file")]
    job_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ResultArgs {
    #[schemars(description = "Stable task ID returned by transcribe_file")]
    job_id: String,
    #[serde(default)]
    #[schemars(description = "Set true only when the full text is needed; defaults to false")]
    include_text: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ExportArgs {
    #[schemars(description = "Stable task ID")]
    job_id: String,
    #[schemars(description = "txt or json")]
    format: String,
    #[schemars(description = "Absolute output path under a configured --allow-root")]
    path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct AddNodeArgs {
    #[schemars(description = "Friendly name for the node")]
    name: String,
    #[schemars(
        description = "Plain host string: IP, .local name, tailnet name or any DNS name. No URL scheme."
    )]
    host: String,
    #[serde(default)]
    #[schemars(description = "Optional port; defaults to 7878")]
    port: Option<u16>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateNodeArgs {
    #[schemars(description = "Saved node id from list_nodes")]
    id: String,
    #[serde(default)]
    #[schemars(description = "Enable or disable dispatching to this node")]
    enabled: Option<bool>,
    #[serde(default)]
    #[schemars(description = "New plain host string; no URL scheme")]
    host: Option<String>,
    #[serde(default)]
    #[schemars(description = "New port")]
    port: Option<u16>,
    #[serde(default)]
    #[schemars(description = "New remote model id; empty string clears it")]
    model_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct NodeIdArgs {
    #[schemars(description = "Saved node id from list_nodes")]
    id: String,
}

// ---------------------------------------------------------------------------
// Tool replies
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ModelInfo {
    id: String,
    display_name: String,
    family: String,
    bytes: u64,
    selected: bool,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ModelsReply {
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_model_id: Option<String>,
    models: Vec<ModelInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    library_path: Option<String>,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct JobReply {
    job_id: String,
    state: String,
    phase: String,
    completed_chunks: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_chunks: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct QueueItem {
    job_id: String,
    position: usize,
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    owner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct FinishedItem {
    job_id: String,
    state: String,
    phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct JobsReply {
    #[serde(skip_serializing_if = "Option::is_none")]
    active: Option<String>,
    pending: Vec<QueueItem>,
    finished: Vec<FinishedItem>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ResultReply {
    job_id: String,
    source_file_name: String,
    model_id: String,
    engine: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
    preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    full_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    segment_count: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ExportReply {
    path: String,
    format: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CancelReply {
    job_id: String,
    state: String,
    cancellation_requested: bool,
    message: String,
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// 拒绝任何 URL scheme：host 必须是纯主机字符串（IP / `.local` / tailnet 名 /
/// 任意 DNS 名）。任务书 §8 测试 7 用 `ssh://x` 断言这里会拒绝。
fn validate_host_value(host: &str) -> Result<String, String> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return Err("A LAN node needs a host.".to_owned());
    }
    if trimmed.contains("://") {
        return Err(format!(
            "The host must be a plain name or address, not a URL (got '{trimmed}'); a scheme like 'ssh://' is not accepted."
        ));
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("The host must not contain whitespace or control characters.".to_owned());
    }
    Ok(trimmed.to_owned())
}

/// 名称非空即可；它只作为数据显示，绝不进入任何命令行。
fn validate_name_value(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("A LAN node needs a name.".to_owned());
    }
    Ok(trimmed.to_owned())
}

/// 端口必须在 1..=65535；0 会被当成「未设置」而静默回落，因此显式拒绝。
fn validate_port_value(port: Option<u16>) -> Result<Option<u16>, String> {
    match port {
        Some(0) => Err("The port must be between 1 and 65535.".to_owned()),
        other => Ok(other),
    }
}

impl VoiceToTextServer {
    fn new(data_dir: PathBuf, roots: Vec<PathBuf>) -> Result<Self, String> {
        let mut allowed = Vec::new();
        for root in roots {
            let canonical = fs::canonicalize(&root)
                .map_err(|_| format!("--allow-root cannot be read: {}", root.display()))?;
            if !canonical.is_dir() {
                return Err(format!(
                    "--allow-root must be a directory: {}",
                    root.display()
                ));
            }
            allowed.push(canonical);
        }
        Ok(VoiceToTextServer {
            tool_router: Self::tool_router(),
            data_dir,
            allowed_roots: allowed,
        })
    }

    /// Reject anything that is not a saved, enabled node id before any IPC or
    /// network request. The MCP surface never accepts host/port/url.
    fn validate_node(&self, node_id: &str) -> Result<(), String> {
        let path = self.data_dir.join("lan-settings.json");
        let bytes = fs::read(&path).map_err(|_| {
            "No saved LAN node matches this id; add the node in the app first.".to_owned()
        })?;
        let settings: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "The saved LAN settings are invalid.".to_owned())?;
        let nodes = settings
            .get("nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let node = nodes
            .iter()
            .find(|node| node.get("id").and_then(Value::as_str) == Some(node_id));
        let Some(node) = node else {
            return Err(format!("Unknown LAN node id: {node_id}"));
        };
        if !node
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(format!("The LAN node {node_id} is disabled."));
        }
        Ok(())
    }

    fn connect(&self) -> Result<ServiceClient, String> {
        if let Ok(client) = ServiceClient::connect(&self.data_dir) {
            return Ok(client);
        }
        let binary = service_binary()?;
        let mut command = Command::new(binary);
        command
            .arg("serve")
            .arg("--data-dir")
            .arg(&self.data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for root in &self.allowed_roots {
            command.arg("--allow-root").arg(root);
        }
        command
            .spawn()
            .map_err(|error| format!("Cannot start the local service: {error}"))?;
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            match ServiceClient::connect(&self.data_dir) {
                Ok(client) => return Ok(client),
                Err(error) => {
                    if Instant::now() >= deadline {
                        return Err(format!("The local service did not become ready: {error}"));
                    }
                    thread::sleep(Duration::from_millis(120));
                }
            }
        }
    }

    fn authorize_input(&self, supplied: &str) -> Result<PathBuf, String> {
        if self.allowed_roots.is_empty() {
            return Err(
                "MCP file access is disabled. Start the server with --allow-root directories."
                    .to_owned(),
            );
        }
        let path = Path::new(supplied);
        if !path.is_absolute() {
            return Err("The input path must be absolute.".to_owned());
        }
        let canonical = fs::canonicalize(path)
            .map_err(|error| format!("The path cannot be accessed ({}).", error.kind()))?;
        if !self
            .allowed_roots
            .iter()
            .any(|root| canonical.starts_with(root))
        {
            return Err(
                "The path is outside the directories authorized for this MCP server.".to_owned(),
            );
        }
        Ok(canonical)
    }

    fn authorize_output(&self, supplied: &str) -> Result<PathBuf, String> {
        if self.allowed_roots.is_empty() {
            return Err(
                "MCP file access is disabled. Start the server with --allow-root directories."
                    .to_owned(),
            );
        }
        let path = Path::new(supplied);
        if !path.is_absolute() {
            return Err("The output path must be absolute.".to_owned());
        }
        let parent = path
            .parent()
            .ok_or_else(|| "The output path has no parent directory.".to_owned())?;
        let canonical = fs::canonicalize(parent).map_err(|error| {
            format!(
                "The output directory cannot be accessed ({}).",
                error.kind()
            )
        })?;
        if !self
            .allowed_roots
            .iter()
            .any(|root| canonical.starts_with(root))
        {
            return Err("The output directory is outside the authorized roots.".to_owned());
        }
        canonical
            .join(path.file_name().unwrap_or_default())
            .into_os_string()
            .into_string()
            .map(PathBuf::from)
            .map_err(|_| "The output path is not valid UTF-8.".to_owned())
    }

    fn data_of(response: Response) -> Result<Value, String> {
        if !response.ok {
            return Err(response
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| "The local service rejected the request.".to_owned()));
        }
        Ok(response.data.unwrap_or(Value::Null))
    }
}

fn service_binary() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("LOCAL_TRANSCRIBE_SERVICE") {
        return Ok(PathBuf::from(path));
    }
    let exe = env::current_exe().map_err(|error| error.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "Cannot locate the service binary directory.".to_owned())?;
    for candidate in [SERVICE_BINARY, "voice-to-text-service.exe"] {
        let path = dir.join(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err("voice-to-text-service was not found next to this executable.".to_owned())
}

#[tool_router]
impl VoiceToTextServer {
    /// List imported models and the currently selected default.
    #[tool(
        name = "list_models",
        description = "List imported local models and the selected default.",
        annotations(read_only_hint = true)
    )]
    async fn list_models(&self) -> Result<Json<ModelsReply>, String> {
        let mut client = self.connect()?;
        let data = Self::data_of(client.request(&Request::Models)?)?;
        let mut reply: ModelsReply =
            serde_json::from_value(data).map_err(|error| error.to_string())?;
        reply.note = PROTOCOL_NOTE.to_owned();
        Ok(Json(reply))
    }

    /// List saved LAN nodes with a short reachability probe.
    #[tool(
        name = "list_nodes",
        description = "List saved LAN nodes, whether each is reachable, and its current model."
    )]
    async fn list_nodes(&self) -> Result<Json<serde_json::Value>, String> {
        let mut client = self.connect()?;
        let data = Self::data_of(client.request(&Request::LanNodes)?)?;
        Ok(Json(data))
    }

    /// Add a saved node after probing its capabilities.
    #[tool(
        name = "add_node",
        description = "Add a LAN node after probing it; returns a capabilities summary. Unreachable hosts are rejected."
    )]
    async fn add_node(&self, Parameters(args): Parameters<AddNodeArgs>) -> Result<Json<Value>, String> {
        // 在连接服务之前先校验：坏输入不产生任何网络或 IPC 动作。
        let name = validate_name_value(&args.name)?;
        let host = validate_host_value(&args.host)?;
        let port = validate_port_value(args.port)?;
        let mut client = self.connect()?;
        let data = Self::data_of(client.request(&Request::LanAdd {
            name,
            host,
            port,
            // MCP 不提供 force：不可达的地址一律拒绝，避免存进一个用不了的节点。
            force: false,
        })?)?;
        Ok(Json(data))
    }

    /// Change a saved node's dispatch fields.
    #[tool(
        name = "update_node",
        description = "Update a saved node: enabled, host, port or remote model id."
    )]
    async fn update_node(
        &self,
        Parameters(args): Parameters<UpdateNodeArgs>,
    ) -> Result<Json<Value>, String> {
        if args.id.trim().is_empty() {
            return Err("A node id is required.".to_owned());
        }
        let host = match args.host.as_deref() {
            Some(host) => Some(validate_host_value(host)?),
            None => None,
        };
        let port = validate_port_value(args.port)?;
        if args.enabled.is_none() && host.is_none() && port.is_none() && args.model_id.is_none() {
            return Err(
                "Nothing to change: pass at least one of enabled, host, port or modelId.".to_owned(),
            );
        }
        let mut client = self.connect()?;
        let data = Self::data_of(client.request(&Request::LanSet {
            node: args.id,
            host,
            port,
            model: args.model_id,
            name: None,
        })?)?;
        Ok(Json(data))
    }

    /// Remove a node from the local list only; the remote machine is untouched.
    #[tool(
        name = "remove_node",
        description = "Forget a saved node locally; nothing is sent to the other machine."
    )]
    async fn remove_node(
        &self,
        Parameters(args): Parameters<NodeIdArgs>,
    ) -> Result<Json<Value>, String> {
        if args.id.trim().is_empty() {
            return Err("A node id is required.".to_owned());
        }
        let mut client = self.connect()?;
        let data = Self::data_of(client.request(&Request::LanRemove { node: args.id })?)?;
        Ok(Json(data))
    }

    /// Probe one saved node for reachability, protocol version and models.
    #[tool(
        name = "node_status",
        description = "Probe one saved node: reachability, protocol version and available models.",
        annotations(read_only_hint = true)
    )]
    async fn node_status(
        &self,
        Parameters(args): Parameters<NodeIdArgs>,
    ) -> Result<Json<Value>, String> {
        if args.id.trim().is_empty() {
            return Err("A node id is required.".to_owned());
        }
        let mut client = self.connect()?;
        let data = Self::data_of(client.request(&Request::LanNodeProbe { node: args.id })?)?;
        Ok(Json(data))
    }

    /// Submit a local media file; returns immediately with a job ID.
    #[tool(
        name = "transcribe_file",
        description = "Queue a transcription and return its job ID immediately."
    )]
    async fn transcribe_file(
        &self,
        Parameters(args): Parameters<TranscribeArgs>,
    ) -> Result<Json<JobReply>, String> {
        if let Some(node_id) = args.node_id.as_deref() {
            self.validate_node(node_id)?;
        }
        let path = self.authorize_input(&args.path)?;
        let mut client = self.connect()?;
        let response = client.request(&Request::Submit {
            path: path.to_string_lossy().into_owned(),
            model_dir: None,
            model_id: args.model_id,
            owner: Some("mcp".to_owned()),
            node: args.node_id,
            lan_model: None,
            fallback_local: false,
        })?;
        if !response.ok {
            return Err(response
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| "The local service rejected the task.".to_owned()));
        }
        let job = response
            .job
            .ok_or_else(|| "The service returned no job.".to_owned())?;
        Ok(Json(JobReply {
            job_id: job.job_id,
            state: job.state,
            phase: job.phase,
            completed_chunks: job.completed_chunks,
            total_chunks: job.total_chunks,
            error: job.error,
        }))
    }

    /// Current status of one job.
    #[tool(
        name = "get_job_status",
        description = "Read a job's state, progress and error.",
        annotations(read_only_hint = true)
    )]
    async fn get_job_status(
        &self,
        Parameters(args): Parameters<JobIdArgs>,
    ) -> Result<Json<JobReply>, String> {
        let mut client = self.connect()?;
        let response = client.request(&Request::Status {
            job_id: args.job_id,
        })?;
        if !response.ok {
            return Err(response
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| "No such job.".to_owned()));
        }
        let job = response.job.ok_or_else(|| "No such job.".to_owned())?;
        Ok(Json(JobReply {
            job_id: job.job_id,
            state: job.state,
            phase: job.phase,
            completed_chunks: job.completed_chunks,
            total_chunks: job.total_chunks,
            error: job.error,
        }))
    }

    /// Queue snapshot including waiting and failed jobs.
    #[tool(
        name = "list_jobs",
        description = "List the shared queue (active, waiting, recently finished).",
        annotations(read_only_hint = true)
    )]
    async fn list_jobs(&self) -> Result<Json<JobsReply>, String> {
        let mut client = self.connect()?;
        let data = Self::data_of(client.request(&Request::Jobs)?)?;
        Ok(Json(
            serde_json::from_value(data).map_err(|error| error.to_string())?,
        ))
    }

    /// Result metadata plus a bounded preview; full text is opt-in.
    #[tool(
        name = "get_job_result",
        description = "Read result metadata and a preview; full text is opt-in.",
        annotations(read_only_hint = true)
    )]
    async fn get_job_result(
        &self,
        Parameters(args): Parameters<ResultArgs>,
    ) -> Result<Json<ResultReply>, String> {
        let mut client = self.connect()?;
        let data = Self::data_of(client.request(&Request::JobResult {
            job_id: args.job_id,
            full: args.include_text.unwrap_or(false),
        })?)?;
        Ok(Json(
            serde_json::from_value(data).map_err(|error| error.to_string())?,
        ))
    }

    /// Cancel a running job or remove a waiting one.
    #[tool(
        name = "cancel_job",
        description = "Cancel a running job or remove a waiting one."
    )]
    async fn cancel_job(
        &self,
        Parameters(args): Parameters<JobIdArgs>,
    ) -> Result<Json<CancelReply>, String> {
        let mut client = self.connect()?;
        let job_id = args.job_id;
        let response = client.request(&Request::Cancel {
            job_id: job_id.clone(),
        })?;
        if !response.ok {
            return Err(response
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| "The job cannot be cancelled.".to_owned()));
        }
        let state = response
            .job
            .as_ref()
            .map(|job| job.state.clone())
            .unwrap_or_else(|| "cancelling".to_owned());
        Ok(Json(CancelReply {
            job_id,
            state,
            cancellation_requested: true,
            message: "Cancellation requested; stable segments are kept.".to_owned(),
        }))
    }

    /// Re-queue a failed job with its original model.
    #[tool(
        name = "retry_job",
        description = "Re-queue a failed job using its original model snapshot."
    )]
    async fn retry_job(
        &self,
        Parameters(args): Parameters<JobIdArgs>,
    ) -> Result<Json<JobReply>, String> {
        let mut client = self.connect()?;
        let response = client.request(&Request::Retry {
            job_id: args.job_id,
        })?;
        if !response.ok {
            return Err(response
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| "The job cannot be retried.".to_owned()));
        }
        let job = response
            .job
            .ok_or_else(|| "The service returned no job.".to_owned())?;
        Ok(Json(JobReply {
            job_id: job.job_id,
            state: job.state,
            phase: job.phase,
            completed_chunks: job.completed_chunks,
            total_chunks: job.total_chunks,
            error: job.error,
        }))
    }

    /// Export a finished result to TXT or JSON inside an authorized root.
    #[tool(
        name = "export_result",
        description = "Export a finished result to TXT or JSON under an authorized root."
    )]
    async fn export_result(
        &self,
        Parameters(args): Parameters<ExportArgs>,
    ) -> Result<Json<ExportReply>, String> {
        let target = self.authorize_output(&args.path)?;
        let mut client = self.connect()?;
        let data = Self::data_of(client.request(&Request::Export {
            job_id: args.job_id,
            path: target.to_string_lossy().into_owned(),
            format: args.format,
        })?)?;
        Ok(Json(
            serde_json::from_value(data).map_err(|error| error.to_string())?,
        ))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for VoiceToTextServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "local-transcribe-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(PROTOCOL_NOTE)
    }
}

async fn serve() -> Result<(), String> {
    let mut data_dir = None;
    let mut roots = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => {
                data_dir = Some(PathBuf::from(
                    args.next().ok_or("--data-dir needs a value")?,
                ));
            }
            "--allow-root" => {
                roots.push(PathBuf::from(
                    args.next().ok_or("--allow-root needs a value")?,
                ));
            }
            "--help" | "-h" => {
                println!("local-transcribe-mcp [--data-dir DIR] [--allow-root DIR]...");
                return Ok(());
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }
    let data_dir = match data_dir {
        Some(dir) => dir,
        None => default_data_dir()?,
    };
    let server = VoiceToTextServer::new(data_dir, roots)?;
    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|error| format!("MCP handshake failed: {error}"))?;
    service.waiting().await.map_err(|error| error.to_string())?;
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = serve().await {
        eprintln!("local-transcribe-mcp: {error}");
        process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("local-transcribe-mcp-{now}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn exposes_the_documented_tool_set() {
        let server = VoiceToTextServer::new(temp_root(), Vec::new()).unwrap();
        let mut names = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            [
                "add_node",
                "cancel_job",
                "export_result",
                "get_job_result",
                "get_job_status",
                "list_jobs",
                "list_models",
                "list_nodes",
                "node_status",
                "remove_node",
                "retry_job",
                "transcribe_file",
                "update_node",
            ]
        );
    }

    /// 任务书 §8 测试 8：任何工具都不接受任意命令参数（schema 断言）。
    #[test]
    fn no_tool_accepts_an_arbitrary_command_parameter() {
        let server = VoiceToTextServer::new(temp_root(), Vec::new()).unwrap();
        // 这些名字一旦出现在参数里，就等于给模型开了「在本机执行任意东西」的口子。
        let forbidden = [
            "command", "cmd", "exec", "execute", "shell", "sh", "bash", "zsh", "powershell",
            "eval", "script", "argv", "args", "env", "cwd", "url", "uri", "scheme",
        ];
        for tool in server.tool_router.list_all() {
            let mut properties = Vec::new();
            collect_property_names(&Value::Object((*tool.input_schema).clone()), &mut properties);
            for property in &properties {
                assert!(
                    !forbidden.contains(&property.as_str()),
                    "tool '{}' must not accept a '{}' parameter",
                    tool.name,
                    property
                );
            }
            // 参数必须封闭在声明的字段里，不能随手塞进别的键。
            assert_ne!(
                tool.input_schema.get("additionalProperties"),
                Some(&Value::Bool(true)),
                "tool '{}' must not accept arbitrary extra parameters",
                tool.name
            );
        }
    }

    /// 递归收集 JSON Schema 里所有属性名（含 `$ref` 之外的内联 object）。
    fn collect_property_names(schema: &Value, out: &mut Vec<String>) {
        match schema {
            Value::Object(map) => {
                if let Some(Value::Object(properties)) = map.get("properties") {
                    for (name, child) in properties {
                        out.push(name.clone());
                        collect_property_names(child, out);
                    }
                }
                for value in map.values() {
                    collect_property_names(value, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect_property_names(item, out);
                }
            }
            _ => {}
        }
    }

    /// 任务书 §8 测试 7：`add_node` 传未知 scheme（`ssh://x`）在连接服务之前就被拒绝。
    #[test]
    fn add_node_rejects_a_url_scheme_before_touching_the_service() {
        let data = temp_root();
        let server = VoiceToTextServer::new(data.clone(), Vec::new()).unwrap();
        let args = AddNodeArgs {
            name: "desk".to_owned(),
            host: "ssh://x".to_owned(),
            port: None,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = match runtime.block_on(server.add_node(Parameters(args))) {
            Ok(_) => panic!("a URL scheme must be rejected"),
            Err(error) => error,
        };
        assert!(error.contains("ssh://"), "unhelpful error: {error}");
        // 校验先于 connect()：服务不应被拉起，也不该写任何设置。
        assert!(!data.join("service.sock").exists());
        assert!(!data.join("service.port").exists());
        assert!(!data.join("lan-settings.json").exists());

        // 控制字符同样拒绝。
        assert!(validate_host_value("host\tname").is_err());
        assert!(validate_host_value("").is_err());
        assert!(validate_host_value("   ").is_err());
        // 纯主机字符串照常通过（IP / .local / tailnet 名 / DNS）。
        assert!(validate_host_value("192.168.1.5").is_ok());
        assert!(validate_host_value("mbp.local").is_ok());
        assert!(validate_host_value("box.tail-scale.ts.net").is_ok());
        assert!(validate_host_value("ssh://x").is_err());
        // 端口 0 显式拒绝，避免被当成「未设置」。
        assert!(validate_port_value(Some(0)).is_err());
        assert!(validate_port_value(None).is_ok());
    }

    #[test]
    fn unknown_and_disabled_nodes_are_rejected_without_starting_a_service() {
        let base = temp_root();
        let allowed = base.join("allowed");
        fs::create_dir_all(&allowed).unwrap();
        let media = allowed.join("voice.wav");
        fs::write(&media, b"x").unwrap();
        let data = base.join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            data.join("lan-settings.json"),
            r#"{"nodes":[
                {"id":"known","name":"desk","host":"127.0.0.1","port":9,"enabled":true},
                {"id":"off","name":"off","host":"127.0.0.1","port":9,"enabled":false}
            ]}"#,
        )
        .unwrap();
        let server = VoiceToTextServer::new(data.clone(), vec![allowed]).unwrap();
        assert!(server.validate_node("known").is_ok());
        assert!(server.validate_node("missing").is_err());
        assert!(server.validate_node("off").is_err());

        let args = TranscribeArgs {
            path: media.to_string_lossy().into_owned(),
            model_id: None,
            node_id: Some("missing".to_owned()),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(server.transcribe_file(Parameters(args)));
        assert!(result.is_err(), "unknown node must be rejected");
        // Nothing tried to start the local service (no socket/port file).
        assert!(!data.join("service.sock").exists());
        assert!(!data.join("service.port").exists());
    }

    #[test]
    fn unknown_node_id_makes_no_network_request() {
        let base = temp_root();
        let allowed = base.join("allowed");
        fs::create_dir_all(&allowed).unwrap();
        let media = allowed.join("voice.wav");
        fs::write(&media, b"x").unwrap();
        let data = base.join("data");
        fs::create_dir_all(&data).unwrap();

        // A real listener on the configured node address: any connection would
        // be accepted here, so the test can prove the request never happened.
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        fs::write(
            data.join("lan-settings.json"),
            format!(
                r#"{{"nodes":[{{"id":"known","name":"desk","host":"127.0.0.1","port":{port},"enabled":true}}]}}"#
            ),
        )
        .unwrap();

        let server = VoiceToTextServer::new(data, vec![allowed]).unwrap();
        let args = TranscribeArgs {
            path: media.to_string_lossy().into_owned(),
            model_id: None,
            node_id: Some("missing".to_owned()),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(server.transcribe_file(Parameters(args)));
        assert!(result.is_err(), "unknown node must be rejected");

        listener.set_nonblocking(true).unwrap();
        match listener.accept() {
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Ok(_) => panic!("unknown node id must not open a network connection"),
            Err(error) => panic!("unexpected accept error: {error}"),
        }
    }

    #[test]
    fn file_access_is_limited_to_canonical_authorized_directories() {
        let base = temp_root();
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        fs::create_dir_all(&allowed).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let permitted = allowed.join("voice.wav");
        let denied = outside.join("voice.wav");
        fs::write(&permitted, b"x").unwrap();
        fs::write(&denied, b"x").unwrap();

        let server = VoiceToTextServer::new(base.join("data"), vec![allowed]).unwrap();
        assert!(server.authorize_input(permitted.to_str().unwrap()).is_ok());
        assert!(server.authorize_input(denied.to_str().unwrap()).is_err());
        assert!(server.authorize_output(denied.to_str().unwrap()).is_err());
    }

    #[test]
    fn no_root_means_file_access_is_denied() {
        let base = temp_root();
        let media = base.join("voice.wav");
        fs::write(&media, b"x").unwrap();
        let server = VoiceToTextServer::new(base.join("data"), Vec::new()).unwrap();
        let error = server
            .authorize_input(media.to_str().unwrap())
            .expect_err("no roots must deny access");
        assert!(error.contains("--allow-root"), "{error}");
    }
}
