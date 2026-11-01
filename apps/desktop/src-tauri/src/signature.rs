//! 运行时读取自身签名状态（设置 → 关于 的构建标签）。
//!
//! 不写死「未签名开发构建」：签名身份来自 Security 框架对当前 `.app` 的静态
//! 代码检查；公证结论只在本地能读到 stapler 写入的票据时才声明，绝不猜测。

use serde::Serialize;

/// 关于页要展示的构建状态。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInfo {
    /// `none` | `adhoc` | `signed` | `notarized`
    pub state: &'static str,
    /// Developer ID 的 TeamIdentifier；ad-hoc 与未签名时为 None。
    pub team_id: Option<String>,
    /// 证书主题，例如 `Developer ID Application: …`。
    pub authority: Option<String>,
}

impl SignatureInfo {
    fn unsigned() -> Self {
        SignatureInfo {
            state: "none",
            team_id: None,
            authority: None,
        }
    }
}

/// 当前进程所在 `.app`（取不到时返回 None）。
fn bundle_root() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("app"))
        .map(|path| path.to_path_buf())
}

/// stapler 会把票据写在 `<app>/Contents/CodeResources`，以 `s8ch` 开头。
/// 读不到或格式不符时视为「没有本地公证凭据」，不影响签名身份的结论。
fn has_stapled_ticket(app: &std::path::Path) -> bool {
    use std::io::Read;
    let ticket = app.join("Contents").join("CodeResources");
    let Ok(metadata) = std::fs::metadata(&ticket) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() < 100 {
        return false;
    }
    let Ok(mut file) = std::fs::File::open(&ticket) else {
        return false;
    };
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() {
        return false;
    }
    &magic == b"s8ch"
}

/// 读取当前构建的签名与本地可验证的公证凭据。
pub fn current() -> SignatureInfo {
    let Some(app) = bundle_root() else {
        return SignatureInfo::unsigned();
    };
    let Some((team_id, authority)) = platform::signing_identity(&app) else {
        return SignatureInfo::unsigned();
    };
    let notarized = has_stapled_ticket(&app);
    let state = match (team_id.is_empty(), notarized) {
        // TeamIdentifier 为空 = ad-hoc 或未签名。
        (true, _) => "adhoc",
        (false, true) => "notarized",
        (false, false) => "signed",
    };
    SignatureInfo {
        state,
        team_id: if team_id.is_empty() {
            None
        } else {
            Some(team_id)
        },
        authority,
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    type CFTypeRef = *const c_void;
    type OSStatus = i32;

    /// `kSecCSSigningInformation`
    const SEC_CS_SIGNING_INFORMATION: u32 = 0x0000_0002;
    const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecCodeInfoTeamIdentifier: CFTypeRef;
        static kSecCodeInfoCertificates: CFTypeRef;
        fn SecStaticCodeCreateWithPath(
            path: CFTypeRef,
            flags: u32,
            static_code: *mut CFTypeRef,
        ) -> OSStatus;
        fn SecCodeCopySigningInformation(
            code: CFTypeRef,
            flags: u32,
            information: *mut CFTypeRef,
        ) -> OSStatus;
        fn SecCertificateCopySubjectSummary(certificate: CFTypeRef) -> CFTypeRef;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFURLCreateFromFileSystemRepresentation(
            allocator: CFTypeRef,
            buffer: *const u8,
            length: isize,
            is_directory: u8,
        ) -> CFTypeRef;
        fn CFDictionaryGetValue(dictionary: CFTypeRef, key: *const c_void) -> CFTypeRef;
        fn CFArrayGetCount(array: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
        fn CFStringGetCString(
            string: CFTypeRef,
            buffer: *mut c_char,
            buffer_size: isize,
            encoding: u32,
        ) -> u8;
        fn CFStringGetLength(string: CFTypeRef) -> isize;
        fn CFRelease(value: CFTypeRef);
    }

    /// 把一个 CFString 转成 Rust 字符串；调用方负责释放原对象。
    unsafe fn cf_string_to_owned(value: CFTypeRef) -> Option<String> {
        if value.is_null() {
            return None;
        }
        let length = unsafe { CFStringGetLength(value) };
        if length <= 0 {
            return Some(String::new());
        }
        let capacity = length * 4 + 1;
        let mut buffer = vec![0 as c_char; capacity as usize];
        let converted = unsafe {
            CFStringGetCString(
                value,
                buffer.as_mut_ptr(),
                capacity,
                CF_STRING_ENCODING_UTF8,
            )
        };
        if converted == 0 {
            return None;
        }
        Some(
            unsafe { CStr::from_ptr(buffer.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )
    }

    /// 用 Security 框架读取签名信息；返回 `(team_id, authority)`。
    pub(super) fn signing_identity(app: &Path) -> Option<(String, Option<String>)> {
        let path = CString::new(app.as_os_str().as_bytes()).ok()?;
        unsafe {
            let url = CFURLCreateFromFileSystemRepresentation(
                std::ptr::null(),
                path.as_bytes().as_ptr(),
                path.as_bytes().len() as isize,
                1,
            );
            if url.is_null() {
                return None;
            }

            let mut code: CFTypeRef = std::ptr::null();
            let created = SecStaticCodeCreateWithPath(url, 0, &mut code);
            CFRelease(url);
            if created != 0 || code.is_null() {
                return None;
            }

            let mut information: CFTypeRef = std::ptr::null();
            let copied =
                SecCodeCopySigningInformation(code, SEC_CS_SIGNING_INFORMATION, &mut information);
            CFRelease(code);
            if copied != 0 || information.is_null() {
                return None;
            }

            let team = CFDictionaryGetValue(information, kSecCodeInfoTeamIdentifier);
            let team_id = cf_string_to_owned(team);

            let authority = {
                let certificates = CFDictionaryGetValue(information, kSecCodeInfoCertificates);
                if !certificates.is_null() && CFArrayGetCount(certificates) > 0 {
                    let leaf = CFArrayGetValueAtIndex(certificates, 0);
                    let summary = SecCertificateCopySubjectSummary(leaf);
                    let name = cf_string_to_owned(summary);
                    if !summary.is_null() {
                        CFRelease(summary);
                    }
                    name
                } else {
                    None
                }
            };

            CFRelease(information);
            Some((team_id.unwrap_or_default(), authority))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::path::Path;

    pub(super) fn signing_identity(_app: &Path) -> Option<(String, Option<String>)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_ticket_is_not_treated_as_notarized() {
        let dir = std::env::temp_dir().join(format!(
            "lt-signature-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(dir.join("Contents")).unwrap();
        assert!(!has_stapled_ticket(&dir));

        // 有文件但格式不对（例如普通资源封印）不能算公证凭据。
        std::fs::write(dir.join("Contents").join("CodeResources"), vec![0u8; 4096]).unwrap();
        assert!(!has_stapled_ticket(&dir));

        let mut ticket = b"s8ch".to_vec();
        ticket.extend(std::iter::repeat_n(0u8, 2048));
        std::fs::write(dir.join("Contents").join("CodeResources"), &ticket).unwrap();
        assert!(has_stapled_ticket(&dir));
    }

    #[test]
    fn unsigned_state_has_no_team_id() {
        let info = SignatureInfo::unsigned();
        assert_eq!(info.state, "none");
        assert!(info.team_id.is_none());
    }
}
