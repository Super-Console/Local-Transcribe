//! Read-only permission diagnostics for the troubleshooting panel.
//!
//! Local Network is handled by `lan::diagnostics` (it is not a TCC service).
//! This module covers the two macOS privacy prompts the recorder uses:
//! microphone capture and system-audio / screen capture.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPermissionStatus {
    /// authorized | denied | restricted | not_determined | unknown
    pub microphone: String,
    /// granted | denied | unknown
    pub screen_capture: String,
}

pub(crate) fn status() -> AppPermissionStatus {
    #[cfg(target_os = "macos")]
    {
        AppPermissionStatus {
            microphone: microphone_label(native::microphone_status()),
            screen_capture: if native::screen_capture_granted() {
                "granted"
            } else {
                "denied"
            }
            .to_owned(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        AppPermissionStatus {
            microphone: "unknown".to_owned(),
            screen_capture: "unknown".to_owned(),
        }
    }
}

/// `AVAuthorizationStatus` values: 0 not determined, 1 restricted,
/// 2 denied, 3 authorized. Anything else is reported as unknown.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn microphone_label(raw: i64) -> String {
    match raw {
        0 => "not_determined",
        1 => "restricted",
        2 => "denied",
        3 => "authorized",
        _ => "unknown",
    }
    .to_owned()
}

#[cfg(target_os = "macos")]
mod native {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {
        static AVMediaTypeAudio: *const AnyObject;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }

    pub(super) fn microphone_status() -> i64 {
        unsafe {
            let media: *const AnyObject = AVMediaTypeAudio;
            let status: isize = msg_send![
                class!(AVCaptureDevice),
                authorizationStatusForMediaType: media
            ];
            status as i64
        }
    }

    pub(super) fn request_microphone_access() -> Result<bool, String> {
        use std::sync::mpsc;

        let (sender, receiver) = mpsc::sync_channel(1);
        let callback: block2::RcBlock<dyn Fn(objc2::runtime::Bool)> =
            block2::RcBlock::new(move |granted: objc2::runtime::Bool| -> () {
                let _ = sender.send(granted.as_bool());
            });
        unsafe {
            let media: *const AnyObject = AVMediaTypeAudio;
            let _: () = msg_send![
                class!(AVCaptureDevice),
                requestAccessForMediaType: media,
                completionHandler: &*callback
            ];
        }
        receiver
            .recv_timeout(std::time::Duration::from_secs(90))
            .map_err(|_| "The microphone permission request timed out.".to_owned())
    }

    pub(super) fn screen_capture_granted() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }
}

/// Open the matching macOS privacy pane. Local Network reuses the existing
/// command because its URL lives next to the LAN diagnostics.
pub(crate) fn open_pane(pane: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let url = match pane {
            "microphone" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            "screen_capture" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            other => return Err(format!("Unknown privacy pane: {other}")),
        };
        std::process::Command::new("open")
            .arg(url)
            .status()
            .map_err(|error| format!("Cannot open System Settings: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pane;
        Err("Privacy panes are managed by macOS.".to_owned())
    }
}

/// Ask macOS for microphone permission only after the user presses Allow.
pub(crate) fn request_microphone() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let current = native::microphone_status();
        if current != 0 {
            return Ok(microphone_label(current));
        }
        native::request_microphone_access()
            .map(|granted| if granted { "authorized" } else { "denied" }.to_owned())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok("unknown".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::microphone_label;

    #[test]
    fn status_is_queryable_and_stays_in_the_documented_set() {
        let status = super::status();
        assert!(
            matches!(
                status.microphone.as_str(),
                "authorized" | "denied" | "restricted" | "not_determined" | "unknown"
            ),
            "unexpected microphone state: {}",
            status.microphone
        );
        assert!(
            matches!(
                status.screen_capture.as_str(),
                "granted" | "denied" | "unknown"
            ),
            "unexpected screen-capture state: {}",
            status.screen_capture
        );
    }

    #[test]
    fn microphone_states_map_to_stable_labels() {
        assert_eq!(microphone_label(0), "not_determined");
        assert_eq!(microphone_label(1), "restricted");
        assert_eq!(microphone_label(2), "denied");
        assert_eq!(microphone_label(3), "authorized");
        assert_eq!(microphone_label(-1), "unknown");
        assert_eq!(microphone_label(99), "unknown");
    }
}
