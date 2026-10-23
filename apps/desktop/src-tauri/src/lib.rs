use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{Menu, MenuItem, MenuItemKind},
};
use voice_to_text_core::{
    InputFile, JobState, JobStatus, JobStore, LocalModel, ModelFamily, ModelLibrary, Transcript,
    inspect_model_directory, transcribe_file_streaming,
};
use voice_to_text_lan::protocol::{CollectReport, NodeInfo, StorageInfo, Task, TaskResult};

mod lan;
mod lan_host;
mod recording;

const JOB_EVENT: &str = "voice-to-text:job-status";
const SEGMENT_EVENT: &str = "voice-to-text:transcript-segment";

#[derive(Default)]
struct AppState {
    jobs: Mutex<HashMap<String, JobStatus>>,
    transcripts: Mutex<HashMap<String, Transcript>>,
    source_paths: Mutex<HashMap<String, PathBuf>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelSummary {
    id: String,
    display_name: String,
    family: String,
}

/// One stable segment pushed to the UI while a job is still running.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SegmentEvent {
    job_id: String,
    index: usize,
    total_chunks: usize,
    start_ms: u64,
    end_ms: u64,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryModelSummary {
    id: String,
    display_name: String,
    family: String,
    bytes: u64,
    selected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageSummary {
    library_path: String,
    library_bytes: u64,
    library_count: usize,
    history_path: String,
    history_bytes: u64,
    history_count: usize,
    jobs_path: String,
    jobs_bytes: u64,
    jobs_count: usize,
    data_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryEntry {
    job_id: String,
    source_file_name: String,
    model_id: String,
    duration_ms: Option<u64>,
    preview: String,
    updated_at_ms: u128,
}

fn summarize_model(model: &LocalModel) -> ModelSummary {
    ModelSummary {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        family: match model.family {
            ModelFamily::Paraformer => "Paraformer".to_owned(),
            ModelFamily::SenseVoice => "SenseVoice".to_owned(),
            ModelFamily::Whisper => "Whisper".to_owned(),
            ModelFamily::Qwen3Asr => "Qwen3-ASR".to_owned(),
            ModelFamily::FireRedAsr => "FireRedASR".to_owned(),
        },
    }
}

pub(crate) fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|_| "The app's local data folder is unavailable.".to_owned())
}

fn model_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("selected-model.json"))
}

pub(crate) fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn read_selected_model(app: &AppHandle) -> Result<Option<LocalModel>, String> {
    let path = model_config_path(app)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes =
        fs::read(path).map_err(|_| "Could not read the saved model selection.".to_owned())?;
    let model: LocalModel = serde_json::from_slice(&bytes).map_err(|_| {
        "The saved model selection is invalid. Please choose the model folder again.".to_owned()
    })?;
    let current = inspect_model_directory(&model.directory)?;
    if current != model {
        return Err(
            "The selected model files changed. Please choose the model folder again.".to_owned(),
        );
    }
    Ok(Some(model))
}

fn save_selected_model(app: &AppHandle, model: &LocalModel) -> Result<(), String> {
    let path = model_config_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "The app's local data folder is unavailable.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|_| "Could not prepare the app's local model settings.".to_owned())?;
    let temporary = parent.join("selected-model.json.tmp");
    let contents = serde_json::to_vec_pretty(model)
        .map_err(|_| "Could not encode the selected model settings.".to_owned())?;
    fs::write(&temporary, contents)
        .map_err(|_| "Could not save the selected model settings.".to_owned())?;
    fs::rename(&temporary, &path)
        .map_err(|_| "Could not finish saving the selected model settings.".to_owned())?;
    let saved: LocalModel = serde_json::from_slice(
        &fs::read(&path).map_err(|_| "Could not verify the saved model settings.".to_owned())?,
    )
    .map_err(|_| "Could not verify the saved model settings.".to_owned())?;
    if &saved != model {
        return Err("The saved model selection did not match the chosen model.".to_owned());
    }
    Ok(())
}

#[cfg(test)]
fn is_safe_job_id(job_id: &str) -> bool {
    !job_id.is_empty()
        && job_id.len() <= 64
        && job_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn transcript_path(app: &AppHandle, job_id: &str) -> Result<PathBuf, String> {
    JobStore::new(app_data_dir(app)?).result_path(job_id)
}

fn save_transcript(app: &AppHandle, transcript: &Transcript) -> Result<(), String> {
    let path = transcript_path(app, &transcript.job_id)?;
    let parent = path
        .parent()
        .ok_or_else(|| "The local results folder is unavailable.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|_| "Could not prepare the local results folder.".to_owned())?;
    let temporary = parent.join(format!(".{}.json.tmp", transcript.job_id));
    let contents = transcript
        .to_json()
        .map_err(|_| "Could not encode the transcription result.".to_owned())?;
    fs::write(&temporary, contents.as_bytes())
        .map_err(|_| "Could not save the transcription result.".to_owned())?;
    fs::rename(&temporary, &path)
        .map_err(|_| "Could not finish saving the transcription result.".to_owned())?;
    let saved = fs::read_to_string(&path)
        .map_err(|_| "Could not verify the saved transcription result.".to_owned())?;
    if saved != contents {
        return Err(
            "The saved transcription result did not match the generated result.".to_owned(),
        );
    }
    Ok(())
}

fn load_transcript(app: &AppHandle, state: &AppState, job_id: &str) -> Result<Transcript, String> {
    if let Some(transcript) = state
        .transcripts
        .lock()
        .map_err(|_| "The local task state is unavailable.".to_owned())?
        .get(job_id)
        .cloned()
    {
        return Ok(transcript);
    }
    let path = transcript_path(app, job_id)?;
    let bytes = fs::read(path)
        .map_err(|_| "The saved transcription result could not be found.".to_owned())?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "The saved transcription result is invalid.".to_owned())
}

fn update_job(app: &AppHandle, state: &AppState, status: JobStatus) {
    if let Ok(mut jobs) = state.jobs.lock() {
        jobs.insert(status.job_id.clone(), status.clone());
    }
    let _ = app.emit(JOB_EVENT, status);
}

fn is_media_extension(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "wav"
            | "wave"
            | "aiff"
            | "aif"
            | "caf"
            | "mp3"
            | "mp2"
            | "mp1"
            | "m4a"
            | "mp4"
            | "mov"
            | "mkv"
            | "ogg"
            | "oga"
            | "flac"
            | "aac"
            | "opus"
            | "3gp"
            | "webm"
    )
}

