use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use fs2::FileExt;

static COMPUTE_LOCK_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Configure one lock shared by the desktop, service and supervised LAN node.
/// Repeating the same path is harmless; changing it after first use is rejected.
pub fn set_compute_lock_path(path: PathBuf) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot prepare the compute lock folder: {error}"))?;
    }
    match COMPUTE_LOCK_PATH.set(path.clone()) {
        Ok(()) => Ok(()),
        Err(_) if COMPUTE_LOCK_PATH.get() == Some(&path) => Ok(()),
        Err(_) => {
            Err("The shared compute lock path was already configured differently.".to_owned())
        }
    }
}

/// The guard remains alive for as long as the ASR or LLM model is resident.
/// It uses an OS advisory lock, so the kernel releases it if a process exits.
pub struct ComputeLockGuard {
    _file: File,
}

pub fn acquire_compute_lock_for_model_directory(
    model_directory: &Path,
) -> Result<ComputeLockGuard, String> {
    let path = COMPUTE_LOCK_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| inferred_lock_path(model_directory));
    acquire_at(&path)
}

pub fn acquire_compute_lock_for_models_directory(
    models_directory: &Path,
) -> Result<ComputeLockGuard, String> {
    let path = COMPUTE_LOCK_PATH.get().cloned().unwrap_or_else(|| {
        models_directory
            .parent()
            .unwrap_or(models_directory)
            .join(".compute.lock")
    });
    acquire_at(&path)
}

fn inferred_lock_path(model_directory: &Path) -> PathBuf {
    // The built-in ASR models live at <app-data>/models/<model-id>; LLM files
    // live at <app-data>/llm. Production entry points configure the exact path,
    // while this keeps standalone core callers coordinated for the default layout.
    let root = model_directory
        .parent()
        .and_then(Path::parent)
        .unwrap_or(model_directory);
    root.join(".compute.lock")
}

fn acquire_at(path: &Path) -> Result<ComputeLockGuard, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot prepare the compute lock folder: {error}"))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("Cannot open the shared compute lock: {error}"))?;
    FileExt::try_lock_exclusive(&file).map_err(|_| {
        "Another Local Transcribe process is busy with a large model. Wait for its idle-unload timeout, then retry.".to_owned()
    })?;
    Ok(ComputeLockGuard { _file: file })
}

#[cfg(test)]
mod tests {
    use super::acquire_at;

    #[test]
    fn a_second_runtime_cannot_hold_the_shared_compute_lock_until_release() {
        let path = std::env::temp_dir().join(format!(
            "voice-to-text-compute-lock-{}-{}.lock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = acquire_at(&path).expect("first runtime owns the lock");
        assert!(
            acquire_at(&path).is_err(),
            "second runtime must be excluded"
        );
        drop(first);
        assert!(acquire_at(&path).is_ok(), "lock is available after unload");
    }
}
