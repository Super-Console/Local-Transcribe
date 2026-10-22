use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process, thread,
};

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerConfig},
    schemars::JsonSchema,
    tool, tool_handler, tool_router, Json, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use voice_to_text_core::{
    inspect_model_directory, transcribe_file_cancellable, InputFile, JobState, JobStatus, JobStore,
    LocalModel, ModelFamily, Transcript,
};

#[derive(Debug, Clone)]
struct VoiceToTextServer {
    tool_router: ToolRouter<Self>,
    store: JobStore,
    allowed_roots: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct TranscribeArgs {
    #[schemars(
        description = "Absolute path of a local audio/video file under a configured --allow-root"
    )]
    path: String,
    #[serde(default)]
    #[schemars(
        description = "Optional exact ID from list_models; omitted means use the desktop-selected model"
    )]
    model_id: Option<String>,
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
    #[schemars(
        description = "Set true only when the full recognized text is needed; defaults to false"
    )]
    include_text: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ModelCatalog {
    selected_model_id: Option<String>,
    models: Vec<ModelInfo>,
    note: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ModelInfo {
    id: String,
    display_name: String,
    family: String,
    selected: bool,
    layout_validated: bool,
    capabilities: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct TaskReply {
    job_id: String,
    state: String,
    phase: String,
    model_id: Option<String>,
    completed_chunks: usize,
    total_chunks: Option<usize>,
    transcript_available: bool,
    result_saved: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct TranscriptReply {
    job_id: String,
    source_file_name: String,
    model_id: String,
    engine: String,
    language: Option<String>,
    duration_ms: Option<u64>,
    preview: String,
    full_text: Option<String>,
    result_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CancelReply {
    job_id: String,
    state: String,
    cancellation_requested: bool,
    message: String,
}

impl VoiceToTextServer {
    fn new(data_dir: PathBuf, configured_roots: Vec<PathBuf>) -> Result<Self, String> {
        let mut allowed_roots = Vec::with_capacity(configured_roots.len());
        for root in configured_roots {
            let canonical = fs::canonicalize(root)
                .map_err(|_| "A configured MCP input directory is unavailable.".to_owned())?;
            if !canonical.is_dir() {
                return Err("Every MCP --allow-root entry must be a directory.".to_owned());
            }
            allowed_roots.push(canonical);
        }
        allowed_roots.sort();
        allowed_roots.dedup();
        Ok(Self {
            tool_router: Self::tool_router(),
            store: JobStore::new(data_dir),
            allowed_roots,
        })
    }

    fn selected_model(&self) -> Result<Option<LocalModel>, String> {
        let path = self.store.data_dir().join("selected-model.json");
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("Could not read the saved local model selection.".to_owned()),
        };
        let saved: LocalModel = serde_json::from_slice(&bytes)
            .map_err(|_| "The saved local model selection is invalid.".to_owned())?;
        let current = inspect_model_directory(&saved.directory)
            .map_err(|_| "The selected local model is no longer valid.".to_owned())?;
        if current != saved {
            return Err("The selected local model files changed; select the model again in the desktop app.".to_owned());
        }
        Ok(Some(saved))
    }

    fn available_models(&self) -> Result<(Option<LocalModel>, Vec<LocalModel>), String> {
        let selected = self.selected_model()?;
        let mut models = BTreeMap::<String, LocalModel>::new();
        if let Some(model) = selected.as_ref() {
            models.insert(model.id.clone(), model.clone());
        }
        let directory = self.store.data_dir().join("models");
        if directory.is_dir() {
            let mut folders = fs::read_dir(directory)
                .map_err(|_| "Could not inspect the local models folder.".to_owned())?
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            folders.sort();
            for folder in folders {
                if let Ok(model) = inspect_model_directory(&folder) {
                    models.entry(model.id.clone()).or_insert(model);
                }
            }
        }
        Ok((selected, models.into_values().collect()))
    }

    fn authorize_input(&self, supplied: &str) -> Result<PathBuf, String> {
        if self.allowed_roots.is_empty() {
            return Err("MCP file access is disabled. Configure one or more --allow-root directories before submitting a file.".to_owned());
        }
        let path = Path::new(supplied);
        if !path.is_absolute() {
            return Err("The input file path must be absolute.".to_owned());
        }
        let canonical = fs::canonicalize(path)
            .map_err(|_| "The input file could not be accessed.".to_owned())?;
        if !canonical.is_file() {
            return Err("The input path must point to a regular file.".to_owned());
        }
        if !self
            .allowed_roots
            .iter()
            .any(|root| canonical.starts_with(root))
        {
            return Err(
                "The input file is outside the directories authorized for this MCP server."
                    .to_owned(),
            );
        }
        InputFile::inspect(&canonical)
            .map_err(|_| "The input file could not be validated.".to_owned())?;
        Ok(canonical)
    }

    fn task_status(&self, job_id: &str) -> Result<JobStatus, String> {
        if let Some(status) = self.store.job_status(job_id)? {
            return Ok(status);
        }
        if let Ok(transcript) = self.store.transcript(job_id) {
            return Ok(completed_status(&transcript));
        }
        Err("No transcription task or saved result matches that task ID.".to_owned())
    }

    fn begin_transcription(&self, args: TranscribeArgs) -> Result<TaskReply, String> {
        let path = self.authorize_input(&args.path)?;
        let (selected, models) = self.available_models()?;
        let model = match args.model_id {
            Some(requested) => models
                .into_iter()
                .find(|model| model.id == requested)
                .ok_or_else(|| "That model ID is not available. Use list_models to see valid local IDs.".to_owned())?,
            None => selected.ok_or_else(|| "No model is selected. Choose a local model in the desktop app or pass a model_id returned by list_models.".to_owned())?,
        };
        let job_id = JobStore::new_job_id();
        let initial = JobStatus {
            job_id: job_id.clone(),
            state: JobState::Running,
            phase: "Queued".to_owned(),
            completed_chunks: 0,
            total_chunks: None,
            transcript_available: false,
            result_saved: false,
            error: None,
        };
        let lease = self.store.begin_job(initial.clone())?;
        let store = self.store.clone();
        let worker_job_id = job_id.clone();
        let model_id = model.id.clone();
        let worker = thread::Builder::new()
            .name(format!("mcp-transcribe-{job_id}"))
            .spawn(move || run_transcription(store, lease, worker_job_id, path, model));
        if worker.is_err() {
            let _ = self.store.job_status(&job_id);
            return Err("Could not start the local transcription worker.".to_owned());
        }
        Ok(task_reply(initial, Some(model_id)))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for VoiceToTextServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("voice-to-text-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("Local audio/video transcription using the selected installed model. File access is restricted to directories explicitly passed with --allow-root. The server uses stdio only and never uploads source media.")
    }
}

#[tool_router]
impl VoiceToTextServer {
    /// List only structurally valid local models; model files are verified again when recognition begins.
    #[tool(
        name = "list_models",
        description = "List the desktop-selected local model and valid model folders under the app's models directory.",
        annotations(read_only_hint = true)
    )]
    async fn list_models(&self) -> Result<Json<ModelCatalog>, String> {
        let (selected, models) = self.available_models()?;
        let selected_model_id = selected.as_ref().map(|model| model.id.clone());
        let models = models
            .into_iter()
            .map(|model| ModelInfo {
                id: model.id.clone(),
                display_name: model.display_name,
                family: family_name(model.family).to_owned(),
                selected: selected_model_id.as_deref() == Some(model.id.as_str()),
                layout_validated: true,
                capabilities: vec![
                    "local speech recognition".to_owned(),
                    "audio files or media with a decodable audio track".to_owned(),
                ],
            })
            .collect();
        Ok(Json(ModelCatalog {
            selected_model_id,
            models,
            note: "Layout validation checks required model files; runtime compatibility is verified when the model is loaded.".to_owned(),
        }))
    }

    /// Submit one file and return immediately with a stable task ID.
    #[tool(
        name = "transcribe_file",
        description = "Start local transcription for an absolute file path under a configured --allow-root. Returns immediately; use get_job_status to poll.",
        annotations(open_world_hint = false)
    )]
    async fn transcribe_file(
        &self,
        Parameters(args): Parameters<TranscribeArgs>,
    ) -> Result<Json<TaskReply>, String> {
        self.begin_transcription(args).map(Json)
    }

    /// Read the latest durable status for a task created by either the desktop app or MCP.
    #[tool(
        name = "get_job_status",
        description = "Get current state, chunk progress, and any error for a local transcription task.",
        annotations(read_only_hint = true)
    )]
    async fn get_job_status(
        &self,
        Parameters(args): Parameters<JobIdArgs>,
    ) -> Result<Json<TaskReply>, String> {
        self.task_status(&args.job_id)
            .map(|status| Json(task_reply(status, None)))
    }

    /// Read metadata and a bounded preview; full text is opt-in.
    #[tool(
        name = "get_job_result",
        description = "Read the saved local result. Returns metadata, a 1000-character preview, and the result path; set include_text=true only when the complete transcript is needed.",
        annotations(read_only_hint = true)
    )]
    async fn get_job_result(
        &self,
        Parameters(args): Parameters<ResultArgs>,
    ) -> Result<Json<TranscriptReply>, String> {
        let status = self.task_status(&args.job_id)?;
        if status.state == JobState::Running {
            return Err(
                "The transcription is still running; check again after it finishes.".to_owned(),
            );
        }
        let transcript = self.store.transcript(&args.job_id)?;
        let mut preview: String = transcript.text.chars().take(1000).collect();
        if transcript.text.chars().count() > 1000 {
            preview.push('…');
        }
        let result_path = self
            .store
            .result_path(&args.job_id)?
            .to_string_lossy()
            .into_owned();
        Ok(Json(TranscriptReply {
            job_id: transcript.job_id,
            source_file_name: transcript.source_file_name,
            model_id: transcript.model_id,
            engine: transcript.engine,
            language: transcript.language,
            duration_ms: transcript.duration_ms,
            preview,
            full_text: args
                .include_text
                .unwrap_or(false)
                .then_some(transcript.text),
            result_path,
        }))
    }

    /// Request cooperative cancellation at the next decoding or recognition boundary.
    #[tool(
        name = "cancel_job",
        description = "Request cancellation of a running local task. Reports the latest task state; a task may finish just before the request is observed.",
        annotations(open_world_hint = false)
    )]
    async fn cancel_job(
        &self,
        Parameters(args): Parameters<JobIdArgs>,
    ) -> Result<Json<CancelReply>, String> {
        let current = self.task_status(&args.job_id)?;
        if current.state != JobState::Running {
            return Ok(Json(CancelReply {
                job_id: args.job_id,
                state: state_name(current.state).to_owned(),
                cancellation_requested: false,
                message: "The task is already terminal and cannot be cancelled.".to_owned(),
            }));
        }
        let requested = self.store.request_cancel(&args.job_id)?;
        let latest = self.task_status(&args.job_id)?;
        Ok(Json(CancelReply {
            job_id: args.job_id,
            state: state_name(latest.state).to_owned(),
            cancellation_requested: requested,
            message: if requested {
                "Cancellation was recorded; the worker will stop at its next safe checkpoint."
                    .to_owned()
            } else {
                "The task completed before cancellation could be recorded.".to_owned()
            },
        }))
    }
}