fn prepare_export_path(
    path: &Path,
    format: &str,
    source: Option<&Path>,
) -> Result<PathBuf, String> {
    let extension = match format {
        "txt" => "txt",
        "json" => "json",
        _ => return Err("Choose TXT or JSON as the export format.".to_owned()),
    };
    let mut target = path.to_path_buf();
    if target.extension().is_none() {
        target.set_extension(extension);
    }
    if target
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case(extension))
    {
        return Err("The export filename extension must match the selected format.".to_owned());
    }
    if target
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(is_media_extension)
    {
        return Err(
            "Exporting over an audio or video file is blocked to protect the source media."
                .to_owned(),
        );
    }
    if let Some(source) = source
        && let (Ok(source), Ok(target_existing)) = (source.canonicalize(), target.canonicalize())
        && source == target_existing
    {
        return Err(
            "The export destination is the selected source file; it will not be overwritten."
                .to_owned(),
        );
    }
    if !target.parent().is_some_and(Path::is_dir) {
        return Err("The export destination folder does not exist.".to_owned());
    }
    Ok(target)
}

fn family_label(family: ModelFamily) -> &'static str {
    match family {
        ModelFamily::Paraformer => "Paraformer",
        ModelFamily::SenseVoice => "SenseVoice",
        ModelFamily::Whisper => "Whisper",
        ModelFamily::Qwen3Asr => "Qwen3-ASR",
        ModelFamily::FireRedAsr => "FireRedASR",
    }
}

fn library_summary(
    app: &AppHandle,
    entry: voice_to_text_core::LibraryModel,
) -> Result<LibraryModelSummary, String> {
    let selected = read_selected_model(app)?.map(|model| model.id);
    Ok(LibraryModelSummary {
        id: entry.model.id.clone(),
        display_name: entry.model.display_name.clone(),
        family: family_label(entry.model.family).to_owned(),
        bytes: entry.bytes,
        selected: selected.as_deref() == Some(entry.model.id.as_str()),
    })
}

/// 让窗口外观（系统材质、标题栏）跟随应用主题。
#[tauri::command]
fn sync_window_theme(dark: bool, window: tauri::WebviewWindow) -> Result<(), String> {
    let theme = if dark {
        tauri::Theme::Dark
    } else {
        tauri::Theme::Light
    };
    window
        .set_theme(Some(theme))
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    set_native_window_background(&window, dark);
    Ok(())
}

