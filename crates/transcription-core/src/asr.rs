use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sherpa_onnx::{
    OfflineDolphinModelConfig, OfflineFireRedAsrCtcModelConfig, OfflineFireRedAsrModelConfig,
    OfflineFunASRNanoModelConfig, OfflineParaformerModelConfig, OfflineQwen3ASRModelConfig,
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
    OfflineTransducerModelConfig, OfflineWhisperModelConfig, OfflineZipformerCtcModelConfig,
};

use crate::{decode_audio_file, punctuation, InputFile, Transcript, TranscriptSegment};

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
    directory: std::path::PathBuf,
    recognizer: OfflineRecognizer,
    last_used: std::time::Instant,
}

static RECOGNIZER_CACHE: std::sync::Mutex<Option<CachedRecognizer>> = std::sync::Mutex::new(None);
static IDLE_UNLOADER: std::sync::Once = std::sync::Once::new();

const IDLE_UNLOAD: std::time::Duration = std::time::Duration::from_secs(120);

fn spawn_idle_unloader() {
    IDLE_UNLOADER.call_once(|| {
        std::thread::Builder::new()
            .name("model-idle-unload".to_owned())
            .spawn(|| loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                if let Ok(mut cache) = RECOGNIZER_CACHE.lock() {
                    let expired = cache
                        .as_ref()
                        .is_some_and(|entry| entry.last_used.elapsed() >= IDLE_UNLOAD);
                    if expired {
                        *cache = None;
                    }
                }
            })
            .ok();
    });
}

/// 当前确实驻留在内存中的模型 id（顶部绿点只认这个值）。
pub fn loaded_model_id() -> Option<String> {
    RECOGNIZER_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.as_ref().map(|entry| entry.model_id.clone()))
}

pub fn idle_unload_seconds() -> u64 {
    IDLE_UNLOAD.as_secs()
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

    let audio = decode_audio_file(&input.path, SAMPLE_RATE)?;
    if is_cancelled() {
        return Err("Transcription was cancelled.".to_owned());
    }
    let punctuator = match options.punctuation_model.as_deref() {
        Some(path) => Some(punctuation::Punctuator::create(path)?),
        None => None,
    };
    let chunk_samples = SAMPLE_RATE as usize * chunk_seconds();
    let chunks: Vec<&[f32]> = audio.samples.chunks(chunk_samples).collect();
    if chunks.is_empty() {
        return Err("The selected media contains no audio to transcribe.".to_owned());
    }
    let total_duration_ms = (audio.samples.len() * 1_000 / SAMPLE_RATE as usize) as u64;

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
        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            "sherpa-onnx could not load this model. Verify that its files are a compatible export."
                .to_owned()
        })?;
        *cache = Some(CachedRecognizer {
            model_id: model.id.clone(),
            directory: model.directory.clone(),
            recognizer,
            last_used: std::time::Instant::now(),
        });
    }
    let recognizer = &cache
        .as_ref()
        .ok_or_else(|| "The model cache is unavailable.".to_owned())?
        .recognizer;
    if is_cancelled() {
        return Err("Transcription was cancelled.".to_owned());
    }

    let mut segments = Vec::with_capacity(chunks.len());
    for (index, samples) in chunks.iter().enumerate() {
        if is_cancelled() {
            return Err("Transcription was cancelled.".to_owned());
        }
        let stream = recognizer.create_stream();
        stream.accept_waveform(SAMPLE_RATE as i32, samples);
        recognizer.decode(&stream);
        let result = stream
            .get_result()
            .ok_or_else(|| "The speech engine returned no result for an audio chunk.".to_owned())?;
        let text = collapse_degenerate_repetitions(result.text.trim());
        if !text.is_empty() {
            let text = match &punctuator {
                Some(punctuator) => punctuator.punctuate(&text),
                None => text,
            };
            let start_ms = (index * chunk_samples * 1_000 / SAMPLE_RATE as usize) as u64;
            let end_sample = ((index + 1) * chunk_samples).min(audio.samples.len());
            let end_ms = (end_sample * 1_000 / SAMPLE_RATE as usize) as u64;
            let segment = TranscriptSegment {
                start_ms,
                end_ms,
                text,
            };
            on_segment(TranscriptionSegmentUpdate {
                index: segments.len(),
                total_chunks: chunks.len(),
                segment: segment.clone(),
            });
            segments.push(segment);
        }
        on_progress(TranscriptionProgress {
            completed_chunks: index + 1,
            total_chunks: chunks.len(),
            total_duration_ms,
        });
        if is_cancelled() {
            return Err("Transcription was cancelled.".to_owned());
        }
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
    if let Some(entry) = cache.as_mut() {
        entry.last_used = std::time::Instant::now();
    }
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
        duration_ms: Some(audio.duration_ms),
        text,
        segments: Some(segments),
    })
}

#[cfg(test)]
mod tests {
    use super::{collapse_degenerate_repetitions, inspect_model_directory, ModelFamily};
    use std::{fs, path::PathBuf};

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
