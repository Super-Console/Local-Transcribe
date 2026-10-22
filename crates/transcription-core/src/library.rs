//! Application-managed model library.
//!
//! Importing copies a validated model folder into the app's own models
//! directory, keeps the user's download untouched, and only exposes the copy
//! after it has been re-validated and atomically renamed into place.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt;

use crate::asr::{inspect_model_directory, LocalModel};

const LIBRARY_LOCK: &str = ".library.lock";
const TEMP_PREFIX: &str = ".import-";
const RESERVED_MARGIN_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryModel {
    pub model: LocalModel,
    pub bytes: u64,
}

pub struct ModelLibrary {
    directory: PathBuf,
}

impl ModelLibrary {
    pub fn new(app_data_dir: impl AsRef<Path>) -> Self {
        ModelLibrary {
            directory: app_data_dir.as_ref().join("models"),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Every valid model folder in the library; invalid leftovers are skipped
    /// rather than presented as usable.
    pub fn list(&self) -> Result<Vec<LibraryModel>, String> {
        if !self.directory.is_dir() {
            return Ok(Vec::new());
        }
        let mut models = Vec::new();
        for entry in fs::read_dir(&self.directory)
            .map_err(|error| format!("Cannot read the model library: {error}"))?
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || !entry.path().is_dir() {
                continue;
            }
            if let Ok(model) = inspect_model_directory(entry.path()) {
                let bytes = directory_size(&entry.path()).unwrap_or(0);
                models.push(LibraryModel { model, bytes });
            }
        }
        models.sort_by(|left, right| left.model.id.cmp(&right.model.id));
        Ok(models)
    }

    /// Copy `source` into the library. The source is never modified.
    pub fn import(&self, source: impl AsRef<Path>) -> Result<LibraryModel, String> {
        let source = source.as_ref();
        let inspected = inspect_model_directory(source)?;
        let source_name = source
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        // 库内目录沿用源文件夹名：模型识别依赖目录名中的家族信息，
        // 同时 id 由“家族前缀 + 目录名”稳定得出，避免重复加前缀。
        let entry_name = safe_folder_name(&source_name)?;
        let source_bytes = directory_size(source)?;

        fs::create_dir_all(&self.directory)
            .map_err(|error| format!("Cannot create the model library: {error}"))?;
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.directory.join(LIBRARY_LOCK))
            .map_err(|error| format!("Cannot open the model library lock: {error}"))?;
        lock.lock_exclusive()
            .map_err(|error| format!("The model library is busy: {error}"))?;

        let target = self.directory.join(&entry_name);
        if target.exists() {
            return Err(format!(
                "同名模型已在库中（{}）。如需重新导入，请先删除库中的副本。",
                entry_name
            ));
        }
        ensure_space(&self.directory, source_bytes)?;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let temp = self
            .directory
            .join(format!("{TEMP_PREFIX}{entry_name}-{nonce}"));
        if let Err(error) = copy_tree(source, &temp) {
            let _ = move_to_trash(&temp, &default_trash_dir());
            return Err(error);
        }

        // Re-validate the copy: only a complete folder becomes visible.
        let copied = match inspect_model_directory(&temp) {
            Ok(copied) => copied,
            Err(error) => {
                let _ = move_to_trash(&temp, &default_trash_dir());
                return Err(format!("复制后的模型校验失败：{error}"));
            }
        };
        if copied.family != inspected.family {
            let _ = move_to_trash(&temp, &default_trash_dir());
            return Err("复制后的模型与源模型类型不一致，已放弃导入。".to_owned());
        }

        fs::rename(&temp, &target).map_err(|error| {
            let _ = move_to_trash(&temp, &default_trash_dir());
            format!("无法把模型移动到库目录：{error}")
        })?;

        let model = inspect_model_directory(&target)?;
        let bytes = directory_size(&target).unwrap_or(source_bytes);
        Ok(LibraryModel { model, bytes })
    }

    /// Remove a library copy by moving it to the OS trash; user downloads stay.
    pub fn delete(&self, id: &str) -> Result<PathBuf, String> {
        self.delete_to(id, &default_trash_dir())
    }

    pub fn delete_to(&self, id: &str, trash: &Path) -> Result<PathBuf, String> {
        let target = self.path_for(id)?;
        move_to_trash(&target, trash)
    }

    /// Resolve a model id to its library folder by re-validating the library;
    /// never trust an id as a path fragment.
    pub fn path_for(&self, id: &str) -> Result<PathBuf, String> {
        self.list()?
            .into_iter()
            .find(|entry| entry.model.id == id)
            .map(|entry| entry.model.directory)
            .ok_or_else(|| "库中没有这个模型。".to_owned())
    }
}

fn ensure_space(directory: &Path, required: u64) -> Result<(), String> {
    let available = fs2::available_space(directory)
        .map_err(|error| format!("Cannot check free disk space: {error}"))?;
    if available < required.saturating_add(RESERVED_MARGIN_BYTES) {
        return Err(format!(
            "磁盘空间不足：导入需要约 {} MB，当前可用 {} MB。",
            required / 1_048_576,
            available / 1_048_576
        ));
    }
    Ok(())
}

fn safe_folder_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err("这个模型文件夹名不能作为库目录名，请重命名后再导入。".to_owned());
    }
    Ok(name.to_owned())
}

fn directory_size(directory: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    let mut stack = vec![directory.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current)
            .map_err(|error| format!("Cannot read {}: {error}", current.display()))?;
        for entry in entries.flatten() {
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.is_dir() {
                stack.push(entry.path());
            } else if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    Ok(total)
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("Cannot prepare the model copy: {error}"))?;
    let entries = fs::read_dir(source)
        .map_err(|error| format!("Cannot read {}: {error}", source.display()))?;
    for entry in entries.flatten() {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Cannot inspect a model entry: {error}"))?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        if file_type.is_symlink() {
            return Err(
                "模型目录包含符号链接，出于安全原因未导入；请先把模型文件实际放回目录。".to_owned(),
            );
        }
        if file_type.is_dir() {
            copy_tree(&from, &to)?;
        } else if file_type.is_file() {
            fs::copy(&from, &to)
                .map_err(|error| format!("Cannot copy {}: {error}", from.display()))?;
        }
    }
    Ok(())
}

pub fn default_trash_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".Trash"))
        .unwrap_or_else(|| std::env::temp_dir().join("local-transcribe-trash"))
}

/// Move a folder into the trash instead of deleting it permanently.
fn move_to_trash(target: &Path, trash: &Path) -> Result<PathBuf, String> {
    if !target.exists() {
        return Err("要移除的目录不存在。".to_owned());
    }
    fs::create_dir_all(trash).map_err(|error| format!("Cannot prepare the trash: {error}"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".to_owned());
    let mut attempt = 0u32;
    loop {
        let candidate = trash.join(if attempt == 0 {
            format!("{name}-{stamp}")
        } else {
            format!("{name}-{stamp}-{attempt}")
        });
        if candidate.exists() {
            attempt += 1;
            continue;
        }
        return fs::rename(target, &candidate)
            .map(|_| candidate)
            .map_err(|error| format!("无法移入废纸篓：{error}"));
    }
}
