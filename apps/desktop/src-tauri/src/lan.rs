//! 局域网 Compute V1.1 · Client 端命令层。
//!
//! 消费 `voice-to-text-lan` 的公共 API：mDNS 发现、任务提交（4 MiB 分块上传）、
//! 状态轮询、取消、结果取回与收集。设置持久化在应用数据目录
//! `lan-settings.json`（原子写）；访问令牌只存本地文件，绝不打印日志。
//!
//! 本模块的对外数据一律 snake_case（与 LAN 协议 wire 格式一致）。
//! 转写阶段文案以稳定 key（`lan.phase.*`）返回，由前端 i18n 渲染。

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use voice_to_text_core::{
    InputFile, JobLease, JobState, JobStatus, JobStore, LocalModel, Transcript, TranscriptSegment,
    transcribe_file_streaming,
};
use voice_to_text_lan::client::{LanClient, LanError, discover_blocking};
use voice_to_text_lan::protocol::{
    Capabilities, CollectReport, NodeInfo, StorageInfo, Task, TaskResult, TaskState,
};

use crate::{AppState, app_data_dir, apply_retention, save_transcript, unix_now_ms, update_job};

/// 传输进度事件名；载荷为 [`LanProgressEvent`]。
pub(crate) const LAN_PROGRESS_EVENT: &str = "lan://progress";

const LAN_SETTINGS_FILE: &str = "lan-settings.json";
/// 单个上传分块大小（任务书：4 MiB）。
const CHUNK_BYTES: u64 = 4 * 1024 * 1024;
/// 单次节点请求（健康、能力、状态、结果）的响应上限。
const NODE_TIMEOUT: Duration = Duration::from_secs(3);
/// 任务状态轮询间隔。
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// 轮询连续失败多少次判定与节点失联。
const MAX_POLL_FAILURES: u32 = 5;

/// 派发模式。`prefer_lan` 时新任务自动发往已启用且在线的节点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanMode {
    #[serde(alias = "prefer_local")]
    LocalOnly,
    PreferLan,
}

/// 一个已保存的节点配置；`id` 为空时由 `set_lan_settings` 派发 UUID。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanNodeConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    /// 节点访问令牌。只存本地配置文件，不写日志。
    pub token: String,
    pub enabled: bool,
}

/// 局域网计算设置：模式 + 节点列表。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanSettings {
    pub mode: LanMode,
    pub nodes: Vec<LanNodeConfig>,
}

impl Default for LanSettings {
    fn default() -> Self {
        Self {
            mode: LanMode::LocalOnly,
            nodes: Vec::new(),
        }
    }
}

/// 单节点探测结果。
#[derive(Debug, Clone, Serialize)]
pub struct LanNodeStatus {
    pub reachable: bool,
    pub capabilities: Option<Capabilities>,
    pub latency_ms: Option<u64>,
    /// Storage probe result; independent from reachability.
    pub storage: Option<StorageInfo>,
    /// Non-fatal probe problem (e.g. storage rejected the token), kept
    /// separate so it can never overwrite a reachable node as offline.
    pub warning: Option<String>,
}

/// Assemble the node overview from independent probes. Reachability is
/// decided by the health probe alone: a node that answers but refuses a
/// later call (bad token, storage endpoint down) is still online, and the
/// caller gets the specific problem in `warning`.
fn assemble_status(
    health: Result<bool, String>,
    latency_ms: u64,
    capabilities: Option<Capabilities>,
    storage: Result<StorageInfo, String>,
) -> LanNodeStatus {
    if !matches!(health, Ok(true)) {
        return LanNodeStatus {
            reachable: false,
            capabilities: None,
            latency_ms: None,
            storage: None,
            warning: None,
        };
    }
    match storage {
        Ok(storage) => LanNodeStatus {
            reachable: true,
            capabilities,
            latency_ms: Some(latency_ms),
            storage: Some(storage),
            warning: None,
        },
        Err(warning) => LanNodeStatus {
            reachable: true,
            capabilities,
            latency_ms: Some(latency_ms),
            storage: None,
            warning: Some(warning),
        },
    }
}

/// `lan://progress` 事件载荷：某个节点任务的传输进度。
#[derive(Debug, Clone, Serialize)]
pub struct LanProgressEvent {
    pub task_id: String,
    pub sent: u64,
    pub total: u64,
}

