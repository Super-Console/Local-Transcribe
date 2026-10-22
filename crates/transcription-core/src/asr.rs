use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sherpa_onnx::{
    OfflineParaformerModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineSenseVoiceModelConfig,
};

use crate::{decode_audio_file, InputFile, Transcript, TranscriptSegment};

const SAMPLE_RATE: u32 = 16_000;
const CHUNK_SECONDS: usize = 30;
const SHERPA_VERSION: &str = "1.13.8";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelFamily {
    Paraformer,
    SenseVoice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub id: String,
    pub display_name: String,
    pub family: ModelFamily,
    pub directory: PathBuf,
    pub model_path: PathBuf,
    pub tokens_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionProgress {
    pub completed_chunks: usize,
    pub total_chunks: usize,
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

    let directory_name = directory
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let lower_name = directory_name.to_ascii_lowercase();
    let family = if lower_name.contains("paraformer") {
        ModelFamily::Paraformer
    } else if lower_name.contains("sensevoice") || lower_name.contains("sense-voice") {
        ModelFamily::SenseVoice
    } else {
        return Err(
            "This folder is not a recognized Paraformer or SenseVoice model directory.".to_owned(),
        );
    };

    let model_path = ["model.int8.onnx", "model.onnx"]
        .iter()
        .map(|filename| directory.join(filename))
        .find(|candidate| {
            candidate.is_file() && candidate.metadata().is_ok_and(|meta| meta.len() > 0)
        })
        .ok_or_else(|| {
            "The model folder must contain a non-empty model.int8.onnx or model.onnx file."
                .to_owned()
        })?;
    let tokens_path = directory.join("tokens.txt");
    let tokens = fs::metadata(&tokens_path)
        .map_err(|_| "The model folder must contain tokens.txt.".to_owned())?;
    if !tokens.is_file() || tokens.len() == 0 {
        return Err("The model folder must contain a non-empty tokens.txt file.".to_owned());
    }

    let model_id = format!(
        "{}-{directory_name}",
        match family {
            ModelFamily::Paraformer => "paraformer",
            ModelFamily::SenseVoice => "sensevoice",
        }
    );
    Ok(LocalModel {
        id: model_id,
        display_name: directory_name.to_owned(),
        family,
        directory: directory.to_path_buf(),
        model_path,
        tokens_path,
    })
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
    mut is_cancelled: impl FnMut() -> bool,
    mut on_progress: impl FnMut(TranscriptionProgress),
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
    let chunk_samples = SAMPLE_RATE as usize * CHUNK_SECONDS;
    let chunks: Vec<&[f32]> = audio.samples.chunks(chunk_samples).collect();
    if chunks.is_empty() {
        return Err("The selected media contains no audio to transcribe.".to_owned());
    }

    let mut config = OfflineRecognizerConfig::default();
    config.model_config.tokens = Some(model.tokens_path.to_string_lossy().into_owned());
    config.model_config.num_threads = 2;
    config.model_config.provider = Some("cpu".to_owned());
    match model.family {
        ModelFamily::Paraformer => {
            config.model_config.paraformer = OfflineParaformerModelConfig {
                model: Some(model.model_path.to_string_lossy().into_owned()),
            };
        }
        ModelFamily::SenseVoice => {
            config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                model: Some(model.model_path.to_string_lossy().into_owned()),
                language: Some("auto".to_owned()),
                use_itn: true,
            };
        }
    }
    let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
        "sherpa-onnx could not load this model. Verify that its files are a compatible export."
            .to_owned()
    })?;
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
        let text = result.text.trim().to_owned();
        if !text.is_empty() {
            let start_ms = (index * chunk_samples * 1_000 / SAMPLE_RATE as usize) as u64;
            let end_sample = ((index + 1) * chunk_samples).min(audio.samples.len());
            let end_ms = (end_sample * 1_000 / SAMPLE_RATE as usize) as u64;
            segments.push(TranscriptSegment {
                start_ms,
                end_ms,
                text,
            });
        }
        on_progress(TranscriptionProgress {
            completed_chunks: index + 1,
            total_chunks: chunks.len(),
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
            ModelFamily::SenseVoice => None,
        },
        duration_ms: Some(audio.duration_ms),
        text,
        segments: Some(segments),
    })
}
