//! Local Core Service prototype (P0).
//!
//! One process owns the inference queue for a single user:
//!
//! - single instance enforced by an exclusive `fs2` lock next to the endpoint file,
//! - clients connect over a Unix domain socket created with mode `0600`
//!   (same-user isolation comes from the file permissions); on Windows, where
//!   std has no Unix socket, over a loopback-only TCP port (127.0.0.1) whose
//!   number is written to `service.port` in the same data folder,
//! - newline-delimited JSON requests; no non-loopback network listener anywhere,
//! - one worker thread executes jobs strictly one at a time; jobs whose model
//!   is not bound yet stay visible as `waiting_for_model` and are skipped by
//!   the scheduler without changing their visible queue order.
//!
//! Known P0 limits (tracked for P1): the queue itself is in memory only and model
//! identity is a directory snapshot rather than a registered model ID.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use voice_to_text_core::{
    inspect_model_directory, transcribe_file_cancellable, InputFile, JobState, JobStatus, JobStore,
    LocalModel, ModelFamily, ModelLibrary,
};

pub const PROTOCOL_VERSION: u32 = 1;
pub const SOCKET_FILE: &str = "service.sock";
pub const PORT_FILE: &str = "service.port";
pub const LOCK_FILE: &str = "service.lock";

/// The platform-local stream the newline-delimited JSON protocol runs over:
/// a Unix socket stream on macOS/Linux, a loopback TCP stream on Windows.
#[cfg(unix)]
pub type LocalStream = UnixStream;
#[cfg(windows)]
pub type LocalStream = std::net::TcpStream;

#[cfg(unix)]
type LocalListener = UnixListener;
#[cfg(windows)]
type LocalListener = std::net::TcpListener;

/// How long the worker keeps retrying the cross-process dispatch lock before
/// the queued job is marked failed with the real reason.
const BEGIN_JOB_RETRY_ATTEMPTS: u32 = 100;
const BEGIN_JOB_RETRY_DELAY: Duration = Duration::from_millis(200);

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Submit {
        path: String,
        #[serde(default)]
        model_dir: Option<String>,
        /// 已登记模型的 id；与 model_dir 二选一，优先 id。
        #[serde(default)]
        model_id: Option<String>,
        /// gui | cli | mcp — 记录任务发起方，便于外部任务不被 GUI 误取消。
        #[serde(default)]
        owner: Option<String>,
    },
    Status {
        job_id: String,
    },
    Cancel {
        job_id: String,
    },
    Runs,
    Models,
    ImportModel {
        path: String,
    },
    SelectModel {
        id: String,
    },
    Jobs,
    Retry {
        job_id: String,
    },
    Remove {
        job_id: String,
    },
    Reorder {
        job_id: String,
        #[serde(default)]
        after: Option<String>,
    },
    Export {
        job_id: String,
        path: String,
        format: String,
    },
    JobResult {
        job_id: String,
        #[serde(default)]
        full: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<JobReply>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runs: Option<Vec<RunInterval>>,
    /// 结构化结果：模型列表、队列快照、导出路径等。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Response {
    fn failure(code: &str, message: impl Into<String>) -> Self {
        Response {
            ok: false,
            error: Some(ErrorBody {
                code: code.to_owned(),
                message: message.into(),
            }),
            service: None,
            job: None,
            runs: None,
            data: None,
        }
    }

    fn ok_data(data: Value) -> Self {
        Response {
            ok: true,
            error: None,
            service: None,
            job: None,
            runs: None,
            data: Some(data),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub pid: u32,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobReply {
    pub job_id: String,
    /// queued | waiting_for_model | running | completed | failed | cancelled
    pub state: String,
    pub phase: String,
    pub completed_chunks: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_chunks: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunInterval {
    pub job_id: String,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn default_data_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("VOICE_TO_TEXT_APP_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library/Application Support/com.timmy.voicetotextapp"))
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|local| local.join("com.timmy.voicetotextapp"))
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .map(|path| path.join("com.timmy.voicetotextapp"))
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".local/share/com.timmy.voicetotextapp"))
            })
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
}

pub fn socket_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SOCKET_FILE)
}

/// Windows-only endpoint file: holds the loopback TCP port of a live service.
#[cfg(windows)]
pub fn port_path(data_dir: &Path) -> PathBuf {
    data_dir.join(PORT_FILE)
}

/// The file a client checks to tell "service running" from "not started yet".
pub fn endpoint_file(data_dir: &Path) -> PathBuf {
    #[cfg(unix)]
    {
        socket_path(data_dir)
    }
    #[cfg(windows)]
    {
        port_path(data_dir)
    }
}

