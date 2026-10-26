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
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use voice_to_text_core::{ModelLibrary, default_trash_dir, inspect_model_directory};
use voice_to_text_lan::llm::{LLM_MODELS, LlmModelSpec, model_spec};

pub const DOWNLOAD_EVENT: &str = "voice-to-text:model-download";

const HF_BASE: &str = "https://huggingface.co";
const HF_MIRROR: &str = "https://hf-mirror.com";
/// 语言模型下载的总尝试次数（主源/镜像交替，全部失败才报错；断点续传保留进度）。
const LLM_DOWNLOAD_ATTEMPTS: usize = 8;
const ASR_MEMORY_OVERHEAD_BYTES: u64 = 256 * 1024 * 1024;

/// Planning estimate for resident ASR/helper models, not a measured peak.
/// The 1.7x package allowance plus 256 MiB approximates existing FireRedASR2
/// and Zipformer CTC measurements; all other catalog rows remain estimates.
pub fn estimated_asr_memory_bytes(model_bytes: u64) -> u64 {
    model_bytes
        .saturating_mul(17)
        .saturating_div(10)
        .saturating_add(ASR_MEMORY_OVERHEAD_BYTES)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogKind {
    Asr,
    Punctuation,
}

/// 模型在界面上的分组：准确优先 / 速度优先 / 辅助模型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogTier {
    Quality,
    Speed,
    Punctuation,
    Language,
}

impl CatalogTier {
    fn as_str(self) -> &'static str {
        match self {
            CatalogTier::Quality => "quality",
            CatalogTier::Speed => "speed",
            CatalogTier::Punctuation => "punctuation",
            CatalogTier::Language => "language",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub family: &'static str,
    pub directory: &'static str,
    pub repository: &'static str,
    pub kind: CatalogKind,
    pub tier: CatalogTier,
    /// 语言标签，只用于列表副标题；空字符串表示清单原本没写语言（不补猜）。
    pub languages: &'static str,
    pub bytes: u64,
    /// (仓库内路径, 本地文件名, 字节数)
    pub files: &'static [(&'static str, &'static str, u64)],
}

const PARAFORMER_FILES: &[(&str, &str, u64)] = &[
    ("model.int8.onnx", "model.int8.onnx", 243_371_218),
    ("tokens.txt", "tokens.txt", 75_756),
];
const SENSEVOICE_FILES: &[(&str, &str, u64)] = &[
    ("model.int8.onnx", "model.int8.onnx", 237_115_547),
    ("tokens.txt", "tokens.txt", 315_894),
];
const X_ASR_FILES: &[(&str, &str, u64)] = &[
    (
        "encoder-epoch-99-avg-1.int8.onnx",
        "encoder-epoch-99-avg-1.int8.onnx",
        161_744_450,
    ),
    (
        "decoder-epoch-99-avg-1.onnx",
        "decoder-epoch-99-avg-1.onnx",
        11_309_084,
    ),
    (
        "joiner-epoch-99-avg-1.int8.onnx",
        "joiner-epoch-99-avg-1.int8.onnx",
        2_581_422,
    ),
    ("tokens.txt", "tokens.txt", 59_240),
];
const ZIPFORMER_CTC_FILES: &[(&str, &str, u64)] = &[
    ("model.int8.onnx", "model.int8.onnx", 62_666_860),
    ("tokens.txt", "tokens.txt", 13_366),
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
    // ---- 准确优先（实测 CER 从低到高）----
    CatalogEntry {
        id: "x-asr-zh-en",
        name: "X-ASR Zipformer",
        family: "X-ASR",
        languages: "zh/en",
        directory: "sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03",
        repository: "csukuangfj2/sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03",
        kind: CatalogKind::Asr,
        tier: CatalogTier::Quality,
        bytes: 175_694_196,
        files: X_ASR_FILES,
    },
    CatalogEntry {
        id: "fire-red-asr2",
        name: "FireRedASR2 (int8)",
        family: "FireRedASR",
        languages: "zh/en",
        directory: "sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26",
        repository: "csukuangfj2/sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26",
        kind: CatalogKind::Asr,
        tier: CatalogTier::Quality,
        bytes: 1_234_657_933,
        files: FIRERED_FILES,
    },
    CatalogEntry {
        id: "qwen3-asr-0.6b",
        name: "Qwen3-ASR 0.6B (int8)",
        family: "Qwen3-ASR",
        languages: "",
        directory: "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
        repository: "csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
        kind: CatalogKind::Asr,
        tier: CatalogTier::Quality,
        bytes: 987_015_347,
        files: QWEN3_FILES,
    },
    // ---- 速度优先 ----
    CatalogEntry {
        id: "sensevoice-small",
        name: "SenseVoice small（2025）",
        family: "SenseVoice",
        languages: "zh/en/ja/ko/yue",
        directory: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
        repository: "csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
        kind: CatalogKind::Asr,
        tier: CatalogTier::Speed,
        bytes: 237_431_441,
        files: SENSEVOICE_FILES,
    },
    CatalogEntry {
        id: "paraformer-zh",
        name: "Paraformer · 2023-09-14",
        family: "Paraformer",
        languages: "zh",
        directory: "sherpa-onnx-paraformer-zh-2023-09-14",
        repository: "csukuangfj/sherpa-onnx-paraformer-zh-2023-09-14",
        kind: CatalogKind::Asr,
        tier: CatalogTier::Speed,
        bytes: 243_446_974,
        files: PARAFORMER_FILES,
    },
    CatalogEntry {
        id: "zipformer-ctc-small-zh",
        name: "Zipformer CTC small（体积最小）",
        family: "Zipformer CTC",
        languages: "zh",
        directory: "sherpa-onnx-zipformer-ctc-small-zh-int8-2025-07-16",
        repository: "csukuangfj/sherpa-onnx-zipformer-ctc-small-zh-int8-2025-07-16",
        kind: CatalogKind::Asr,
        tier: CatalogTier::Speed,
        bytes: 62_680_226,
        files: ZIPFORMER_CTC_FILES,
    },
    // ---- 辅助 ----
    CatalogEntry {
        id: "punctuation-zh-en",
        name: "CT-Transformer punctuation",
        family: "Punctuation",
        languages: "zh/en",
        directory: "punctuation-ct-transformer",
        repository: "csukuangfj/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12",
        kind: CatalogKind::Punctuation,
        tier: CatalogTier::Punctuation,
        bytes: 294_372_519,
        files: PUNCT_FILES,
    },
];

/// Language label of a catalog entry by its exact directory name.
pub fn languages_for_directory(directory: &str) -> Option<&'static str> {
    CATALOG
        .iter()
        .find(|entry| entry.directory == directory)
        .map(|entry| entry.languages)
}

