use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sherpa_onnx::{
    OfflineDolphinModelConfig, OfflineFireRedAsrCtcModelConfig, OfflineFireRedAsrModelConfig,
    OfflineFunASRNanoModelConfig, OfflineParaformerModelConfig, OfflineQwen3ASRModelConfig,
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
    OfflineTransducerModelConfig, OfflineWhisperModelConfig, OfflineZipformerCtcModelConfig,
};

use crate::{
    decode_audio_file_streaming, probe_audio_duration_ms, punctuation, InputFile, Transcript,
    TranscriptSegment,
};

const SAMPLE_RATE: u32 = 16_000;
const CHUNK_SECONDS: usize = 30;
const SHERPA_VERSION: &str = "1.13.8";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelFamily {
    Paraformer,
    SenseVoice,
    Whisper,
    Qwen3Asr,
    FireRedAsr,
    /// FireRedASR2 CTC branch (single graph, no attention decoder).
    FireRedCtc,
    /// Offline Zipformer CTC (single graph).
    ZipformerCtc,
    /// Dolphin multilingual CTC (single graph).
    Dolphin,
    /// TeleSpeech Chinese CTC (single graph).
    TeleSpeech,
    /// FunASR Nano: embedding + encoder adaptor + Qwen3-based LLM decoder.
    FunAsrNano,
    /// Offline transducer (encoder + decoder + joiner).
    Transducer,
}

/// The exact on-disk layout detected inside a model directory. Every family
/// declares its own required files; there is no "any downloaded model" path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ModelFiles {
    /// Paraformer / SenseVoice: one graph plus a flat vocabulary.
    SingleFile { model: PathBuf, tokens: PathBuf },
    /// Whisper ONNX export: encoder, decoder and a Whisper vocabulary.
    Whisper {
        encoder: PathBuf,
        decoder: PathBuf,
        tokens: PathBuf,
    },
    /// Qwen3-ASR ONNX export: frontend, encoder, KV-cache decoder, tokenizer dir.
    Qwen3Asr {
        conv_frontend: PathBuf,
        encoder: PathBuf,
        decoder: PathBuf,
        tokenizer_dir: PathBuf,
    },
    /// FireRedASR (v1 AED / v2 AED) ONNX export: encoder, decoder, flat vocabulary.
    FireRedAsr {
        encoder: PathBuf,
        decoder: PathBuf,
        tokens: PathBuf,
    },
    /// FunASR Nano export: embedding, encoder adaptor, LLM decoder, Qwen tokenizer.
    FunAsrNano {
        embedding: PathBuf,
        encoder_adaptor: PathBuf,
        llm: PathBuf,
        tokenizer_dir: PathBuf,
    },
    /// Transducer export (e.g. X-ASR): encoder, decoder, joiner, flat vocabulary.
    Transducer {
        encoder: PathBuf,
        decoder: PathBuf,
        joiner: PathBuf,
        tokens: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub id: String,
    pub display_name: String,
    pub family: ModelFamily,
    pub directory: PathBuf,
    pub files: ModelFiles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionProgress {
    pub completed_chunks: usize,
    pub total_chunks: usize,
    /// 音频总时长（毫秒）。界面据此计算实时倍速与剩余时间。
    pub total_duration_ms: u64,
}

/// One stable segment, reported as soon as the engine has decoded it.
///
/// `index` is the 0-based position in the final segment list and never
/// changes afterwards; the UI can append without re-reading the transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionSegmentUpdate {
    pub index: usize,
    pub total_chunks: usize,
    pub segment: TranscriptSegment,
}

/// Supported layouts, named in every rejection message so users know what import can accept.
/// 常驻识别器缓存：同一模型连续任务复用，切换模型在任务边界卸载，
/// 整个进程空闲 120 秒后释放（见任务书：模型空闲卸载默认两分钟）。
struct CachedRecognizer {
    model_id: String,
    directory: PathBuf,
    recognizer: OfflineRecognizer,
    _compute_lock: crate::ComputeLockGuard,
    /// 最后一次使用结束的时间（进程启动以来的毫秒）。任务结束、成功、失败、
    /// 取消都会刷新；空闲卸载只认这个值。
    last_used_ms: Arc<AtomicU64>,
}

static RECOGNIZER_CACHE: Mutex<Option<CachedRecognizer>> = Mutex::new(None);
/// 驻留模型 id，供界面查询。独立于识别器缓存，因此转写期间查询不会阻塞。
static CURRENT_MODEL: Mutex<Option<String>> = Mutex::new(None);
static IDLE_UNLOADER: std::sync::Once = std::sync::Once::new();
static CLOCK: OnceLock<Instant> = OnceLock::new();

/// 五个闭合档位（秒）。永不提供「不卸载」。
pub const IDLE_UNLOAD_CHOICES: [u64; 5] = [20, 120, 600, 1800, 7200];
pub const DEFAULT_IDLE_UNLOAD_SECONDS: u64 = 120;

static IDLE_UNLOAD: OnceLock<Duration> = OnceLock::new();

/// 进程内一次性设定空闲卸载时长；必须在任何推理开始前调用。
pub fn set_idle_unload(seconds: u64) -> Result<(), String> {
    if !IDLE_UNLOAD_CHOICES.contains(&seconds) {
        return Err(format!(
            "idle unload must be one of {IDLE_UNLOAD_CHOICES:?} seconds; 'never' is not supported"
        ));
    }
    IDLE_UNLOAD
        .set(Duration::from_secs(seconds))
        .map_err(|_| "idle unload was already set for this process".to_owned())
}

fn idle_unload() -> Duration {
    *IDLE_UNLOAD.get_or_init(|| Duration::from_secs(DEFAULT_IDLE_UNLOAD_SECONDS))
}

