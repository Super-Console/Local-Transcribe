//! 官方模型下载：内置清单、自动下载、校验并安装到模型库。
//!
//! 设计要点：
//! - 清单只包含本仓库已经实现解码器布局的模型族（见 transcription-core 的
//!   `detect_layout`），因此「能下载」等于「能用」，不提供任意模型。
//! - 下载源为 Hugging Face 官方仓库；直连失败时逐个文件回退到 hf-mirror.com。
//! - 先下载到应用数据目录下的临时文件夹，用 `inspect_model_directory` 校验通过后
//!   再交给模型库导入；失败或取消时把临时目录移入回收站，不做永久删除。
//! - 标点模型是辅助模型，安装到 `models/punctuation-ct-transformer/`，不进入模型库。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use voice_to_text_core::{ModelLibrary, default_trash_dir, inspect_model_directory};

pub const DOWNLOAD_EVENT: &str = "voice-to-text:model-download";

const HF_BASE: &str = "https://huggingface.co";
const HF_MIRROR: &str = "https://hf-mirror.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogKind {
    Asr,
    Punctuation,
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub family: &'static str,
    pub directory: &'static str,
    pub repository: &'static str,
    pub kind: CatalogKind,
    pub bytes: u64,
    /// (仓库内路径, 本地文件名, 字节数)
    pub files: &'static [(&'static str, &'static str, u64)],
}

const PARAFORMER_FILES: &[(&str, &str, u64)] = &[
    ("model.int8.onnx", "model.int8.onnx", 243_371_218),
    ("tokens.txt", "tokens.txt", 75_756),
];
const SENSEVOICE_FILES: &[(&str, &str, u64)] = &[
    ("model.int8.onnx", "model.int8.onnx", 239_233_841),
    ("tokens.txt", "tokens.txt", 315_894),
];
const WHISPER_FILES: &[(&str, &str, u64)] = &[
    (
        "small-encoder.int8.onnx",
        "small-encoder.int8.onnx",
        112_442_483,
    ),
    (
        "small-decoder.int8.onnx",
        "small-decoder.int8.onnx",
        262_226_114,
    ),
    ("small-tokens.txt", "small-tokens.txt", 816_730),
];
const QWEN3_FILES: &[(&str, &str, u64)] = &[
    ("conv_frontend.onnx", "conv_frontend.onnx", 44_148_281),
    ("encoder.int8.onnx", "encoder.int8.onnx", 182_491_662),
    ("decoder.int8.onnx", "decoder.int8.onnx", 755_914_231),
    ("tokenizer/vocab.json", "tokenizer/vocab.json", 2_776_833),
    ("tokenizer/merges.txt", "tokenizer/merges.txt", 1_671_853),
    (
        "tokenizer/tokenizer_config.json",
        "tokenizer/tokenizer_config.json",
        12_487,
    ),
];
const FIRERED_FILES: &[(&str, &str, u64)] = &[
    ("encoder.int8.onnx", "encoder.int8.onnx", 817_286_833),
    ("decoder.int8.onnx", "decoder.int8.onnx", 417_291_928),
    ("tokens.txt", "tokens.txt", 79_172),
];
const PUNCT_FILES: &[(&str, &str, u64)] = &[("model.onnx", "model.onnx", 294_372_519)];

pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "paraformer-zh",
        name: "Paraformer zh · 2023-09-14",
        family: "Paraformer",
        directory: "sherpa-onnx-paraformer-zh-2023-09-14",
        repository: "csukuangfj/sherpa-onnx-paraformer-zh-2023-09-14",
        kind: CatalogKind::Asr,
        bytes: 243_446_974,
        files: PARAFORMER_FILES,
    },
    CatalogEntry {
        id: "sensevoice-small",
        name: "SenseVoice small · zh/en/ja/ko/yue",
        family: "SenseVoice",
        directory: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
        repository: "csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
        kind: CatalogKind::Asr,
        bytes: 239_549_735,
        files: SENSEVOICE_FILES,
    },
    CatalogEntry {
        id: "whisper-small",
        name: "Whisper small · multilingual",
        family: "Whisper",
        directory: "sherpa-onnx-whisper-small",
        repository: "csukuangfj/sherpa-onnx-whisper-small",
        kind: CatalogKind::Asr,
        bytes: 375_485_327,
        files: WHISPER_FILES,
    },
    CatalogEntry {
        id: "qwen3-asr-0.6b",
        name: "Qwen3-ASR 0.6B (int8)",
        family: "Qwen3-ASR",
        directory: "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
        repository: "csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
        kind: CatalogKind::Asr,
        bytes: 987_015_347,
        files: QWEN3_FILES,
    },
    CatalogEntry {
        id: "fire-red-asr2",
        name: "FireRedASR2 (int8) · zh/en",
        family: "FireRedASR",
        directory: "sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26",
        repository: "csukuangfj2/sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26",
        kind: CatalogKind::Asr,
        bytes: 1_234_657_933,
        files: FIRERED_FILES,
    },
    CatalogEntry {
        id: "punctuation-zh-en",
        name: "CT-Transformer punctuation · zh/en",
        family: "Punctuation",
        directory: "punctuation-ct-transformer",
        repository: "csukuangfj/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12",
        kind: CatalogKind::Punctuation,
        bytes: 294_372_519,
        files: PUNCT_FILES,
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogState {
    pub id: String,
    pub name: String,
    pub family: String,
    pub kind: String,
    pub bytes: u64,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadEvent {
    id: String,
    state: String,
    received: u64,
    total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    library_id: Option<String>,
}

struct ActiveDownload {
    id: String,
    cancel: Arc<AtomicBool>,
}

static ACTIVE: Mutex<Option<ActiveDownload>> = Mutex::new(None);

pub fn punctuation_model_path(app_data: &Path) -> PathBuf {
    app_data
        .join("models")
        .join("punctuation-ct-transformer")
        .join("model.onnx")
}

fn entry_by_id(id: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|entry| entry.id == id)
}

pub fn states(app_data: &Path) -> Vec<CatalogState> {
    CATALOG
        .iter()
        .map(|entry| {
            let installed = match entry.kind {
                CatalogKind::Asr => app_data.join("models").join(entry.directory).is_dir(),
                CatalogKind::Punctuation => punctuation_model_path(app_data).is_file(),
            };
            CatalogState {
                id: entry.id.to_owned(),
                name: entry.name.to_owned(),
                family: entry.family.to_owned(),
                kind: match entry.kind {
                    CatalogKind::Asr => "asr",
                    CatalogKind::Punctuation => "punctuation",
                }
                .to_owned(),
                bytes: entry.bytes,
                installed,
            }
        })
        .collect()
}

fn emit(
    app: &AppHandle,
    id: &str,
    state: &str,
    received: u64,
    total: u64,
    error: Option<&str>,
    library_id: Option<&str>,
) {
    let _ = app.emit(
        DOWNLOAD_EVENT,
        DownloadEvent {
            id: id.to_owned(),
            state: state.to_owned(),
            received,
            total,
            error: error.map(str::to_owned),
            library_id: library_id.map(str::to_owned),
        },
    );
}

pub fn start(app: AppHandle, id: &str) -> Result<(), String> {
    let entry = entry_by_id(id).ok_or_else(|| format!("未知的模型：{id}"))?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut active = ACTIVE.lock().map_err(|_| "下载状态不可用。".to_owned())?;
        if let Some(existing) = active.as_ref() {
            return Err(format!("模型「{}」正在下载，请等待完成。", existing.id));
        }
        *active = Some(ActiveDownload {
            id: id.to_owned(),
            cancel: Arc::clone(&cancel),
        });
    }
    let app_data = crate::app_data_dir(&app)?;
    let cancel_flag = Arc::clone(&cancel);
    tauri::async_runtime::spawn(async move {
        let outcome = run(&app, entry, &app_data, &cancel_flag).await;
        if cancel_flag.load(Ordering::Relaxed) {
            emit(
                &app,
                entry.id,
                "cancelled",
                0,
                entry.bytes,
                Some("下载已取消。"),
                None,
            );
        } else if let Err(error) = &outcome {
            emit(&app, entry.id, "error", 0, entry.bytes, Some(error), None);
        }
        if let Ok(mut active) = ACTIVE.lock() {
            *active = None;
        }
    });
    Ok(())
}