fn should_select_downloaded_asr(
    preferred_model_id: Option<&str>,
    has_selected_model: bool,
    downloaded_model_id: &str,
) -> bool {
    match preferred_model_id {
        Some(preferred) => preferred == downloaded_model_id,
        None => !has_selected_model,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogState {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub family: String,
    pub kind: String,
    pub tier: String,
    pub languages: String,
    pub bytes: u64,
    pub installed: bool,
    pub estimated_memory_bytes: u64,
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

fn llm_installed_path(app_data: &Path, model: &LlmModelSpec) -> PathBuf {
    app_data.join("llm").join(model.file_name)
}

/// Where a catalog entry lands once installed.
pub fn installed_path(app_data: &Path, id: &str) -> Option<PathBuf> {
    if let Some(model) = model_spec(id) {
        return Some(llm_installed_path(app_data, model));
    }
    let entry = entry_by_id(id)?;
    Some(match entry.kind {
        CatalogKind::Asr => app_data.join("models").join(entry.directory),
        CatalogKind::Punctuation => punctuation_model_path(app_data),
    })
}

/// Remove an installed catalog model by moving it to the OS trash.
pub fn delete_installed(app_data: &Path, id: &str) -> Result<PathBuf, String> {
    delete_installed_to(app_data, id, &default_trash_dir())
}

fn delete_installed_to(app_data: &Path, id: &str, trash: &Path) -> Result<PathBuf, String> {
    if let Some(model) = model_spec(id) {
        let target = llm_installed_path(app_data, model);
        if !target.exists() {
            return Err("这个模型还没有安装。".to_owned());
        }
        return move_any_to_trash(&target, trash);
    }
    let entry = entry_by_id(id).ok_or_else(|| "未知的模型条目。".to_owned())?;
    let target = match entry.kind {
        CatalogKind::Asr => app_data.join("models").join(entry.directory),
        CatalogKind::Punctuation => punctuation_model_path(app_data),
    };
    if !target.exists() {
        return Err("这个模型还没有安装。".to_owned());
    }
    match entry.kind {
        CatalogKind::Asr => {
            let model = inspect_model_directory(&target)?;
            ModelLibrary::new(app_data).delete_to(&model.id, trash)
        }
        CatalogKind::Punctuation => move_any_to_trash(&target, trash),
    }
}

/// Move a file (or directory) into `trash`, keeping the original name.
fn move_any_to_trash(target: &Path, trash: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(trash).map_err(|error| format!("Cannot prepare the trash: {error}"))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".to_owned());
    let destination = trash.join(format!("{name}.{stamp}"));
    fs::rename(target, &destination)
        .map_err(|error| format!("Cannot move to the trash: {error}"))?;
    Ok(destination)
}

fn entry_by_id(id: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|entry| entry.id == id)
}

pub fn states(app_data: &Path) -> Vec<CatalogState> {
    let mut states = CATALOG
        .iter()
        .map(|entry| {
            let installed = match entry.kind {
                CatalogKind::Asr => app_data.join("models").join(entry.directory).is_dir(),
                CatalogKind::Punctuation => punctuation_model_path(app_data).is_file(),
            };
            CatalogState {
                id: entry.id.to_owned(),
                name: entry.name.to_owned(),
                directory: entry.directory.to_owned(),
                family: entry.family.to_owned(),
                kind: match entry.kind {
                    CatalogKind::Asr => "asr",
                    CatalogKind::Punctuation => "punctuation",
                }
                .to_owned(),
                tier: entry.tier.as_str().to_owned(),
                languages: entry.languages.to_owned(),
                bytes: entry.bytes,
                installed,
                estimated_memory_bytes: estimated_asr_memory_bytes(entry.bytes),
            }
        })
        .collect::<Vec<_>>();
    states.extend(LLM_MODELS.iter().map(|model| {
        let path = llm_installed_path(app_data, model);
        CatalogState {
            id: model.id.to_owned(),
            name: model.name.to_owned(),
            directory: model.file_name.to_owned(),
            family: model.family.to_owned(),
            kind: "llm".to_owned(),
            tier: CatalogTier::Language.as_str().to_owned(),
            languages: model.languages.to_owned(),
            bytes: model.bytes,
            installed: fs::metadata(path)
                .map(|metadata| metadata.len() == model.bytes)
                .unwrap_or(false),
            estimated_memory_bytes: voice_to_text_lan::llm::required_memory_bytes(model.bytes),
        }
    }));
    states
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
    let entry = entry_by_id(id);
    let llm_model = model_spec(id);
    if entry.is_none() && llm_model.is_none() {
        return Err(format!("未知的模型：{id}"));
    }
    let app_data = crate::app_data_dir(&app)?;
    let download_id = id.to_owned();
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
    let cancel_flag = Arc::clone(&cancel);
    tauri::async_runtime::spawn(async move {
        let total = entry.map_or_else(|| llm_model.unwrap().bytes, |entry| entry.bytes);
        let outcome = match (entry, llm_model) {
            (Some(entry), _) => run(&app, entry, &app_data, &cancel_flag).await,
            (_, Some(model)) => run_language_model(&app, model, &app_data, &cancel_flag).await,
            (None, None) => unreachable!("validated catalog entry"),
        };
        if cancel_flag.load(Ordering::Relaxed) {
            emit(
                &app,
                &download_id,
                "cancelled",
                0,
                total,
                Some("下载已取消。"),
                None,
            );
        } else if let Err(error) = &outcome {
            emit(&app, &download_id, "error", 0, total, Some(error), None);
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
        let mut progress = |received: u64, total: u64| {
            emit(app, entry.id, "downloading", received, total, None, None);
        };
        // base：本文件之前的累计进度；断流重试按暂存大小续传，不丢弃已下载部分。
        let base = received;
        match fetch(
            &client,
            &url_primary,
            &target,
            cancel,
            &mut received,
            base,
            total,
            *bytes,
            &mut progress,
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
                    &mut received,
                    base,
                    total,
                    *bytes,
                    &mut progress,
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
            let preferred = crate::onboarding::read_state(app_data)?.preferred_asr_model_id;
            if should_select_downloaded_asr(
                preferred.as_deref(),
                crate::read_selected_model(app)?.is_some(),
                &imported.model.id,
            ) {
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

async fn run_language_model(
    app: &AppHandle,
    model: &'static LlmModelSpec,
    app_data: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let temp = app_data.join("llm").join(".downloads").join(model.id);
    fs::create_dir_all(&temp).map_err(|error| format!("无法创建下载目录：{error}"))?;
    let target = temp.join(model.file_name);
    let total = model.bytes;
    let mut received = 0;
    emit(app, model.id, "downloading", 0, total, None, None);

    if verify_llm_file(&target, model.bytes, model.sha256).is_err() {
        let client = reqwest::Client::builder()
            .user_agent(format!("LocalTranscribe/{}", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(45))
            .build()
            .map_err(|error| format!("无法初始化下载器：{error}"))?;
        let remote = format!(
            "{}/{}/resolve/{}/{}",
            HF_BASE, model.repository, model.revision, model.file_name
        );
        let mirror = format!(
            "{}/{}/resolve/{}/{}",
            HF_MIRROR, model.repository, model.revision, model.file_name
        );
        let mut progress =
            |got: u64, total: u64| emit(app, model.id, "downloading", got, total, None, None);
        // 断点续传 + 自动重试：主源/镜像交替，断流不再把已有进度清零，
        // 也不需要用户手动重试；取消随时生效且保留暂存文件供下次续传。
        fetch_with_fallback(
            &client,
            &[remote.clone(), mirror.clone()],
            &target,
            cancel,
            &mut received,
            total,
            model.bytes,
            LLM_DOWNLOAD_ATTEMPTS,
            &mut progress,
        )
        .await?;
    } else {
        received = total;
        emit(app, model.id, "downloading", received, total, None, None);
    }

    if cancel.load(Ordering::Relaxed) {
        move_to_trash(&temp);
        return Ok(());
    }
    verify_llm_file(&target, model.bytes, model.sha256)?;
    emit(app, model.id, "installing", total, total, None, None);

    let destination = llm_installed_path(app_data, model);
    let parent = destination
        .parent()
        .ok_or_else(|| "无法确定大模型安装目录。".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建大模型目录：{error}"))?;
    if destination.exists() {
        move_to_trash(&destination);
    }
    fs::rename(&target, &destination).map_err(|error| format!("无法安装大模型：{error}"))?;
    move_to_trash(&temp);
    emit(app, model.id, "done", total, total, None, None);
    Ok(())
}

fn verify_llm_file(path: &Path, expected_bytes: u64, expected_sha256: &str) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| format!("无法读取模型文件：{error}"))?;
    if metadata.len() != expected_bytes {
        return Err(format!(
            "模型文件体积不符：收到 {} 字节，预期 {expected_bytes} 字节。",
            metadata.len()
        ));
    }
    let mut file = fs::File::open(path).map_err(|error| format!("无法打开模型文件：{error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("无法校验模型文件：{error}"))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_sha256 {
        return Err("模型文件 SHA-256 不匹配；请重新下载。".to_owned());
    }
    Ok(())
}

/// 下载单个文件到 `target`，累计进度经回调上报（便于无界面测试）。
///
/// `base` 是此前文件已计入的累计字节数，本文件的进度按 `base + 已写入` 上报。
/// 已有暂存文件会带 `Range` 断点续传；服务器不支持时从零重写并把进度归零，
/// 避免主源断流切镜像时把已有进度清零重下。
#[allow(clippy::too_many_arguments)]
async fn fetch(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
    cancel: &AtomicBool,
    received: &mut u64,
    base: u64,
    total: u64,
    file_bytes: u64,
    on_progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<(), String> {
    // 已达完整体积却仍需下载（校验未过的旧文件），归零避免 Range 越界死循环。
    let mut start = fs::metadata(target).map(|meta| meta.len()).unwrap_or(0);
    if start >= file_bytes {
        start = 0;
    }
    let mut request = client.get(url);
    if start > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={start}-"));
    }
    let mut response = request
        .send()
        .await
        .map_err(|error| format!("下载失败：{error}"))?;
    if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        *received = base + start;
    } else {
        // 服务器无视或拒绝了 Range：从零重写，进度同步归零。
        start = 0;
        *received = base;
    }
    if !response.status().is_success() {
        return Err(format!("下载失败（HTTP {}）：{url}", response.status()));
    }
    let mut file = if start > 0 {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(target)
            .map_err(|error| format!("无法写入文件：{error}"))?
    } else {
        fs::File::create(target).map_err(|error| format!("无法写入文件：{error}"))?
    };
    on_progress(*received, total);
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
            on_progress(*received, total);
            last_emit = Instant::now();
        }
    }
    file.flush()
        .map_err(|error| format!("无法完成写入：{error}"))?;
    on_progress(*received, total);
    Ok(())
}

/// 主源与镜像交替的断点续传下载：每次失败保留暂存文件并按其大小续传，
/// 直到成功或用尽尝试次数；取消随时生效（暂存文件保留供下次续传）。
#[allow(clippy::too_many_arguments)]
async fn fetch_with_fallback(
    client: &reqwest::Client,
    urls: &[String],
    target: &Path,
    cancel: &AtomicBool,
    received: &mut u64,
    total: u64,
    file_bytes: u64,
    attempts: usize,
    on_progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<(), String> {
    let attempts = attempts.max(1);
    let mut last_error = "未知错误".to_owned();
    for attempt in 0..attempts {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let url = &urls[attempt % urls.len()];
        match fetch(
            client,
            url,
            target,
            cancel,
            received,
            0,
            total,
            file_bytes,
            on_progress,
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(error) => {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                last_error = format!("{url}：{error}");
                // 断流后短暂退避再重试；暂存文件保留，重试从原大小续传。
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(format!(
        "下载失败（已尝试 {attempts} 次主源与镜像，均告失败）：{last_error}"
    ))
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
    use super::{
        CATALOG, CatalogKind, CatalogTier, delete_installed_to, entry_by_id, installed_path,
        punctuation_model_path, should_select_downloaded_asr, states, verify_llm_file,
    };
    use voice_to_text_lan::llm::{LLM_MODELS, model_spec};

    #[test]
    fn downloading_a_different_model_does_not_override_an_explicit_uninstalled_preference() {
        assert!(!should_select_downloaded_asr(
            Some("recommended-model"),
            false,
            "another-model"
        ));
        assert!(should_select_downloaded_asr(
            Some("downloaded-model"),
            false,
            "downloaded-model"
        ));
        assert!(!should_select_downloaded_asr(
            None,
            true,
            "downloaded-model"
        ));
        assert!(should_select_downloaded_asr(
            None,
            false,
            "downloaded-model"
        ));
    }

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
                        "X-ASR" => lower.contains("x-asr"),
                        "Zipformer CTC" => lower.contains("zipformer"),
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
    fn catalog_offers_three_quality_and_three_speed_models() {
        let quality = CATALOG
            .iter()
            .filter(|entry| entry.kind == CatalogKind::Asr && entry.tier == CatalogTier::Quality)
            .count();
        let speed = CATALOG
            .iter()
            .filter(|entry| entry.kind == CatalogKind::Asr && entry.tier == CatalogTier::Speed)
            .count();
        assert_eq!(quality, 3);
        assert_eq!(speed, 3);
    }

    #[test]
    fn every_catalog_model_includes_a_memory_estimate() {
        let states = states(std::path::Path::new("/definitely-not-installed"));
        assert_eq!(states.len(), CATALOG.len() + LLM_MODELS.len());
        for state in states {
            assert!(
                state.estimated_memory_bytes > state.bytes,
                "{} estimate should exceed file size",
                state.id
            );
        }
    }

    #[test]
    fn asr_memory_estimate_uses_a_labeled_planning_heuristic() {
        assert_eq!(
            super::estimated_asr_memory_bytes(1_000_000_000),
            1_968_435_456
        );
    }

    #[test]
    fn lookup_returns_known_entries_only() {
        assert!(entry_by_id("paraformer-zh").is_some());
        assert!(entry_by_id("not-a-model").is_none());
    }

    #[test]
    fn language_model_catalog_pins_official_files_sizes_revisions_and_hashes() {
        assert_eq!(LLM_MODELS.len(), 3);
        let expected = [
            (
                "qwen3-4b",
                "Qwen/Qwen3-4B-GGUF",
                "bc640142c66e1fdd12af0bd68f40445458f3869b",
                "Qwen3-4B-Q4_K_M.gguf",
                2_497_280_256,
                "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
            ),
            (
                "qwen3-8b",
                "Qwen/Qwen3-8B-GGUF",
                "7c41481f57cb95916b40956ab2f0b139b296d974",
                "Qwen3-8B-Q4_K_M.gguf",
                5_027_783_488,
                "d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785",
            ),
            (
                "qwen3-1.7b",
                "Qwen/Qwen3-1.7B-GGUF",
                "90862c4b9d2787eaed51d12237eafdfe7c5f6077",
                "Qwen3-1.7B-Q8_0.gguf",
                1_834_426_016,
                "061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a",
            ),
        ];
        for (model, (id, repository, revision, file_name, bytes, sha256)) in
            LLM_MODELS.iter().zip(expected)
        {
            assert_eq!(model.id, id);
            assert_eq!(model.repository, repository);
            assert_eq!(model.revision, revision);
            assert_eq!(model.file_name, file_name);
            assert_eq!(model.bytes, bytes);
            assert_eq!(model.sha256, sha256);
            assert_eq!(model_spec(model.id), Some(model), "{}", model.id);
            assert_eq!(model.license, "Apache-2.0", "{}", model.id);
        }
    }

    #[test]
    fn language_model_catalog_rows_include_memory_estimates() {
        let states = super::states(std::path::Path::new("/definitely-not-installed"));
        for id in ["qwen3-4b", "qwen3-8b", "qwen3-1.7b"] {
            let state = states
                .iter()
                .find(|state| state.id == id)
                .expect("catalog row");
            assert_eq!(state.kind, "llm");
            assert_eq!(state.tier, "language");
            assert!(state.estimated_memory_bytes > state.bytes);
            assert!(!state.installed);
        }
    }

    #[test]
    fn language_model_install_path_and_delete_use_the_trash() {
        let root =
            std::env::temp_dir().join(format!("llm-catalog-delete-{}", uuid::Uuid::new_v4()));
        let app_data = root.join("data");
        let target = installed_path(&app_data, "qwen3-4b").expect("Qwen path");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"test model").unwrap();
        let trash = root.join("trash");

        let moved = delete_installed_to(&app_data, "qwen3-4b", &trash).expect("move to trash");
        assert!(!target.exists());
        assert_eq!(std::fs::read(moved).unwrap(), b"test model");
    }

    #[test]
    fn language_model_files_are_rejected_when_size_or_digest_differs() {
        let root = std::env::temp_dir().join(format!("llm-digest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("small-test.gguf");
        std::fs::write(&file, b"verified model bytes").unwrap();

        assert!(
            verify_llm_file(
                &file,
                20,
                "03cfa25d83f5eaa1faac98ed6ceaaf0e7afe3c273a1e1502c2714ebe10b8263e"
            )
            .is_ok()
        );
        assert!(
            verify_llm_file(
                &file,
                21,
                "03cfa25d83f5eaa1faac98ed6ceaaf0e7afe3c273a1e1502c2714ebe10b8263e"
            )
            .is_err()
        );
        assert!(verify_llm_file(&file, 20, &"0".repeat(64)).is_err());
    }

    #[test]
    #[ignore = "network access to huggingface.co; run manually"]
    fn downloads_a_small_catalog_file_with_progress() {
        use std::sync::atomic::AtomicBool;
        let target = std::env::temp_dir().join("lt-download-test-tokens.txt");
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .read_timeout(std::time::Duration::from_secs(45))
                .build()
                .expect("client builds");
            let url = "https://huggingface.co/csukuangfj/sherpa-onnx-paraformer-zh-2023-09-14/resolve/main/tokens.txt";
            let mut received = 0u64;
            let mut samples: Vec<u64> = Vec::new();
            let mut on_progress = |got: u64, _total: u64| {
                samples.push(got);
            };
            super::fetch(
                &client,
                url,
                &target,
                &AtomicBool::new(false),
                &mut received,
                0,
                75_756,
                75_756,
                &mut on_progress,
            )
            .await
            .expect("download succeeds");
            assert_eq!(received, 75_756);
            assert!(!samples.is_empty(), "progress callback fired");
            assert!(
                samples.windows(2).all(|pair| pair[0] <= pair[1]),
                "progress must not go backwards"
            );
        });
        let size = std::fs::metadata(&target)
            .expect("downloaded file exists")
            .len();
        assert_eq!(size, 75_756, "downloaded bytes match the manifest");
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn deleting_an_installed_asr_model_moves_it_to_trash() {
        let root = std::env::temp_dir().join(format!("catalog-delete-{}", uuid::Uuid::new_v4()));
        let app_data = root.join("data");
        let model_dir = app_data
            .join("models")
            .join("sherpa-onnx-paraformer-zh-2023-09-14");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.onnx"), b"layout-only").unwrap();
        std::fs::write(model_dir.join("tokens.txt"), b"a 1\n").unwrap();
        let trash = root.join("trash");

        let moved = delete_installed_to(&app_data, "paraformer-zh", &trash)
            .expect("installed model can be deleted");
        assert!(!model_dir.exists(), "library copy leaves the model folder");
        assert!(moved.exists(), "the model folder lands in the trash");
    }

    #[test]
    fn deleting_a_model_that_is_not_installed_reports_it() {
        let root = std::env::temp_dir().join(format!("catalog-delete-{}", uuid::Uuid::new_v4()));
        let error = delete_installed_to(&root.join("data"), "paraformer-zh", &root.join("trash"))
            .expect_err("nothing to delete");
        assert!(error.contains("还没有安装"), "unexpected message: {error}");
    }

    #[test]
    fn deleting_the_punctuation_model_moves_the_file_to_trash() {
        let root = std::env::temp_dir().join(format!("catalog-delete-{}", uuid::Uuid::new_v4()));
        let app_data = root.join("data");
        let file = punctuation_model_path(&app_data);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"layout-only").unwrap();
        let trash = root.join("trash");

        let moved = delete_installed_to(&app_data, "punctuation-zh-en", &trash)
            .expect("punctuation model can be deleted");
        assert!(!file.exists());
        assert!(moved.exists());
    }

    #[test]
    fn punctuation_path_is_inside_models() {
        let path = punctuation_model_path(std::path::Path::new("/tmp/app"));
        assert!(path.ends_with("models/punctuation-ct-transformer/model.onnx"));
    }

    // ---- 断点续传（HTTP Range）：主源断流后镜像/重试不得清零已有进度 ----

    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    use super::{fetch, fetch_with_fallback};

    /// 极简 HTTP 服务器：记录原始请求头；`honor_range=false` 时无视 Range 返回 200 全量；
    /// `truncate_after=Some(n)` 时首个连接只发送 n 字节后直接断开（模拟断流）。
    fn spawn_range_server(
        body: Vec<u8>,
        honor_range: bool,
        truncate_after: Option<usize>,
        connections: usize,
    ) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("server address");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for index in 0..connections {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut request = Vec::new();
                let mut buffer = [0u8; 2048];
                loop {
                    let count = stream.read(&mut buffer).expect("read request");
                    if count == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..count]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                log.lock()
                    .expect("request log")
                    .push(String::from_utf8_lossy(&request).into_owned());
                let range_start = String::from_utf8_lossy(&request).lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if !name.trim().eq_ignore_ascii_case("range") {
                        return None;
                    }
                    let spec = value.trim().strip_prefix("bytes=")?;
                    spec.split('-').next()?.trim().parse::<usize>().ok()
                });
                let start = match range_start {
                    Some(value) if honor_range => value,
                    _ => 0,
                };
                let payload = body[start..].to_vec();
                let status = if start > 0 {
                    "206 Partial Content"
                } else {
                    "200 OK"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                )
                .expect("write headers");
                let cut = if index == 0 {
                    truncate_after.map(|count| count.min(payload.len()))
                } else {
                    None
                };
                match cut {
                    Some(count) => stream.write_all(&payload[..count]).expect("write partial"),
                    None => stream.write_all(&payload).expect("write body"),
                }
                stream.flush().expect("flush");
            }
        });
        (format!("http://{address}/model.gguf"), seen)
    }

    fn fetch_test_client() -> reqwest::Client {
        // 测试走本机回环，显式绕过环境代理，避免 Clash 代理拦截 127.0.0.1。
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("test client")
    }

    #[tokio::test]
    async fn fetch_resumes_a_partial_file_after_interruption() {
        let body: Vec<u8> = (0..65_536).map(|index| (index % 251) as u8).collect();
        let (url, seen) = spawn_range_server(body.clone(), true, Some(9_777), 2);
        let root = std::env::temp_dir().join(format!("llm-resume-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("temp dir");
        let target = root.join("model.gguf");
        let cancel = AtomicBool::new(false);
        let client = fetch_test_client();
        let mut progress = |_: u64, _: u64| {};

        let mut received = 0u64;
        let first = fetch(
            &client,
            &url,
            &target,
            &cancel,
            &mut received,
            0,
            body.len() as u64,
            body.len() as u64,
            &mut progress,
        )
        .await;
        assert!(first.is_err(), "abrupt close must surface as an error");
        assert_eq!(
            std::fs::metadata(&target).expect("partial file").len(),
            9_777,
            "bytes written before interruption are kept"
        );

        let mut received = 0u64;
        let second = fetch(
            &client,
            &url,
            &target,
            &cancel,
            &mut received,
            0,
            body.len() as u64,
            body.len() as u64,
            &mut progress,
        )
        .await;
        assert!(second.is_ok(), "resume attempt succeeds: {second:?}");
        assert_eq!(
            std::fs::read(&target).expect("resumed file"),
            body,
            "resumed file matches the full body"
        );
        assert_eq!(received, body.len() as u64);
        let requests = seen.lock().expect("request log");
        assert_eq!(requests.len(), 2);
        assert!(
            requests[1]
                .to_ascii_lowercase()
                .contains("range: bytes=9777-"),
            "second request must resume at 9777: {:?}",
            requests[1]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn fetch_rewrites_when_the_server_ignores_range() {
        let body: Vec<u8> = (0..8_192).map(|index| (index % 251) as u8).collect();
        let (url, seen) = spawn_range_server(body.clone(), false, None, 1);
        let root = std::env::temp_dir().join(format!("llm-norange-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("temp dir");
        let target = root.join("model.gguf");
        std::fs::write(&target, &body[..1_024]).expect("partial file");
        let cancel = AtomicBool::new(false);
        let client = fetch_test_client();
        let mut progress = |_: u64, _: u64| {};

        let mut received = 0u64;
        let outcome = fetch(
            &client,
            &url,
            &target,
            &cancel,
            &mut received,
            0,
            body.len() as u64,
            body.len() as u64,
            &mut progress,
        )
        .await;
        assert!(outcome.is_ok(), "plain full download succeeds: {outcome:?}");
        assert_eq!(
            std::fs::read(&target).expect("rewritten file"),
            body,
            "a 200 answer restarts from zero without duplicating bytes"
        );
        assert_eq!(received, body.len() as u64);
        assert!(
            seen.lock().expect("request log")[0]
                .to_ascii_lowercase()
                .contains("range: bytes=1024-"),
            "resume was requested before the fallback"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn fetch_restarts_a_complete_sized_stale_file() {
        let body: Vec<u8> = (0..4_096).map(|index| (index % 251) as u8).collect();
        let (url, seen) = spawn_range_server(body.clone(), true, None, 1);
        let root = std::env::temp_dir().join(format!("llm-stale-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("temp dir");
        let target = root.join("model.gguf");
        std::fs::write(&target, vec![7u8; body.len() + 10]).expect("stale file");
        let cancel = AtomicBool::new(false);
        let client = fetch_test_client();
        let mut progress = |_: u64, _: u64| {};

        let mut received = 0u64;
        let outcome = fetch(
            &client,
            &url,
            &target,
            &cancel,
            &mut received,
            0,
            body.len() as u64,
            body.len() as u64,
            &mut progress,
        )
        .await;
        assert!(
            outcome.is_ok(),
            "stale full-size file restarts: {outcome:?}"
        );
        assert_eq!(std::fs::read(&target).expect("rewritten file"), body);
        assert!(
            !seen.lock().expect("request log")[0]
                .to_ascii_lowercase()
                .contains("range:"),
            "an oversized stale file must not request a Range"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn fallback_retries_and_resumes_after_interruption() {
        let body: Vec<u8> = (0..49_152).map(|index| (index % 251) as u8).collect();
        let (url, seen) = spawn_range_server(body.clone(), true, Some(4_096), 2);
        let root = std::env::temp_dir().join(format!("llm-fallback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("temp dir");
        let target = root.join("model.gguf");
        let cancel = AtomicBool::new(false);
        let client = fetch_test_client();
        let mut progress = |_: u64, _: u64| {};

        let mut received = 0u64;
        let outcome = fetch_with_fallback(
            &client,
            &[url.clone(), url.clone()],
            &target,
            &cancel,
            &mut received,
            body.len() as u64,
            body.len() as u64,
            4,
            &mut progress,
        )
        .await;
        assert!(outcome.is_ok(), "automatic retry succeeds: {outcome:?}");
        assert_eq!(std::fs::read(&target).expect("resumed file"), body);
        assert_eq!(received, body.len() as u64);
        let requests = seen.lock().expect("request log");
        assert_eq!(requests.len(), 2, "first attempt failed, second resumed");
        assert!(
            requests[1]
                .to_ascii_lowercase()
                .contains("range: bytes=4096-"),
            "retry must resume at 4096: {:?}",
            requests[1]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn fallback_reports_the_last_error_when_attempts_run_out() {
        let body: Vec<u8> = (0..16_384).map(|index| (index % 251) as u8).collect();
        let (url, _seen) = spawn_range_server(body.clone(), true, Some(1_024), 1);
        let root = std::env::temp_dir().join(format!("llm-fallback-err-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("temp dir");
        let target = root.join("model.gguf");
        let cancel = AtomicBool::new(false);
        let client = fetch_test_client();
        let mut progress = |_: u64, _: u64| {};

        let mut received = 0u64;
        let outcome = fetch_with_fallback(
            &client,
            std::slice::from_ref(&url),
            &target,
            &cancel,
            &mut received,
            body.len() as u64,
            body.len() as u64,
            1,
            &mut progress,
        )
        .await;
        let error = outcome.expect_err("exhausted attempts fail");
        assert!(error.contains("已尝试 1 次"), "unexpected message: {error}");
        assert_eq!(
            std::fs::metadata(&target).expect("partial file").len(),
            1_024,
            "partial data is kept for the next manual retry"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