/// 校验设置：节点必须有 id/名称/主机/端口，且 id 不重复。错误信息不含令牌。
fn validate_lan_settings(settings: &LanSettings) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for node in &settings.nodes {
        if node.id.trim().is_empty() {
            return Err("A LAN node is missing its identifier.".to_owned());
        }
        if !ids.insert(node.id.clone()) {
            return Err("Two LAN nodes share the same identifier.".to_owned());
        }
        if node.name.trim().is_empty() {
            return Err("A LAN node needs a name.".to_owned());
        }
        if node.host.trim().is_empty() {
            return Err("A LAN node needs a host name or address.".to_owned());
        }
        if node.port == 0 {
            return Err("A LAN node needs a port between 1 and 65535.".to_owned());
        }
    }
    Ok(())
}

/// 给空 id 的节点派发 UUID v4（前端新建节点时填空 id 即可）。
fn assign_missing_ids(settings: &mut LanSettings) {
    for node in &mut settings.nodes {
        if node.id.trim().is_empty() {
            node.id = uuid::Uuid::new_v4().to_string();
        }
    }
}

/// 按保存顺序返回启用中的节点（派发与探测只看这些）。
fn enabled_nodes(settings: &LanSettings) -> Vec<LanNodeConfig> {
    settings
        .nodes
        .iter()
        .filter(|node| node.enabled)
        .cloned()
        .collect()
}

/// 任务九态 → 前端 i18n key（`lan.phase.*`），由界面按语言渲染。
fn task_phase(state: TaskState) -> &'static str {
    match state {
        TaskState::Init => "lan.phase.preparing",
        TaskState::FileTransferring => "lan.phase.uploading",
        TaskState::Queued => "lan.phase.queued",
        TaskState::Transcribing => "lan.phase.transcribing",
        TaskState::PostProcessing => "lan.phase.post_processing",
        TaskState::Completed => "lan.phase.completed",
        TaskState::Failed => "lan.phase.failed",
        TaskState::CancelRequested => "lan.phase.cancelling",
        TaskState::Cancelled => "lan.phase.cancelled",
    }
}

/// 节点结果 → 本地历史记录（与本机转写同一 `Transcript` 结构）。
fn transcript_from_lan(
    job_id: &str,
    source_file_name: &str,
    model_id: &str,
    result: &TaskResult,
) -> Transcript {
    Transcript {
        schema_version: "1".to_owned(),
        job_id: job_id.to_owned(),
        source_file_name: source_file_name.to_owned(),
        model_id: model_id.to_owned(),
        model_version: None,
        engine: format!(
            "lan-compute/v{}",
            voice_to_text_lan::protocol::PROTOCOL_VERSION
        ),
        language: None,
        duration_ms: Some(result.duration_ms),
        text: result.transcript.clone(),
        segments: Some(
            result
                .segments
                .iter()
                .map(|segment| TranscriptSegment {
                    start_ms: segment.start_ms,
                    end_ms: segment.end_ms,
                    text: segment.text.clone(),
                })
                .collect(),
        ),
    }
}

fn lan_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join(LAN_SETTINGS_FILE))
}

pub(crate) fn read_lan_settings(app: &AppHandle) -> Result<LanSettings, String> {
    let path = lan_config_path(app)?;
    if !path.exists() {
        return Ok(LanSettings::default());
    }
    let bytes =
        fs::read(&path).map_err(|_| "Could not read the LAN compute settings.".to_owned())?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "The saved LAN compute settings are invalid.".to_owned())
}

/// 原子写（同 selected-model.json / retention.json 的做法），写后回读校验。
fn write_lan_settings(app: &AppHandle, settings: &LanSettings) -> Result<(), String> {
    validate_lan_settings(settings)?;
    let path = lan_config_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "The app's local data folder is unavailable.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|_| "Could not prepare the LAN compute settings.".to_owned())?;
    let temporary = parent.join(format!("{LAN_SETTINGS_FILE}.tmp"));
    let contents = serde_json::to_vec_pretty(settings)
        .map_err(|_| "Could not encode the LAN compute settings.".to_owned())?;
    fs::write(&temporary, &contents)
        .map_err(|_| "Could not save the LAN compute settings.".to_owned())?;
    fs::rename(&temporary, &path)
        .map_err(|_| "Could not save the LAN compute settings.".to_owned())?;
    let saved =
        fs::read(&path).map_err(|_| "Could not verify the LAN compute settings.".to_owned())?;
    if saved != contents {
        return Err("The saved LAN compute settings did not match the chosen settings.".to_owned());
    }
    Ok(())
}