fn lock_path(data_dir: &Path) -> PathBuf {
    data_dir.join(LOCK_FILE)
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

// ---------------------------------------------------------------------------
// Local IPC transport
//
// macOS/Linux keep the original Unix domain socket (0600). Windows binds a
// loopback-only TCP port and records it in `service.port`; the `fs2` single
// instance lock still guards the endpoint, so a stale port file can never be
// mistaken for a live service. The wire protocol is identical everywhere.
// ---------------------------------------------------------------------------

fn bind_local(data_dir: &Path) -> Result<LocalListener, ServerStartError> {
    #[cfg(unix)]
    {
        let socket = socket_path(data_dir);
        // The exclusive lock proves no live server owns this socket file, so a
        // leftover file is stale after a crash.
        if socket.exists() {
            let _ = fs::remove_file(&socket);
        }
        let listener = UnixListener::bind(&socket).map_err(|error| {
            ServerStartError::Other(format!(
                "Cannot bind the local service socket {} (Unix socket paths are limited to about 104 bytes, so the data folder path must stay short): {error}",
                socket.display()
            ))
        })?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).map_err(|error| {
            ServerStartError::Other(format!("Cannot secure the local service socket: {error}"))
        })?;
        Ok(listener)
    }
    #[cfg(windows)]
    {
        let port_file = port_path(data_dir);
        // The exclusive lock proves no live server owns this endpoint file, so
        // a leftover file is stale after a crash.
        if port_file.exists() {
            let _ = fs::remove_file(&port_file);
        }
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|error| {
            ServerStartError::Other(format!(
                "Cannot bind the loopback service port (127.0.0.1): {error}"
            ))
        })?;
        let port = listener
            .local_addr()
            .map_err(|error| {
                ServerStartError::Other(format!("Cannot read the bound service port: {error}"))
            })?
            .port();
        fs::write(&port_file, port.to_string()).map_err(|error| {
            ServerStartError::Other(format!("Cannot write the service port file: {error}"))
        })?;
        Ok(listener)
    }
}

fn connect_local(data_dir: &Path) -> Result<LocalStream, String> {
    #[cfg(unix)]
    {
        let path = socket_path(data_dir);
        UnixStream::connect(&path).map_err(|error| {
            format!(
                "Cannot reach the local service socket {}: {error}. Is the service running?",
                path.display()
            )
        })
    }
    #[cfg(windows)]
    {
        let port_file = port_path(data_dir);
        let port: u16 = fs::read_to_string(&port_file)
            .map_err(|error| {
                format!(
                    "Cannot read the service port file {}: {error}. Is the service running?",
                    port_file.display()
                )
            })?
            .trim()
            .parse()
            .map_err(|_| "The service port file is invalid. Is the service running?".to_owned())?;
        std::net::TcpStream::connect(("127.0.0.1", port)).map_err(|error| {
            format!(
                "Cannot reach the local service on 127.0.0.1:{port}: {error}. Is the service running?"
            )
        })
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

pub struct ServerOptions {
    pub data_dir: PathBuf,
    pub allow_roots: Vec<PathBuf>,
}

/// Why the service refused to start.
#[derive(Debug)]
pub enum ServerStartError {
    /// Another instance already holds the lock; the caller should exit
    /// with a distinct code so scripts can detect it.
    AlreadyRunning(String),
    Other(String),
}

impl std::fmt::Display for ServerStartError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerStartError::AlreadyRunning(message) => write!(formatter, "{message}"),
            ServerStartError::Other(message) => write!(formatter, "{message}"),
        }
    }
}

#[derive(Clone)]
struct QueuedJob {
    job_id: String,
    path: PathBuf,
    model: Option<LocalModel>,
    owner: String,
}

