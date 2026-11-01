#[cfg(windows)]
use std::path::Path;

/// Build a protected DACL containing only SYSTEM and the current user's SID.
/// Validate the SID before interpolating it into SDDL so this helper cannot
/// accidentally admit another ACE through malformed input.
#[cfg(any(windows, test))]
pub(crate) fn user_only_socket_sddl(user_sid: &str) -> Result<String, String> {
    let mut fields = user_sid.split('-');
    if fields.next() != Some("S")
        || fields
            .next()
            .and_then(|field| field.parse::<u32>().ok())
            .is_none()
    {
        return Err("The current Windows user SID is invalid.".to_owned());
    }
    let remaining: Vec<_> = fields.collect();
    if remaining.len() < 2
        || remaining.len() > 15
        || remaining
            .iter()
            .any(|field| field.is_empty() || !field.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err("The current Windows user SID is invalid.".to_owned());
    }
    Ok(format!("D:P(A;;GA;;;SY)(A;;GA;;;{user_sid})"))
}

#[cfg(windows)]
pub(crate) fn secure_socket_path(path: &Path) -> Result<(), String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorDacl, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    };
    let sid = current_user_sid()?;
    let sddl = user_only_socket_sddl(&sid)?;
    let descriptor_text: Vec<u16> = std::ffi::OsStr::new(&sddl)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut descriptor = null_mut();
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor_text.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    };
    if converted == 0 {
        return Err(format!(
            "Could not create a private Windows service-socket ACL: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut dacl_present = 0;
    let mut dacl_defaulted = 0;
    let mut dacl = null_mut();
    let has_dacl = unsafe {
        GetSecurityDescriptorDacl(
            descriptor,
            &mut dacl_present,
            &mut dacl,
            &mut dacl_defaulted,
        )
    };
    if has_dacl == 0 || dacl_present == 0 || dacl.is_null() {
        unsafe {
            LocalFree(descriptor.cast::<c_void>());
        }
        return Err("Could not extract a Windows service-socket ACL.".to_owned());
    }

    let path_text: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let result = unsafe {
        SetNamedSecurityInfoW(
            path_text.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            dacl,
            null(),
        )
    };
    unsafe {
        LocalFree(descriptor.cast::<c_void>());
    }
    if result != ERROR_SUCCESS {
        return Err(format!(
            "Could not restrict the Windows service socket to this user: {}",
            std::io::Error::from_raw_os_error(result as i32)
        ));
    }
    verify_socket_acl(path, &sid)
}

#[cfg(windows)]
fn current_user_sid() -> Result<String, String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStringExt;
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(format!(
            "Could not read the current Windows user token: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut required = 0u32;
    unsafe {
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut required);
    }
    let mut buffer = vec![0u8; required as usize];
    let token_result = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    };
    unsafe {
        CloseHandle(token);
    }
    if token_result == 0 {
        return Err(format!(
            "Could not read the current Windows user SID: {}",
            std::io::Error::last_os_error()
        ));
    }
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut text = null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
        return Err(format!(
            "Could not format the current Windows user SID: {}",
            std::io::Error::last_os_error()
        ));
    }
    let sid =
        unsafe { std::ffi::OsString::from_wide(std::slice::from_raw_parts(text, sid_len(text))) }
            .to_string_lossy()
            .into_owned();
    unsafe {
        LocalFree(text.cast::<c_void>());
    }
    Ok(sid)
}

#[cfg(windows)]
unsafe fn sid_len(mut value: *const u16) -> usize {
    let mut length = 0;
    while *value != 0 {
        length += 1;
        value = value.add(1);
    }
    length
}

