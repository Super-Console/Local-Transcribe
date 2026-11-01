//! 随应用一起分发的二进制（MCP / CLI / service）与生成的客户端配置。
//!
//! 三个入口必须能被同一份安装包带出去，并且彼此能找到对方：MCP 与 CLI 都
//! 通过「与自己同目录的 `voice-to-text-service`」拉起本地服务。因此这里的路径
//! 解析以**当前可执行文件的真实路径**为基准（`canonicalize`），这样从
//! `~/.local/bin/local-transcribe` 这个软链启动时仍然找得到 App 包内的服务。

use std::path::{Path, PathBuf};

use serde_json::json;

/// MCP 服务器在 App 包内的可执行文件名（不含平台后缀）。
pub const MCP_BINARY: &str = "voice-to-text-mcp";
/// 命令行客户端在 App 包内的可执行文件名（不含平台后缀）。
pub const CLI_BINARY: &str = "local-transcribe";
/// 本地服务在 App 包内的可执行文件名（不含平台后缀）。
pub const SERVICE_BINARY: &str = "voice-to-text-service";

/// 当前平台的可执行文件名，`stem` 不含后缀。
pub fn executable_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

/// 装着 sidecar 的目录：运行中可执行文件的真实父目录。
pub fn sidecar_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Cannot locate the folder of the running executable.".to_owned())
}

/// 定位同目录下必须存在的 sidecar；缺失时给出可读的错误（开发构建常见）。
pub fn bundled_binary(stem: &str) -> Result<PathBuf, String> {
    let dir = sidecar_dir()?;
    let path = dir.join(executable_name(stem));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "This build does not include {} at {}. Install the packaged app to use it.",
            executable_name(stem),
            path.display()
        ))
    }
}

/// 把用户输入的 `~` 展开成主目录；其余路径原样返回。
pub fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(value)
}

/// 渲染可直接粘贴的 MCP 客户端配置。
///
/// 这台机器上只有当前用户能连上服务的本地 socket，读取哪份媒体文件由请求
/// 本身决定，没有额外的目录白名单需要在这里列出。
pub fn render_mcp_config(command: &Path) -> Result<String, String> {
    if !command.is_absolute() {
        return Err(format!(
            "The MCP server path must be absolute: {}",
            command.display()
        ));
    }
    let config = json!({
        "mcpServers": {
            "local-transcribe": {
                "command": command.to_string_lossy(),
                "args": Vec::<String>::new(),
            }
        }
    });
    serde_json::to_string_pretty(&config).map_err(|error| error.to_string())
}

/// 配置之外的说明文字：stdout 保持纯 JSON，说明走 stderr。
pub fn mcp_config_note() -> String {
    [
        "The server reads any absolute media path the caller names.",
        "Only the current user can connect to the local service socket.",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute(value: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:\\{value}"))
        } else {
            PathBuf::from(format!("/{value}"))
        }
    }

    #[test]
    fn config_lists_no_arguments() {
        let command =
            absolute("Applications/Local Transcribe.app/Contents/MacOS/voice-to-text-mcp");
        let rendered = render_mcp_config(&command).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let server = &parsed["mcpServers"]["local-transcribe"];
        assert_eq!(
            server["command"].as_str().unwrap(),
            command.to_string_lossy()
        );
        assert_eq!(server["args"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn relative_paths_are_refused() {
        let command = PathBuf::from("voice-to-text-mcp");
        assert!(render_mcp_config(&command).is_err());
    }

    #[test]
    fn the_note_does_not_mention_a_directory_whitelist() {
        let note = mcp_config_note();
        assert!(!note.contains("--allow-root"));
        assert!(!note.contains("roots add"));
        assert!(note.contains("absolute"));
    }

    #[test]
    fn executable_names_carry_the_platform_suffix() {
        let name = executable_name("voice-to-text-mcp");
        if cfg!(windows) {
            assert_eq!(name, "voice-to-text-mcp.exe");
        } else {
            assert_eq!(name, "voice-to-text-mcp");
        }
    }
}