/// 服务生命周期内记住每个任务的文件与模型快照，重试时沿用。
#[derive(Clone)]
struct KnownJob {
    path: PathBuf,
    model: Option<LocalModel>,
    owner: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelEntry {
    id: String,
    display_name: String,
    family: String,
    bytes: u64,
    selected: bool,
}

struct QueueState {
    pending: VecDeque<QueuedJob>,
    known: std::collections::HashMap<String, KnownJob>,
    active: Option<String>,
    /// Jobs the service terminated without a persisted store entry (cancelled
    /// while still queued, or rejected before start).
    finished_in_memory: Vec<JobReply>,
    runs: Vec<RunInterval>,
}

struct Shared {
    store: JobStore,
    allow_roots: Vec<PathBuf>,
    queue: Mutex<QueueState>,
}

/// Run the service until the process is stopped. Blocks.
///
/// Returns an error without listening when another instance already owns the
/// lock; the caller should exit with a distinct code in that case.
pub fn run_server(options: ServerOptions) -> Result<(), ServerStartError> {
    let data_dir = options.data_dir;
    fs::create_dir_all(&data_dir).map_err(|error| {
        ServerStartError::Other(format!("Cannot prepare the service data folder: {error}"))
    })?;

    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        // Keep any previous pid text until the lock is ours; then it is wiped.
        .truncate(false)
        .open(lock_path(&data_dir))
        .map_err(|error| {
            ServerStartError::Other(format!("Cannot open the service lock: {error}"))
        })?;
    if let Err(error) = FileExt::try_lock_exclusive(&lock) {
        let holder = fs::read_to_string(lock_path(&data_dir)).unwrap_or_default();
        return Err(ServerStartError::AlreadyRunning(format!(
            "Another Local Transcribe service instance is already running{}. ({error})",
            holder.trim()
        )));
    }
    let _ = lock.set_len(0);
    let mut lock = lock;
    let _ = writeln!(lock, "pid={}", std::process::id());
    let _ = lock.sync_all();

    let listener = bind_local(&data_dir)?;

    let shared = Arc::new(Shared {
        store: JobStore::new(&data_dir),
        allow_roots: canonical_roots(&options.allow_roots).map_err(ServerStartError::Other)?,
        queue: Mutex::new(QueueState {
            pending: VecDeque::new(),
            known: std::collections::HashMap::new(),
            active: None,
            finished_in_memory: Vec::new(),
            runs: Vec::new(),
        }),
    });

    {
        let shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("service-worker".to_owned())
            .spawn(move || worker_loop(shared))
            .map_err(|error| {
                ServerStartError::Other(format!("Cannot start the service worker: {error}"))
            })?;
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    let _ = handle_connection(stream, &shared);
                });
            }
            Err(error) => {
                eprintln!("service: accept failed: {error}");
            }
        }
    }
    Ok(())
}

fn canonical_roots(roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut canonical = Vec::with_capacity(roots.len());
    for root in roots {
        let path = fs::canonicalize(root).map_err(|_| {
            format!(
                "The authorized directory cannot be read: {}",
                root.display()
            )
        })?;
        if !path.is_dir() {
            return Err(format!(
                "An authorized root must be a directory: {}",
                root.display()
            ));
        }
        canonical.push(path);
    }
    Ok(canonical)
}

fn handle_connection(mut stream: LocalStream, shared: &Shared) -> Result<(), String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|error| format!("Cannot read the request: {error}"))?;
    if line.trim().is_empty() {
        return Err("empty request".to_owned());
    }
    let response = match serde_json::from_str::<Request>(line.trim()) {
        Ok(request) => dispatch(request, shared),
        Err(error) => Response::failure("bad_request", format!("Invalid request: {error}")),
    };
    let mut payload = serde_json::to_string(&response)
        .map_err(|error| format!("Cannot serialize the response: {error}"))?;
    payload.push('\n');
    stream
        .write_all(payload.as_bytes())
        .map_err(|error| format!("Cannot write the response: {error}"))
}

fn dispatch(request: Request, shared: &Shared) -> Response {
    match request {
        Request::Ping => Response {
            ok: true,
            error: None,
            service: Some(ServiceInfo {
                pid: std::process::id(),
                protocol_version: PROTOCOL_VERSION,
            }),
            job: None,
            runs: None,
            data: None,
        },
        Request::Submit {
            path,
            model_dir,
            model_id,
            owner,
        } => submit(path, model_dir, model_id, owner, shared),
        Request::Status { job_id } => status(&job_id, shared),
        Request::Cancel { job_id } => cancel(&job_id, shared),
        Request::Models => models(shared),
        Request::ImportModel { path } => import_model(&path, shared),
        Request::SelectModel { id } => select_model(&id, shared),
        Request::Jobs => jobs(shared),
        Request::Retry { job_id } => retry(&job_id, shared),
        Request::Remove { job_id } => remove(&job_id, shared),
        Request::Reorder { job_id, after } => reorder(&job_id, after, shared),
        Request::Export {
            job_id,
            path,
            format,
        } => export(&job_id, &path, &format, shared),
        Request::JobResult { job_id, full } => job_result(&job_id, full, shared),
        Request::Runs => {
            let runs = shared
                .queue
                .lock()
                .map(|state| state.runs.clone())
                .unwrap_or_default();
            Response {
                ok: true,
                error: None,
                service: None,
                job: None,
                runs: Some(runs),
                data: None,
            }
        }
    }
}