fn find_node(app: &AppHandle, node_id: &str) -> Result<LanNodeConfig, String> {
    read_lan_settings(app)?
        .nodes
        .into_iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| "That LAN node is no longer in the settings.".to_owned())
}

fn client_for(node: &LanNodeConfig) -> LanClient {
    LanClient::new(node.host.clone(), node.port, node.token.clone())
}

/// 单次节点请求：超时返回可读错误；`LanError` 的展示从不携带令牌。
async fn timed<T>(future: impl Future<Output = Result<T, LanError>>) -> Result<T, String> {
    match tokio::time::timeout(NODE_TIMEOUT, future).await {
        Ok(result) => result.map_err(|error| error.to_string()),
        Err(_) => Err("The node did not respond in time.".to_owned()),
    }
}

/// 创建任务 + 分块上传；`progress(task_id, sent, total)` 在每个分块被接受后调用。
async fn submit_upload(
    client: &LanClient,
    path: &Path,
    model_id: Option<&str>,
    progress: impl Fn(&str, u64, u64) + Send + 'static,
) -> Result<Task, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("media")
        .to_owned();
    let file_size = fs::metadata(path)
        .map_err(|_| "The selected file could not be read.".to_owned())?
        .len();
    let chunks_total = file_size.div_ceil(CHUNK_BYTES).max(1) as u32;
    let task = timed(client.create_task(&file_name, file_size, chunks_total, model_id)).await?;
    let progress_id = task.id.clone();
    client
        .upload_file(&task.id, path, CHUNK_BYTES, move |sent, total| {
            progress(&progress_id, sent, total);
        })
        .await
        .map_err(|error| error.to_string())?;
    Ok(task)
}