/// 透明标题栏条显示的是 NSWindow 背景色；把它调成与页面 `--bg` 一致
/// （浅色 #faf9f5 / 深色 #1e1e20，见 app.css），避免标题栏与内容出现接缝色差。
#[cfg(target_os = "macos")]
fn set_native_window_background(window: &tauri::WebviewWindow, dark: bool) {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use tauri::Manager;

    let app = window.app_handle().clone();
    let window = window.clone();
    // AppKit 只能在主线程调用；裸指针不能跨线程，所以在闭包内获取。
    let _ = app.run_on_main_thread(move || {
        let Ok(pointer) = window.ns_window() else {
            return;
        };
        unsafe {
            let ns_window = pointer as *mut AnyObject;
            let (r, g, b): (f64, f64, f64) = if dark {
                (30.0 / 255.0, 30.0 / 255.0, 32.0 / 255.0)
            } else {
                (250.0 / 255.0, 249.0 / 255.0, 245.0 / 255.0)
            };
            let color: *mut AnyObject =
                msg_send![class!(NSColor), colorWithRed: r, green: g, blue: b, alpha: 1.0f64];
            let _: () = msg_send![ns_window, setBackgroundColor: color];
        }
    });
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceStatus {
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    socket_path: String,
    data_dir: String,
    protocol_version: u32,
}

/// 供设置·集成展示：本地服务是否在跑、socket 与数据目录在哪。
#[tauri::command]
fn service_status(app: AppHandle) -> Result<ServiceStatus, String> {
    let data_dir = app_data_dir(&app)?;
    let mut status = ServiceStatus {
        running: false,
        pid: None,
        socket_path: voice_to_text_service::endpoint_file(&data_dir)
            .to_string_lossy()
            .into_owned(),
        data_dir: data_dir.to_string_lossy().into_owned(),
        protocol_version: voice_to_text_service::PROTOCOL_VERSION,
    };
    if let Ok(mut client) = voice_to_text_service::ServiceClient::connect(&data_dir)
        && let Ok(response) = client.request(&voice_to_text_service::Request::Ping)
        && let Some(info) = response.service
    {
        status.running = true;
        status.pid = Some(info.pid);
    }
    Ok(status)
}

#[tauri::command]
fn inspect_input(path: String) -> Result<InputFile, String> {
    InputFile::inspect(path).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_selected_model(app: AppHandle) -> Result<Option<ModelSummary>, String> {
    read_selected_model(&app).map(|model| model.as_ref().map(summarize_model))
}

#[tauri::command]
fn set_selected_model(path: String, app: AppHandle) -> Result<ModelSummary, String> {
    let model = inspect_model_directory(path)?;
    save_selected_model(&app, &model)?;
    Ok(summarize_model(&model))
}

#[tauri::command]
async fn start_transcription(
    path: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<JobStatus, String> {
    let input = InputFile::inspect(&path).map_err(|error| error.to_string())?;
    let selected_model = read_selected_model(&app)?;
    // 局域网派发：prefer_lan 且存在已启用、在线的节点时把任务交给节点执行；
    // 其余模式（以及节点不可达时）回退本机转写。
    let dispatch_node = {
        let settings = lan::read_lan_settings(&app)?;
        if settings.mode == lan::LanMode::PreferLan {
            lan::first_reachable_node(&settings).await?
        } else {
            None
        }
    };
    if let Some(node) = dispatch_node {
        return lan::start_lan_transcription(&app, state.inner(), &input, node, selected_model);
    }
    let model = selected_model
        .ok_or_else(|| "Choose a local Paraformer or SenseVoice model folder first.".to_owned())?;
    let store = JobStore::new(app_data_dir(&app)?);
    let state = Arc::clone(state.inner());
    let started_at_ms = unix_now_ms();
    {
        let mut jobs = state
            .jobs
            .lock()
            .map_err(|_| "The local task state is unavailable.".to_owned())?;
        if jobs.values().any(|job| job.state == JobState::Running) {
            return Err("A transcription task is already running. Wait for it to finish before starting another.".to_owned());
        }
        let job_id = JobStore::new_job_id();
        let status = JobStatus {
            job_id: job_id.clone(),
            state: JobState::Running,
            phase: "正在读取并解码音频".to_owned(),
            completed_chunks: 0,
            total_chunks: None,
            started_at_ms: Some(started_at_ms),
            total_duration_ms: None,
            transcript_available: false,
            result_saved: false,
            error: None,
        };
        let mut lease = store.begin_job(status.clone())?;
        jobs.insert(job_id.clone(), status.clone());
        state
            .source_paths
            .lock()
            .map_err(|_| "The local task state is unavailable.".to_owned())?
            .insert(job_id.clone(), input.path.clone());

        let worker_app = app.clone();
        let worker_state = Arc::clone(&state);
        let worker_store = store.clone();
        let worker_job_id = job_id.clone();
        let source_path = input.path;
        let response_status = status.clone();
        let worker = thread::Builder::new()
            .name(format!("transcribe-{job_id}"))
            .spawn(move || {
                let result = transcribe_file_streaming(
                    &source_path,
                    &model,
                    || {
                        worker_store
                            .cancellation_requested(&worker_job_id)
                            .unwrap_or(false)
                    },
                    |progress| {
                        let progress_status = JobStatus {
                            job_id: worker_job_id.clone(),
                            state: JobState::Running,
                            phase: format!(
                                "识别中：{} / {} 段",
                                progress.completed_chunks, progress.total_chunks
                            ),
                            completed_chunks: progress.completed_chunks,
                            total_chunks: Some(progress.total_chunks),
                            started_at_ms: Some(started_at_ms),
                            total_duration_ms: Some(progress.total_duration_ms),
                            transcript_available: false,
                            result_saved: false,
                            error: None,
                        };
                        let _ = lease.save_status(progress_status.clone());
                        update_job(&worker_app, &worker_state, progress_status);
                    },
                    |update| {
                        let _ = worker_app.emit(
                            SEGMENT_EVENT,
                            SegmentEvent {
                                job_id: worker_job_id.clone(),
                                index: update.index,
                                total_chunks: update.total_chunks,
                                start_ms: update.segment.start_ms,
                                end_ms: update.segment.end_ms,
                                text: update.segment.text,
                            },
                        );
                    },
                );
                let mut transcript = match result {
                    Ok(transcript) => transcript,
                    Err(error) => {
                        let cancelled = worker_store
                            .cancellation_requested(&worker_job_id)
                            .unwrap_or(false);
                        let progress = worker_state
                            .jobs
                            .lock()
                            .ok()
                            .and_then(|jobs| {
                                jobs.get(&worker_job_id).map(|job| {
                                    (
                                        job.completed_chunks,
                                        job.total_chunks,
                                        job.total_duration_ms,
                                    )
                                })
                            })
                            .unwrap_or_default();
                        let status = JobStatus {
                            job_id: worker_job_id.clone(),
                            state: if cancelled {
                                JobState::Cancelled
                            } else {
                                JobState::Failed
                            },
                            phase: if cancelled {
                                "转写已取消"
                            } else {
                                "转写失败"
                            }
                            .to_owned(),
                            completed_chunks: progress.0,
                            total_chunks: progress.1,
                            started_at_ms: Some(started_at_ms),
                            total_duration_ms: progress.2,
                            transcript_available: false,
                            result_saved: false,
                            error: if cancelled { None } else { Some(error) },
                        };
                        let _ = lease.save_status(status.clone());
                        update_job(&worker_app, &worker_state, status);
                        return;
                    }
                };
                transcript.job_id = worker_job_id.clone();
                let final_progress = worker_state
                    .jobs
                    .lock()
                    .ok()
                    .and_then(|jobs| {
                        jobs.get(&worker_job_id).map(|job| {
                            (
                                job.completed_chunks,
                                job.total_chunks,
                                job.total_duration_ms,
                            )
                        })
                    })
                    .unwrap_or_default();
                let save_result = save_transcript(&worker_app, &transcript);
                if let Ok(mut transcripts) = worker_state.transcripts.lock() {
                    transcripts.insert(worker_job_id.clone(), transcript);
                }
                let status = JobStatus {
                    job_id: worker_job_id,
                    state: JobState::Completed,
                    phase: if save_result.is_ok() {
                        "转写完成".to_owned()
                    } else {
                        "转写完成，但本地历史保存失败".to_owned()
                    },
                    completed_chunks: final_progress.0,
                    total_chunks: final_progress.1,
                    started_at_ms: Some(started_at_ms),
                    total_duration_ms: final_progress.2,
                    transcript_available: true,
                    result_saved: save_result.is_ok(),
                    error: save_result.err(),
                };
                let _ = lease.save_status(status.clone());
                update_job(&worker_app, &worker_state, status);
                // 任务结束后按保留策略清理旧结果（默认关闭）。
                let _ = apply_retention(&worker_app);
            });
        if worker.is_err() {
            jobs.remove(&response_status.job_id);
            if let Ok(mut sources) = state.source_paths.lock() {
                sources.remove(&response_status.job_id);
            }
            let _ = store.job_status(&response_status.job_id);
            return Err("Could not start the transcription worker.".to_owned());
        }
        Ok(response_status)
    }
}

#[tauri::command]
fn cancel_transcription(job_id: String, app: AppHandle) -> Result<bool, String> {
    let store = JobStore::new(app_data_dir(&app)?);
    store.request_cancel(&job_id)
}

#[tauri::command]
fn library_directory(app: AppHandle) -> Result<String, String> {
    Ok(ModelLibrary::new(app_data_dir(&app)?)
        .directory()
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
fn library_models(app: AppHandle) -> Result<Vec<LibraryModelSummary>, String> {
    let library = ModelLibrary::new(app_data_dir(&app)?);
    let selected = read_selected_model(&app)?.map(|model| model.id);
    Ok(library
        .list()?
        .into_iter()
        .map(|entry| LibraryModelSummary {
            id: entry.model.id.clone(),
            display_name: entry.model.display_name.clone(),
            family: family_label(entry.model.family).to_owned(),
            bytes: entry.bytes,
            selected: selected.as_deref() == Some(entry.model.id.as_str()),
        })
        .collect())
}

/// Import = validate + copy into the app-managed library; the download stays.
#[tauri::command]
fn import_model(path: String, app: AppHandle) -> Result<LibraryModelSummary, String> {
    let library = ModelLibrary::new(app_data_dir(&app)?);
    let imported = library.import(&path)?;
    if read_selected_model(&app)?.is_none() {
        save_selected_model(&app, &imported.model)?;
    }
    library_summary(&app, imported)
}

#[tauri::command]
fn select_library_model(id: String, app: AppHandle) -> Result<LibraryModelSummary, String> {
    let library = ModelLibrary::new(app_data_dir(&app)?);
    let path = library.path_for(&id)?;
    let model = inspect_model_directory(&path)?;
    save_selected_model(&app, &model)?;
    let bytes = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    library_summary(&app, voice_to_text_core::LibraryModel { model, bytes })
}

#[tauri::command]
fn delete_library_model(id: String, app: AppHandle) -> Result<String, String> {
    if let Some(selected) = read_selected_model(&app)?
        && selected.id == id
    {
        return Err("这是当前默认模型，请先把默认模型换成其他模型再删除。".to_owned());
    }
    let library = ModelLibrary::new(app_data_dir(&app)?);
    let moved = library.delete(&id)?;
    Ok(moved.to_string_lossy().into_owned())
}

fn directory_usage(directory: &Path) -> (u64, usize) {
    let mut bytes = 0u64;
    let mut files = 0usize;
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata()
                && metadata.is_file()
            {
                bytes = bytes.saturating_add(metadata.len());
                files += 1;
            }
        }
    }
    (bytes, files)
}

/// 设置·存储 使用的路径与占用信息（模型库、历史结果）。
#[tauri::command]
fn storage_summary(app: AppHandle) -> Result<StorageSummary, String> {
    let data_dir = app_data_dir(&app)?;
    let library = ModelLibrary::new(&data_dir);
    let models = library.list()?;
    let library_bytes = models.iter().map(|entry| entry.bytes).sum();
    let history = data_dir.join("transcripts");
    let (history_bytes, history_count) = directory_usage(&history);
    let jobs = data_dir.join("jobs");
    let (jobs_bytes, jobs_count) = directory_usage(&jobs);
    Ok(StorageSummary {
        library_path: library.directory().to_string_lossy().into_owned(),
        library_bytes,
        library_count: models.len(),
        history_path: history.to_string_lossy().into_owned(),
        history_bytes,
        history_count,
        jobs_path: jobs.to_string_lossy().into_owned(),
        jobs_bytes,
        jobs_count,
        data_path: data_dir.to_string_lossy().into_owned(),
    })
}

/// 模型运行时状态：绿点只表示模型确实驻留内存（任务书：空闲 120 秒卸载）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelRuntime {
    loaded_model_id: Option<String>,
    idle_unload_seconds: u64,
}

#[tauri::command]
fn list_audio_devices() -> Vec<String> {
    recording::list_devices()
}

#[tauri::command]
fn list_audio_apps() -> Vec<recording::AudioApp> {
    recording::list_apps()
}

#[tauri::command]
fn start_recording(
    tracks: Vec<recording::TrackSpec>,
    app: AppHandle,
    state: State<'_, std::sync::Arc<recording::RecordingState>>,
) -> Result<(), String> {
    recording::start(&state, &app, tracks)
}

/// 停止录音要等音频线程收尾（可能需要几秒），必须异步执行，否则会卡住界面。
#[tauri::command]
async fn stop_recording(
    state: State<'_, std::sync::Arc<recording::RecordingState>>,
) -> Result<recording::RecordedTracks, String> {
    let state = std::sync::Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || recording::stop(&state))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn recording_status(
    state: State<'_, std::sync::Arc<recording::RecordingState>>,
) -> recording::RecordingStatus {
    recording::status(&state)
}

#[tauri::command]
fn model_runtime() -> ModelRuntime {
    ModelRuntime {
        loaded_model_id: voice_to_text_core::loaded_model_id(),
        idle_unload_seconds: voice_to_text_core::idle_unload_seconds(),
    }
}

/// 历史与任务记录的自动清理策略。默认关闭：未经用户设置绝不自动删除。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetentionPolicy {
    /// "off" | "age" | "size"
    mode: String,
    /// 按天清理：1 / 3 / 5 / 7 / 30 天
    age_days: u32,
    /// 按容量清理：1 / 5 / 10 GiB
    max_bytes: u64,
}

const GIB: u64 = 1_073_741_824;

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            mode: "off".to_owned(),
            age_days: 7,
            max_bytes: GIB,
        }
    }
}