pub fn cancel() -> Result<(), String> {
    let active = ACTIVE.lock().map_err(|_| "下载状态不可用。".to_owned())?;
    match active.as_ref() {
        Some(running) => {
            running.cancel.store(true, Ordering::Relaxed);
            Ok(())
        }
        None => Err("当前没有正在进行的下载。".to_owned()),
    }
}

async fn run(
    app: &AppHandle,
    entry: &'static CatalogEntry,
    app_data: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let temp = app_data
        .join("models")
        .join(".downloads")
        .join(entry.directory);
    fs::create_dir_all(&temp).map_err(|error| format!("无法创建下载目录：{error}"))?;

    let client = reqwest::Client::builder()
        .user_agent(format!("LocalTranscribe/{}", env!("CARGO_PKG_VERSION")))
        // 网络守护：连接 15 秒、单次读取 45 秒无数据即判为断流，
        // 避免下载无声挂起；界面会显示原因并提供重试入口。
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(45))
        .build()
        .map_err(|error| format!("无法初始化下载器：{error}"))?;

    let total = entry.bytes;
    let mut received: u64 = 0;
    emit(app, entry.id, "downloading", 0, total, None, None);

    for (remote, local, bytes) in entry.files {
        if cancel.load(Ordering::Relaxed) {
            move_to_trash(&temp);
            return Ok(());
        }
        let target = temp.join(local);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("无法创建目录：{error}"))?;
        }
        // 断点式跳过：文件已存在且大小一致时不再重复下载。
        if fs::metadata(&target).map(|meta| meta.len()).unwrap_or(0) == *bytes {
            received += *bytes;
            emit(app, entry.id, "downloading", received, total, None, None);
            continue;
        }
        let url_primary = format!("{HF_BASE}/{}/resolve/main/{}", entry.repository, remote);
        let url_mirror = format!("{HF_MIRROR}/{}/resolve/main/{}", entry.repository, remote);
        match fetch(
            &client,
            &url_primary,
            &target,
            cancel,
            app,
            entry.id,
            &mut received,
            total,
        )
        .await
        {
            Ok(()) => {}
            Err(primary_error) => {
                if cancel.load(Ordering::Relaxed) {
                    move_to_trash(&temp);
                    return Ok(());
                }
                fetch(
                    &client,
                    &url_mirror,
                    &target,
                    cancel,
                    app,
                    entry.id,
                    &mut received,
                    total,
                )
                .await
                .map_err(|mirror_error| {
                    format!("{primary_error}（镜像同样失败：{mirror_error}）")
                })?;
            }
        }
    }

    emit(app, entry.id, "installing", received, total, None, None);
    match entry.kind {
        CatalogKind::Asr => {
            // 校验失败会直接返回错误；不把未验证的模型文件交给模型库。
            inspect_model_directory(&temp)?;
            let library = ModelLibrary::new(app_data);
            let imported = library.import(&temp)?;
            if crate::read_selected_model(app)?.is_none() {
                crate::save_selected_model(app, &imported.model)?;
            }
            move_to_trash(&temp);
            emit(
                app,
                entry.id,
                "done",
                total,
                total,
                None,
                Some(&imported.model.id),
            );
        }
        CatalogKind::Punctuation => {
            let source = temp.join("model.onnx");
            if fs::metadata(&source).map(|meta| meta.len()).unwrap_or(0) == 0 {
                return Err("标点模型下载不完整，请重试。".to_owned());
            }
            let target = punctuation_model_path(app_data);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| format!("无法创建目录：{error}"))?;
            }
            if target.exists() {
                move_to_trash(&target);
            }
            fs::rename(&source, &target).map_err(|error| format!("无法安装标点模型：{error}"))?;
            move_to_trash(&temp);
            emit(app, entry.id, "done", total, total, None, None);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn fetch(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
    cancel: &AtomicBool,
    app: &AppHandle,
    id: &str,
    received: &mut u64,
    total: u64,
) -> Result<(), String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("下载失败：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("下载失败（HTTP {}）：{url}", response.status()));
    }
    let mut file = fs::File::create(target).map_err(|error| format!("无法写入文件：{error}"))?;
    let mut last_emit = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        // 定时器：45 秒收不到任何数据即视为网络中断，向上抛出可读原因。
        let next = tokio::time::timeout(Duration::from_secs(45), response.chunk()).await;
        let chunk = match next {
            Ok(Ok(Some(chunk))) => chunk,
            Ok(Ok(None)) => break,
            Ok(Err(error)) => return Err(format!("下载中断：{error}")),
            Err(_) => {
                return Err("网络超时：45 秒没有收到数据。请检查网络或代理后重试。".to_owned());
            }
        };
        file.write_all(&chunk)
            .map_err(|error| format!("无法写入文件：{error}"))?;
        *received += chunk.len() as u64;
        if last_emit.elapsed() >= Duration::from_millis(200) {
            emit(app, id, "downloading", *received, total, None, None);
            last_emit = Instant::now();
        }
    }
    file.flush()
        .map_err(|error| format!("无法完成写入：{error}"))?;
    emit(app, id, "downloading", *received, total, None, None);
    Ok(())
}