/// 派发决策：首个通过认证且启用计算的节点（每个最多 [`NODE_TIMEOUT`]）。
pub(crate) async fn first_reachable_node(
    settings: &LanSettings,
) -> Result<Option<LanNodeConfig>, String> {
    for node in enabled_nodes(settings) {
        let client = client_for(&node);
        // A health response alone does not prove authentication or compute readiness.
        if matches!(timed(client.capabilities()).await, Ok(caps) if caps.compute_enabled) {
            return Ok(Some(node));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// 命令实现（`#[tauri::command]` 包装在 lib.rs，与既有命令同处一层）
// ---------------------------------------------------------------------------

/// 保存设置：空 id 派发 UUID，校验后原子写入并回读验证。
pub(crate) fn save_settings(
    app: &AppHandle,
    mut settings: LanSettings,
) -> Result<LanSettings, String> {
    assign_missing_ids(&mut settings);
    validate_lan_settings(&settings)?;
    write_lan_settings(app, &settings)?;
    Ok(settings)
}

pub(crate) fn set_assignment(app: &AppHandle, mode: LanMode) -> Result<LanSettings, String> {
    let mut settings = read_lan_settings(app)?;
    settings.mode = mode;
    write_lan_settings(app, &settings)?;
    Ok(settings)
}

/// mDNS 扫描局域网节点（阻塞实现放线程池，避免卡住界面）。
pub(crate) async fn discover_nodes(timeout_ms: u32) -> Result<Vec<NodeInfo>, String> {
    let timeout = Duration::from_millis(u64::from(timeout_ms.clamp(500, 15_000)));
    tauri::async_runtime::spawn_blocking(move || discover_blocking(timeout))
        .await
        .map_err(|error| format!("Node discovery failed: {error}"))
}

/// 探测一个已保存节点：可达性、能力与延迟。节点不可达不是错误。
pub(crate) async fn node_status(app: &AppHandle, node_id: &str) -> Result<LanNodeStatus, String> {
    let node = find_node(app, node_id)?;
    let client = client_for(&node);
    let started = Instant::now();
    let health = timed(client.health()).await;
    let latency_ms = started.elapsed().as_millis() as u64;
    // Probes stay independent: only the health probe decides reachability,
    // and later probes run solely for reachable nodes (they would just burn
    // timeouts otherwise).
    let storage = if matches!(health, Ok(true)) {
        timed(client.storage())
            .await
            .map_err(|error| error.to_string())
    } else {
        Err(String::new()) // not probed; the unreachable branch discards it
    };
    let capabilities = if matches!(health, Ok(true)) {
        timed(client.capabilities()).await.ok()
    } else {
        None
    };
    Ok(assemble_status(
        health.map_err(|error| error.to_string()),
        latency_ms,
        capabilities,
        storage,
    ))
}

/// 发送一个文件到节点并返回节点任务 id（上传进度走 `lan://progress` 事件）。
pub(crate) async fn submit_file(
    app: &AppHandle,
    node_id: &str,
    file_path: &str,
    model_id: Option<&str>,
) -> Result<String, String> {
    let node = find_node(app, node_id)?;
    let client = client_for(&node);
    let progress_app = app.clone();
    let task = submit_upload(
        &client,
        Path::new(file_path),
        model_id,
        move |task_id, sent, total| {
            let _ = progress_app.emit(
                LAN_PROGRESS_EVENT,
                LanProgressEvent {
                    task_id: task_id.to_owned(),
                    sent,
                    total,
                },
            );
        },
    )
    .await?;
    Ok(task.id)
}

pub(crate) async fn list_tasks(app: &AppHandle, node_id: &str) -> Result<Vec<Task>, String> {
    let node = find_node(app, node_id)?;
    timed(client_for(&node).tasks()).await
}

pub(crate) async fn task_status(
    app: &AppHandle,
    node_id: &str,
    task_id: &str,
) -> Result<Task, String> {
    let node = find_node(app, node_id)?;
    timed(client_for(&node).task(task_id)).await
}

pub(crate) async fn cancel_task(
    app: &AppHandle,
    node_id: &str,
    task_id: &str,
) -> Result<Task, String> {
    let node = find_node(app, node_id)?;
    timed(client_for(&node).cancel(task_id)).await
}

pub(crate) async fn fetch_result(
    app: &AppHandle,
    node_id: &str,
    task_id: &str,
) -> Result<TaskResult, String> {
    let node = find_node(app, node_id)?;
    timed(client_for(&node).result(task_id)).await
}

/// 取回节点任务结果并写入本地历史（同一条记录可在历史页/结果视图打开）。
pub(crate) async fn save_result(
    app: &AppHandle,
    state: &Arc<AppState>,
    node_id: &str,
    task_id: &str,
) -> Result<Transcript, String> {
    let node = find_node(app, node_id)?;
    let client = client_for(&node);
    let task = timed(client.task(task_id)).await?;
    let result = timed(client.result(task_id)).await?;
    let model_id = task
        .model_id
        .clone()
        .unwrap_or_else(|| "lan-node".to_owned());
    let transcript = transcript_from_lan(&task.id, &task.file_name, &model_id, &result);
    save_transcript(app, &transcript)?;
    if let Ok(mut transcripts) = state.transcripts.lock() {
        transcripts.insert(transcript.job_id.clone(), transcript.clone());
    }
    Ok(transcript)
}

/// 把节点上已完成的任务导出到节点侧目录（路径属于节点，不是本机）。
pub(crate) async fn collect(
    app: &AppHandle,
    node_id: &str,
    output_dir: &str,
) -> Result<CollectReport, String> {
    if output_dir.trim().is_empty() {
        return Err("Choose a folder on the node for the export.".to_owned());
    }
    let node = find_node(app, node_id)?;
    timed(client_for(&node).collect(output_dir)).await
}

pub(crate) async fn node_storage(app: &AppHandle, node_id: &str) -> Result<StorageInfo, String> {
    let node = find_node(app, node_id)?;
    timed(client_for(&node).storage()).await
}

// ---------------------------------------------------------------------------
// 自动派发：prefer_lan 时把本机转写任务交给节点执行
// ---------------------------------------------------------------------------

/// 一次节点转写的全部上下文；工作循环持有它并负责收尾。
struct LanJob {
    app: AppHandle,
    state: Arc<AppState>,
    store: JobStore,
    lease: std::sync::Mutex<JobLease>,
    job_id: String,
    source_path: PathBuf,
    file_name: String,
    node: LanNodeConfig,
    model_id: Option<String>,
    local_model: Option<LocalModel>,
    failure: Option<String>,
    started_at_ms: u64,
}

impl LanJob {
    /// 每次状态变化都持久化 + 推给界面（与本机任务同一事件）。
    fn publish(&self, status: JobStatus) {
        if let Ok(mut lease) = self.lease.lock() {
            let _ = lease.save_status(status.clone());
        }
        update_job(&self.app, &self.state, status);
    }

    fn running_status(&self, phase: &str, completed_chunks: usize) -> JobStatus {
        JobStatus {
            job_id: self.job_id.clone(),
            state: JobState::Running,
            phase: phase.to_owned(),
            completed_chunks,
            total_chunks: None,
            started_at_ms: Some(self.started_at_ms),
            total_duration_ms: None,
            transcript_available: false,
            result_saved: false,
            error: None,
        }
    }

    fn finish(&mut self, state: JobState, phase: &str, error: Option<String>) {
        // Keep the same running job and lease until the fallback is resolved.
        if state == JobState::Failed {
            self.failure = Some(error.unwrap_or_else(|| "LAN transcription failed.".into()));
            return;
        }
        self.publish(JobStatus {
            job_id: self.job_id.clone(),
            state,
            phase: phase.to_owned(),
            completed_chunks: 0,
            total_chunks: None,
            started_at_ms: Some(self.started_at_ms),
            total_duration_ms: None,
            transcript_available: state == JobState::Completed,
            result_saved: state == JobState::Completed && error.is_none(),
            error,
        });
    }
}

/// 注册任务并把执行交给节点：上传 → 轮询 → 取回结果写入本地历史。
/// 与本机转写共用 JobStore（同一时刻只允许一个在跑的任务）。
pub(crate) fn start_lan_transcription(
    app: &AppHandle,
    state: &Arc<AppState>,
    input: &InputFile,
    node: LanNodeConfig,
    local_model: Option<LocalModel>,
) -> Result<JobStatus, String> {
    let store = JobStore::new(app_data_dir(app)?);
    let status = JobStatus {
        job_id: JobStore::new_job_id(),
        state: JobState::Running,
        phase: task_phase(TaskState::Init).to_owned(),
        completed_chunks: 0,
        total_chunks: None,
        started_at_ms: Some(unix_now_ms()),
        total_duration_ms: None,
        transcript_available: false,
        result_saved: false,
        error: None,
    };
    let (lease, response_status) = {
        let mut jobs = state
            .jobs
            .lock()
            .map_err(|_| "The local task state is unavailable.".to_owned())?;
        if jobs.values().any(|job| job.state == JobState::Running) {
            return Err("A transcription task is already running. Wait for it to finish before starting another.".to_owned());
        }
        let lease = store.begin_job(status.clone())?;
        jobs.insert(status.job_id.clone(), status.clone());
        state
            .source_paths
            .lock()
            .map_err(|_| "The local task state is unavailable.".to_owned())?
            .insert(status.job_id.clone(), input.path.clone());
        (lease, status.clone())
    };

    let job = LanJob {
        app: app.clone(),
        state: Arc::clone(state),
        store: store.clone(),
        lease: std::sync::Mutex::new(lease),
        job_id: status.job_id.clone(),
        source_path: input.path.clone(),
        file_name: input.file_name.clone(),
        node,
        model_id: local_model.as_ref().map(|model| model.id.clone()),
        local_model,
        failure: None,
        started_at_ms: status.started_at_ms.unwrap_or_default(),
    };
    tauri::async_runtime::spawn(async move {
        run_lan_job(job).await;
    });
    Ok(response_status)
}

async fn run_lan_job(mut job: LanJob) {
    run_remote_job(&mut job).await;
    if let Some(error) = job.failure.take() {
        let cancelled = job
            .store
            .cancellation_requested(&job.job_id)
            .unwrap_or(true);
        if should_fallback(JobState::Failed, cancelled) {
            // Inference is blocking; never run it on the async executor.
            let _ =
                tauri::async_runtime::spawn_blocking(move || run_local_fallback(job, error)).await;
        } else {
            job.finish(JobState::Cancelled, "lan.phase.cancelled", None);
        }
    }
}

fn should_fallback(state: JobState, cancellation_requested: bool) -> bool {
    state == JobState::Failed && !cancellation_requested
}

fn run_local_fallback(job: LanJob, remote_error: String) {
    let cancelled = || {
        job.store
            .cancellation_requested(&job.job_id)
            .unwrap_or(true)
    };
    if cancelled() {
        let mut status = job.running_status("lan.phase.cancelled", 0);
        status.state = JobState::Cancelled;
        job.publish(status);
        return;
    }
    job.publish(job.running_status("lan.phase.fallback", 0));
    let result = job.local_model.as_ref()
        .ok_or_else(|| format!("LAN failed: {remote_error} Local fallback unavailable: choose and install a local model first."))
        .and_then(|model| transcribe_file_streaming(
            &job.source_path, model, cancelled,
            |progress| {
                let mut status = job.running_status("lan.phase.fallback", progress.completed_chunks);
                status.total_chunks = Some(progress.total_chunks);
                status.total_duration_ms = Some(progress.total_duration_ms);
                job.publish(status);
            },
            |update| {
                let _ = job.app.emit(crate::SEGMENT_EVENT, crate::SegmentEvent {
                    job_id: job.job_id.clone(), index: update.index, total_chunks: update.total_chunks,
                    start_ms: update.segment.start_ms, end_ms: update.segment.end_ms, text: update.segment.text,
                });
            },
        ));
    let mut status = job.running_status("lan.phase.failed", 0);
    if cancelled() {
        status.state = JobState::Cancelled;
        status.phase = "lan.phase.cancelled".into();
    } else {
        match result {
            Ok(mut transcript) => {
                transcript.job_id = job.job_id.clone();
                status.total_duration_ms = transcript.duration_ms;
                let saved = save_transcript(&job.app, &transcript);
                if let Ok(mut transcripts) = job.state.transcripts.lock() {
                    transcripts.insert(job.job_id.clone(), transcript);
                }
                status.state = JobState::Completed;
                status.phase = "lan.phase.completed".into();
                status.transcript_available = true;
                status.result_saved = saved.is_ok();
                status.error = saved.err();
            }
            Err(error) => {
                status.state = JobState::Failed;
                status.error = Some(format!(
                    "LAN attempt failed: {remote_error} Local fallback failed: {error}"
                ));
            }
        }
    }
    job.publish(status);
}

async fn run_remote_job(job: &mut LanJob) {
    let client = client_for(&job.node);
    let progress_app = job.app.clone();
    let upload = submit_upload(
        &client,
        &job.source_path,
        job.model_id.as_deref(),
        move |task_id, sent, total| {
            let _ = progress_app.emit(
                LAN_PROGRESS_EVENT,
                LanProgressEvent {
                    task_id: task_id.to_owned(),
                    sent,
                    total,
                },
            );
        },
    )
    .await;
    let task = match upload {
        Ok(task) => task,
        Err(error) => {
            job.finish(JobState::Failed, "lan.phase.failed", Some(error));
            return;
        }
    };

    if job
        .store
        .cancellation_requested(&job.job_id)
        .unwrap_or(true)
    {
        let _ = timed(client.cancel(&task.id)).await;
        job.finish(JobState::Cancelled, "lan.phase.cancelled", None);
        return;
    }

    // 上传完成后不再本地驱动进度：按节点状态机轮询直到终态。
    let mut failures = 0u32;
    let mut last_state = task.state;
    let mut final_task = None;
    loop {
        if job
            .store
            .cancellation_requested(&job.job_id)
            .unwrap_or(false)
        {
            let _ = timed(client.cancel(&task.id)).await;
            job.finish(JobState::Cancelled, "lan.phase.cancelled", None);
            return;
        }
        match timed(client.task(&task.id)).await {
            Ok(current) => {
                failures = 0;
                if current.state != last_state {
                    last_state = current.state;
                    let phase = task_phase(current.state);
                    let status = job.running_status(phase, 0);
                    job.publish(status);
                }
                if current.state.is_terminal() {
                    final_task = Some(current);
                    break;
                }
            }
            Err(_) => {
                failures += 1;
                if failures >= MAX_POLL_FAILURES {
                    break;
                }
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    let Some(final_task) = final_task else {
        // Best effort: a disconnected node may continue computing. Its result
        // is never merged with the local retry or saved as a second history row.
        let _ = timed(client.cancel(&task.id)).await;
        job.finish(
            JobState::Failed,
            "lan.phase.failed",
            Some("Lost contact with the node while the task was running.".to_owned()),
        );
        return;
    };
    match final_task.state {
        TaskState::Completed => match timed(client.result(&task.id)).await {
            Ok(result) => {
                let model_id = final_task
                    .model_id
                    .clone()
                    .unwrap_or_else(|| "lan-node".to_owned());
                let transcript =
                    transcript_from_lan(&job.job_id, &job.file_name, &model_id, &result);
                let save_result = save_transcript(&job.app, &transcript);
                if let Ok(mut transcripts) = job.state.transcripts.lock() {
                    transcripts.insert(job.job_id.clone(), transcript);
                }
                let status = JobStatus {
                    job_id: job.job_id.clone(),
                    state: JobState::Completed,
                    phase: task_phase(TaskState::Completed).to_owned(),
                    completed_chunks: 0,
                    total_chunks: None,
                    started_at_ms: Some(job.started_at_ms),
                    total_duration_ms: Some(result.duration_ms),
                    transcript_available: true,
                    result_saved: save_result.is_ok(),
                    error: save_result.err(),
                };
                job.publish(status);
            }
            Err(error) => job.finish(JobState::Failed, "lan.phase.failed", Some(error)),
        },
        TaskState::Cancelled => job.finish(JobState::Cancelled, "lan.phase.cancelled", None),
        _ => {
            let message = final_task
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| "The node could not finish the task.".to_owned());
            job.finish(JobState::Failed, "lan.phase.failed", Some(message));
        }
    }
    // 任务结束后按保留策略清理旧结果（默认关闭）。
    let _ = apply_retention(&job.app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_to_text_lan::protocol::Segment;

    #[test]
    fn legacy_prefer_local_migrates_to_local_only() {
        let settings: LanSettings =
            serde_json::from_str(r#"{"mode":"prefer_local","nodes":[]}"#).unwrap();
        assert_eq!(settings.mode, LanMode::LocalOnly);
        assert!(
            serde_json::to_string(&settings)
                .unwrap()
                .contains("local_only")
        );
    }

    #[test]
    fn only_failures_fall_back_never_cancellations_or_success() {
        assert!(should_fallback(JobState::Failed, false));
        assert!(!should_fallback(JobState::Failed, true));
        assert!(!should_fallback(JobState::Cancelled, false));
        assert!(!should_fallback(JobState::Completed, false));
    }

    #[test]
    fn automatic_dispatch_skips_compute_off_and_bad_pairing() {
        use voice_to_text_lan::node::{NodeConfig, NodeServer};
        let root = std::env::temp_dir().join(format!("lan-dispatch-{}", uuid::Uuid::new_v4()));
        let running = NodeServer::start(NodeConfig {
            data_dir: root,
            listen: "127.0.0.1:0".parse().unwrap(),
            token: "test-pairing".into(),
            ..NodeConfig::default()
        })
        .unwrap();
        let mut candidate = node("loopback", "test", true);
        candidate.host = "127.0.0.1".into();
        candidate.port = running.port();
        for token in ["test-pairing", "wrong-pairing"] {
            candidate.token = token.into();
            let settings = LanSettings {
                mode: LanMode::PreferLan,
                nodes: vec![candidate.clone()],
            };
            assert!(
                tauri::async_runtime::block_on(first_reachable_node(&settings))
                    .unwrap()
                    .is_none()
            );
        }
        running.shutdown();
        running.wait();
    }

    fn node(id: &str, name: &str, enabled: bool) -> LanNodeConfig {
        LanNodeConfig {
            id: id.to_owned(),
            name: name.to_owned(),
            host: "REDACTED-LAN-IP".to_owned(),
            port: 7878,
            token: "secret-token".to_owned(),
            enabled,
        }
    }

    fn storage_ok() -> StorageInfo {
        StorageInfo {
            free_bytes: 100,
            total_bytes: 200,
            data_dir: "/tmp/data".to_owned(),
        }
    }

    /// A failing storage probe must not flip a reachable node to offline:
    /// a wrong token is an auth problem, not a connectivity problem.
    #[test]
    fn storage_failure_keeps_node_reachable() {
        let status = assemble_status(
            Ok(true),
            12,
            None,
            Err("node returned status 401: unauthorized".to_owned()),
        );
        assert!(
            status.reachable,
            "node answered the health probe: it is online"
        );
        assert_eq!(status.latency_ms, Some(12));
        assert!(status.storage.is_none());
        assert!(status.warning.as_deref().is_some_and(|w| w.contains("401")));
    }

    #[test]
    fn health_failure_marks_node_unreachable() {
        let status = assemble_status(Err("timeout".to_owned()), 0, None, Ok(storage_ok()));
        assert!(!status.reachable);
        assert!(status.storage.is_none());
        assert!(status.capabilities.is_none());
    }

    #[test]
    fn independent_probes_all_ok() {
        let status = assemble_status(Ok(true), 5, None, Ok(storage_ok()));
        assert!(status.reachable);
        assert!(status.storage.is_some());
        assert!(status.warning.is_none());
    }

    #[test]
    fn lan_settings_reject_incomplete_nodes() {
        let ok = LanSettings {
            mode: LanMode::PreferLan,
            nodes: vec![node("a", "desk", true)],
        };
        assert!(validate_lan_settings(&ok).is_ok());

        let missing_name = LanSettings {
            nodes: vec![node("a", "  ", true)],
            ..ok.clone()
        };
        assert!(validate_lan_settings(&missing_name).is_err());

        let missing_host = LanSettings {
            nodes: vec![LanNodeConfig {
                host: String::new(),
                ..node("a", "desk", true)
            }],
            ..ok.clone()
        };
        assert!(validate_lan_settings(&missing_host).is_err());

        let zero_port = LanSettings {
            nodes: vec![LanNodeConfig {
                port: 0,
                ..node("a", "desk", true)
            }],
            ..ok.clone()
        };
        assert!(validate_lan_settings(&zero_port).is_err());

        let duplicate_id = LanSettings {
            nodes: vec![node("a", "desk", true), node("a", "study", true)],
            ..ok.clone()
        };
        assert!(validate_lan_settings(&duplicate_id).is_err());
    }

    #[test]
    fn empty_node_ids_are_reassigned_to_uuids() {
        let mut settings = LanSettings {
            mode: LanMode::PreferLan,
            nodes: vec![node("", "desk", true), node("kept", "study", true)],
        };
        assign_missing_ids(&mut settings);
        let generated = &settings.nodes[0].id;
        assert_ne!(generated, "");
        assert_eq!(generated.len(), 36, "UUID v4 text form: {generated}");
        assert!(generated.contains('-'));
        assert_eq!(settings.nodes[1].id, "kept");
        assert!(validate_lan_settings(&settings).is_ok());
    }

    #[test]
    fn lan_settings_default_to_local_only_and_round_trip() {
        let settings = LanSettings::default();
        assert_eq!(settings.mode, LanMode::LocalOnly);

        let json = serde_json::to_string(&settings).expect("serialize settings");
        assert!(json.contains("\"mode\":\"local_only\""), "{json}");

        let with_node = LanSettings {
            mode: LanMode::PreferLan,
            nodes: vec![node("a", "desk", true)],
        };
        let json = serde_json::to_string(&with_node).expect("serialize settings");
        assert!(json.contains("\"mode\":\"prefer_lan\""), "{json}");
        assert!(json.contains("\"port\":7878"), "{json}");
        let back: LanSettings = serde_json::from_str(&json).expect("deserialize settings");
        assert_eq!(back, with_node);
    }

    #[test]
    fn dispatch_only_considers_enabled_nodes() {
        let settings = LanSettings {
            mode: LanMode::PreferLan,
            nodes: vec![node("a", "off", false), node("b", "on", true)],
        };
        let enabled = enabled_nodes(&settings);
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, "b");

        let none = LanSettings {
            nodes: vec![node("a", "off", false)],
            ..settings
        };
        assert!(enabled_nodes(&none).is_empty());
    }

    #[test]
    fn task_states_map_to_stable_phase_keys() {
        let cases = [
            (TaskState::Init, "lan.phase.preparing"),
            (TaskState::FileTransferring, "lan.phase.uploading"),
            (TaskState::Queued, "lan.phase.queued"),
            (TaskState::Transcribing, "lan.phase.transcribing"),
            (TaskState::PostProcessing, "lan.phase.post_processing"),
            (TaskState::Completed, "lan.phase.completed"),
            (TaskState::Failed, "lan.phase.failed"),
            (TaskState::CancelRequested, "lan.phase.cancelling"),
            (TaskState::Cancelled, "lan.phase.cancelled"),
        ];
        for (state, expected) in cases {
            assert_eq!(task_phase(state), expected, "{state:?}");
        }
    }

    #[test]
    fn lan_result_becomes_local_transcript() {
        let result = TaskResult {
            transcript: "你好，世界".to_owned(),
            segments: vec![Segment {
                start_ms: 0,
                end_ms: 900,
                text: "你好，世界".to_owned(),
            }],
            duration_ms: 900,
        };
        let transcript = transcript_from_lan(
            "0b0f7b6a-58f0-4f0e-9c22-6b1a2b6a0e11",
            "talk.mp3",
            "sensevoice-small",
            &result,
        );
        assert_eq!(transcript.job_id, "0b0f7b6a-58f0-4f0e-9c22-6b1a2b6a0e11");
        assert_eq!(transcript.source_file_name, "talk.mp3");
        assert_eq!(transcript.model_id, "sensevoice-small");
        assert_eq!(transcript.text, "你好，世界");
        assert_eq!(transcript.duration_ms, Some(900));
        let segments = transcript.segments.expect("segments mapped");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_ms, 0);
        assert_eq!(segments[0].end_ms, 900);
        assert_eq!(segments[0].text, "你好，世界");
        assert!(
            transcript.engine.contains("lan"),
            "engine: {}",
            transcript.engine
        );
    }
}