/// Authorize the media path against the explicitly configured roots
/// (fail closed, same contract as the MCP server).
fn authorize_input(shared: &Shared, supplied: &str) -> Result<PathBuf, String> {
    if shared.allow_roots.is_empty() {
        return Err(
            "Service file access is disabled. Start the service with at least one --allow-root directory."
                .to_owned(),
        );
    }
    let path = Path::new(supplied);
    if !path.is_absolute() {
        return Err("The input file path must be absolute.".to_owned());
    }
    let canonical =
        fs::canonicalize(path).map_err(|_| "The input file could not be accessed.".to_owned())?;
    if !canonical.is_file() {
        return Err("The input path must point to a regular file.".to_owned());
    }
    if !shared
        .allow_roots
        .iter()
        .any(|root| canonical.starts_with(root))
    {
        return Err(
            "The input file is outside the directories authorized for this service.".to_owned(),
        );
    }
    InputFile::inspect(&canonical)
        .map_err(|_| "The input file could not be validated.".to_owned())?;
    Ok(canonical)
}

fn submit(
    path: String,
    model_dir: Option<String>,
    model_id: Option<String>,
    owner: Option<String>,
    shared: &Shared,
) -> Response {
    let canonical = match authorize_input(shared, &path) {
        Ok(value) => value,
        Err(message) => return Response::failure("unauthorized", message),
    };
    let data_dir = shared.store.data_dir();
    let model = if let Some(id) = model_id {
        let library = ModelLibrary::new(data_dir);
        let path = match library.path_for(&id) {
            Ok(path) => path,
            Err(message) => return Response::failure("not_found", message),
        };
        match inspect_model_directory(&path) {
            Ok(model) => Some(model),
            Err(message) => return Response::failure("invalid_model", message),
        }
    } else if let Some(dir) = model_dir {
        match inspect_model_directory(&dir) {
            Ok(model) => Some(model),
            Err(message) => return Response::failure("invalid_model", message),
        }
    } else {
        match read_selected_model(data_dir) {
            Ok(model) => model,
            Err(message) => return Response::failure("selection_error", message),
        }
    };
    let job_id = JobStore::new_job_id();
    let mut queue = match shared.queue.lock() {
        Ok(queue) => queue,
        Err(_) => return Response::failure("service_error", "The service queue is unavailable."),
    };
    let reply = JobReply {
        job_id: job_id.clone(),
        state: if model.is_some() {
            "queued".to_owned()
        } else {
            "waiting_for_model".to_owned()
        },
        phase: if model.is_some() {
            "Queued".to_owned()
        } else {
            "Waiting for a model".to_owned()
        },
        completed_chunks: 0,
        total_chunks: None,
        error: None,
    };
    let owner = owner.unwrap_or_else(|| "cli".to_owned());
    let owner = match owner.as_str() {
        "gui" | "cli" | "mcp" => owner,
        _ => "cli".to_owned(),
    };
    queue.known.insert(
        job_id.clone(),
        KnownJob {
            path: canonical.clone(),
            model: model.clone(),
            owner: owner.clone(),
        },
    );
    queue.pending.push_back(QueuedJob {
        job_id,
        path: canonical,
        model,
        owner,
    });
    Response {
        ok: true,
        error: None,
        service: None,
        job: Some(reply),
        runs: None,
        data: None,
    }
}

fn state_name(state: JobState) -> &'static str {
    match state {
        JobState::Running => "running",
        JobState::Completed => "completed",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
    }
}

fn status(job_id: &str, shared: &Shared) -> Response {
    if let Ok(queue) = shared.queue.lock() {
        if let Some(job) = queue.pending.iter().find(|job| job.job_id == job_id) {
            let (state, phase) = if job.model.is_some() {
                ("queued", "Queued")
            } else {
                ("waiting_for_model", "Waiting for a model")
            };
            return Response {
                ok: true,
                error: None,
                service: None,
                job: Some(JobReply {
                    job_id: job_id.to_owned(),
                    state: state.to_owned(),
                    phase: phase.to_owned(),
                    completed_chunks: 0,
                    total_chunks: None,
                    error: None,
                }),
                runs: None,
                data: None,
            };
        }
        if queue.active.as_deref() == Some(job_id) {
            // Fall through to the persisted status below; the entry exists.
        }
        if let Some(finished) = queue
            .finished_in_memory
            .iter()
            .find(|reply| reply.job_id == job_id)
        {
            let reply = finished.clone();
            return Response {
                ok: true,
                error: None,
                service: None,
                job: Some(reply),
                runs: None,
                data: None,
            };
        }
    }
    match shared.store.job_status(job_id) {
        Ok(Some(status)) => Response {
            ok: true,
            error: None,
            service: None,
            job: Some(JobReply {
                job_id: status.job_id,
                state: state_name(status.state).to_owned(),
                phase: status.phase,
                completed_chunks: status.completed_chunks,
                total_chunks: status.total_chunks,
                error: status.error,
            }),
            runs: None,
            data: None,
        },
        Ok(None) => Response::failure("not_found", "No such task is known to the local service."),
        Err(error) => Response::failure("service_error", error),
    }
}

