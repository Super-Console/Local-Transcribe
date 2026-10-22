use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

/// Read-only metadata for a file selected by the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputFile {
    pub path: PathBuf,
    pub file_name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputFileError {
    NotRegularFile,
    CannotRead,
}

impl fmt::Display for InputFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRegularFile => {
                formatter.write_str("The selected input is not a regular file.")
            }
            Self::CannotRead => formatter.write_str("The selected input cannot be read."),
        }
    }
}

impl std::error::Error for InputFileError {}

impl InputFile {
    /// Inspect a user-selected file without opening it for writing or modifying it.
    pub fn inspect(path: impl AsRef<Path>) -> Result<Self, InputFileError> {
        let path = path.as_ref();
        let metadata = fs::metadata(path).map_err(|_| InputFileError::CannotRead)?;
        if !metadata.is_file() {
            return Err(InputFileError::NotRegularFile);
        }

        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unnamed file".to_owned());

        Ok(Self {
            path: path.to_path_buf(),
            file_name,
            size_bytes: metadata.len(),
        })
    }
}