fn now_ms() -> u64 {
    CLOCK
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

/// 持有到一次转写结束；无论成功、失败还是取消，Drop 都会刷新使用时间。
struct UseGuard(Arc<AtomicU64>);

impl Drop for UseGuard {
    fn drop(&mut self) {
        self.0.store(now_ms(), Ordering::Relaxed);
    }
}

fn expired(last_used_ms: u64, now_ms: u64, idle: Duration) -> bool {
    now_ms.saturating_sub(last_used_ms) >= idle.as_millis() as u64
}

fn spawn_idle_unloader() {
    IDLE_UNLOADER.call_once(|| {
        std::thread::Builder::new()
            .name("model-idle-unload".to_owned())
            .spawn(|| loop {
                std::thread::sleep(Duration::from_secs(1));
                let idle = idle_unload();
                if let Ok(mut cache) = RECOGNIZER_CACHE.lock() {
                    let stale = cache.as_ref().is_some_and(|entry| {
                        expired(entry.last_used_ms.load(Ordering::Relaxed), now_ms(), idle)
                    });
                    if stale {
                        *cache = None;
                        if let Ok(mut current) = CURRENT_MODEL.lock() {
                            *current = None;
                        }
                        eprintln!("[model] idle unload after {}s", idle_unload().as_secs());
                    }
                }
            })
            .ok();
    });
}

/// 当前确实驻留在内存中的模型 id（顶部绿点只认这个值）。
pub fn loaded_model_id() -> Option<String> {
    CURRENT_MODEL
        .lock()
        .ok()
        .and_then(|current| current.clone())
}

/// Explicitly release the in-process ASR cache so the LLM can occupy the
/// single large-model memory slot. If transcription is active, this waits on
/// the same cache lock until that operation reaches its task boundary.
pub fn unload_cached_model() -> Result<bool, String> {
    let mut cache = RECOGNIZER_CACHE
        .lock()
        .map_err(|_| "The transcription model cache is unavailable.".to_owned())?;
    let was_loaded = cache.is_some();
    *cache = None;
    let mut current = CURRENT_MODEL
        .lock()
        .map_err(|_| "The transcription model status is unavailable.".to_owned())?;
    *current = None;
    Ok(was_loaded)
}

pub fn idle_unload_seconds() -> u64 {
    idle_unload().as_secs()
}

const SUPPORTED_LAYOUTS: &str = "Supported layouts: sherpa-onnx Whisper ONNX exports (encoder + decoder + tokens.txt), sherpa-onnx Qwen3-ASR exports (conv_frontend.onnx + encoder/decoder + tokenizer/), sherpa-onnx FunASR Nano exports (embedding + encoder_adaptor + llm + tokenizer/), sherpa-onnx FireRedASR exports (encoder + decoder + tokens.txt), offline transducers (encoder + decoder + joiner + tokens.txt), and single-graph CTC exports: Paraformer, SenseVoice, FireRedASR2 CTC, Zipformer CTC, Dolphin or TeleSpeech (model.onnx + tokens.txt, keep the original folder name).";

fn non_empty_file(path: &Path) -> bool {
    path.is_file() && path.metadata().is_ok_and(|meta| meta.len() > 0)
}

/// Find a non-empty file in `directory` whose file name ends with one of `suffixes`.
fn find_by_suffix(directory: &Path, suffixes: &[&str]) -> Option<PathBuf> {
    let mut matches = fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            non_empty_file(path)
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| suffixes.iter().any(|suffix| name.ends_with(suffix)))
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches.into_iter().next()
}

fn find_exact(directory: &Path, name: &str) -> Option<PathBuf> {
    let candidate = directory.join(name);
    non_empty_file(&candidate).then_some(candidate)
}

/// Find a non-empty file whose name contains every needle (for exports whose
/// file names carry a training suffix such as `encoder-epoch-99-avg-1.onnx`).
fn find_containing(directory: &Path, needles: &[&str]) -> Option<PathBuf> {
    let mut matches = fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            non_empty_file(path)
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| needles.iter().all(|needle| name.contains(needle)))
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches.into_iter().next()
}

/// Locate a sub-directory that holds a complete Qwen-style tokenizer.
fn find_tokenizer_dir(directory: &Path) -> Option<PathBuf> {
    let mut candidates = fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().find(|path| {
        ["vocab.json", "merges.txt", "tokenizer.json"]
            .iter()
            .all(|file| non_empty_file(&path.join(file)))
    })
}

/// Whisper vocabularies published for sherpa-onnx store one base64-encoded
/// token per line (`IQ== 0`) and may omit the `<|startoftranscript|>` entries,
/// so the entry count is the reliable signal besides the marker itself.
struct VocabularyShape {
    entries: usize,
    has_whisper_marker: bool,
}

impl VocabularyShape {
    fn looks_like_whisper(&self) -> bool {
        self.has_whisper_marker || self.entries >= 40_000
    }
}

/// base64("|startoftranscript|") and base64("<|startoftranscript|>").
const WHISPER_MARKERS: [&str; 3] = [
    "|startoftranscript|",
    "fHN0YXJ0b2Z0cmFuc2NyaXB0fA==",
    "PHxzdGFydG9mdHJhbnNjcmlwdHw+",
];

fn inspect_vocabulary(path: &Path) -> Result<VocabularyShape, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read the model vocabulary: {error}"))?;
    let entries = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let has_whisper_marker = WHISPER_MARKERS
        .iter()
        .any(|marker| content.contains(marker));
    Ok(VocabularyShape {
        entries,
        has_whisper_marker,
    })
}

