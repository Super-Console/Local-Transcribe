use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub job_id: String,
    pub state: JobState,
    pub phase: String,
    pub completed_chunks: usize,
    pub total_chunks: Option<usize>,
    /// 任务进入运行态时的 Unix 毫秒时间戳；用于界面计算实时倍速与剩余时间。
    #[serde(default)]
    pub started_at_ms: Option<u64>,
    /// 音频总时长（解码后得到）；用于倍速与 ETA，未知时为 None。
    #[serde(default)]
    pub total_duration_ms: Option<u64>,
    pub transcript_available: bool,
    pub result_saved: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JobStore {
    data_dir: PathBuf,
}

pub struct JobLease {
    store: JobStore,
    job_id: String,
    lock_file_path: PathBuf,
    lock_file: Option<File>,
    terminal: bool,
}

impl JobStore {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn jobs_dir(&self) -> PathBuf {
        self.data_dir.join("jobs")
    }

    pub fn transcripts_dir(&self) -> PathBuf {
        self.data_dir.join("transcripts")
    }

    pub fn result_path(&self, job_id: &str) -> Result<PathBuf, String> {
        validate_job_id(job_id)?;
        Ok(self.transcripts_dir().join(format!("{job_id}.json")))
    }

    pub fn new_job_id() -> String {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        format!("job-{now:x}-{:x}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn begin_job(&self, initial: JobStatus) -> Result<JobLease, String> {
        validate_job_id(&initial.job_id)?;
        if initial.state != JobState::Running {
            return Err("A new transcription task must start in the running state.".to_owned());
        }
        fs::create_dir_all(self.jobs_dir())
            .map_err(|_| "Could not prepare the local task store.".to_owned())?;
        fs::create_dir_all(self.transcripts_dir())
            .map_err(|_| "Could not prepare the local results store.".to_owned())?;

        let dispatch_lock_path = self.jobs_dir().join(".dispatch.lock");
        let dispatch_lock = open_lock(&dispatch_lock_path)?;
        FileExt::lock_exclusive(&dispatch_lock)
            .map_err(|_| "The local task store is busy; try again shortly.".to_owned())?;

        self.recover_interrupted_jobs()?;
        if self.has_active_job()? {
            return Err(
                "A transcription task is already running. Wait for it to finish before starting another."
                    .to_owned(),
            );
        }

        let status_path = self.status_path(&initial.job_id)?;
        if status_path.exists() {
            return Err("That transcription task ID already exists.".to_owned());
        }
        let lock_file_path = self.job_lock_path(&initial.job_id)?;
        let lock_file = open_lock(&lock_file_path)?;
        FileExt::try_lock_exclusive(&lock_file)
            .map_err(|_| "That transcription task ID is already in use.".to_owned())?;
        self.write_status(&initial)?;
        drop(dispatch_lock);

        Ok(JobLease {
            store: self.clone(),
            job_id: initial.job_id,
            lock_file_path,
            lock_file: Some(lock_file),
            terminal: false,
        })
    }

    pub fn job_status(&self, job_id: &str) -> Result<Option<JobStatus>, String> {
        validate_job_id(job_id)?;
        fs::create_dir_all(self.jobs_dir())
            .map_err(|_| "Could not read the local task store.".to_owned())?;
        let dispatch_lock = open_lock(&self.jobs_dir().join(".dispatch.lock"))?;
        FileExt::lock_exclusive(&dispatch_lock)
            .map_err(|_| "The local task store is busy; try again shortly.".to_owned())?;
        self.recover_job(job_id)?;
        let result = self.read_status(job_id)?;
        drop(dispatch_lock);
        Ok(result)
    }

    pub fn cancellation_requested(&self, job_id: &str) -> Result<bool, String> {
        Ok(self.cancel_path(job_id)?.exists())
    }

    pub fn active_job_status(&self) -> Result<Option<JobStatus>, String> {
        fs::create_dir_all(self.jobs_dir())
            .map_err(|_| "Could not read the local task store.".to_owned())?;
        let dispatch_lock = open_lock(&self.jobs_dir().join(".dispatch.lock"))?;
        FileExt::lock_exclusive(&dispatch_lock)
            .map_err(|_| "The local task store is busy; try again shortly.".to_owned())?;
        self.recover_interrupted_jobs()?;
        let entries = fs::read_dir(self.jobs_dir())
            .map_err(|_| "Could not inspect the local task store.".to_owned())?;
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("json")
            {
                continue;
            }
            let Ok(bytes) = fs::read(entry.path()) else {
                continue;
            };
            let Ok(status) = serde_json::from_slice::<JobStatus>(&bytes) else {
                continue;
            };
            if status.state == JobState::Running && self.job_is_live(&status.job_id)? {
                drop(dispatch_lock);
                return Ok(Some(status));
            }
        }
        drop(dispatch_lock);
        Ok(None)
    }

