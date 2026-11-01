//! 更新检查。
//!
//! 启动时（默认开启）向官网 `updates.json` 发一个普通 GET，比较版本号；有新版本
//! 时由前端弹窗，用户点“更新”才在浏览器里打开 GitHub release 页面。
//!
//! 为什么不用 GitHub API：GitHub 在部分机器上不可达，而官网是自有域名。请求不带
//! 任何标识（无 UID、无机器码、无使用数据），只取一个静态 JSON。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// 官网上的版本清单（纯静态文件，随站点一起发布）。
pub const FEED_URL: &str = "https://apps.timhuang.net/transcribe/updates.json";
const SETTINGS_FILE: &str = "update-settings.json";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettings {
    pub auto_check: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check_at: Option<String>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        UpdateSettings {
            auto_check: true,
            last_check_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// 用户点“更新”后打开的地址（GitHub release）。
    pub url: String,
    #[serde(default)]
    pub notes_url: Option<String>,
    #[serde(default)]
    pub released_at: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Feed {
    #[serde(default)]
    schema_version: u32,
    latest: UpdateInfo,
}

fn settings_path() -> Result<PathBuf, String> {
    let dir = voice_to_text_service::default_data_dir()?;
    Ok(dir.join(SETTINGS_FILE))
}

fn load_settings_at(path: &Path) -> Result<UpdateSettings, String> {
    match fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|error| format!("更新设置损坏：{error}"))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(UpdateSettings::default()),
        Err(error) => Err(format!("无法读取更新设置：{error}")),
    }
}

pub fn load_settings() -> Result<UpdateSettings, String> {
    load_settings_at(&settings_path()?)
}

fn save_settings(settings: &UpdateSettings) -> Result<(), String> {
    save_settings_at(&settings_path()?, settings)
}

fn save_settings_at(path: &Path, settings: &UpdateSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let payload = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    let temp = path.with_file_name(format!("update-settings-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temp, payload).map_err(|error| format!("无法保存更新设置：{error}"))?;
    fs::rename(&temp, path).map_err(|error| format!("无法替换更新设置（临时文件保留）：{error}"))
}

/// 版本号按“数字段”比较：`1.7.0` > `1.6.0`，后缀（beta/-rc）不参与。
pub fn is_newer(current: &str, candidate: &str) -> bool {
    fn parts(value: &str) -> Option<[u64; 3]> {
        // 只比较主版本段；`-beta.2` / `+build` 这类后缀不参与。
        let core = value.split(['-', '+']).next().unwrap_or(value);
        let mut components = core.split('.');
        let major = components.next()?.parse().ok()?;
        let minor = components.next()?.parse().ok()?;
        let patch = components.next()?.parse().ok()?;
        if components.next().is_some() {
            return None;
        }
        Some([major, minor, patch])
    }
    match (parts(current), parts(candidate)) {
        (Some(current), Some(candidate)) => candidate > current,
        _ => false,
    }
}

fn parse_feed(body: &str) -> Result<UpdateInfo, String> {
    let feed: Feed =
        serde_json::from_str(body).map_err(|error| format!("更新清单格式无效：{error}"))?;
    if feed.schema_version != 0 && feed.schema_version != 1 {
        return Err(format!("更新清单版本 {} 不受支持。", feed.schema_version));
    }
    let parts: Vec<_> = feed.latest.version.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || part.parse::<u64>().is_err()
        })
    {
        return Err("更新清单中的版本号无效：应为数字形式的主版本.次版本.修订版本。".to_owned());
    }
    validate_release_url(&feed.latest.url)?;
    Ok(feed.latest)
}

async fn fetch_feed() -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(FEED_URL)
        .header("User-Agent", format!("LocalTranscribe/{CURRENT_VERSION}"))
        .send()
        .await
        .map_err(|error| format!("无法连接更新服务器：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("更新服务器返回 {}", response.status()));
    }
    let body = response.text().await.map_err(|error| error.to_string())?;
    parse_feed(&body)
}

#[tauri::command]
pub fn update_settings() -> Result<UpdateSettings, String> {
    load_settings()
}

