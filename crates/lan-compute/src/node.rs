//! LAN compute node: HTTP API, mDNS announcement and the serial
//! transcription worker.
//!
//! Network surface: one HTTP listener (LAN-reachable) where every route
//! except `/api/v1/health` requires `Authorization: Bearer <token>` compared
//! in constant time, plus an optional mDNS announcement. The token is never
//! logged and never appears in `Debug` output.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path as AxumPath, Query, State};
use axum::http::{header, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use serde::Deserialize;
use tokio::sync::Notify;

use voice_to_text_core::{inspect_model_directory, transcribe_file_cancellable, LocalModel};

use crate::protocol::{
    ApiError, Capabilities, CollectReport, Segment, StorageInfo, Task, TaskError, TaskResult,
    TaskState, UploadProgress, MAX_UPLOAD_CHUNK_BYTES, MDNS_SERVICE_TYPE, PROTOCOL_VERSION,
    TXT_COMPUTE, TXT_COMPUTE_OFF, TXT_COMPUTE_ON, TXT_MODEL, TXT_NAME, TXT_ROLE, TXT_VER,
};
use crate::store::{StoreError, TaskStore};

/// How often the worker looks for queued work and stray cancel requests.
const WORKER_POLL: Duration = Duration::from_millis(100);

/// How long a `CancelRequested` task that is not transcribing stays visible
/// in that state before the worker resolves it to `Cancelled`. This keeps the
/// cancel response deterministic for the caller.
const CANCEL_GRACE: Duration = Duration::from_millis(1200);

/// Everything a node needs to run: naming, storage, security and compute.
#[derive(Clone)]
pub struct NodeConfig {
    /// Display name announced over mDNS and shown to clients.
    pub name: String,
    /// Directory holding `tasks/`, payload files and the task JSON files.
    pub data_dir: PathBuf,
    /// Bind address; port `0` picks a free port reported by [`RunningNode::port`].
    pub listen: SocketAddr,
    /// Bearer token required on every route except `/api/v1/health`.
    pub token: String,
    /// Whether this node transcribes locally (`Queued` -> `Completed`).
    pub compute_enabled: bool,
    /// Compute role advertised to peers: `"gpu"`, `"cpu"` or `"auto"`.
    pub role: String,
    /// sherpa-onnx model directory used by the compute worker.
    pub model_dir: Option<PathBuf>,
    /// Only paths inside this root may be written by `POST /api/v1/collect`.
    pub media_root: Option<PathBuf>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            name: "local-transcribe-node".to_owned(),
            data_dir: PathBuf::from("local-transcribe-data"),
            listen: SocketAddr::from(([0, 0, 0, 0], 0)),
            token: String::new(),
            compute_enabled: false,
            role: "auto".to_owned(),
            model_dir: None,
            media_root: None,
        }
    }
}

impl std::fmt::Debug for NodeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeConfig")
            .field("name", &self.name)
            .field("data_dir", &self.data_dir)
            .field("listen", &self.listen)
            .field("token", &"[REDACTED]")
            .field("compute_enabled", &self.compute_enabled)
            .field("role", &self.role)
            .field("model_dir", &self.model_dir)
            .field("media_root", &self.media_root)
            .finish()
    }
}

/// Failure raised while starting the node. Never contains the token.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("invalid node configuration: {0}")]
    Configuration(String),
    #[error("cannot bind {addr}: {message}")]
    Bind { addr: SocketAddr, message: String },
    #[error("cannot start node runtime: {0}")]
    Runtime(String),
    #[error("model directory is not usable: {0}")]
    Model(String),
    #[error("io error: {0}")]
    Io(String),
}

/// A node running in its own threads and runtime.
///
/// Call [`RunningNode::shutdown`] to stop it and [`RunningNode::wait`] to
/// block until both the HTTP server and the worker have exited.
pub struct RunningNode {
    port: u16,
    shutdown: Arc<Notify>,
    stopping: Arc<AtomicBool>,
    server_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    worker_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    mdns: Mutex<Option<(ServiceDaemon, String)>>,
}