/// Detect the model layout strictly from the files that are actually present.
fn detect_layout(directory: &Path) -> Result<(ModelFamily, ModelFiles), String> {
    let directory_name = directory
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    // FunASR Nano: embedding.int8.onnx is unique to this family.
    if let Some(embedding) = find_exact(directory, "embedding.int8.onnx")
        .or_else(|| find_exact(directory, "embedding.onnx"))
    {
        let encoder_adaptor = find_exact(directory, "encoder_adaptor.int8.onnx")
            .or_else(|| find_exact(directory, "encoder_adaptor.onnx"))
            .ok_or_else(|| {
                "This FunASR Nano folder is incomplete: encoder_adaptor.int8.onnx (or encoder_adaptor.onnx) is missing."
                    .to_owned()
            })?;
        let llm = find_exact(directory, "llm.int8.onnx")
            .or_else(|| find_exact(directory, "llm.onnx"))
            .ok_or_else(|| {
                "This FunASR Nano folder is incomplete: llm.int8.onnx (or llm.onnx) is missing."
                    .to_owned()
            })?;
        let tokenizer_dir = find_tokenizer_dir(directory).ok_or_else(|| {
            "This FunASR Nano folder is incomplete: the Qwen tokenizer folder (vocab.json, merges.txt, tokenizer.json) is missing."
                .to_owned()
        })?;
        return Ok((
            ModelFamily::FunAsrNano,
            ModelFiles::FunAsrNano {
                embedding,
                encoder_adaptor,
                llm,
                tokenizer_dir,
            },
        ));
    }

    // Offline transducer: a joiner graph is the distinguishing file.
    if find_containing(directory, &["joiner", ".onnx"]).is_some() {
        let encoder = find_containing(directory, &["encoder", ".onnx"])
            .filter(|path| !path.to_string_lossy().contains("encoder_adaptor"))
            .ok_or_else(|| {
                "This transducer folder is incomplete: the encoder .onnx file is missing."
                    .to_owned()
            })?;
        let decoder = find_containing(directory, &["decoder", ".onnx"]).ok_or_else(|| {
            "This transducer folder is incomplete: the decoder .onnx file is missing.".to_owned()
        })?;
        let joiner = find_containing(directory, &["joiner", ".onnx"]).ok_or_else(|| {
            "This transducer folder is incomplete: the joiner .onnx file is missing.".to_owned()
        })?;
        let tokens = find_by_suffix(directory, &["-tokens.txt", "tokens.txt"])
            .ok_or_else(|| "This transducer folder must contain tokens.txt.".to_owned())?;
        return Ok((
            ModelFamily::Transducer,
            ModelFiles::Transducer {
                encoder,
                decoder,
                joiner,
                tokens,
            },
        ));
    }

    // Qwen3-ASR: conv_frontend.onnx is unique to this family.
    if let Some(conv_frontend) = find_exact(directory, "conv_frontend.onnx") {
        let encoder = find_exact(directory, "encoder.int8.onnx")
            .or_else(|| find_exact(directory, "encoder.onnx"))
            .ok_or_else(|| {
                "This Qwen3-ASR folder is incomplete: encoder.int8.onnx (or encoder.onnx) is missing."
                    .to_owned()
            })?;
        let decoder = find_exact(directory, "decoder.int8.onnx")
            .or_else(|| find_exact(directory, "decoder.onnx"))
            .ok_or_else(|| {
                "This Qwen3-ASR folder is incomplete: decoder.int8.onnx (or decoder.onnx) is missing."
                    .to_owned()
            })?;
        let tokenizer_dir = directory.join("tokenizer");
        for required in ["vocab.json", "merges.txt", "tokenizer_config.json"] {
            if !non_empty_file(&tokenizer_dir.join(required)) {
                return Err(format!(
                    "This Qwen3-ASR folder is incomplete: tokenizer/{required} is missing or empty."
                ));
            }
        }
        return Ok((
            ModelFamily::Qwen3Asr,
            ModelFiles::Qwen3Asr {
                conv_frontend,
                encoder,
                decoder,
                tokenizer_dir,
            },
        ));
    }

    // Encoder + decoder pairs: transducer or Whisper.
    let encoder = find_by_suffix(
        directory,
        &[
            "-encoder.int8.onnx",
            "-encoder.onnx",
            "encoder.int8.onnx",
            "encoder.onnx",
        ],
    );
    let decoder = find_by_suffix(
        directory,
        &[
            "-decoder.int8.onnx",
            "-decoder.onnx",
            "decoder.int8.onnx",
            "decoder.onnx",
        ],
    );
    if encoder.is_some() || decoder.is_some() {
        let encoder = encoder.ok_or_else(|| {
            "This encoder/decoder folder is incomplete: the encoder .onnx file is missing."
                .to_owned()
        })?;
        let decoder = decoder.ok_or_else(|| {
            "This encoder/decoder folder is incomplete: the decoder .onnx file is missing."
                .to_owned()
        })?;
        let tokens = find_by_suffix(directory, &["-tokens.txt", "tokens.txt"]).ok_or_else(|| {
            "This encoder/decoder folder has no tokens.txt vocabulary, so it is not a Whisper export."
                .to_owned()
        })?;
        // FireRedASR uses a char-level vocabulary, so it must be identified by
        // the published folder name before the Whisper vocabulary check runs.
        if directory_name.contains("fire-red") || directory_name.contains("firered") {
            return Ok((
                ModelFamily::FireRedAsr,
                ModelFiles::FireRedAsr {
                    encoder,
                    decoder,
                    tokens,
                },
            ));
        }
        let vocabulary = inspect_vocabulary(&tokens)?;
        if !vocabulary.looks_like_whisper() {
            return Err(format!(
                "This encoder/decoder vocabulary does not look like a Whisper export ({} entries, no Whisper marker). {SUPPORTED_LAYOUTS}",
                vocabulary.entries
            ));
        }
        return Ok((
            ModelFamily::Whisper,
            ModelFiles::Whisper {
                encoder,
                decoder,
                tokens,
            },
        ));
    }

    // Single-file families: model.onnx / model.int8.onnx + tokens.txt.
    let model = find_exact(directory, "model.int8.onnx")
        .or_else(|| find_exact(directory, "model.onnx"))
        .ok_or_else(|| {
            format!("No recognizable model files were found in this folder. {SUPPORTED_LAYOUTS}")
        })?;
    let tokens = find_exact(directory, "tokens.txt")
        .ok_or_else(|| "The model folder must contain tokens.txt.".to_owned())?;
    let family = if directory_name.contains("sensevoice") || directory_name.contains("sense-voice")
    {
        ModelFamily::SenseVoice
    } else if directory_name.contains("fire-red") && directory_name.contains("ctc") {
        ModelFamily::FireRedCtc
    } else if directory_name.contains("zipformer") && directory_name.contains("ctc") {
        ModelFamily::ZipformerCtc
    } else if directory_name.contains("dolphin") {
        ModelFamily::Dolphin
    } else if directory_name.contains("telespeech") {
        ModelFamily::TeleSpeech
    } else if directory_name.contains("paraformer") {
        ModelFamily::Paraformer
    } else {
        return Err(format!(
            "A single-graph model was found, but its family is unknown from the folder name; keep the original Paraformer, SenseVoice, FireRedASR2 CTC, Zipformer CTC, Dolphin or TeleSpeech folder name. {SUPPORTED_LAYOUTS}"
        ));
    };
    Ok((family, ModelFiles::SingleFile { model, tokens }))
}

