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

/// 渲染可直接粘贴的 MCP 客户端配置。默认不授权任何目录。
pub fn render_mcp_config(command: &Path, roots: &[PathBuf]) -> Result<String, String> {
    if !command.is_absolute() {
        return Err(format!(
            "The MCP server path must be absolute: {}",
            command.display()
        ));
    }
    let mut args: Vec<String> = Vec::new();
    for root in roots {
        if !root.is_absolute() {
            return Err(format!(
                "--allow-root needs an absolute directory: {}",
                root.display()
            ));
        }
        args.push("--allow-root".to_owned());
        args.push(root.to_string_lossy().into_owned());
    }
    let config = json!({
        "mcpServers": {
            "local-transcribe": {
                "command": command.to_string_lossy(),
                "args": args,
            }
        }
    });
    serde_json::to_string_pretty(&config).map_err(|error| error.to_string())
}

/// 配置之外的说明文字：stdout 保持纯 JSON，说明走 stderr。
pub fn mcp_config_note() -> String {
    [
        "With no --allow-root the server refuses to read any file.",
        "To authorize a directory, add one --allow-root pair to \"args\", e.g.",
        "  local-transcribe mcp-config --allow-root \"$HOME/Movies\"",
        "Paths must be absolute; symlinks that escape an authorized root are rejected.",
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
    fn config_without_roots_authorizes_nothing() {
        let command =
            absolute("Applications/Local Transcribe.app/Contents/MacOS/voice-to-text-mcp");
        let rendered = render_mcp_config(&command, &[]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let server = &parsed["mcpServers"]["local-transcribe"];
        assert_eq!(
            server["command"].as_str().unwrap(),
            command.to_string_lossy()
        );
        assert_eq!(server["args"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn config_keeps_each_root_as_a_separate_argument_pair() {
        let command = absolute("bin/voice-to-text-mcp");
        let roots = vec![absolute("Movies"), absolute("Documents/Recordings")];
        let rendered = render_mcp_config(&command, &roots).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let args = parsed["mcpServers"]["local-transcribe"]["args"]
            .as_array()
            .unwrap();
        let expected: Vec<String> = roots
            .iter()
            .flat_map(|root| {
                [
                    "--allow-root".to_owned(),
                    root.to_string_lossy().into_owned(),
                ]
            })
            .collect();
        let actual: Vec<String> = args
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn relative_paths_are_refused() {
        let command = PathBuf::from("voice-to-text-mcp");
        assert!(render_mcp_config(&command, &[]).is_err());
        assert!(render_mcp_config(
            &absolute("bin/voice-to-text-mcp"),
            &[PathBuf::from("Movies")]
        )
        .is_err());
    }

    #[test]
    fn the_note_explains_how_to_add_a_root() {
        let note = mcp_config_note();
        assert!(note.contains("--allow-root"));
        assert!(note.contains("mcp-config"));
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