impl RunningNode {
    /// Bound TCP port (useful with `listen` port `0`).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Stop the HTTP server, withdraw the mDNS record and end the current
    /// transcription. Idempotent.
    pub fn shutdown(&self) {
        if self.stopping.swap(true, Ordering::SeqCst) {
            return;
        }
        self.shutdown.notify_one();
        if let Some((daemon, fullname)) = take_lock(&self.mdns) {
            let _ = daemon.unregister(&fullname);
            let _ = daemon.shutdown();
            eprintln!("[lan-node] mDNS announcement withdrawn");
        }
    }

    /// Block until the server and worker threads have exited.
    pub fn wait(&self) {
        if let Some(handle) = take_lock(&self.server_thread) {
            let _ = handle.join();
        }
        if let Some(handle) = take_lock(&self.worker_thread) {
            let _ = handle.join();
        }
    }
}

/// Starts a LAN compute node.
pub struct NodeServer;

impl NodeServer {
    /// Validate the configuration, bind the listener, start the compute
    /// worker and (for non-loopback listeners) announce over mDNS.
    pub fn start(config: NodeConfig) -> Result<RunningNode, NodeError> {
        Self::start_scoped(config, None)
    }

    /// Desktop sharing: bind one RFC1918 interface and reject peers outside
    /// that interface's subnet. Never trusts forwarded/proxy headers.
    pub fn start_on_lan(config: NodeConfig, mask: Ipv4Addr) -> Result<RunningNode, NodeError> {
        let IpAddr::V4(address) = config.listen.ip() else {
            return Err(NodeError::Configuration(
                "Choose a private IPv4 interface.".into(),
            ));
        };
        let scope = LanScope::new(address, mask)?;
        Self::start_scoped(config, Some(scope))
    }