fn retention_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("retention.json"))
}

fn read_retention(app: &AppHandle) -> Result<RetentionPolicy, String> {
    let path = retention_config_path(app)?;
    if !path.exists() {
        return Ok(RetentionPolicy::default());
    }
    let bytes = fs::read(path).map_err(|_| "Could not read the cleanup settings.".to_owned())?;
    Ok(serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tauri::command]
fn get_retention_policy(app: AppHandle) -> Result<RetentionPolicy, String> {
    read_retention(&app)
}

fn validate_retention(policy: &RetentionPolicy) -> Result<(), String> {
    if !matches!(policy.mode.as_str(), "off" | "age" | "size") {
        return Err("Unknown cleanup mode.".to_owned());
    }
    if !matches!(policy.age_days, 1 | 3 | 5 | 7 | 30) {
        return Err("Unsupported cleanup age.".to_owned());
    }
    if ![GIB, 5 * GIB, 10 * GIB].contains(&policy.max_bytes) {
        return Err("Unsupported cleanup size.".to_owned());
    }
    Ok(())
}

#[tauri::command]
fn set_retention_policy(
    policy: RetentionPolicy,
    app: AppHandle,
) -> Result<RetentionPolicy, String> {
    validate_retention(&policy)?;
    let path = retention_config_path(&app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "The app's local data folder is unavailable.".to_owned())?;
    fs::create_dir_all(parent).map_err(|_| "Could not prepare the cleanup settings.".to_owned())?;
    let temporary = parent.join("retention.json.tmp");
    let contents = serde_json::to_vec_pretty(&policy)
        .map_err(|_| "Could not encode the cleanup settings.".to_owned())?;
    fs::write(&temporary, contents)
        .map_err(|_| "Could not save the cleanup settings.".to_owned())?;
    fs::rename(&temporary, &path).map_err(|_| "Could not save the cleanup settings.".to_owned())?;
    Ok(policy)
}

/// 清理结果（条数与字节），供启动日志与后续界面提示使用。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RetentionOutcome {
    removed: usize,
    bytes: u64,
}