#[cfg(windows)]
fn verify_socket_acl(path: &Path, user_sid: &str) -> Result<(), String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

    let path_text: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut descriptor = null_mut();
    let result = unsafe {
        GetNamedSecurityInfoW(
            path_text.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(format!(
            "Could not read back the Windows service-socket ACL: {}",
            std::io::Error::from_raw_os_error(result as i32)
        ));
    }
    let mut text = null_mut();
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            null_mut(),
        )
    };
    unsafe {
        LocalFree(descriptor.cast::<c_void>());
    }
    if converted == 0 {
        return Err(format!(
            "Could not verify the Windows service-socket ACL: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut length = 0usize;
    unsafe {
        while *text.add(length) != 0 {
            length += 1;
        }
    }
    let actual = unsafe { std::ffi::OsString::from_wide(std::slice::from_raw_parts(text, length)) }
        .to_string_lossy()
        .into_owned();
    unsafe {
        LocalFree(text.cast::<c_void>());
    }
    if !private_dacl_matches(&actual, user_sid) {
        return Err(format!(
            "Windows service-socket ACL readback did not match the current user's private ACL: {actual} (expected trustee {user_sid})"
        ));
    }
    Ok(())
}

#[cfg(any(windows, test))]
fn private_dacl_matches(actual: &str, user_sid: &str) -> bool {
    let Some((_, dacl)) = actual.split_once("D:") else {
        return false;
    };
    let Some(first_ace) = dacl.find('(') else {
        return false;
    };
    let flags = &dacl[..first_ace];
    if flags != "P" && flags != "PAI" {
        return false;
    }

    let mut rest = &dacl[first_ace..];
    let mut trustees = Vec::new();
    while !rest.is_empty() {
        let Some(ace) = rest.strip_prefix('(') else {
            return false;
        };
        let Some(end) = ace.find(')') else {
            return false;
        };
        let fields: Vec<_> = ace[..end].split(';').collect();
        if fields.len() != 6
            || fields[0] != "A"
            || !fields[1].is_empty()
            || (fields[2] != "GA" && fields[2] != "FA")
            || !fields[3].is_empty()
            || !fields[4].is_empty()
        {
            return false;
        }
        // Windows may add an explicit OWNER RIGHTS entry; the owner is already
        // the current user, so it cannot widen access beyond that account.
        let trustee_is_allowed = fields[5] == "SY"
            || fields[5] == user_sid
            || fields[5] == "OW"
            || fields[5] == "S-1-3-4";
        if !trustee_is_allowed {
            return false;
        }
        trustees.push(fields[5]);
        rest = &ace[end + 1..];
    }

    trusts_system_and_user(&trustees, user_sid)
}

#[cfg(any(windows, test))]
fn trusts_system_and_user(trustees: &[&str], user_sid: &str) -> bool {
    let has_system = trustees.contains(&"SY");
    let has_user = trustees.contains(&user_sid);
    if !has_system || !has_user {
        return false;
    }
    // Nothing beyond SYSTEM, the current user and the owner-rights entry.
    let mut limit = 0usize;
    for trustee in trustees {
        if *trustee == "OW" || *trustee == "S-1-3-4" {
            continue;
        }
        limit += 1;
    }
    limit == 2 && trustees.len() <= 3
}

#[cfg(test)]
mod tests {
    use super::{private_dacl_matches, user_only_socket_sddl};

    #[test]
    fn socket_acl_is_protected_and_names_only_the_current_user_and_system() {
        assert_eq!(
            user_only_socket_sddl("S-1-5-21-111-222-333-1001").unwrap(),
            "D:P(A;;GA;;;SY)(A;;GA;;;S-1-5-21-111-222-333-1001)"
        );
        assert!(user_only_socket_sddl("S-1-5-21-1-2-3-4)(A;;GA;;;WD").is_err());
    }

    #[test]
    fn acl_readback_accepts_windows_canonical_full_access_encoding() {
        let actual = "O:BAG:S-1-5-21-1-2-3-513D:PAI(A;;FA;;;SY)(A;;FA;;;S-1-5-21-111-222-333-1001)";
        assert!(private_dacl_matches(actual, "S-1-5-21-111-222-333-1001"));
        // Windows adds an explicit OWNER RIGHTS entry in some environments.
        let with_owner_rights = "D:PAI(A;;FA;;;SY)(A;;FA;;;S-1-5-21-111-222-333-1001)(A;;FA;;;OW)";
        assert!(private_dacl_matches(
            with_owner_rights,
            "S-1-5-21-111-222-333-1001"
        ));
        assert!(!private_dacl_matches(
            "D:PAI(A;;FA;;;SY)(A;;FA;;;S-1-5-21-111-222-333-1001)(A;;FA;;;BA)",
            "S-1-5-21-111-222-333-1001"
        ));
        assert!(!private_dacl_matches(
            "D:(A;;FA;;;SY)(A;;FA;;;S-1-5-21-111-222-333-1001)",
            "S-1-5-21-111-222-333-1001"
        ));
        assert!(!private_dacl_matches(
            "D:P(A;;FA;;;SY)(A;;FA;;;S-1-5-21-111-222-333-1001)(A;;FA;;;BU)",
            "S-1-5-21-111-222-333-1001"
        ));
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::secure_socket_path;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn secured_socket_remains_connectable_by_its_owner() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lt-secure-socket-{stamp}.sock"));
        let listener = uds_windows::UnixListener::bind(&path).unwrap();
        secure_socket_path(&path).unwrap();
        assert!(uds_windows::UnixStream::connect(&path).is_ok());
        drop(listener);
        fs::remove_file(path).unwrap();
    }
}