    fn start_scoped(config: NodeConfig, scope: Option<LanScope>) -> Result<RunningNode, NodeError> {
        if config.token.trim().is_empty() {
            return Err(NodeError::Configuration(
                "A pairing token is required.".into(),
            ));
        }
        let model = match (&config.model_dir, config.compute_enabled) {
            (Some(directory), _) => Some(Arc::new(
                inspect_model_directory(directory).map_err(NodeError::Model)?,
            )),
            (None, true) => {
                return Err(NodeError::Model(
                    "compute is enabled but no model directory is configured".to_owned(),
                ));
            }
            (None, false) => None,
        };
        let model_id = model.as_ref().map(|model| model.id.clone());

        let store = Arc::new(TaskStore::new(config.data_dir.clone()));

        let std_listener =
            std::net::TcpListener::bind(config.listen).map_err(|error| NodeError::Bind {
                addr: config.listen,
                message: error.to_string(),
            })?;
        std_listener
            .set_nonblocking(true)
            .map_err(|error| NodeError::Io(error.to_string()))?;
        let port = std_listener
            .local_addr()
            .map_err(|error| NodeError::Io(error.to_string()))?
            .port();

        let stopping = Arc::new(AtomicBool::new(false));
        let worker_thread = {
            let store = Arc::clone(&store);
            let stopping = Arc::clone(&stopping);
            let compute_enabled = config.compute_enabled;
            std::thread::Builder::new()
                .name("lan-compute-worker".to_owned())
                .spawn(move || worker_loop(store, model, compute_enabled, stopping))
                .map_err(|error| NodeError::Io(error.to_string()))?
        };

        // A loopback-only listener is never reachable through the LAN, so it
        // is not announced; registration failures are non-fatal either way.
        let mdns = if config.listen.ip().is_loopback() {
            eprintln!("[lan-node] loopback listener: mDNS announcement skipped");
            None
        } else {
            register_mdns(&config, port, model_id.as_deref())
        };

        let shutdown = Arc::new(Notify::new());
        let state = AppState {
            store,
            token: Arc::from(config.token.as_str()),
            compute_enabled: config.compute_enabled,
            role: config.role.clone(),
            model_id,
            media_root: config.media_root.clone(),
        };
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let server_thread = {
            let shutdown = Arc::clone(&shutdown);
            std::thread::Builder::new()
                .name("lan-node-http".to_owned())
                .spawn(move || {
                    let runtime = match tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            let _ = ready_tx.send(Err(format!("cannot build runtime: {error}")));
                            return;
                        }
                    };
                    runtime.block_on(async move {
                        let listener = match tokio::net::TcpListener::from_std(std_listener) {
                            Ok(listener) => listener,
                            Err(error) => {
                                let _ = ready_tx
                                    .send(Err(format!("cannot register listener: {error}")));
                                return;
                            }
                        };
                        let mut app = build_router(state);
                        if let Some(scope) = scope {
                            app =
                                app.layer(middleware::from_fn_with_state(scope, require_lan_peer));
                        }
                        let _ = ready_tx.send(Ok(()));
                        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
                        let mut server = tokio::spawn(async move {
                            axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
                                .with_graceful_shutdown(async move { let _ = stop_rx.await; })
                                .await
                        });
                        tokio::select! {
                            result = &mut server => {
                                if let Ok(Err(error)) = result {
                                    eprintln!("[lan-node] http server error: {error}");
                                }
                            }
                            () = shutdown.notified() => {
                                let _ = stop_tx.send(());
                                // A peer may stop sending halfway through its body.
                                // Allow a brief drain, then drop the runtime so an
                                // incomplete request cannot keep the app alive.
                                if tokio::time::timeout(Duration::from_secs(1), &mut server).await.is_err() {
                                    server.abort();
                                    let _ = server.await;
                                }
                            }
                        }
                    });
                })
                .map_err(|error| NodeError::Io(error.to_string()))?
        };

        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(message)) => {
                stopping.store(true, Ordering::SeqCst);
                return Err(NodeError::Runtime(message));
            }
            Err(error) => {
                stopping.store(true, Ordering::SeqCst);
                return Err(NodeError::Runtime(error.to_string()));
            }
        }

        eprintln!("[lan-node] \"{}\" serving on port {port}", config.name);
        Ok(RunningNode {
            port,
            shutdown,
            stopping,
            server_thread: Mutex::new(Some(server_thread)),
            worker_thread: Mutex::new(Some(worker_thread)),
            mdns: Mutex::new(mdns),
        })
    }
}

#[derive(Clone, Copy)]
struct LanScope {
    address: Ipv4Addr,
    mask: Ipv4Addr,
}

impl LanScope {
    fn new(address: Ipv4Addr, mask: Ipv4Addr) -> Result<Self, NodeError> {
        let bits = u32::from(mask);
        if !address.is_private() || bits == 0 || (!bits & (!bits).wrapping_add(1)) != 0 {
            return Err(NodeError::Configuration(
                "Choose a private LAN address with a valid subnet mask.".into(),
            ));
        }
        Ok(Self { address, mask })
    }

    fn allows(self, peer: IpAddr) -> bool {
        matches!(peer, IpAddr::V4(ip) if ip.is_private()
            && (u32::from(ip) & u32::from(self.mask)) == (u32::from(self.address) & u32::from(self.mask)))
    }
}

async fn require_lan_peer(
    State(scope): State<LanScope>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    if scope.allows(peer.ip()) {
        next.run(request).await
    } else {
        error_response(
            StatusCode::FORBIDDEN,
            "lan_only",
            "Only peers on the selected LAN subnet are allowed",
        )
    }
}

// ---------------------------------------------------------------------------
// HTTP API
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    store: Arc<TaskStore>,
    token: Arc<str>,
    compute_enabled: bool,
    role: String,
    model_id: Option<String>,
    media_root: Option<PathBuf>,
}

fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/v1/capabilities", get(capabilities))
        .route("/api/v1/tasks", post(create_task).get(list_tasks))
        .route("/api/v1/tasks/{id}", get(get_task))
        .route("/api/v1/tasks/{id}/files/content", put(upload_content))
        .route("/api/v1/tasks/{id}/file", get(file_progress))
        .route("/api/v1/tasks/{id}/cancel", post(cancel_task))
        .route("/api/v1/tasks/{id}/result", get(task_result))
        .route("/api/v1/collect", post(collect))
        .route("/api/v1/storage", get(storage))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_CHUNK_BYTES as usize))
        .layer(middleware::from_fn_with_state(state.clone(), require_token));
    Router::new()
        .route("/api/v1/health", get(health))
        .merge(protected)
        .with_state(state)
}

async fn require_token(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|provided| constant_time_eq(provided.as_bytes(), state.token.as_bytes()));
    if authorized {
        next.run(request).await
    } else {
        error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or invalid bearer token",
        )
    }
}

/// Constant-time comparison so a wrong token cannot be guessed byte by byte.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "proto_version": PROTOCOL_VERSION,
    }))
}

async fn capabilities(State(state): State<AppState>) -> Json<Capabilities> {
    Json(Capabilities {
        proto_version: PROTOCOL_VERSION,
        max_chunk_bytes: MAX_UPLOAD_CHUNK_BYTES,
        compute_enabled: state.compute_enabled,
        role: state.role.clone(),
        model_id: state.model_id.clone(),
        platform: std::env::consts::OS.to_owned(),
    })
}

#[derive(Debug, Deserialize)]
struct CreateTaskRequest {
    file_name: String,
    file_size: u64,
    chunks_total: u32,
    model_id: Option<String>,
}

async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Response {
    if state.compute_enabled
        && request
            .model_id
            .as_ref()
            .is_some_and(|id| state.model_id.as_ref() != Some(id))
    {
        return error_response(
            StatusCode::CONFLICT,
            "model_unavailable",
            "The selected model is not available on this node.",
        );
    }
    let model_id = if state.compute_enabled {
        state.model_id.as_deref()
    } else {
        request.model_id.as_deref()
    };
    let task = state.store.create_task(
        &request.file_name,
        request.file_size,
        request.chunks_total,
        model_id,
    );
    match state
        .store
        .transition(&task.id, TaskState::FileTransferring)
    {
        Ok(task) => (StatusCode::CREATED, Json(task)).into_response(),
        Err(error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "transition_failed",
            &error.to_string(),
        ),
    }
}

async fn list_tasks(State(state): State<AppState>) -> Json<Vec<Task>> {
    Json(state.store.list())
}

async fn get_task(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match state.store.get(&id) {
        Some(task) => Json(task).into_response(),
        None => error_response(StatusCode::NOT_FOUND, "not_found", "unknown task id"),
    }
}

#[derive(Debug, Deserialize)]
struct UploadQuery {
    offset: u64,
}

async fn upload_content(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<UploadQuery>,
    body: Bytes,
) -> Response {
    let Some(task) = state.store.get(&id) else {
        return error_response(StatusCode::NOT_FOUND, "not_found", "unknown task id");
    };
    if task.state != TaskState::FileTransferring {
        return error_response(
            StatusCode::CONFLICT,
            "not_acceptable",
            "task is no longer accepting file bytes",
        );
    }
    if query.offset > task.received_bytes {
        return conflict_with_progress(
            &task,
            "offset_mismatch",
            "offset skips past received bytes",
        );
    }

    // A resent chunk overlaps what the node already holds: only the missing
    // tail is appended, so a retry after any interruption is safe.
    let skip = (task.received_bytes - query.offset) as usize;
    let data = if skip >= body.len() {
        &body[..0]
    } else {
        &body[skip..]
    };

    if !data.is_empty() {
        let payload = payload_path(&state.store, &id);
        if let Some(parent) = payload.parent() {
            if let Err(error) = tokio::fs::create_dir_all(parent).await {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "io_error",
                    &error.to_string(),
                );
            }
        }
        let mut file = match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&payload)
            .await
        {
            Ok(file) => file,
            Err(error) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "io_error",
                    &error.to_string(),
                )
            }
        };
        use tokio::io::AsyncWriteExt;
        if let Err(error) = file.write_all(data).await {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                &error.to_string(),
            );
        }
        if let Err(error) = file.flush().await {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                &error.to_string(),
            );
        }

        let received_bytes = task.received_bytes + data.len() as u64;
        let chunks_received = task.chunks_received.saturating_add(1);
        if let Err(error) = state
            .store
            .set_received(&id, received_bytes, chunks_received)
        {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_error",
                &error.to_string(),
            );
        }
        if received_bytes >= task.file_size {
            // Whole payload on disk: the task becomes runnable.
            let _ = state.store.transition(&id, TaskState::Queued);
        }
    }

    match state.store.get(&id) {
        Some(task) => Json(UploadProgress {
            received_bytes: task.received_bytes,
            chunks_received: task.chunks_received,
        })
        .into_response(),
        None => error_response(StatusCode::NOT_FOUND, "not_found", "unknown task id"),
    }
}

