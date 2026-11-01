mod asr;
mod compute_lock;
mod input_file;
mod jobs;
mod library;
mod media;
mod punctuation;
mod trash;

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
pub use media::{
    decode_audio_file, decode_audio_file_streaming, probe_audio_duration_ms, DecodedAudio,
    DecodedAudioMeta,
};
pub use trash::move_to_trash;

use serde::{Deserialize, Serialize};

/// Canonical desktop and local data-directory identifier.
pub const APP_IDENTIFIER: &str = "net.timhuang.localtranscribe";

/// Resolve the platform-local data directory shared by the service and LAN node.
/// The desktop GUI uses Tauri's `app_local_data_dir`, configured with the same ID.
pub fn default_app_data_dir() -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .map(|home| {
                home.join("Library/Application Support")
                    .join(APP_IDENTIFIER)
            })
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .map(|local| local.join(APP_IDENTIFIER))
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .map(|path| path.join(APP_IDENTIFIER))
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .map(|home| home.join(".local/share").join(APP_IDENTIFIER))
            })
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
}

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

#[cfg(test)]
mod app_data_dir_tests {
    use super::{default_app_data_dir, APP_IDENTIFIER};

    #[test]
    fn default_app_data_dir_uses_the_canonical_identifier() {
        let directory = default_app_data_dir().expect("platform data directory should resolve");
        assert_eq!(
            directory.file_name().and_then(|name| name.to_str()),
            Some(APP_IDENTIFIER)
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            directory,
            std::path::PathBuf::from(std::env::var_os("HOME").expect("HOME is set"))
                .join("Library/Application Support")
                .join(APP_IDENTIFIER)
        );
    }
}