fn run_transcription(
    store: JobStore,
    mut lease: voice_to_text_core::JobLease,
    job_id: String,
    path: PathBuf,
    model: LocalModel,
) {
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
                job_id,
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
                total_chunks: previous.and_then(|job| job.total_chunks),
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
                job_id,
                state: if cancelled {
                    JobState::Cancelled
                } else {
                    JobState::Failed
                },
                phase: if cancelled { "Cancelled" } else { "Failed" }.to_owned(),
                completed_chunks: previous.as_ref().map_or(0, |job| job.completed_chunks),
                total_chunks: previous.and_then(|job| job.total_chunks),
                transcript_available: false,
                result_saved: false,
                error: if cancelled { None } else { Some(error) },
            };
            let _ = lease.save_status(status);
        }
    }
}

fn task_reply(status: JobStatus, model_id: Option<String>) -> TaskReply {
    TaskReply {
        job_id: status.job_id,
        state: state_name(status.state).to_owned(),
        phase: status.phase,
        model_id,
        completed_chunks: status.completed_chunks,
        total_chunks: status.total_chunks,
        transcript_available: status.transcript_available,
        result_saved: status.result_saved,
        error: status.error,
    }
}

fn completed_status(transcript: &Transcript) -> JobStatus {
    JobStatus {
        job_id: transcript.job_id.clone(),
        state: JobState::Completed,
        phase: "Completed".to_owned(),
        completed_chunks: transcript.segments.as_ref().map_or(0, Vec::len),
        total_chunks: transcript.segments.as_ref().map(Vec::len),
        transcript_available: true,
        result_saved: true,
        error: None,
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

fn family_name(family: ModelFamily) -> &'static str {
    match family {
        ModelFamily::Paraformer => "Paraformer",
        ModelFamily::SenseVoice => "SenseVoice",
    }
}

fn default_data_dir() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("VOICE_TO_TEXT_APP_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library/Application Support/com.timmy.voicetotextapp"))
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
    #[cfg(target_os = "windows")]
    {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|local| local.join("com.timmy.voicetotextapp"))
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .map(|path| path.join("com.timmy.voicetotextapp"))
            .or_else(|| {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".local/share/com.timmy.voicetotextapp"))
            })
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
}