#[tauri::command]
pub fn set_update_settings(auto_check: bool) -> Result<UpdateSettings, String> {
    let mut settings = load_settings()?;
    settings.auto_check = auto_check;
    save_settings(&settings)?;
    Ok(settings)
}

/// `force = true` 是设置页的“立即检查”，不受开关限制。
#[tauri::command]
pub async fn check_for_update(force: bool) -> Result<Option<UpdateInfo>, String> {
    let settings = load_settings()?;
    if !force && !settings.auto_check {
        return Ok(None);
    }
    let info = fetch_feed().await?;
    if is_newer(CURRENT_VERSION, &info.version) {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

fn validate_release_url(url: &str) -> Result<(), String> {
    const PREFIX: &str = "https://github.com/Super-Console/Local-Transcribe/releases/tag/";
    let tag = url
        .strip_prefix(PREFIX)
        .ok_or("更新地址不是本项目的 GitHub release。")?;
    if tag.is_empty()
        || !tag
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
    {
        return Err("更新地址中的版本无效。".to_owned());
    }
    Ok(())
}

/// Only open a release page in the official repository, without a shell.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    validate_release_url(&url)?;
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(&url)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开浏览器：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically_not_lexically() {
        assert!(is_newer("1.6.0", "1.7.0"));
        assert!(is_newer("1.6.0", "1.10.0"), "10 > 6");
        assert!(is_newer("1.6.0", "1.6.1"));
        assert!(!is_newer("1.7.0", "1.7.0"));
        assert!(!is_newer("1.7.0", "1.6.9"));
        assert!(
            !is_newer("1.7.0", "1.7.0-beta.2"),
            "pre-release is not newer"
        );
    }

    #[test]
    fn version_comparison_rejects_malformed_component_counts() {
        assert!(
            !is_newer("1.7.0", "1.7.0.1"),
            "a four-component candidate must not be treated as a newer release"
        );
        assert!(
            !is_newer("invalid", "1.7.1"),
            "an invalid current version must not make every candidate newer"
        );
    }

    #[test]
    fn the_feed_must_be_https_and_understood() {
        let ok = r#"{"schemaVersion":1,"latest":{"version":"1.7.0","url":"https://github.com/Super-Console/Local-Transcribe/releases/tag/1.7.0"}}"#;
        assert_eq!(parse_feed(ok).unwrap().version, "1.7.0");

        let insecure =
            r#"{"schemaVersion":1,"latest":{"version":"1.7.0","url":"http://example.com"}}"#;
        assert!(parse_feed(insecure).is_err());

        let future =
            r#"{"schemaVersion":9,"latest":{"version":"1.7.0","url":"https://example.com"}}"#;
        assert!(parse_feed(future).is_err());

        let malformed = r#"{"schemaVersion":1,"latest":{"version":"999.garbage.0","url":"https://github.com/Super-Console/Local-Transcribe/releases/tag/999.garbage.0"}}"#;
        assert!(parse_feed(malformed).is_err());
    }

    #[test]
    fn only_https_urls_are_opened() {
        assert!(open_external("http://example.com".to_owned()).is_err());
        assert!(open_external("file:///etc/passwd".to_owned()).is_err());
        assert!(
            validate_release_url(
                "https://github.com/Super-Console/Local-Transcribe/releases/tag/1.7.0"
            )
            .is_ok()
        );
        assert!(
            validate_release_url(
                "https://github.com.evil.test/Super-Console/Local-Transcribe/releases/tag/1.7.0"
            )
            .is_err()
        );
        assert!(
            validate_release_url(
                "https://github.com/Super-Console/Local-Transcribe/releases/tag/1.7.0&calc.exe"
            )
            .is_err()
        );
    }

    #[test]
    fn disabled_preference_round_trips_and_corruption_is_an_error() {
        let dir = std::env::temp_dir().join(format!(
            "lt-update-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        save_settings_at(
            &path,
            &UpdateSettings {
                auto_check: false,
                last_check_at: None,
            },
        )
        .unwrap();
        assert!(!load_settings_at(&path).unwrap().auto_check);
        fs::write(&path, b"broken").unwrap();
        assert!(load_settings_at(&path).is_err());
    }
}