fn cancel(job_id: &str, shared: &Shared) -> Response {
    let mut queue = match shared.queue.lock() {
        Ok(queue) => queue,
        Err(_) => return Response::failure("service_error", "The service queue is unavailable."),
    };
    if let Some(index) = queue.pending.iter().position(|job| job.job_id == job_id) {
        queue.pending.remove(index);
        let reply = JobReply {
            job_id: job_id.to_owned(),
            state: "cancelled".to_owned(),
            phase: "Cancelled before start".to_owned(),
            completed_chunks: 0,
            total_chunks: None,
            error: None,
        };
        queue.finished_in_memory.push(reply.clone());
        return Response {
            ok: true,
            error: None,
            service: None,
            job: Some(reply),
            runs: None,
            data: None,
        };
    }
    drop(queue);
    match shared.store.job_status(job_id) {
        Ok(Some(status)) => {
            if status.state != JobState::Running {
                return Response::failure(
                    "not_cancellable",
                    "That task has already finished; cancelling is not possible.",
                );
            }
            match shared.store.request_cancel(job_id) {
                Ok(true) => Response {
                    ok: true,
                    error: None,
                    service: None,
                    job: Some(JobReply {
                        job_id: job_id.to_owned(),
                        state: "cancelling".to_owned(),
                        phase: "Cancellation requested".to_owned(),
                        completed_chunks: status.completed_chunks,
                        total_chunks: status.total_chunks,
                        error: None,
                    }),
                    runs: None,
                    data: None,
                },
                Ok(false) => Response::failure("not_found", "No such task is known to the store."),
                Err(error) => Response::failure("service_error", error),
            }
        }
        Ok(None) => Response::failure("not_found", "No such task is known to the local service."),
        Err(error) => Response::failure("service_error", error),
    }
}

// ---------------------------------------------------------------------------
// Model library, queue and export (shared by CLI and MCP clients)
// ---------------------------------------------------------------------------

fn family_label(family: ModelFamily) -> &'static str {
    match family {
        ModelFamily::Paraformer => "Paraformer",
        ModelFamily::SenseVoice => "SenseVoice",
        ModelFamily::Whisper => "Whisper",
        ModelFamily::Qwen3Asr => "Qwen3-ASR",
        ModelFamily::FireRedAsr => "FireRedASR",
    }
}

fn selected_model_path(data_dir: &Path) -> PathBuf {
    data_dir.join("selected-model.json")
}

fn read_selected_model(data_dir: &Path) -> Result<Option<LocalModel>, String> {
    let path = selected_model_path(data_dir);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|_| "无法读取已选模型记录。".to_owned())?;
    let model: LocalModel = serde_json::from_slice(&bytes)
        .map_err(|_| "已选模型记录已损坏，请重新选择。".to_owned())?;
    Ok(Some(model))
}

fn write_selected_model(data_dir: &Path, model: &LocalModel) -> Result<(), String> {
    let path = selected_model_path(data_dir);
    let temp = data_dir.join(format!(".selected-model-{}.tmp", std::process::id()));
    let payload = serde_json::to_vec_pretty(model).map_err(|error| error.to_string())?;
    fs::write(&temp, &payload).map_err(|error| format!("无法保存模型选择：{error}"))?;
    fs::rename(&temp, &path).map_err(|error| format!("无法保存模型选择：{error}"))
}

fn models(shared: &Shared) -> Response {
    let data_dir = shared.store.data_dir();
    let library = ModelLibrary::new(data_dir);
    let selected = match read_selected_model(data_dir) {
        Ok(selected) => selected,
        Err(message) => return Response::failure("selection_error", message),
    };
    match library.list() {
        Ok(entries) => {
            let models: Vec<ModelEntry> = entries
                .into_iter()
                .map(|entry| ModelEntry {
                    id: entry.model.id.clone(),
                    display_name: entry.model.display_name.clone(),
                    family: family_label(entry.model.family).to_owned(),
                    bytes: entry.bytes,
                    selected: selected
                        .as_ref()
                        .is_some_and(|model| model.id == entry.model.id),
                })
                .collect();
            Response::ok_data(json!({
                "selectedModelId": selected.map(|model| model.id),
                "models": models,
                "libraryPath": library.directory().to_string_lossy(),
            }))
        }
        Err(message) => Response::failure("library_error", message),
    }
}