/// Validate a user-selected sherpa-onnx model directory; never downloads or copies model weights.
pub fn inspect_model_directory(path: impl AsRef<Path>) -> Result<LocalModel, String> {
    let directory = path.as_ref();
    let metadata = fs::metadata(directory)
        .map_err(|error| format!("Cannot read the selected model folder: {error}"))?;
    if !metadata.is_dir() {
        return Err(
            "Select the folder containing the model files, not an individual file.".to_owned(),
        );
    }

    let (family, files) = detect_layout(directory)?;
    let directory_name = directory
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let family_prefix = match family {
        ModelFamily::Paraformer => "paraformer",
        ModelFamily::SenseVoice => "sensevoice",
        ModelFamily::Whisper => "whisper",
        ModelFamily::Qwen3Asr => "qwen3-asr",
        ModelFamily::FireRedAsr => "fire-red-asr",
        ModelFamily::FireRedCtc => "fire-red-ctc",
        ModelFamily::ZipformerCtc => "zipformer-ctc",
        ModelFamily::Dolphin => "dolphin",
        ModelFamily::TeleSpeech => "telespeech",
        ModelFamily::FunAsrNano => "funasr-nano",
        ModelFamily::Transducer => "transducer",
    };
    Ok(LocalModel {
        id: format!("{family_prefix}-{directory_name}"),
        display_name: directory_name.to_owned(),
        family,
        directory: directory.to_path_buf(),
        files,
    })
}

/// 解码退化重复折叠。自回归模型偶尔陷入循环（实机样本：同一字连续 231 次），
/// 这里做保守兜底：连续同一字 ≥6 次保留 2 次；同一短语（1–6 字）连续 ≥4 次且
/// 总长度 ≥12 字保留 2 次。正常口语重复（「对对对」「好的好的」）不受影响。
/// 去掉模型输出的控制标记（`<sil>`、`<unk>`、`<|endoftext|>` 等），它们不是正文。
fn strip_special_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(['<', '[']) {
        let (head, tail) = rest.split_at(start);
        out.push_str(head);
        let open = tail.chars().next().unwrap_or('<');
        let close = if open == '<' { '>' } else { ']' };
        let inner_end = tail[1..].find(close).map(|offset| offset + 1);
        let candidate = inner_end.map(|end| (&tail[1..end], &tail[end + 1..]));
        let keep = match candidate {
            Some((inner, remaining))
                if !inner.trim().is_empty()
                    && inner.chars().count() <= 32
                    && inner.chars().all(|ch| {
                        ch.is_ascii_alphanumeric()
                            || matches!(ch, '_' | '|' | '-' | '/' | '.' | ':' | ' ')
                    }) =>
            {
                rest = remaining;
                false
            }
            _ => true,
        };
        if keep {
            out.push(open);
            rest = &tail[open.len_utf8()..];
        }
    }
    out.push_str(rest);
    out.trim().to_owned()
}

/// 模型的最小输入长度（1 秒）。短于它的分块（例如 120.065 秒素材切出的
/// 65 毫秒尾巴）会让 onnxruntime 的 Conv 节点抛出 `Invalid input shape` 的
/// C++ 异常；Rust 捕获不了外部异常，进程会直接 abort。补静音到 1 秒即可，
/// 时间轴仍按真实样本数计算。
const MIN_MODEL_SAMPLES: usize = SAMPLE_RATE as usize;

/// 0.5 秒以下的尾巴不送模型：说不了完整的词，模型只会输出噪声幻觉
/// （实测 Paraformer 对 65 毫秒尾巴会吐出"的的一的一的"）。
const MIN_TRANSCRIBE_SAMPLES: usize = SAMPLE_RATE as usize / 2;

fn is_too_short_to_transcribe(samples: &[f32]) -> bool {
    samples.len() < MIN_TRANSCRIBE_SAMPLES
}

fn samples_for_model(samples: &[f32]) -> std::borrow::Cow<'_, [f32]> {
    if samples.len() >= MIN_MODEL_SAMPLES {
        return std::borrow::Cow::Borrowed(samples);
    }
    let mut padded = samples.to_vec();
    padded.resize(MIN_MODEL_SAMPLES, 0.0);
    std::borrow::Cow::Owned(padded)
}

/// 判断一个分块是否基本没有声音：48 kHz/16 kHz 下 20 ms 一帧，
/// 高于 -55 dBFS 的帧不超过 2 帧（≤40 ms）时视为静音。
///
/// 静音输入会让 ASR 模型"幻觉"出不存在的句子（实测 Paraformer 会输出
/// "怎么了什么"），所以这类分块根本不送进模型。
fn is_silent_chunk(samples: &[f32]) -> bool {
    const THRESHOLD: f32 = 0.001_778; // -55 dBFS
    const MAX_ACTIVE_FRAMES: usize = 2;
    if samples.is_empty() {
        return true;
    }
    let frame_len = (SAMPLE_RATE as usize / 50).max(1);
    let mut active = 0usize;
    for frame in samples.chunks(frame_len) {
        let energy =
            frame.iter().map(|sample| sample * sample).sum::<f32>() / frame.len().max(1) as f32;
        if energy.sqrt() >= THRESHOLD {
            active += 1;
            if active > MAX_ACTIVE_FRAMES {
                return false;
            }
        }
    }
    true
}