async fn file_progress(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match state.store.get(&id) {
        Some(task) => Json(UploadProgress {
            received_bytes: task.received_bytes,
            chunks_received: task.chunks_received,
        })
        .into_response(),
        None => error_response(StatusCode::NOT_FOUND, "not_found", "unknown task id"),
    }
}

async fn cancel_task(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match state.store.request_cancel(&id) {
        Ok(task) => Json(task).into_response(),
        Err(StoreError::NotFound(_)) => {
            error_response(StatusCode::NOT_FOUND, "not_found", "unknown task id")
        }
        Err(StoreError::IllegalTransition { .. }) => match state.store.get(&id) {
            Some(task) => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": ApiError {
                        code: "not_cancellable".to_owned(),
                        message: "task can no longer be cancelled".to_owned(),
                    },
                    "task": task,
                })),
            )
                .into_response(),
            None => error_response(StatusCode::NOT_FOUND, "not_found", "unknown task id"),
        },
        Err(error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store_error",
            &error.to_string(),
        ),
    }
}

async fn task_result(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match state.store.get(&id) {
        None => error_response(StatusCode::NOT_FOUND, "not_found", "unknown task id"),
        Some(task) => {
            if task.state == TaskState::Completed {
                if let Some(result) = task.result.clone() {
                    return Json(result).into_response();
                }
            }
            (StatusCode::CONFLICT, Json(task)).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct CollectRequest {
    output_dir: String,
}

async fn collect(State(state): State<AppState>, Json(request): Json<CollectRequest>) -> Response {
    let Some(media_root) = state.media_root.clone() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "media_root_not_configured",
            "collect requires a media root on this node",
        );
    };
    if request.output_dir.trim().is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_output_dir",
            "output_dir must not be empty",
        );
    }
    // Relative paths are interpreted inside the media root. Containment is
    // checked twice: lexically before creating anything, then again on the
    // canonical paths so symlinks cannot escape the root either.
    let root_absolute = if media_root.is_absolute() {
        normalize_path(&media_root)
    } else {
        let base = std::env::current_dir().unwrap_or_default();
        normalize_path(&base.join(&media_root))
    };
    let requested = PathBuf::from(&request.output_dir);
    let requested = if requested.is_absolute() {
        requested
    } else {
        root_absolute.join(requested)
    };
    let normalized = normalize_path(&requested);
    if !normalized.starts_with(&root_absolute) {
        return error_response(
            StatusCode::FORBIDDEN,
            "outside_media_root",
            "output_dir must stay inside the media root",
        );
    }
    if let Err(error) = tokio::fs::create_dir_all(&normalized).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io_error",
            &error.to_string(),
        );
    }
    let output_dir = match tokio::fs::canonicalize(&normalized).await {
        Ok(path) => path,
        Err(error) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                &error.to_string(),
            )
        }
    };
    let root_canonical = match tokio::fs::canonicalize(&media_root).await {
        Ok(path) => path,
        Err(error) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                &error.to_string(),
            )
        }
    };
    if !output_dir.starts_with(&root_canonical) {
        return error_response(
            StatusCode::FORBIDDEN,
            "outside_media_root",
            "output_dir must stay inside the media root",
        );
    }

    let store = Arc::clone(&state.store);
    let report = tokio::task::spawn_blocking(move || export_completed(&store, &output_dir))
        .await
        .unwrap_or_default();
    Json(report).into_response()
}