fn import_model(path: &str, shared: &Shared) -> Response {
    let data_dir = shared.store.data_dir();
    let library = ModelLibrary::new(data_dir);
    match library.import(path) {
        Ok(entry) => {
            if read_selected_model(data_dir).ok().flatten().is_none() {
                if let Err(message) = write_selected_model(data_dir, &entry.model) {
                    return Response::failure("selection_error", message);
                }
            }
            Response::ok_data(json!({
                "id": entry.model.id,
                "displayName": entry.model.display_name,
                "family": family_label(entry.model.family),
                "bytes": entry.bytes,
            }))
        }
        Err(message) => Response::failure("import_failed", message),
    }
}

fn select_model(id: &str, shared: &Shared) -> Response {
    let data_dir = shared.store.data_dir();
    let library = ModelLibrary::new(data_dir);
    let path = match library.path_for(id) {
        Ok(path) => path,
        Err(message) => return Response::failure("not_found", message),
    };
    let model = match inspect_model_directory(&path) {
        Ok(model) => model,
        Err(message) => return Response::failure("invalid_model", message),
    };
    if let Err(message) = write_selected_model(data_dir, &model) {
        return Response::failure("selection_error", message);
    }
    Response::ok_data(json!({ "id": model.id, "displayName": model.display_name }))
}

fn jobs(shared: &Shared) -> Response {
    let Ok(queue) = shared.queue.lock() else {
        return Response::failure("service_error", "队列不可用。");
    };
    let pending: Vec<Value> = queue
        .pending
        .iter()
        .enumerate()
        .map(|(index, job)| {
            json!({
                "jobId": job.job_id,
                "position": index,
                "state": if job.model.is_some() { "queued" } else { "waiting_for_model" },
                "model": job.model.as_ref().map(|model| model.display_name.clone()),
                "owner": job.owner,
                "source": job.path.file_name().map(|name| name.to_string_lossy().into_owned()),
            })
        })
        .collect();
    let active = queue.active.clone();
    let finished: Vec<Value> = queue
        .finished_in_memory
        .iter()
        .map(|job| {
            json!({
                "jobId": job.job_id,
                "state": job.state,
                "phase": job.phase,
                "error": job.error,
            })
        })
        .collect();
    Response::ok_data(json!({
        "active": active,
        "pending": pending,
        "finished": finished,
    }))
}

fn retry(job_id: &str, shared: &Shared) -> Response {
    let mut queue = match shared.queue.lock() {
        Ok(queue) => queue,
        Err(_) => return Response::failure("service_error", "队列不可用。"),
    };
    let Some(known) = queue.known.get(job_id).cloned() else {
        return Response::failure(
            "not_found",
            "服务重启后无法重试更早的任务，请从界面重新导入文件。",
        );
    };
    let new_id = JobStore::new_job_id();
    queue.known.insert(
        new_id.clone(),
        KnownJob {
            path: known.path.clone(),
            model: known.model.clone(),
            owner: known.owner.clone(),
        },
    );
    queue.pending.push_back(QueuedJob {
        job_id: new_id.clone(),
        path: known.path,
        model: known.model,
        owner: known.owner,
    });
    Response {
        ok: true,
        error: None,
        service: None,
        job: Some(JobReply {
            job_id: new_id,
            state: "queued".to_owned(),
            phase: "Queued".to_owned(),
            completed_chunks: 0,
            total_chunks: None,
            error: None,
        }),
        runs: None,
        data: None,
    }
}

fn remove(job_id: &str, shared: &Shared) -> Response {
    let mut queue = match shared.queue.lock() {
        Ok(queue) => queue,
        Err(_) => return Response::failure("service_error", "队列不可用。"),
    };
    let Some(index) = queue.pending.iter().position(|job| job.job_id == job_id) else {
        return Response::failure(
            "not_cancellable",
            "只能移除还在等待的任务；运行中的任务请先取消。",
        );
    };
    queue.pending.remove(index);
    queue.known.remove(job_id);
    Response::ok_data(json!({ "removed": job_id }))
}

fn reorder(job_id: &str, after: Option<String>, shared: &Shared) -> Response {
    let mut queue = match shared.queue.lock() {
        Ok(queue) => queue,
        Err(_) => return Response::failure("service_error", "队列不可用。"),
    };
    let Some(index) = queue.pending.iter().position(|job| job.job_id == job_id) else {
        return Response::failure("not_found", "等待队列里没有这个任务。");
    };
    let Some(job) = queue.pending.remove(index) else {
        return Response::failure("not_found", "等待队列里没有这个任务。");
    };
    let insert_at = match after.as_deref() {
        None => queue.pending.len(),
        Some(anchor) => match queue.pending.iter().position(|item| item.job_id == anchor) {
            Some(position) => position + 1,
            None => queue.pending.len(),
        },
    };
    queue.pending.insert(insert_at, job);
    let order: Vec<&str> = queue
        .pending
        .iter()
        .map(|item| item.job_id.as_str())
        .collect();
    Response::ok_data(json!({ "order": order }))
}