fn collapse_degenerate_repetitions(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut index = 0;
    while index < chars.len() {
        let mut best: Option<(usize, usize)> = None;
        for unit_len in 1..=6usize {
            if index + unit_len * 4 > chars.len() {
                break;
            }
            let unit = &chars[index..index + unit_len];
            let mut repeats = 1;
            while index + (repeats + 1) * unit_len <= chars.len()
                && &chars[index + repeats * unit_len..index + (repeats + 1) * unit_len] == unit
            {
                repeats += 1;
            }
            let total = repeats * unit_len;
            let degenerate =
                (unit_len == 1 && repeats >= 6) || (unit_len > 1 && repeats >= 4 && total >= 12);
            if degenerate && total > best.map_or(0, |(_, total)| total) {
                best = Some((unit_len, total));
            }
        }
        match best {
            Some((unit_len, total)) => {
                out.extend_from_slice(&chars[index..index + 2 * unit_len]);
                index += total;
            }
            None => {
                out.push(chars[index]);
                index += 1;
            }
        }
    }
    out.into_iter().collect()
}

/// 转写后处理选项。默认为空，行为与旧版完全一致。
#[derive(Debug, Clone, Default)]
pub struct TranscriptionOptions {
    /// 可选中文标点模型（`model.onnx`）路径；为空时不做标点后处理。
    pub punctuation_model: Option<PathBuf>,
}