async fn storage(State(state): State<AppState>) -> Json<StorageInfo> {
    let data_dir = state.store.data_dir().to_path_buf();
    let total_bytes = fs2::total_space(&data_dir).unwrap_or(0);
    let free_bytes = fs2::available_space(&data_dir).unwrap_or(0);
    Json(StorageInfo {
        data_dir: data_dir.display().to_string(),
        total_bytes,
        free_bytes,
    })
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(ApiError {
            code: code.to_owned(),
            message: message.to_owned(),
        }),
    )
        .into_response()
}

fn conflict_with_progress(task: &Task, code: &str, message: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({
            "error": ApiError { code: code.to_owned(), message: message.to_owned() },
            "received_bytes": task.received_bytes,
            "chunks_received": task.chunks_received,
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Collect export
// ---------------------------------------------------------------------------

/// Export every completed task into `output_dir`: one `<id>.json` snapshot
/// plus one `<id>.zip` DATASET bundle (transcript + manifest with sha256).
pub fn export_completed(store: &TaskStore, output_dir: &Path) -> CollectReport {
    let mut report = CollectReport::default();
    for task in store.list() {
        if task.state != TaskState::Completed || task.result.is_none() {
            continue;
        }
        match export_one(&task, output_dir) {
            Ok(()) => report.exported.push(task.id.clone()),
            Err(error) => report.failed.push((task.id.clone(), error)),
        }
    }
    report
}

fn export_one(task: &Task, output_dir: &Path) -> Result<(), String> {
    let result = task
        .result
        .as_ref()
        .ok_or_else(|| "task has no result".to_owned())?;

    let json_path = output_dir.join(format!("{}.json", task.id));
    let snapshot = serde_json::to_vec_pretty(task).map_err(|error| error.to_string())?;
    std::fs::write(&json_path, snapshot).map_err(|error| error.to_string())?;

    let zip_path = output_dir.join(format!("{}.zip", task.id));
    let file = std::fs::File::create(&zip_path).map_err(|error| error.to_string())?;
    let mut writer = zip::ZipWriter::new(file);

    let transcript = result.transcript.as_bytes();
    writer
        .start_file("transcript.txt", zip::write::SimpleFileOptions::default())
        .map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut writer, transcript).map_err(|error| error.to_string())?;

    let manifest = serde_json::json!({
        "proto_version": PROTOCOL_VERSION,
        "task_id": task.id,
        "file_name": task.file_name,
        "model_id": task.model_id,
        "duration_ms": result.duration_ms,
        "segments": result.segments.len(),
        "transcript_file": "transcript.txt",
        "sha256": sha256_hex(transcript),
        "generated_at_unix_ms": unix_now_ms(),
    });
    let manifest_bytes =
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?;
    writer
        .start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut writer, manifest_bytes.as_bytes())
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Resolve `..` and `.` lexically so containment can be checked before any
/// directory is created.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

// ---------------------------------------------------------------------------
// Compute worker
// ---------------------------------------------------------------------------

fn worker_loop(
    store: Arc<TaskStore>,
    model: Option<Arc<LocalModel>>,
    compute_enabled: bool,
    stopping: Arc<AtomicBool>,
) {
    let mut cancel_seen: HashMap<String, Instant> = HashMap::new();
    while !stopping.load(Ordering::SeqCst) {
        // Resolve stray cancel requests (tasks that were never picked up).
        for task in store.list() {
            if task.state == TaskState::CancelRequested {
                let seen = cancel_seen
                    .entry(task.id.clone())
                    .or_insert_with(Instant::now);
                if seen.elapsed() >= CANCEL_GRACE {
                    cancel_seen.remove(&task.id);
                    if store.transition(&task.id, TaskState::Cancelled).is_ok() {
                        eprintln!("[lan-node] task {} cancelled", task.id);
                    }
                }
            } else {
                cancel_seen.remove(&task.id);
            }
        }

        // Strictly one transcription at a time, oldest queued task first.
        if compute_enabled {
            if let Some(model) = &model {
                let next = store
                    .list()
                    .into_iter()
                    .rfind(|task| task.state == TaskState::Queued);
                if let Some(task) = next {
                    eprintln!("[lan-node] transcribing task {}", task.id);
                    run_task(&store, &task, model, &stopping);
                }
            }
        }
        std::thread::sleep(WORKER_POLL);
    }
}

fn run_task(store: &TaskStore, task: &Task, model: &LocalModel, stopping: &AtomicBool) {
    let payload = payload_path(store, &task.id);
    if store.transition(&task.id, TaskState::Transcribing).is_err() {
        return;
    }
    let result = transcribe_file_cancellable(
        &payload,
        model,
        || {
            stopping.load(Ordering::SeqCst)
                || store
                    .get(&task.id)
                    .is_some_and(|task| task.state == TaskState::CancelRequested)
        },
        |_progress| {},
    );

    let state_now = store.get(&task.id).map(|task| task.state);
    match result {
        Ok(transcript) => {
            let task_result = TaskResult {
                transcript: transcript.text,
                segments: transcript
                    .segments
                    .unwrap_or_default()
                    .into_iter()
                    .map(|segment| Segment {
                        start_ms: segment.start_ms,
                        end_ms: segment.end_ms,
                        text: segment.text,
                    })
                    .collect(),
                duration_ms: transcript.duration_ms.unwrap_or(0),
            };
            match state_now {
                Some(TaskState::Transcribing) => {
                    let _ = store.transition(&task.id, TaskState::PostProcessing);
                    finish_with_result(store, &task.id, task_result);
                }
                Some(TaskState::CancelRequested) => {
                    // The run finished while a cancel was pending: the state
                    // machine allows a successful wrap-up to land as completed.
                    finish_with_result(store, &task.id, task_result);
                }
                _ => {}
            }
        }
        Err(error) => match state_now {
            Some(TaskState::CancelRequested) => {
                if store.transition(&task.id, TaskState::Cancelled).is_ok() {
                    eprintln!("[lan-node] task {} cancelled", task.id);
                }
            }
            Some(TaskState::Transcribing) => {
                let failure = if stopping.load(Ordering::SeqCst) {
                    TaskError {
                        code: "interrupted".to_owned(),
                        message: "服务中断，任务未完成".to_owned(),
                    }
                } else {
                    TaskError {
                        code: "transcription_failed".to_owned(),
                        message: error,
                    }
                };
                let _ = store.finish_err(&task.id, failure);
            }
            _ => {}
        },
    }
}

fn finish_with_result(store: &TaskStore, id: &str, result: TaskResult) {
    match store.finish_ok(id, result) {
        Ok(done) => eprintln!("[lan-node] task {} completed", done.id),
        Err(error) => eprintln!("[lan-node] cannot store result for task {id}: {error}"),
    }
}

/// Payload of one task: `<data_dir>/tasks/<id>/payload.bin`.
fn payload_path(store: &TaskStore, id: &str) -> PathBuf {
    store.data_dir().join("tasks").join(id).join("payload.bin")
}

// ---------------------------------------------------------------------------
// mDNS
// ---------------------------------------------------------------------------

fn register_mdns(
    config: &NodeConfig,
    port: u16,
    model_id: Option<&str>,
) -> Option<(ServiceDaemon, String)> {
    let daemon = match ServiceDaemon::new() {
        Ok(daemon) => daemon,
        Err(error) => {
            eprintln!("[lan-node] mDNS unavailable, node stays reachable at host:port: {error}");
            return None;
        }
    };
    let instance = if config.name.trim().is_empty() {
        "local-transcribe-node"
    } else {
        config.name.as_str()
    };
    let ver = PROTOCOL_VERSION.to_string();
    let properties: Vec<(&str, &str)> = vec![
        (TXT_VER, ver.as_str()),
        (TXT_NAME, instance),
        (
            TXT_COMPUTE,
            if config.compute_enabled {
                TXT_COMPUTE_ON
            } else {
                TXT_COMPUTE_OFF
            },
        ),
        (TXT_ROLE, config.role.as_str()),
        (TXT_MODEL, model_id.unwrap_or("none")),
    ];
    let hostname = format!("{}.local.", sanitize_hostname(instance));
    let address = if config.listen.ip().is_unspecified() {
        detect_primary_ipv4()
    } else {
        Some(config.listen.ip())
    };
    let info = match address {
        Some(address) => ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            instance,
            &hostname,
            address,
            port,
            &properties[..],
        ),
        None => ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            instance,
            &hostname,
            (),
            port,
            &properties[..],
        ),
    };
    let info = match info {
        Ok(info) if config.listen.ip().is_unspecified() => info.enable_addr_auto(),
        Ok(info) => info,
        Err(error) => {
            eprintln!("[lan-node] cannot build mDNS record: {error}");
            let _ = daemon.shutdown();
            return None;
        }
    };
    let fullname = info.get_fullname().to_owned();
    if let Err(error) = daemon.register(info) {
        eprintln!(
            "[lan-node] mDNS registration failed, node stays reachable at host:port: {error}"
        );
        let _ = daemon.shutdown();
        return None;
    }
    eprintln!("[lan-node] announced {fullname}");
    Some((daemon, fullname))
}