    pub fn request_cancel(&self, job_id: &str) -> Result<bool, String> {
        let Some(status) = self.job_status(job_id)? else {
            return Ok(false);
        };
        if status.state != JobState::Running {
            return Ok(false);
        }
        let marker = self.cancel_path(job_id)?;
        let created = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
        {
            Ok(mut file) => {
                file.write_all(b"cancel")
                    .map_err(|_| "Could not request task cancellation.".to_owned())?;
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
            Err(_) => return Err("Could not request task cancellation.".to_owned()),
        };
        if !created {
            return Ok(false);
        }
        if self
            .job_status(job_id)?
            .is_some_and(|current| current.state == JobState::Running)
        {
            Ok(true)
        } else {
            let _ = fs::remove_file(marker);
            Ok(false)
        }
    }

    pub fn save_transcript(&self, transcript: &crate::Transcript) -> Result<PathBuf, String> {
        let path = self.result_path(&transcript.job_id)?;
        fs::create_dir_all(self.transcripts_dir())
            .map_err(|_| "Could not prepare the local results store.".to_owned())?;
        let contents = transcript
            .to_json()
            .map_err(|_| "Could not encode the transcription result.".to_owned())?;
        atomic_write(&path, contents.as_bytes())
            .map_err(|_| "Could not save the local transcription result.".to_owned())?;
        let saved = fs::read_to_string(&path)
            .map_err(|_| "Could not verify the local transcription result.".to_owned())?;
        if saved != contents {
            return Err("The saved result did not match the generated transcript.".to_owned());
        }
        Ok(path)
    }

    pub fn transcript(&self, job_id: &str) -> Result<crate::Transcript, String> {
        let path = self.result_path(job_id)?;
        let bytes = fs::read(path)
            .map_err(|_| "The saved transcription result could not be found.".to_owned())?;
        let transcript: crate::Transcript = serde_json::from_slice(&bytes)
            .map_err(|_| "The saved transcription result is invalid.".to_owned())?;
        if transcript.job_id != job_id {
            return Err("The saved result does not match the requested task ID.".to_owned());
        }
        Ok(transcript)
    }

    fn has_active_job(&self) -> Result<bool, String> {
        let entries = fs::read_dir(self.jobs_dir())
            .map_err(|_| "Could not inspect the local task store.".to_owned())?;
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("json")
            {
                continue;
            }
            let Ok(bytes) = fs::read(entry.path()) else {
                continue;
            };
            let Ok(status) = serde_json::from_slice::<JobStatus>(&bytes) else {
                continue;
            };
            if status.state == JobState::Running && self.job_is_live(&status.job_id)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn recover_interrupted_jobs(&self) -> Result<(), String> {
        let entries = fs::read_dir(self.jobs_dir())
            .map_err(|_| "Could not inspect the local task store.".to_owned())?;
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("json")
            {
                continue;
            }
            let Ok(bytes) = fs::read(entry.path()) else {
                continue;
            };
            let Ok(status) = serde_json::from_slice::<JobStatus>(&bytes) else {
                continue;
            };
            if status.state == JobState::Running && !self.job_is_live(&status.job_id)? {
                self.mark_interrupted(status)?;
            }
        }
        Ok(())
    }

    fn recover_job(&self, job_id: &str) -> Result<(), String> {
        let Some(status) = self.read_status(job_id)? else {
            return Ok(());
        };
        if status.state == JobState::Running && !self.job_is_live(job_id)? {
            self.mark_interrupted(status)?;
        }
        Ok(())
    }

    fn job_is_live(&self, job_id: &str) -> Result<bool, String> {
        let lock_path = self.job_lock_path(job_id)?;
        let lock_file = match open_lock(&lock_path) {
            Ok(file) => file,
            Err(_) if !lock_path.exists() => return Ok(false),
            Err(_) => return Err("Could not inspect the local task lock.".to_owned()),
        };
        match FileExt::try_lock_exclusive(&lock_file) {
            Ok(()) => {
                let _ = FileExt::unlock(&lock_file);
                drop(lock_file);
                let _ = fs::remove_file(lock_path);
                Ok(false)
            }
            Err(error) if is_lock_contention(&error) => Ok(true),
            Err(_) => Err("Could not inspect the local task lock.".to_owned()),
        }
    }

    fn mark_interrupted(&self, mut status: JobStatus) -> Result<(), String> {
        status.state = JobState::Failed;
        status.phase = "Interrupted".to_owned();
        status.error = Some("The transcription was interrupted when its worker exited.".to_owned());
        self.write_status(&status)?;
        if let Ok(path) = self.cancel_path(&status.job_id) {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    fn read_status(&self, job_id: &str) -> Result<Option<JobStatus>, String> {
        let path = self.status_path(job_id)?;
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("Could not read the local task status.".to_owned()),
        };
        let status: JobStatus = serde_json::from_slice(&bytes)
            .map_err(|_| "The saved task status is invalid.".to_owned())?;
        if status.job_id != job_id {
            return Err("The saved task status does not match the requested task ID.".to_owned());
        }
        Ok(Some(status))
    }

    fn write_status(&self, status: &JobStatus) -> Result<(), String> {
        validate_job_id(&status.job_id)?;
        let path = self.status_path(&status.job_id)?;
        fs::create_dir_all(self.jobs_dir())
            .map_err(|_| "Could not prepare the local task store.".to_owned())?;
        let bytes = serde_json::to_vec_pretty(status)
            .map_err(|_| "Could not encode the local task status.".to_owned())?;
        atomic_write(&path, &bytes).map_err(|_| "Could not save the local task status.".to_owned())
    }

    fn status_path(&self, job_id: &str) -> Result<PathBuf, String> {
        validate_job_id(job_id)?;
        Ok(self.jobs_dir().join(format!("{job_id}.json")))
    }

    fn job_lock_path(&self, job_id: &str) -> Result<PathBuf, String> {
        validate_job_id(job_id)?;
        Ok(self.jobs_dir().join(format!("{job_id}.lock")))
    }

    fn cancel_path(&self, job_id: &str) -> Result<PathBuf, String> {
        validate_job_id(job_id)?;
        Ok(self.jobs_dir().join(format!("{job_id}.cancel")))
    }
}

impl JobLease {
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn save_status(&mut self, status: JobStatus) -> Result<(), String> {
        if status.job_id != self.job_id {
            return Err("A task worker cannot update a different task.".to_owned());
        }
        self.store.write_status(&status)?;
        if status.state != JobState::Running {
            self.terminal = true;
            if let Ok(path) = self.store.cancel_path(&self.job_id) {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }

    pub fn cancellation_requested(&self) -> Result<bool, String> {
        Ok(self.store.cancel_path(&self.job_id)?.exists())
    }
}

impl Drop for JobLease {
    fn drop(&mut self) {
        if self.terminal {
            if let Some(lock_file) = self.lock_file.take() {
                let _ = FileExt::unlock(&lock_file);
                drop(lock_file);
            }
            let _ = fs::remove_file(&self.lock_file_path);
        }
    }
}

fn validate_job_id(job_id: &str) -> Result<(), String> {
    if job_id.is_empty()
        || job_id.len() > 64
        || !job_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("Invalid transcription task ID.".to_owned());
    }
    Ok(())
}

fn is_lock_contention(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::WouldBlock {
        return true;
    }

    // fs2 uses LockFileEx on Windows, which reports ERROR_LOCK_VIOLATION (33)
    // as ErrorKind::Other instead of WouldBlock when another process owns the lock.
    #[cfg(windows)]
    {
        error.raw_os_error() == Some(33)
    }

    #[cfg(not(windows))]
    {
        false
    }
}
fn open_lock(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| "Could not access the local task lock.".to_owned())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("target path has no parent"))?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| std::io::Error::other("target path has no file name"))?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    replace_file(&temporary, path)
}

#[cfg(not(windows))]
fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }

    let existing: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let replacement: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both paths are valid NUL-terminated UTF-16 buffers for the duration of the call.
    let ok = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            replacement.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod lock_error_tests {
    use super::is_lock_contention;
    use std::io::{Error, ErrorKind};

    #[test]
    fn would_block_is_lock_contention() {
        assert!(is_lock_contention(&Error::from(ErrorKind::WouldBlock)));
    }

    #[cfg(windows)]
    #[test]
    fn windows_lock_violation_is_lock_contention() {
        assert!(is_lock_contention(&Error::from_raw_os_error(33)));
    }
}