/// 把文件移入废纸篓（本项目的删除约定：永不永久删除）。
#[cfg(target_os = "macos")]
fn move_to_trash(path: &Path) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is unavailable.".to_owned())?;
    let trash = PathBuf::from(home).join(".Trash");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("entry");
    let target = trash.join(format!("{name}.{}", unix_now_ms()));
    fs::rename(path, target).map_err(|_| "Could not move the entry to the Trash.".to_owned())
}

#[cfg(not(target_os = "macos"))]
fn move_to_trash(_path: &Path) -> Result<(), String> {
    Err("Automatic cleanup currently supports macOS only.".to_owned())
}

/// 按策略清理历史结果与任务记录（不动模型库、不动用户原始文件）。
fn apply_retention(app: &AppHandle) -> Result<RetentionOutcome, String> {
    let policy = read_retention(app)?;
    if policy.mode == "off" {
        return Ok(RetentionOutcome {
            removed: 0,
            bytes: 0,
        });
    }
    let data_dir = app_data_dir(app)?;
    let now = std::time::SystemTime::now();
    let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    for directory in [data_dir.join("transcripts"), data_dir.join("jobs")] {
        let Ok(read_dir) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            // 只处理结果与状态 JSON；锁文件、取消标记、派发锁一律不动。
            if !name.ends_with(".json") {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let modified = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
            entries.push((path, metadata.len(), modified));
        }
    }
    let mut targets: Vec<(PathBuf, u64)> = Vec::new();
    if policy.mode == "age" {
        let cutoff = now - std::time::Duration::from_secs(u64::from(policy.age_days) * 86_400);
        for (path, bytes, modified) in &entries {
            if *modified < cutoff {
                targets.push((path.clone(), *bytes));
            }
        }
    } else {
        let total: u64 = entries.iter().map(|(_, bytes, _)| *bytes).sum();
        if total > policy.max_bytes {
            let mut sorted = entries.clone();
            sorted.sort_by_key(|(_, _, modified)| *modified);
            let mut current = total;
            for (path, bytes, modified) in sorted {
                if current <= policy.max_bytes {
                    break;
                }
                // 正在写入的新文件（1 分钟内）不参与清理，避免碰到活动任务。
                if now
                    .duration_since(modified)
                    .map(|age| age.as_secs() < 60)
                    .unwrap_or(false)
                {
                    continue;
                }
                current = current.saturating_sub(bytes);
                targets.push((path, bytes));
            }
        }
    }
    let mut removed = 0usize;
    let mut bytes = 0u64;
    for (path, size) in targets {
        if move_to_trash(&path).is_ok() {
            removed += 1;
            bytes = bytes.saturating_add(size);
        }
    }
    Ok(RetentionOutcome { removed, bytes })
}

/// 用系统文件管理器打开一个已存在的目录。
#[tauri::command]
fn open_directory(path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    let canonical = target
        .canonicalize()
        .map_err(|_| "这个目录当前不可访问。".to_owned())?;
    if !canonical.is_dir() {
        return Err("这个路径不是目录。".to_owned());
    }
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(&canonical)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开目录：{error}"))
}

#[tauri::command]
fn get_active_job_status(app: AppHandle) -> Result<Option<JobStatus>, String> {
    JobStore::new(app_data_dir(&app)?).active_job_status()
}

#[tauri::command]
fn get_job_status(
    job_id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<JobStatus, String> {
    let store = JobStore::new(app_data_dir(&app)?);
    if let Some(status) = store.job_status(&job_id)? {
        return Ok(status);
    }
    if let Some(status) = state
        .jobs
        .lock()
        .map_err(|_| "The local task state is unavailable.".to_owned())?
        .get(&job_id)
        .cloned()
    {
        return Ok(status);
    }
    let transcript = load_transcript(&app, state.inner(), &job_id)?;
    if let Ok(mut transcripts) = state.transcripts.lock() {
        transcripts.insert(job_id.clone(), transcript);
    }
    Ok(JobStatus {
        job_id,
        state: JobState::Completed,
        phase: "转写完成（已从本地历史恢复）".to_owned(),
        completed_chunks: 0,
        total_chunks: None,
        started_at_ms: None,
        total_duration_ms: None,
        transcript_available: true,
        result_saved: true,
        error: None,
    })
}

#[tauri::command]
fn get_job_result(
    job_id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Transcript, String> {
    load_transcript(&app, state.inner(), &job_id)
}

/// 搜索命中：文件名或正文命中都返回，带命中上下文片段。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchHit {
    job_id: String,
    source_file_name: String,
    model_id: String,
    duration_ms: Option<u64>,
    snippet: String,
    matched_in_content: bool,
    updated_at_ms: u128,
}

/// 在正文中截取命中位置前后约 40/80 字的片段（按字符，不切坏 UTF-8）。
fn match_snippet(text: &str, needle: &str, before: usize, take: usize) -> String {
    let lowered = text.to_lowercase();
    let Some(byte_index) = lowered.find(needle) else {
        let head: String = text.chars().take(take).collect();
        return head;
    };
    let char_index = lowered[..byte_index].chars().count();
    let start = char_index.saturating_sub(before);
    let mut snippet: String = text.chars().skip(start).take(take).collect();
    if start > 0 {
        snippet.insert(0, '…');
    }
    if text.chars().count() > start + take {
        snippet.push('…');
    }
    snippet
}

#[tauri::command]
fn search_history(query: String, app: AppHandle) -> Result<Vec<SearchHit>, String> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let directory = app_data_dir(&app)?.join("transcripts");
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(directory)
        .map_err(|_| "Could not read local transcription history.".to_owned())?;
    let mut hits = Vec::new();
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(entry.path()) else {
            continue;
        };
        let Ok(transcript) = serde_json::from_slice::<Transcript>(&bytes) else {
            continue;
        };
        let name_hit = transcript.source_file_name.to_lowercase().contains(&needle);
        let content_hit = transcript.text.to_lowercase().contains(&needle);
        if !name_hit && !content_hit {
            continue;
        }
        let updated_at_ms = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_millis());
        let snippet = if content_hit {
            match_snippet(&transcript.text, &needle, 30, 110)
        } else {
            let mut head: String = transcript.text.chars().take(110).collect();
            if transcript.text.chars().count() > 110 {
                head.push('…');
            }
            head
        };
        hits.push(SearchHit {
            job_id: transcript.job_id,
            source_file_name: transcript.source_file_name,
            model_id: transcript.model_id,
            duration_ms: transcript.duration_ms,
            snippet,
            matched_in_content: content_hit,
            updated_at_ms,
        });
    }
    hits.sort_by_key(|entry| std::cmp::Reverse(entry.updated_at_ms));
    Ok(hits)
}