/// Primary IPv4 of this host, detected without sending any traffic.
fn detect_primary_ipv4() -> Option<IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:53").ok()?;
    let address = socket.local_addr().ok()?.ip();
    if address.is_unspecified() {
        None
    } else {
        Some(address)
    }
}

fn sanitize_hostname(name: &str) -> String {
    let mut slug: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        "local-transcribe-node".to_owned()
    } else {
        slug
    }
}

fn take_lock<T>(mutex: &Mutex<Option<T>>) -> Option<T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod lan_scope_tests {
    use super::*;

    #[tokio::test]
    async fn subnet_gate_rejects_real_http_and_ignores_forwarded_headers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let scope = LanScope::new(
            "192.168.10.2".parse().unwrap(),
            "255.255.255.0".parse().unwrap(),
        )
        .unwrap();
        let router = Router::new()
            .route("/health", get(health))
            .layer(middleware::from_fn_with_state(scope, require_lan_peer));
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://127.0.0.1:{port}/health"))
            .header("X-Forwarded-For", "192.168.10.3")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        server.abort();
    }

    #[test]
    fn scope_rejects_wildcard_loopback_public_vpn_and_invalid_masks() {
        for ip in [
            "0.0.0.0",
            "127.0.0.1",
            "8.8.8.8",
            "100.64.0.1",
            "169.254.1.1",
        ] {
            assert!(LanScope::new(ip.parse().unwrap(), "255.255.255.0".parse().unwrap()).is_err());
        }
        for mask in ["0.0.0.0", "255.0.255.0", "128.128.0.0"] {
            assert!(LanScope::new("192.168.10.2".parse().unwrap(), mask.parse().unwrap()).is_err());
        }
    }

    #[test]
    fn scope_allows_only_private_peers_in_the_selected_subnet() {
        let scope = LanScope::new(
            "192.168.10.2".parse().unwrap(),
            "255.255.255.0".parse().unwrap(),
        )
        .unwrap();
        assert!(scope.allows("192.168.10.3".parse().unwrap()));
        for ip in ["192.168.11.3", "10.0.0.1", "8.8.8.8", "127.0.0.1", "::1"] {
            assert!(!scope.allows(ip.parse().unwrap()), "{ip}");
        }
    }
}