/// 下载临时目录与旧标点模型的清理一律进回收站，不永久删除。
fn move_to_trash(target: &Path) {
    if !target.exists() {
        return;
    }
    let trash = default_trash_dir();
    if fs::create_dir_all(&trash).is_err() {
        return;
    }
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".to_owned());
    let destination = trash.join(format!("{name}-{}", crate::unix_now_ms()));
    let _ = fs::rename(target, destination);
}

#[tauri::command]
pub fn model_catalog(app: AppHandle) -> Result<Vec<CatalogState>, String> {
    let app_data = crate::app_data_dir(&app)?;
    Ok(states(&app_data))
}

#[tauri::command]
pub fn download_model(app: AppHandle, id: String) -> Result<(), String> {
    start(app, &id)
}

#[tauri::command]
pub fn cancel_model_download() -> Result<(), String> {
    cancel()
}

#[cfg(test)]
mod tests {
    use super::{CATALOG, CatalogKind, entry_by_id, punctuation_model_path};

    #[test]
    fn catalog_ids_are_unique() {
        let mut ids: Vec<&str> = CATALOG.iter().map(|entry| entry.id).collect();
        ids.sort_unstable();
        let unique = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), unique);
    }

    #[test]
    fn catalog_bytes_match_file_sizes() {
        for entry in CATALOG {
            let sum: u64 = entry.files.iter().map(|(_, _, bytes)| *bytes).sum();
            assert_eq!(sum, entry.bytes, "{} size mismatch", entry.id);
        }
    }

    #[test]
    fn catalog_directories_match_detected_families() {
        for entry in CATALOG {
            match entry.kind {
                CatalogKind::Asr => {
                    let lower = entry.directory.to_lowercase();
                    let expected = match entry.family {
                        "Paraformer" => lower.contains("paraformer"),
                        "SenseVoice" => lower.contains("sense-voice"),
                        "Whisper" => lower.contains("whisper"),
                        "Qwen3-ASR" => lower.contains("qwen3"),
                        "FireRedASR" => lower.contains("fire-red"),
                        other => panic!("unknown family {other}"),
                    };
                    assert!(expected, "{} directory does not match family", entry.id);
                }
                CatalogKind::Punctuation => {
                    assert_eq!(entry.files.len(), 1);
                }
            }
        }
    }

    #[test]
    fn lookup_returns_known_entries_only() {
        assert!(entry_by_id("paraformer-zh").is_some());
        assert!(entry_by_id("not-a-model").is_none());
    }

    #[test]
    fn punctuation_path_is_inside_models() {
        let path = punctuation_model_path(std::path::Path::new("/tmp/app"));
        assert!(path.ends_with("models/punctuation-ct-transformer/model.onnx"));
    }
}