#[tauri::command]
fn list_history(app: AppHandle) -> Result<Vec<HistoryEntry>, String> {
    let directory = app_data_dir(&app)?.join("transcripts");
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(directory)
        .map_err(|_| "Could not read local transcription history.".to_owned())?;
    let mut history = Vec::new();
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(entry.path()) else {
            continue;
        };
        let Ok(transcript) = serde_json::from_slice::<Transcript>(&bytes) else {
            continue;
        };
        let updated_at_ms = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_millis());
        let mut preview: String = transcript.text.chars().take(160).collect();
        if transcript.text.chars().count() > 160 {
            preview.push('…');
        }
        history.push(HistoryEntry {
            job_id: transcript.job_id,
            source_file_name: transcript.source_file_name,
            model_id: transcript.model_id,
            duration_ms: transcript.duration_ms,
            preview,
            updated_at_ms,
        });
    }
    history.sort_by_key(|entry| std::cmp::Reverse(entry.updated_at_ms));
    Ok(history)
}

#[tauri::command]
fn export_transcript(
    job_id: String,
    path: String,
    format: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let transcript = load_transcript(&app, state.inner(), &job_id)?;
    let source_path = state
        .source_paths
        .lock()
        .map_err(|_| "The local task state is unavailable.".to_owned())?
        .get(&job_id)
        .cloned();
    let target = prepare_export_path(Path::new(&path), &format, source_path.as_deref())?;
    let contents = match format.as_str() {
        "txt" => transcript.to_txt(),
        "json" => transcript
            .to_json()
            .map_err(|_| "Could not encode the JSON export.".to_owned())?,
        _ => unreachable!("format was validated above"),
    };
    fs::write(&target, contents.as_bytes())
        .map_err(|_| "Could not write the export file.".to_owned())?;
    let read_back =
        fs::read_to_string(&target).map_err(|_| "Could not verify the export file.".to_owned())?;
    if read_back != contents {
        return Err("The exported file did not match the transcription result.".to_owned());
    }
    Ok(target.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// 局域网 Compute V1.1（Client 端）：实现在 lan.rs，这里是 Tauri 命令包装。
// 设置与节点列表返回值含访问令牌（前端编辑节点需要），只在本机界面与
// 本地配置文件之间传递，不写日志、不进事件。
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_lan_settings(app: AppHandle) -> Result<lan::LanSettings, String> {
    lan::read_lan_settings(&app)
}

#[tauri::command]
fn lan_host_interfaces() -> Result<Vec<lan_host::LanInterface>, String> {
    lan_host::interfaces()
}

#[tauri::command]
fn lan_host_status(
    state: State<'_, Arc<lan_host::HostState>>,
) -> Result<lan_host::HostStatus, String> {
    state.status()
}

#[tauri::command]
fn lan_host_token(state: State<'_, Arc<lan_host::HostState>>) -> Result<String, String> {
    state.token()
}

#[tauri::command]
async fn start_lan_host(
    name: String,
    address: String,
    port: u16,
    app: AppHandle,
    state: State<'_, Arc<lan_host::HostState>>,
) -> Result<lan_host::HostStatus, String> {
    let state = Arc::clone(state.inner());
    let root = app_data_dir(&app)?;
    let model = read_selected_model(&app)?.ok_or("Choose a local model before sharing compute.")?;
    tauri::async_runtime::spawn_blocking(move || state.start(&root, name, address, port, model))
        .await
        .map_err(|_| "Could not start LAN sharing.".to_owned())?
}

#[tauri::command]
async fn stop_lan_host(
    state: State<'_, Arc<lan_host::HostState>>,
) -> Result<lan_host::HostStatus, String> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || state.stop())
        .await
        .map_err(|_| "Could not stop LAN sharing.".to_owned())?
}

#[tauri::command]
fn set_lan_settings(
    settings: lan::LanSettings,
    app: AppHandle,
) -> Result<lan::LanSettings, String> {
    lan::save_settings(&app, settings)
}

#[tauri::command]
fn set_lan_assignment(mode: lan::LanMode, app: AppHandle) -> Result<lan::LanSettings, String> {
    lan::set_assignment(&app, mode)
}

#[tauri::command]
async fn lan_discover(timeout_ms: u32) -> Result<Vec<NodeInfo>, String> {
    lan::discover_nodes(timeout_ms).await
}

#[tauri::command]
async fn lan_node_status(node_id: String, app: AppHandle) -> Result<lan::LanNodeStatus, String> {
    lan::node_status(&app, &node_id).await
}

#[tauri::command]
async fn lan_submit_file(
    node_id: String,
    file_path: String,
    model_id: Option<String>,
    app: AppHandle,
) -> Result<String, String> {
    lan::submit_file(&app, &node_id, &file_path, model_id.as_deref()).await
}

