use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::UNIX_EPOCH,
};

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{Menu, MenuItem, MenuItemKind},
};
use voice_to_text_core::{
    InputFile, JobState, JobStatus, JobStore, LocalModel, ModelFamily, ModelLibrary, Transcript,
    inspect_model_directory, transcribe_file_streaming,
};

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

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|_| "The app's local data folder is unavailable.".to_owned())
}

fn model_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("selected-model.json"))
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
        .map_err(|error| error.to_string())
}

/// Overlay 标题栏会关掉原生拖动；把窗口设为「可拖背景」，
/// 这样任何非交互区域的空白处都能移动窗口，标题栏条也一样。
#[cfg(target_os = "macos")]
fn enable_window_background_drag(app: &tauri::App) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use tauri::Manager;

    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Ok(pointer) = window.ns_window() {
        unsafe {
            let ns_window = pointer as *mut AnyObject;
            // 不再使用 setMovableByWindowBackground：它会把内容区的按住拖动
            // 吃成窗口拖动，导致正文无法拖选。窗口拖动由工具栏/侧栏拖拽条的
            // startDragging 显式承担。
            let _ = ns_window;
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn enable_window_background_drag(_app: &tauri::App) {}

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
    let socket = voice_to_text_service::socket_path(&data_dir);
    let mut status = ServiceStatus {
        running: false,
        pid: None,
        socket_path: socket.to_string_lossy().into_owned(),
        data_dir: data_dir.to_string_lossy().into_owned(),
        protocol_version: voice_to_text_service::PROTOCOL_VERSION,
    };
    if let Ok(mut client) = voice_to_text_service::ServiceClient::connect(&data_dir) {
        if let Ok(response) = client.request(&voice_to_text_service::Request::Ping) {
            if let Some(info) = response.service {
                status.running = true;
                status.pid = Some(info.pid);
            }
        }
    }
    Ok(status)
}

/// 三个标准按钮的绝对位置（窗口坐标）：左侧 30、间距 20，
/// 让它们落在浮起侧栏内部、离圆角有足够净空。
/// macOS 会在获得焦点/缩放时重置按钮 frame，所以每次事件都重新摆放。
#[cfg(target_os = "macos")]
fn place_traffic_lights(window: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSPoint;

    let Ok(pointer) = window.ns_window() else {
        return;
    };
    unsafe {
        let ns_window = pointer as *mut AnyObject;
        for kind in 0..3usize {
            let button: *mut AnyObject = msg_send![ns_window, standardWindowButton: kind];
            if button.is_null() {
                continue;
            }
            let target = NSPoint {
                // 绝对坐标，避免每次事件读到被自己改过的 frame 后逐次漂移；
                // 侧栏卡片左 12、上 8，按钮统一留 12px 内边距：x=24、顶边=20（标题栏高 28、按钮高 12）
                x: 24.0 + (kind as f64) * 20.0,
                y: -4.0,
            };
            let _: () = msg_send![button, setFrameOrigin: target];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn place_traffic_lights(_window: &tauri::WebviewWindow) {}

/// 缩放期间 macOS 会持续把按钮 frame 重置回原生位置，逐次强摆会和它互相打架，
/// 造成红绿灯抖动。这里按“代际计数”防抖：只有停手 PLACE_SETTLE_MS 后才真正摆放一次。
#[cfg(target_os = "macos")]
static PLACE_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[cfg(target_os = "macos")]
const PLACE_SETTLE_MS: u64 = 150;

#[cfg(target_os = "macos")]
fn schedule_place_traffic_lights(window: tauri::WebviewWindow) {
    use std::sync::atomic::Ordering;
    let generation = PLACE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(PLACE_SETTLE_MS));
        if PLACE_GENERATION.load(Ordering::SeqCst) == generation {
            // AppKit 只能在主线程调用：后台线程直接 setFrameOrigin 会抛异常崩溃
            let app = window.app_handle().clone();
            let target = window.clone();
            let _ = app.run_on_main_thread(move || place_traffic_lights(&target));
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn schedule_place_traffic_lights(_window: tauri::WebviewWindow) {}

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
fn start_transcription(
    path: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<JobStatus, String> {
    let input = InputFile::inspect(&path).map_err(|error| error.to_string())?;
    let model = read_selected_model(&app)?
        .ok_or_else(|| "Choose a local Paraformer or SenseVoice model folder first.".to_owned())?;
    let store = JobStore::new(app_data_dir(&app)?);
    let state = Arc::clone(state.inner());
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
                                jobs.get(&worker_job_id)
                                    .map(|job| (job.completed_chunks, job.total_chunks))
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
                        jobs.get(&worker_job_id)
                            .map(|job| (job.completed_chunks, job.total_chunks))
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
                    transcript_available: true,
                    result_saved: save_result.is_ok(),
                    error: save_result.err(),
                };
                let _ = lease.save_status(status.clone());
                update_job(&worker_app, &worker_state, status);
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
    Ok(StorageSummary {
        library_path: library.directory().to_string_lossy().into_owned(),
        library_bytes,
        library_count: models.len(),
        history_path: history.to_string_lossy().into_owned(),
        history_bytes,
        history_count,
    })
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
        .setup(|app| {
            enable_window_background_drag(app);
            if let Some(window) = app.get_webview_window("main") {
                place_traffic_lights(&window);
                let watched = window.clone();
                window.on_window_event(move |event| match event {
                    // 获得焦点时 macOS 才重置按钮，立即摆放一次
                    tauri::WindowEvent::Focused(true) => place_traffic_lights(&watched),
                    // 缩放过程中不抢，停手后落位一次，避免抖动
                    tauri::WindowEvent::Resized(_) => {
                        schedule_place_traffic_lights(watched.clone())
                    }
                    _ => {}
                });
            }
            Ok(())
        })
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
            sync_window_theme,
            service_status,
            get_job_status,
            get_job_result,
            list_history,
            export_transcript
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