fn job_result(job_id: &str, full: bool, shared: &Shared) -> Response {
    let transcript = match shared.store.transcript(job_id) {
        Ok(transcript) => transcript,
        Err(_) => return Response::failure("not_found", "结果尚未生成或已被删除。"),
    };
    let preview: String = transcript.text.chars().take(1000).collect();
    Response::ok_data(json!({
        "jobId": transcript.job_id,
        "sourceFileName": transcript.source_file_name,
        "modelId": transcript.model_id,
        "engine": transcript.engine,
        "language": transcript.language,
        "durationMs": transcript.duration_ms,
        "preview": preview,
        "fullText": if full { Some(transcript.text) } else { None },
        "segmentCount": transcript.segments.as_ref().map(|segments| segments.len()),
    }))
}

fn export(job_id: &str, path: &str, format: &str, shared: &Shared) -> Response {
    let transcript = match shared.store.transcript(job_id) {
        Ok(transcript) => transcript,
        Err(_) => return Response::failure("not_found", "这个任务还没有已保存的结果，无法导出。"),
    };
    let content = match format {
        "txt" => transcript.to_txt(),
        "json" => match transcript.to_json() {
            Ok(json) => json,
            Err(error) => return Response::failure("export_failed", error.to_string()),
        },
        other => return Response::failure("bad_format", format!("不支持的导出格式：{other}")),
    };
    let mut target = PathBuf::from(path);
    if target.extension().is_none() {
        target.set_extension(format);
    }
    let matches_format = target
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(format));
    if !matches_format {
        return Response::failure("bad_path", "导出文件的扩展名必须与格式一致。");
    }
    if !target.parent().is_some_and(Path::is_dir) {
        return Response::failure("bad_path", "导出目录不存在。");
    }
    let temp = target.with_file_name(format!(
        ".{}.{}.tmp",
        target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "export".to_owned()),
        std::process::id()
    ));
    if let Err(error) = fs::write(&temp, content.as_bytes()) {
        return Response::failure("export_failed", format!("无法写入导出文件：{error}"));
    }
    if let Err(error) = fs::rename(&temp, &target) {
        return Response::failure("export_failed", format!("无法完成导出：{error}"));
    }
    Response::ok_data(json!({ "path": target.to_string_lossy(), "format": format }))
}

// ---------------------------------------------------------------------------
// Worker: exactly one inference at a time
// ---------------------------------------------------------------------------

fn worker_loop(shared: Arc<Shared>) {
    loop {
        let next = shared
            .queue
            .lock()
            .map(|state| state.pending.front().cloned())
            .unwrap_or(None);
        let Some(job) = next else {
            thread::sleep(Duration::from_millis(50));
            continue;
        };
        // Jobs without a model are visible but not runnable; they never block
        // the runnable job behind them (skip, do not reorder).
        let Some(model) = job.model.clone() else {
            thread::sleep(Duration::from_millis(200));
            continue;
        };
        let initial = JobStatus {
            job_id: job.job_id.clone(),
            state: JobState::Running,
            phase: "Queued".to_owned(),
            completed_chunks: 0,
            total_chunks: None,
            started_at_ms: Some(unix_now_ms()),
            total_duration_ms: None,
            transcript_available: false,
            result_saved: false,
            error: None,
        };
        match begin_with_retry(&shared.store, initial) {
            Ok(lease) => {
                let _ = shared.queue.lock().map(|mut state| {
                    // Only remove the job we are about to run; the order of the
                    // remaining pending jobs is untouched.
                    if let Some(front) = state.pending.front() {
                        if front.job_id == job.job_id {
                            state.pending.pop_front();
                        }
                    }
                    state.active = Some(job.job_id.clone());
                });
                execute_job(&shared, lease, job.job_id.clone(), job.path.clone(), model);
                let _ = shared.queue.lock().map(|mut state| {
                    state.active = None;
                });
            }
            Err(error) => {
                // Permanent failure: record it in memory so status reports it,
                // and drop it from the pending queue.
                let _ = shared.queue.lock().map(|mut state| {
                    if let Some(front) = state.pending.front() {
                        if front.job_id == job.job_id {
                            state.pending.pop_front();
                        }
                    }
                    state.finished_in_memory.push(JobReply {
                        job_id: job.job_id.clone(),
                        state: "failed".to_owned(),
                        phase: "Could not start".to_owned(),
                        completed_chunks: 0,
                        total_chunks: None,
                        error: Some(error),
                    });
                });
            }
        }
    }
}

