mod asr;
mod compute_lock;
mod input_file;
mod jobs;
mod library;
mod media;
mod punctuation;

pub use asr::{
    idle_unload_seconds, inspect_model_directory, loaded_model_id, set_idle_unload,
    transcribe_file, transcribe_file_cancellable, transcribe_file_streaming,
    transcribe_file_streaming_with_options, unload_cached_model, LocalModel, ModelFamily,
    ModelFiles, TranscriptionOptions, TranscriptionProgress, TranscriptionSegmentUpdate,
    DEFAULT_IDLE_UNLOAD_SECONDS, IDLE_UNLOAD_CHOICES,
};
pub use compute_lock::{
    acquire_compute_lock_for_model_directory, acquire_compute_lock_for_models_directory,
    set_compute_lock_path, ComputeLockGuard,
};
pub use input_file::{InputFile, InputFileError};
pub use jobs::{JobLease, JobState, JobStatus, JobStore};
pub use library::{default_trash_dir, LibraryModel, ModelLibrary};
pub use media::{decode_audio_file, DecodedAudio};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub schema_version: String,
    pub job_id: String,
    pub source_file_name: String,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    pub engine: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<TranscriptSegment>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

impl Transcript {
    /// Serialize this transcript as stable, human-readable JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Return only the recognized text, without metadata or formatting changes.
    pub fn to_txt(&self) -> String {
        self.text.clone()
    }
}