#[tauri::command]
async fn lan_tasks(node_id: String, app: AppHandle) -> Result<Vec<Task>, String> {
    lan::list_tasks(&app, &node_id).await
}

#[tauri::command]
async fn lan_task(node_id: String, task_id: String, app: AppHandle) -> Result<Task, String> {
    lan::task_status(&app, &node_id, &task_id).await
}

#[tauri::command]
async fn lan_cancel_task(node_id: String, task_id: String, app: AppHandle) -> Result<Task, String> {
    lan::cancel_task(&app, &node_id, &task_id).await
}

#[tauri::command]
async fn lan_fetch_result(
    node_id: String,
    task_id: String,
    app: AppHandle,
) -> Result<TaskResult, String> {
    lan::fetch_result(&app, &node_id, &task_id).await
}

/// 取回节点结果并写入本地历史（历史页与结果视图直接可读）。
#[tauri::command]
async fn lan_save_result(
    node_id: String,
    task_id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Transcript, String> {
    lan::save_result(&app, state.inner(), &node_id, &task_id).await
}

#[tauri::command]
async fn lan_collect(
    node_id: String,
    output_dir: String,
    app: AppHandle,
) -> Result<CollectReport, String> {
    lan::collect(&app, &node_id, &output_dir).await
}

#[tauri::command]
async fn lan_storage(node_id: String, app: AppHandle) -> Result<StorageInfo, String> {
    lan::node_storage(&app, &node_id).await
}

/// 侧栏折叠开关：产品决定默认不折叠。
/// 保留完整实现，把这里改回 true 即可恢复（前端 `SIDEBAR_COLLAPSE_ENABLED` 同步打开）。
#[cfg(target_os = "macos")]
const SIDEBAR_COLLAPSE_ENABLED: bool = false;

/// 标题栏左侧的侧栏开关（原生 NSTitlebarAccessoryViewController）：
/// 与红绿灯同一行、圆形系统按钮、SF Symbol "sidebar.left"，点击发事件给前端。
#[cfg(target_os = "macos")]
#[allow(dead_code)]
mod titlebar_toggle {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{AllocAnyThread, MainThreadMarker, sel};
    use objc2_app_kit::{
        NSBezelStyle, NSButton, NSCellImagePosition, NSControlSize, NSImage, NSLayoutAttribute,
        NSTitlebarAccessoryViewController, NSWindow,
    };
    use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
    use std::sync::OnceLock;
    use tauri::{Emitter, Manager};

    pub const EVENT: &str = "voice-to-text:toggle-sidebar";

    static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
    /// 按钮指针（仅在主线程使用），用于同步本地化提示语。
    static BUTTON: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    pub fn set_tooltip(label: &str) {
        use std::sync::atomic::Ordering;
        let raw = BUTTON.load(Ordering::Relaxed);
        if raw == 0 {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let _ = mtm;
        let button: &NSButton = unsafe { &*(raw as *const NSButton) };
        let tip = NSString::from_str(label);
        button.setToolTip(Some(&tip));
    }

    objc2::define_class!(
        #[unsafe(super(NSObject))]
        #[name = "LTSidebarToggleTarget"]
        #[ivars = ()]
        struct ToggleTarget;

        unsafe impl NSObjectProtocol for ToggleTarget {}

        impl ToggleTarget {
            #[unsafe(method(toggleSidebar:))]
            fn toggle_sidebar(&self, _sender: Option<&AnyObject>) {
                if let Some(app) = APP.get() {
                    let _ = app.emit(EVENT, ());
                }
            }
        }
    );

    impl ToggleTarget {
        fn new() -> Retained<Self> {
            let this = Self::alloc();
            unsafe { objc2::msg_send![this, init] }
        }
    }

    pub fn install(window: &tauri::WebviewWindow) -> Result<(), String> {
        let _ = APP.set(window.app_handle().clone());
        let handle = window.app_handle().clone();
        let window = window.clone();
        handle
            .run_on_main_thread(move || {
                if let Err(error) = install_on_main(&window) {
                    eprintln!("titlebar toggle unavailable: {error}");
                }
            })
            .map_err(|error| error.to_string())
    }

    fn install_on_main(window: &tauri::WebviewWindow) -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
        let raw = window.ns_window().map_err(|error| error.to_string())? as *mut NSWindow;
        let ns_window = unsafe { Retained::retain(raw) }.ok_or("window is unavailable")?;

        let symbol = NSString::from_str("sidebar.left");
        let description = NSString::from_str("Toggle sidebar");
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &symbol,
            Some(&description),
        )
        .ok_or("SF Symbol sidebar.left is unavailable")?;

        let target = ToggleTarget::new();
        let button = unsafe {
            NSButton::buttonWithImage_target_action(
                &image,
                Some(&target),
                Some(sel!(toggleSidebar:)),
                mtm,
            )
        };
        button.setBezelStyle(NSBezelStyle::Circular);
        button.setImagePosition(NSCellImagePosition::ImageOnly);
        button.setControlSize(NSControlSize::Small);
        button.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(26.0, 26.0)));
        let tip = NSString::from_str("Toggle sidebar");
        button.setToolTip(Some(&tip));
        BUTTON.store(
            Retained::as_ptr(&button) as usize,
            std::sync::atomic::Ordering::Relaxed,
        );

        let controller = NSTitlebarAccessoryViewController::new(mtm);
        controller.setView(&button);
        controller.setLayoutAttribute(NSLayoutAttribute::Leading);
        ns_window.addTitlebarAccessoryViewController(&controller);

        // NSButton 的 target 是弱引用；按钮与窗口同寿命，这里保留 target 直到进程结束。
        std::mem::forget(target);
        Ok(())
    }
}