/// Retry while another process holds the global dispatch lock, so an
/// externally owned task (MCP/GUI started elsewhere) pauses this queue
/// instead of racing it.
fn begin_with_retry(
    store: &JobStore,
    initial: JobStatus,
) -> Result<voice_to_text_core::JobLease, String> {
    let mut last = String::from("The local task store did not accept the job.");
    for _ in 0..BEGIN_JOB_RETRY_ATTEMPTS {
        match store.begin_job(initial.clone()) {
            Ok(lease) => return Ok(lease),
            Err(error) => {
                last = error;
                thread::sleep(BEGIN_JOB_RETRY_DELAY);
            }
        }
    }
    Err(last)
}

fn execute_job(
    shared: &Shared,
    mut lease: voice_to_text_core::JobLease,
    job_id: String,
    path: PathBuf,
    model: LocalModel,
) {
    let started_unix_ms = unix_now_ms();
    let store = &shared.store;
    let result = transcribe_file_cancellable(
        &path,
        &model,
        || store.cancellation_requested(&job_id).unwrap_or(false),
        |progress| {
            let status = JobStatus {
                job_id: job_id.clone(),
                state: JobState::Running,
                phase: format!(
                    "Transcribing chunk {} of {}",
                    progress.completed_chunks, progress.total_chunks
                ),
                completed_chunks: progress.completed_chunks,
                total_chunks: Some(progress.total_chunks),
                started_at_ms: Some(started_unix_ms),
                total_duration_ms: Some(progress.total_duration_ms),
                transcript_available: false,
                result_saved: false,
                error: None,
            };
            let _ = lease.save_status(status);
        },
    );
    match result {
        Ok(mut transcript) => {
            transcript.job_id = job_id.clone();
            let previous = store.job_status(&job_id).ok().flatten();
            let save_result = store.save_transcript(&transcript);
            let saved = save_result.is_ok();
            let status = JobStatus {
                job_id: job_id.clone(),
                state: if saved {
                    JobState::Completed
                } else {
                    JobState::Failed
                },
                phase: if saved {
                    "Completed"
                } else {
                    "Result save failed"
                }
                .to_owned(),
                completed_chunks: previous.as_ref().map_or(0, |job| job.completed_chunks),
                total_chunks: previous.as_ref().and_then(|job| job.total_chunks),
                started_at_ms: previous
                    .as_ref()
                    .and_then(|job| job.started_at_ms)
                    .or(Some(started_unix_ms)),
                total_duration_ms: previous.as_ref().and_then(|job| job.total_duration_ms),
                transcript_available: saved,
                result_saved: saved,
                error: save_result.err(),
            };
            let _ = lease.save_status(status);
        }
        Err(error) => {
            let cancelled = store.cancellation_requested(&job_id).unwrap_or(false);
            let previous = store.job_status(&job_id).ok().flatten();
            let status = JobStatus {
                job_id: job_id.clone(),
                state: if cancelled {
                    JobState::Cancelled
                } else {
                    JobState::Failed
                },
                phase: if cancelled { "Cancelled" } else { "Failed" }.to_owned(),
                completed_chunks: previous.as_ref().map_or(0, |job| job.completed_chunks),
                total_chunks: previous.as_ref().and_then(|job| job.total_chunks),
                started_at_ms: previous
                    .as_ref()
                    .and_then(|job| job.started_at_ms)
                    .or(Some(started_unix_ms)),
                total_duration_ms: previous.as_ref().and_then(|job| job.total_duration_ms),
                transcript_available: false,
                result_saved: false,
                error: if cancelled { None } else { Some(error) },
            };
            let _ = lease.save_status(status);
        }
    }
    let finished_unix_ms = unix_now_ms();
    if let Ok(mut state) = shared.queue.lock() {
        state.runs.push(RunInterval {
            job_id,
            started_unix_ms,
            finished_unix_ms,
        });
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct ServiceClient {
    stream: LocalStream,
}

impl ServiceClient {
    pub fn connect(data_dir: &Path) -> Result<Self, String> {
        let stream = connect_local(data_dir)?;
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|error| error.to_string())?;
        Ok(ServiceClient { stream })
    }

    pub fn request(&mut self, request: &Request) -> Result<Response, String> {
        let mut payload = serde_json::to_string(request).map_err(|error| error.to_string())?;
        payload.push('\n');
        self.stream
            .write_all(payload.as_bytes())
            .map_err(|error| format!("Cannot reach the local service: {error}"))?;
        self.stream
            .flush()
            .map_err(|error| format!("Cannot reach the local service: {error}"))?;
        let mut reader =
            BufReader::new(self.stream.try_clone().map_err(|error| error.to_string())?);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|error| format!("The local service did not answer: {error}"))?;
        serde_json::from_str(line.trim())
            .map_err(|error| format!("The local service sent an invalid response: {error}"))
    }
}