struct Options {
    data_dir: PathBuf,
    allowed_roots: Vec<PathBuf>,
}

fn parse_options() -> Result<Option<Options>, String> {
    let mut args = env::args_os().skip(1);
    let mut data_dir = None;
    let mut allowed_roots = Vec::new();
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--help") | Some("-h") => {
                println!("Usage: voice-to-text-mcp [--data-dir PATH] [--allow-root DIRECTORY]...\n\nRuns a local MCP server over stdio. No network listener is started.\nPass each directory the MCP client may read using --allow-root.");
                return Ok(None);
            }
            Some("--data-dir") => {
                data_dir = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--data-dir requires a path.".to_owned())?,
                ));
            }
            Some("--allow-root") => allowed_roots
                .push(PathBuf::from(args.next().ok_or_else(|| {
                    "--allow-root requires a directory.".to_owned()
                })?)),
            _ => return Err("Unknown option. Use --help for supported arguments.".to_owned()),
        }
    }
    Ok(Some(Options {
        data_dir: match data_dir {
            Some(path) => path,
            None => default_data_dir()?,
        },
        allowed_roots,
    }))
}

async fn serve() -> Result<(), String> {
    let Some(options) = parse_options()? else {
        return Ok(());
    };
    let server = VoiceToTextServer::new(options.data_dir, options.allowed_roots)?;
    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|_| "Could not start the MCP stdio server.".to_owned())?;
    service
        .waiting()
        .await
        .map_err(|_| "The MCP stdio connection ended with an error.".to_owned())?;
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = serve().await {
        eprintln!("voice-to-text-mcp: {error}");
        process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

    fn temp_root() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!(
            "voice-to-text-mcp-{now}-{}",
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn exposes_exactly_the_five_project_tools() {
        let server = VoiceToTextServer::new(temp_root(), Vec::new()).unwrap();
        let mut names = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            [
                "cancel_job",
                "get_job_result",
                "get_job_status",
                "list_models",
                "transcribe_file"
            ]
        );
    }

    #[test]
    fn file_access_is_limited_to_canonical_authorized_directories() {
        let base = temp_root();
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        fs::create_dir_all(&allowed).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let permitted_file = allowed.join("voice.wav");
        let denied_file = outside.join("voice.wav");
        fs::write(&permitted_file, b"fixture").unwrap();
        fs::write(&denied_file, b"fixture").unwrap();
        let server = VoiceToTextServer::new(base.join("data"), vec![allowed]).unwrap();

        assert_eq!(
            server
                .authorize_input(permitted_file.to_str().unwrap())
                .unwrap(),
            fs::canonicalize(permitted_file).unwrap()
        );
        assert!(server
            .authorize_input(denied_file.to_str().unwrap())
            .unwrap_err()
            .contains("outside"));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_cannot_escape_an_authorized_directory() {
        use std::os::unix::fs::symlink;
        let base = temp_root();
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        fs::create_dir_all(&allowed).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let outside_file = outside.join("voice.wav");
        let linked_file = allowed.join("linked.wav");
        fs::write(&outside_file, b"fixture").unwrap();
        symlink(&outside_file, &linked_file).unwrap();
        let server = VoiceToTextServer::new(base.join("data"), vec![allowed]).unwrap();

        assert!(server
            .authorize_input(linked_file.to_str().unwrap())
            .unwrap_err()
            .contains("outside"));
    }

    #[test]
    fn no_root_means_file_access_is_denied() {
        let server = VoiceToTextServer::new(temp_root(), Vec::new()).unwrap();
        assert!(server
            .authorize_input("/tmp/anything.wav")
            .unwrap_err()
            .contains("disabled"));
    }
}