/// 前端切换语言或侧栏状态时同步原生按钮的提示语。
#[tauri::command]
fn set_sidebar_toggle_label(label: String, app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if crate::SIDEBAR_COLLAPSE_ENABLED {
        let _ = app.run_on_main_thread(move || titlebar_toggle::set_tooltip(&label));
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (label, app);
    Ok(())
}

pub fn menu_event_for_id(id: &str) -> Option<&'static str> {
    match id {
        "open_media" => Some("voice-to-text:open-media"),
        _ => None,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(AppState::default()))
        .manage(Arc::new(lan_host::HostState::default()))
        .manage(std::sync::Arc::new(recording::RecordingState::default()))
        .menu(|app| {
            let menu = Menu::default(app)?;
            let open_media =
                MenuItem::with_id(app, "open_media", "Open…", true, Some("CmdOrCtrl+O"))?;
            let file_menu = menu.items()?.into_iter().find_map(|item| match item {
                MenuItemKind::Submenu(submenu)
                    if submenu.text().ok().as_deref() == Some("File") =>
                {
                    Some(submenu)
                }
                _ => None,
            });
            if let Some(file_menu) = file_menu {
                file_menu.prepend(&open_media)?;
            } else {
                eprintln!("Tauri default File submenu not found; Open menu item unavailable");
            }
            Ok(menu)
        })
        .on_menu_event(|app, event| {
            if let Some(frontend_event) = menu_event_for_id(event.id().as_ref())
                && let Err(error) = app.emit(frontend_event, ())
            {
                eprintln!("failed to emit file menu event: {error}");
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 标题栏侧栏开关（原生按钮，与红绿灯同一行）。
            // 折叠功能已按产品决定关闭；SIDEBAR_COLLAPSE_ENABLED = true 时恢复。
            #[cfg(target_os = "macos")]
            if SIDEBAR_COLLAPSE_ENABLED
                && let Some(window) = app.get_webview_window("main")
                && let Err(error) = titlebar_toggle::install(&window)
            {
                eprintln!("could not install the sidebar toggle: {error}");
            }
            // 启动时按用户的保留策略清理历史与任务记录（默认关闭 = 不删任何东西）。
            let handle = app.handle().clone();
            let outcome = apply_retention(&handle);
            if let Ok(outcome) = outcome
                && outcome.removed > 0
            {
                eprintln!(
                    "retention: moved {} file(s) ({} bytes) to the Trash",
                    outcome.removed, outcome.bytes
                );
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            inspect_input,
            get_selected_model,
            set_selected_model,
            start_transcription,
            cancel_transcription,
            get_active_job_status,
            library_directory,
            library_models,
            import_model,
            select_library_model,
            delete_library_model,
            storage_summary,
            open_directory,
            list_audio_devices,
            list_audio_apps,
            start_recording,
            stop_recording,
            recording_status,
            model_runtime,
            get_retention_policy,
            set_retention_policy,
            set_sidebar_toggle_label,
            sync_window_theme,
            service_status,
            get_job_status,
            get_job_result,
            list_history,
            search_history,
            export_transcript,
            get_lan_settings,
            lan_host_interfaces,
            lan_host_status,
            lan_host_token,
            start_lan_host,
            stop_lan_host,
            set_lan_settings,
            set_lan_assignment,
            lan_discover,
            lan_node_status,
            lan_submit_file,
            lan_tasks,
            lan_task,
            lan_cancel_task,
            lan_fetch_result,
            lan_save_result,
            lan_collect,
            lan_storage
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let _ = app.state::<Arc<lan_host::HostState>>().stop();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{is_safe_job_id, menu_event_for_id, prepare_export_path};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn open_media_menu_item_emits_the_frontend_event() {
        assert_eq!(
            menu_event_for_id("open_media"),
            Some("voice-to-text:open-media")
        );
    }

    #[test]
    fn unrelated_menu_items_are_not_forwarded() {
        assert_eq!(menu_event_for_id("unrelated"), None);
    }

    #[test]
    fn task_ids_are_safe_for_local_result_filenames() {
        assert!(is_safe_job_id("job-195f-ab12-1"));
        assert!(!is_safe_job_id("../outside"));
        assert!(!is_safe_job_id(""));
    }

    #[test]
    fn export_destination_cannot_overwrite_source_media() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("export-safety-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"untouched").unwrap();

        let destination = prepare_export_path(&source, "txt", Some(&source));

        assert!(destination.is_err());
        assert_eq!(fs::read(&source).unwrap(), b"untouched");
    }

    #[test]
    fn export_path_adds_a_missing_extension_and_accepts_only_txt_or_json() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("export-path-{nonce}"));
        fs::create_dir_all(&root).unwrap();

        let txt = prepare_export_path(&root.join("result"), "txt", None).unwrap();
        assert_eq!(
            txt.extension().and_then(|value| value.to_str()),
            Some("txt")
        );
        assert!(prepare_export_path(&root.join("result.srt"), "txt", None).is_err());
        assert!(prepare_export_path(&root.join("result.json"), "pdf", None).is_err());
    }
}

#[cfg(test)]
mod retention_tests {
    use super::{GIB, RetentionPolicy, validate_retention};

    #[test]
    fn retention_policy_validation_rejects_unsupported_values() {
        let ok = RetentionPolicy {
            mode: "age".to_owned(),
            age_days: 7,
            max_bytes: GIB,
        };
        assert!(validate_retention(&ok).is_ok());

        let bad_mode = RetentionPolicy {
            mode: "forever".to_owned(),
            ..ok.clone()
        };
        assert!(validate_retention(&bad_mode).is_err());

        let bad_age = RetentionPolicy {
            mode: "age".to_owned(),
            age_days: 2,
            max_bytes: GIB,
        };
        assert!(validate_retention(&bad_age).is_err());

        let bad_size = RetentionPolicy {
            mode: "size".to_owned(),
            age_days: 7,
            max_bytes: 3 * GIB,
        };
        assert!(validate_retention(&bad_size).is_err());
    }
}

#[cfg(test)]
mod search_tests {
    use super::match_snippet;

    #[test]
    fn snippet_centers_on_the_match_and_keeps_utf8() {
        let text = "开场白很多字很多字很多字很多字很多字很多字很多字关键词后面的内容还有更多更多";
        let snippet = match_snippet(text, "关键词", 4, 16);
        assert!(snippet.contains("关键词"));
        assert!(
            snippet.starts_with('…'),
            "leading ellipsis expected: {snippet}"
        );
        assert!(std::str::from_utf8(snippet.as_bytes()).is_ok());
    }

    #[test]
    fn snippet_falls_back_to_the_head_when_the_needle_is_absent() {
        let snippet = match_snippet("short text", "absent", 5, 20);
        assert_eq!(snippet, "short text");
    }
}