/// 固定切块长度（秒）。`LT_CHUNK_SECONDS` 为仅供 A/B 测量的实验开关。
fn chunk_seconds() -> usize {
    std::env::var("LT_CHUNK_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| (5..=120).contains(value))
        .unwrap_or(CHUNK_SECONDS)
}

/// Transcribe a local media file using an installed model. Progress is chunk-count based, not guessed.
pub fn transcribe_file(
    source_path: impl AsRef<Path>,
    model: &LocalModel,
    on_progress: impl FnMut(TranscriptionProgress),
) -> Result<Transcript, String> {
    transcribe_file_cancellable(source_path, model, || false, on_progress)
}

pub fn transcribe_file_cancellable(
    source_path: impl AsRef<Path>,
    model: &LocalModel,
    is_cancelled: impl FnMut() -> bool,
    on_progress: impl FnMut(TranscriptionProgress),
) -> Result<Transcript, String> {
    transcribe_file_streaming(source_path, model, is_cancelled, on_progress, |_| {})
}

/// Transcribe a local media file and report every stable segment while it is
/// produced, so the UI can append text instead of waiting for the whole file.
pub fn transcribe_file_streaming(
    source_path: impl AsRef<Path>,
    model: &LocalModel,
    is_cancelled: impl FnMut() -> bool,
    on_progress: impl FnMut(TranscriptionProgress),
    on_segment: impl FnMut(TranscriptionSegmentUpdate),
) -> Result<Transcript, String> {
    transcribe_file_streaming_with_options(
        source_path,
        model,
        &TranscriptionOptions::default(),
        is_cancelled,
        on_progress,
        on_segment,
    )
}

/// 与 [`transcribe_file_streaming`] 相同，但允许传入可选后处理（目前是标点模型）。
pub fn transcribe_file_streaming_with_options(
    source_path: impl AsRef<Path>,
    model: &LocalModel,
    options: &TranscriptionOptions,
    mut is_cancelled: impl FnMut() -> bool,
    mut on_progress: impl FnMut(TranscriptionProgress),
    mut on_segment: impl FnMut(TranscriptionSegmentUpdate),
) -> Result<Transcript, String> {
    if is_cancelled() {
        return Err("Transcription was cancelled.".to_owned());
    }
    let input = InputFile::inspect(source_path).map_err(|error| error.to_string())?;
    let verified_model = inspect_model_directory(&model.directory)?;
    if verified_model != *model {
        return Err(
            "The selected model files changed since the model was chosen. Select the model again."
                .to_owned(),
        );
    }

    if is_cancelled() {
        return Err("Transcription was cancelled.".to_owned());
    }
    let punctuator = match options.punctuation_model.as_deref() {
        Some(path) => Some(punctuation::Punctuator::create(path)?),
        None => None,
    };
    let chunk_samples = SAMPLE_RATE as usize * chunk_seconds();
    // 容器给出时长时先估算总段数，界面才有进度；拿不到就报 0，界面会自动隐藏进度条。
    let estimated_duration_ms = probe_audio_duration_ms(&input.path).ok().flatten();
    let estimated_total_chunks = estimated_duration_ms
        .map(|duration_ms| {
            let samples = (duration_ms as u128 * SAMPLE_RATE as u128 / 1_000) as usize;
            samples.div_ceil(chunk_samples).max(1)
        })
        .unwrap_or(0);

    let mut config = OfflineRecognizerConfig::default();
    config.model_config.num_threads = 2;
    config.model_config.provider = Some("cpu".to_owned());
    match (&model.family, &model.files) {
        (
            ModelFamily::Paraformer,
            ModelFiles::SingleFile {
                model: graph,
                tokens,
            },
        )
        | (
            ModelFamily::SenseVoice,
            ModelFiles::SingleFile {
                model: graph,
                tokens,
            },
        ) => {
            config.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
            if model.family == ModelFamily::Paraformer {
                config.model_config.paraformer = OfflineParaformerModelConfig {
                    model: Some(graph.to_string_lossy().into_owned()),
                };
            } else {
                config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                    model: Some(graph.to_string_lossy().into_owned()),
                    language: Some("auto".to_owned()),
                    use_itn: true,
                };
            }
        }
        (
            ModelFamily::Whisper,
            ModelFiles::Whisper {
                encoder,
                decoder,
                tokens,
            },
        ) => {
            config.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
            config.model_config.whisper = OfflineWhisperModelConfig {
                encoder: Some(encoder.to_string_lossy().into_owned()),
                decoder: Some(decoder.to_string_lossy().into_owned()),
                // No language => sherpa-onnx infers the language from the audio.
                language: None,
                task: None,
                // sherpa-onnx docs suggest 300 tail-padded frames for multilingual models.
                tail_paddings: 300,
                // The standard public int8 exports use the 3-output decoder,
                // which cannot supply cross-attention weights for token timestamps.
                enable_token_timestamps: false,
                enable_segment_timestamps: false,
            };
        }
        (
            ModelFamily::Qwen3Asr,
            ModelFiles::Qwen3Asr {
                conv_frontend,
                encoder,
                decoder,
                tokenizer_dir,
            },
        ) => {
            config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
                conv_frontend: Some(conv_frontend.to_string_lossy().into_owned()),
                encoder: Some(encoder.to_string_lossy().into_owned()),
                decoder: Some(decoder.to_string_lossy().into_owned()),
                tokenizer: Some(tokenizer_dir.to_string_lossy().into_owned()),
                ..OfflineQwen3ASRModelConfig::default()
            };
        }
        (
            ModelFamily::FireRedAsr,
            ModelFiles::FireRedAsr {
                encoder,
                decoder,
                tokens,
            },
        ) => {
            config.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
            config.model_config.fire_red_asr = OfflineFireRedAsrModelConfig {
                encoder: Some(encoder.to_string_lossy().into_owned()),
                decoder: Some(decoder.to_string_lossy().into_owned()),
            };
        }
        (
            ModelFamily::FireRedCtc,
            ModelFiles::SingleFile {
                model: graph,
                tokens,
            },
        )
        | (
            ModelFamily::ZipformerCtc,
            ModelFiles::SingleFile {
                model: graph,
                tokens,
            },
        )
        | (
            ModelFamily::Dolphin,
            ModelFiles::SingleFile {
                model: graph,
                tokens,
            },
        )
        | (
            ModelFamily::TeleSpeech,
            ModelFiles::SingleFile {
                model: graph,
                tokens,
            },
        ) => {
            config.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
            let graph = Some(graph.to_string_lossy().into_owned());
            match model.family {
                ModelFamily::FireRedCtc => {
                    config.model_config.fire_red_asr_ctc =
                        OfflineFireRedAsrCtcModelConfig { model: graph };
                }
                ModelFamily::ZipformerCtc => {
                    config.model_config.zipformer_ctc =
                        OfflineZipformerCtcModelConfig { model: graph };
                }
                ModelFamily::Dolphin => {
                    config.model_config.dolphin = OfflineDolphinModelConfig { model: graph };
                }
                _ => config.model_config.telespeech_ctc = graph,
            }
        }
        (
            ModelFamily::FunAsrNano,
            ModelFiles::FunAsrNano {
                embedding,
                encoder_adaptor,
                llm,
                tokenizer_dir,
            },
        ) => {
            config.model_config.funasr_nano = OfflineFunASRNanoModelConfig {
                embedding: Some(embedding.to_string_lossy().into_owned()),
                encoder_adaptor: Some(encoder_adaptor.to_string_lossy().into_owned()),
                llm: Some(llm.to_string_lossy().into_owned()),
                tokenizer: Some(tokenizer_dir.to_string_lossy().into_owned()),
                language: Some("auto".to_owned()),
                ..OfflineFunASRNanoModelConfig::default()
            };
        }
        (
            ModelFamily::Transducer,
            ModelFiles::Transducer {
                encoder,
                decoder,
                joiner,
                tokens,
            },
        ) => {
            config.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
            config.model_config.transducer = OfflineTransducerModelConfig {
                encoder: Some(encoder.to_string_lossy().into_owned()),
                decoder: Some(decoder.to_string_lossy().into_owned()),
                joiner: Some(joiner.to_string_lossy().into_owned()),
            };
        }
        _ => {
            return Err(
                "The selected model's family and file layout do not match. Select the model again."
                    .to_owned(),
            );
        }
    }
    spawn_idle_unloader();
    let mut cache = RECOGNIZER_CACHE
        .lock()
        .map_err(|_| "The model cache is unavailable.".to_owned())?;
    let reusable = cache
        .as_ref()
        .is_some_and(|entry| entry.model_id == model.id && entry.directory == model.directory);
    if !reusable {
        // Release this process's previous ASR model before trying to become the
        // one machine-wide resident model owner.
        *cache = None;
        let compute_lock = crate::acquire_compute_lock_for_model_directory(&model.directory)?;
        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            "sherpa-onnx could not load this model. Verify that its files are a compatible export."
                .to_owned()
        })?;
        *cache = Some(CachedRecognizer {
            model_id: model.id.clone(),
            directory: model.directory.clone(),
            recognizer,
            _compute_lock: compute_lock,
            last_used_ms: Arc::new(AtomicU64::new(now_ms())),
        });
        if let Ok(mut current) = CURRENT_MODEL.lock() {
            *current = Some(model.id.clone());
        }
    }
    let entry = cache
        .as_ref()
        .ok_or_else(|| "The model cache is unavailable.".to_owned())?;
    let recognizer = &entry.recognizer;
    let _use_guard = UseGuard(Arc::clone(&entry.last_used_ms));
    if is_cancelled() {
        return Err("Transcription was cancelled.".to_owned());
    }

    let mut segments: Vec<TranscriptSegment> = Vec::new();
    let mut completed_chunks: usize = 0;
    let mut buffer: Vec<f32> = Vec::with_capacity(chunk_samples);
    let mut total_duration_ms = estimated_duration_ms.unwrap_or(0);

    // 边解码边识别：内存里只保留一个分块，长视频不会再因为整段解码而耗尽内存。
    let meta = decode_audio_file_streaming(&input.path, SAMPLE_RATE, |samples| {
        for &sample in samples {
            buffer.push(sample);
            if buffer.len() < chunk_samples {
                continue;
            }
            if is_cancelled() {
                return Err("Transcription was cancelled.".to_owned());
            }
            completed_chunks += 1;
            transcribe_chunk(
                recognizer,
                punctuator.as_ref(),
                &buffer,
                completed_chunks,
                estimated_total_chunks,
                total_duration_ms,
                (completed_chunks as u64 - 1) * chunk_samples as u64,
                completed_chunks as u64 * chunk_samples as u64,
                &mut segments,
                &mut on_progress,
                &mut on_segment,
            )?;
            buffer.clear();
        }
        Ok(())
    })?;
    total_duration_ms = meta.duration_ms;

    if !buffer.is_empty() {
        if is_cancelled() {
            return Err("Transcription was cancelled.".to_owned());
        }
        completed_chunks += 1;
        transcribe_chunk(
            recognizer,
            punctuator.as_ref(),
            &buffer,
            completed_chunks,
            estimated_total_chunks,
            total_duration_ms,
            (completed_chunks as u64 - 1) * chunk_samples as u64,
            meta.sample_count,
            &mut segments,
            &mut on_progress,
            &mut on_segment,
        )?;
        buffer.clear();
    }
    if completed_chunks == 0 {
        return Err("The selected media contains no audio to transcribe.".to_owned());
    }
    // 估算可能和实际差一段，结束时把真实总数补一次，最终状态才准确。
    if estimated_total_chunks != completed_chunks {
        on_progress(TranscriptionProgress {
            completed_chunks,
            total_chunks: completed_chunks,
            total_duration_ms,
        });
    }

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let unix_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let text = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    drop(cache);

    Ok(Transcript {
        schema_version: "1".to_owned(),
        job_id: format!(
            "{unix_nanos:x}-{:x}",
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ),
        source_file_name: input.file_name,
        model_id: model.id.clone(),
        model_version: None,
        engine: format!("sherpa-onnx/{SHERPA_VERSION}"),
        language: match model.family {
            ModelFamily::Paraformer => Some("zh".to_owned()),
            // Auto-detected languages are reported as unknown until the
            // engine exposes a detected value; never invent one.
            ModelFamily::SenseVoice
            | ModelFamily::Whisper
            | ModelFamily::Qwen3Asr
            | ModelFamily::FireRedAsr
            | ModelFamily::FireRedCtc
            | ModelFamily::ZipformerCtc
            | ModelFamily::Dolphin
            | ModelFamily::TeleSpeech
            | ModelFamily::FunAsrNano
            | ModelFamily::Transducer => None,
        },
        duration_ms: Some(meta.duration_ms),
        text,
        segments: Some(segments),
    })
}

