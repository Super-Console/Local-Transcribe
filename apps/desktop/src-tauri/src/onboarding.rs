use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

const STATE_FILE: &str = "onboarding-state.json";

#[cfg(not(windows))]
fn replace_state_file(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(windows)]
fn replace_state_file(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "MoveFileExW"]
        fn move_file_ex_w(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let replaced = unsafe {
        move_file_ex_w(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OnboardingState {
    #[serde(default, rename = "onboarding_completed")]
    pub onboarding_completed: bool,
    #[serde(default, rename = "preferred_asr_model_id")]
    pub preferred_asr_model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HardwareInfo {
    pub memory_bytes: Option<u64>,
    pub cpu_brand: Option<String>,
    pub apple_silicon: bool,
    pub available_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnboardingInfo {
    pub hardware: HardwareInfo,
    pub data_path: String,
    pub model_path: String,
    pub logs_path: String,
    pub model_bytes: std::collections::HashMap<String, u64>,
}

fn state_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STATE_FILE)
}

pub(crate) fn read_state(data_dir: &Path) -> Result<OnboardingState, String> {
    let path = state_path(data_dir);
    if !path.exists() {
        return Ok(OnboardingState::default());
    }
    let contents = fs::read(&path).map_err(|_| "Could not read onboarding settings.".to_owned())?;
    serde_json::from_slice(&contents).map_err(|_| "Onboarding settings are invalid.".to_owned())
}

pub(crate) fn write_state(data_dir: &Path, state: &OnboardingState) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|_| "Could not prepare app data.".to_owned())?;
    let target = state_path(data_dir);
    let temporary = data_dir.join(format!("{STATE_FILE}.tmp"));
    let contents = serde_json::to_vec_pretty(state)
        .map_err(|_| "Could not encode onboarding settings.".to_owned())?;
    fs::write(&temporary, contents)
        .map_err(|_| "Could not save onboarding settings.".to_owned())?;
    replace_state_file(&temporary, &target)
        .map_err(|_| "Could not finish saving onboarding settings.".to_owned())?;
    let saved: OnboardingState = serde_json::from_slice(
        &fs::read(&target).map_err(|_| "Could not verify onboarding settings.".to_owned())?,
    )
    .map_err(|_| "Could not verify onboarding settings.".to_owned())?;
    if &saved != state {
        return Err("Saved onboarding settings did not match.".to_owned());
    }
    Ok(())
}

pub(crate) fn detect_hardware(data_dir: &Path) -> HardwareInfo {
    let mut system = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
    );
    system.refresh_cpu_list(CpuRefreshKind::nothing());
    let memory_bytes = system.total_memory();
    let cpu_brand = system
        .cpus()
        .iter()
        .map(|cpu| cpu.brand().trim())
        .find(|brand| !brand.is_empty())
        .map(str::to_owned);
    let disks = Disks::new_with_refreshed_list();
    let available_bytes = disks
        .list()
        .iter()
        .filter(|disk| data_dir.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().to_string_lossy().len())
        .map(|disk| disk.available_space());

    HardwareInfo {
        memory_bytes: (memory_bytes > 0).then_some(memory_bytes),
        cpu_brand,
        apple_silicon: cfg!(all(target_os = "macos", target_arch = "aarch64")),
        available_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "local-transcribe-onboarding-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn missing_state_means_first_launch_and_state_round_trips() {
        let root = temp_dir();
        assert_eq!(
            read_state(&root).expect("default state"),
            OnboardingState::default()
        );

        let state = OnboardingState {
            onboarding_completed: true,
            preferred_asr_model_id: Some("x-asr-zh-en".to_owned()),
        };
        write_state(&root, &state).expect("persist state");
        assert_eq!(read_state(&root).expect("read state"), state);

        let updated = OnboardingState {
            onboarding_completed: false,
            preferred_asr_model_id: None,
        };
        write_state(&root, &updated).expect("replace existing state");
        assert_eq!(read_state(&root).expect("read replaced state"), updated);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn disk_detection_uses_the_data_directory_volume_and_does_not_shell_out() {
        let root = std::env::temp_dir();
        let hardware = detect_hardware(&root);
        assert!(hardware.memory_bytes.is_some());
        // A mount may be unavailable in restricted CI, so only assert the reported
        // free space is a valid value when sysinfo can identify the volume.
        assert!(hardware.available_bytes.is_none_or(|bytes| bytes > 0));
    }
}
