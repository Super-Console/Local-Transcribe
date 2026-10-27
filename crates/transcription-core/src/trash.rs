//! 统一的“移入废纸篓”实现：本项目的删除约定是永不永久删除。
//!
//! macOS 移入 `~/.Trash`；Windows 交给回收站（`SHFileOperationW` + `FOF_ALLOWUNDO`）。
//! Windows 没有 `HOME` 环境变量、也没有 `~/.Trash`，所以不能靠路径重命名代替。

use std::path::Path;

/// 把文件移入当前平台的废纸篓/回收站。
pub fn move_to_trash(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|_| "HOME is unavailable.".to_owned())?;
        move_into_directory(path, &std::path::PathBuf::from(home).join(".Trash"))
    }

    #[cfg(target_os = "windows")]
    {
        move_to_windows_recycle_bin(path)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = path;
        Err("Moving entries to the Trash is not supported on this platform.".to_owned())
    }
}

/// 在目标目录里用带时间戳的名字保存，避免同名文件互相覆盖。
#[cfg(any(target_os = "macos", test))]
fn move_into_directory(path: &Path, trash_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(trash_dir)
        .map_err(|_| "Could not prepare the Trash folder.".to_owned())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("entry");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    std::fs::rename(path, trash_dir.join(format!("{name}.{stamp}")))
        .map_err(|error| format!("Could not move the entry to the Trash: {error}"))
}

/// 把文件移入 Windows 回收站。
#[cfg(target_os = "windows")]
fn move_to_windows_recycle_bin(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    /// `SHFILEOPSTRUCTW` (shellapi.h)；字段顺序与对齐按 C 结构体原样声明。
    #[repr(C)]
    struct ShFileOpStructW {
        hwnd: isize,
        w_func: u32,
        from: *const u16,
        to: *const u16,
        flags: u16,
        any_operations_aborted: i32,
        name_mappings: isize,
        progress_title: *const u16,
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHFileOperationW(operation: *mut ShFileOpStructW) -> i32;
    }

    const FO_DELETE: u32 = 0x0003;
    const FOF_SILENT: u16 = 0x0004;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOERRORUI: u16 = 0x0400;

    let mut from: Vec<u16> = path.as_os_str().encode_wide().collect();
    if from.contains(&0) {
        return Err("The entry path cannot be moved to the Recycle Bin.".to_owned());
    }
    // SHFileOperationW 接收的是双 NUL 结尾的路径列表。
    from.extend([0, 0]);

    let mut operation = ShFileOpStructW {
        hwnd: 0,
        w_func: FO_DELETE,
        from: from.as_ptr(),
        to: std::ptr::null(),
        flags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI,
        any_operations_aborted: 0,
        name_mappings: 0,
        progress_title: std::ptr::null(),
    };
    // SAFETY: `from` 是活到调用结束的双 NUL 结尾 UTF-16 缓冲区，其余指针字段均为空，
    // 所以 shell 只会读 `from`，不会写回任何 Rust 内存。
    let code = unsafe { SHFileOperationW(&mut operation) };
    if code != 0 {
        return Err(format!(
            "Could not move the entry to the Recycle Bin (shell error {code})."
        ));
    }
    if operation.any_operations_aborted != 0 {
        return Err("Moving the entry to the Recycle Bin was aborted.".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::move_into_directory;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("lt-trash-{name}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn moves_the_file_out_of_place_with_a_timestamped_name() {
        let root = scratch("move");
        let source = root.join("job-1.json");
        fs::write(&source, b"{\"state\":\"completed\"}").unwrap();
        let trash = root.join("trash");

        move_into_directory(&source, &trash).unwrap();

        assert!(!source.exists());
        let moved: Vec<PathBuf> = fs::read_dir(&trash)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(moved.len(), 1);
        let name = moved[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("job-1.json."), "unexpected name: {name}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_source_reports_an_error_instead_of_pretending_success() {
        let root = scratch("missing");
        let error =
            move_into_directory(&root.join("absent.json"), &root.join("trash")).unwrap_err();
        assert!(error.contains("Could not move"), "{error}");
        let _ = fs::remove_dir_all(&root);
    }
}