/// 识别一个分块并派发进度/片段事件。
#[allow(clippy::too_many_arguments)]
fn transcribe_chunk(
    recognizer: &OfflineRecognizer,
    punctuator: Option<&punctuation::Punctuator>,
    samples: &[f32],
    completed_chunks: usize,
    total_chunks: usize,
    total_duration_ms: u64,
    start_sample: u64,
    end_sample: u64,
    segments: &mut Vec<TranscriptSegment>,
    on_progress: &mut impl FnMut(TranscriptionProgress),
    on_segment: &mut impl FnMut(TranscriptionSegmentUpdate),
) -> Result<(), String> {
    // 过短的尾巴与静音分块都不送进模型：既省算力，也避免模型编造内容。
    if is_too_short_to_transcribe(samples) || is_silent_chunk(samples) {
        on_progress(TranscriptionProgress {
            completed_chunks,
            total_chunks,
            total_duration_ms,
        });
        return Ok(());
    }
    let model_input = samples_for_model(samples);
    let stream = recognizer.create_stream();
    stream.accept_waveform(SAMPLE_RATE as i32, model_input.as_ref());
    recognizer.decode(&stream);
    let result = stream
        .get_result()
        .ok_or_else(|| "The speech engine returned no result for an audio chunk.".to_owned())?;
    let text = collapse_degenerate_repetitions(&strip_special_tokens(result.text.trim()));
    if !text.is_empty() {
        let text = match punctuator {
            Some(punctuator) => punctuator.punctuate(&text),
            None => text,
        };
        let segment = TranscriptSegment {
            start_ms: start_sample * 1_000 / SAMPLE_RATE as u64,
            end_ms: end_sample * 1_000 / SAMPLE_RATE as u64,
            text,
        };
        on_segment(TranscriptionSegmentUpdate {
            index: segments.len(),
            total_chunks,
            segment: segment.clone(),
        });
        segments.push(segment);
    }
    on_progress(TranscriptionProgress {
        completed_chunks,
        total_chunks,
        total_duration_ms,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        collapse_degenerate_repetitions, expired, inspect_model_directory, is_silent_chunk,
        is_too_short_to_transcribe, samples_for_model, set_idle_unload, strip_special_tokens,
        unload_cached_model, ModelFamily, UseGuard, IDLE_UNLOAD_CHOICES, MIN_MODEL_SAMPLES,
    };
    use std::time::Duration;
    use std::{fs, path::PathBuf};

    #[test]
    fn short_tails_are_padded_before_the_model_sees_them() {
        let short = vec![0.2f32; 1_000];
        let padded = samples_for_model(&short);
        assert_eq!(padded.len(), MIN_MODEL_SAMPLES);
        assert_eq!(&padded[..1_000], &short[..]);
        assert!(padded[1_000..].iter().all(|sample| *sample == 0.0));

        let full = vec![0.2f32; 20_000];
        let unchanged = samples_for_model(&full);
        assert_eq!(unchanged.len(), 20_000);
        assert!(matches!(unchanged, std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn tails_shorter_than_half_a_second_are_not_transcribed() {
        assert!(is_too_short_to_transcribe(&vec![0.2f32; 1_000]));
        assert!(!is_too_short_to_transcribe(&vec![0.2f32; 8_000]));
        assert!(!is_too_short_to_transcribe(&vec![0.2f32; 20_000]));
    }

    #[test]
    fn special_tokens_never_reach_the_transcript() {
        assert_eq!(strip_special_tokens("<sil>"), "");
        assert_eq!(strip_special_tokens("<sil>你好<sil>"), "你好");
        assert_eq!(
            strip_special_tokens("<unk><s>开始</s><|endoftext|>"),
            "开始"
        );
        assert_eq!(strip_special_tokens("[sil] 你好"), "你好");
        assert_eq!(strip_special_tokens("正常的一句话。"), "正常的一句话。");
        // 不是控制标记的尖括号内容必须原样保留。
        assert_eq!(strip_special_tokens("a < b"), "a < b");
        assert_eq!(strip_special_tokens("价格<100元"), "价格<100元");
    }

    #[test]
    fn only_near_silent_chunks_are_treated_as_silence() {
        let quiet = vec![0.0f32; 16_000];
        assert!(is_silent_chunk(&quiet));
        assert!(is_silent_chunk(&vec![0.000_1f32; 16_000]));
        assert!(is_silent_chunk(&[]));

        // 一个 20 ms 的爆音不足以构成语音。
        let mut click = vec![0.0f32; 16_000];
        for sample in click.iter_mut().take(320) {
            *sample = 0.5;
        }
        assert!(is_silent_chunk(&click));

        // 0.1 秒的正常音量音频必须继续送去识别。
        let mut tone = vec![0.0f32; 16_000];
        for (index, sample) in tone.iter_mut().take(1_600).enumerate() {
            *sample = 0.2 * (index as f32 * 0.05).sin();
        }
        assert!(!is_silent_chunk(&tone));
        // 安静但持续的语音同样不能被误判为静音。
        assert!(!is_silent_chunk(&vec![0.01f32; 16_000]));
    }

    fn fixture(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("lt-layout-{}-{}", std::process::id(), uuid_like()));
        let directory = root.join(name);
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn uuid_like() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    fn touch(path: PathBuf) {
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn detects_funasr_nano_layout() {
        let directory = fixture("sherpa-onnx-funasr-nano-int8");
        touch(directory.join("embedding.int8.onnx"));
        touch(directory.join("encoder_adaptor.int8.onnx"));
        touch(directory.join("llm.int8.onnx"));
        let tokenizer = directory.join("Qwen3-0.6B");
        fs::create_dir_all(&tokenizer).unwrap();
        for file in ["vocab.json", "merges.txt", "tokenizer.json"] {
            touch(tokenizer.join(file));
        }
        let model = inspect_model_directory(&directory).unwrap();
        assert_eq!(model.family, ModelFamily::FunAsrNano);
    }

    #[test]
    fn detects_transducer_layout_with_epoch_suffixes() {
        let directory = fixture("sherpa-onnx-x-asr-zipformer-int8");
        touch(directory.join("encoder-epoch-99-avg-1.int8.onnx"));
        touch(directory.join("decoder-epoch-99-avg-1.onnx"));
        touch(directory.join("joiner-epoch-99-avg-1.int8.onnx"));
        touch(directory.join("tokens.txt"));
        let model = inspect_model_directory(&directory).unwrap();
        assert_eq!(model.family, ModelFamily::Transducer);
    }

    #[test]
    fn detects_single_graph_ctc_families_by_folder_name() {
        for (folder, expected) in [
            (
                "sherpa-onnx-fire-red-asr2-ctc-zh_en-int8",
                ModelFamily::FireRedCtc,
            ),
            (
                "sherpa-onnx-zipformer-ctc-small-zh-int8",
                ModelFamily::ZipformerCtc,
            ),
            (
                "sherpa-onnx-dolphin-base-ctc-multi-lang-int8",
                ModelFamily::Dolphin,
            ),
            (
                "sherpa-onnx-telespeech-ctc-int8-zh",
                ModelFamily::TeleSpeech,
            ),
        ] {
            let directory = fixture(folder);
            touch(directory.join("model.int8.onnx"));
            touch(directory.join("tokens.txt"));
            let model = inspect_model_directory(&directory).unwrap();
            assert_eq!(model.family, expected, "{folder}");
        }
    }

    #[test]
    fn rejects_unknown_single_graph_folders() {
        let directory = fixture("some-random-model");
        touch(directory.join("model.int8.onnx"));
        touch(directory.join("tokens.txt"));
        assert!(inspect_model_directory(&directory).is_err());
    }

    #[test]
    fn idle_unload_accepts_only_the_five_closed_values() {
        for seconds in IDLE_UNLOAD_CHOICES {
            assert!(seconds > 0);
        }
        // A process may set it only once, so the rejection path is asserted
        // before any successful set in this test binary.
        assert!(set_idle_unload(0).is_err());
        assert!(set_idle_unload(999_999).is_err());
        assert!(set_idle_unload(u64::MAX).is_err());
    }

    #[test]
    fn explicit_unload_reports_an_empty_cache_without_error() {
        assert!(!unload_cached_model().expect("empty ASR cache can be unloaded"));
        assert_eq!(super::loaded_model_id(), None);
    }

    #[test]
    fn dropping_the_use_guard_refreshes_last_used() {
        let last_used = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let _ = super::now_ms();
        {
            let _guard = UseGuard(std::sync::Arc::clone(&last_used));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(last_used.load(std::sync::atomic::Ordering::Relaxed) > 0);
    }

    #[test]
    fn idle_expiry_uses_the_last_use_not_the_load_time() {
        let idle = Duration::from_millis(200);
        // Last use 150 ms ago: still resident.
        assert!(!expired(1_000, 1_150, idle));
        // Last use 250 ms ago: expires.
        assert!(expired(1_000, 1_250, idle));
        // Refreshing the timestamp resets the window.
        assert!(!expired(1_250, 1_300, idle));
    }

    #[test]
    fn collapse_reduces_a_long_run_of_one_character() {
        let degraded = "有".repeat(231) + "。";
        let fixed = collapse_degenerate_repetitions(&degraded);
        assert_eq!(fixed, "有有。");
    }

    #[test]
    fn collapse_keeps_normal_spoken_repetition() {
        for text in ["对对对，就这样", "好的好的", "不不不，不用了"] {
            assert_eq!(collapse_degenerate_repetitions(text), text);
        }
    }

    #[test]
    fn collapse_reduces_a_long_phrase_loop() {
        let degraded = "我们今天".to_owned() + &"讨论".repeat(8) + "这个问题";
        let fixed = collapse_degenerate_repetitions(&degraded);
        assert_eq!(fixed, "我们今天讨论讨论这个问题");
    }

    #[test]
    fn collapse_leaves_long_sentences_untouched() {
        let text = "这次会议主要讨论四件事情，第一是关于新的录音软件，第二是关于课程安排。";
        assert_eq!(collapse_degenerate_repetitions(text), text);
    }
}
