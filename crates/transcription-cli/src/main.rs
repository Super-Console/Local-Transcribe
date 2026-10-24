//! `local-transcribe` — command line client for the Local Transcribe service.
//!
//! Every command prints one JSON object to stdout; diagnostics go to stderr.
//! Exit codes: 0 success, 1 request failure, 2 usage, 4 service unavailable.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use voice_to_text_service::{default_data_dir, Request, Response, ServiceClient};

const USAGE: &str = "Usage:
  local-transcribe status [--data-dir DIR]
  local-transcribe models list [--data-dir DIR]
  local-transcribe models info <model-id> [--data-dir DIR]
  local-transcribe mode local|prefer-lan [--data-dir DIR]
  local-transcribe lan nodes [--data-dir DIR]
  local-transcribe lan list [--data-dir DIR]
      (lan list is an alias of lan nodes)
  local-transcribe lan add <name> <host>[:port] [--port P] [--force] [--data-dir DIR]
  local-transcribe lan remove <node> [--data-dir DIR]
  local-transcribe lan enable <node> [--data-dir DIR]
  local-transcribe lan disable <node> [--data-dir DIR]
  local-transcribe lan set <node> [--host H] [--port P] [--model ID] [--name N] [--data-dir DIR]
  local-transcribe lan scan [--timeout SECS] [--data-dir DIR]
  local-transcribe lan start [<node>] [--data-dir DIR]
      (with a node it starts it over SSH; without it starts the local node)
  local-transcribe lan stop [<node>] [--data-dir DIR]
  local-transcribe lan status [--data-dir DIR]
  local-transcribe jobs add <media> [--model ID] [--language LANG] [--root DIR] [--data-dir DIR]
      [--node NODE] [--lan-model ID] [--fallback-local]
  local-transcribe jobs list [--data-dir DIR]
  local-transcribe jobs show <job-id> [--data-dir DIR]
  local-transcribe jobs cancel <job-id> [--data-dir DIR]
  local-transcribe jobs retry <job-id> [--data-dir DIR]
  local-transcribe jobs reorder <job-id> [--after ID] [--data-dir DIR]
  local-transcribe jobs remove <job-id> [--data-dir DIR]
  local-transcribe export <job-id> --format txt|json --output PATH [--data-dir DIR]

<node> is a saved node id or its unique name. All commands print JSON. The
service is started on demand and shared with the app. Remote start/stop runs
fixed ssh commands (BatchMode, no password prompt); the optional 'ssh' field is
per initiating machine, so do not assume both ends share ssh aliases.";

struct Parsed {
    group: String,
    action: Option<String>,
    positional: Vec<String>,
    data_dir: Option<PathBuf>,
    model: Option<String>,
    language: Option<String>,
    roots: Vec<PathBuf>,
    after: Option<String>,
    format: Option<String>,
    output: Option<PathBuf>,
    node: Option<String>,
    lan_model: Option<String>,
    fallback_local: bool,
    host: Option<String>,
    port: Option<u16>,
    name: Option<String>,
    force: bool,
    timeout_secs: Option<u64>,
}

fn parse(args: Vec<String>) -> Result<Parsed, String> {
    let mut iter = args.into_iter().peekable();
    let mut parsed = Parsed {
        group: String::new(),
        action: None,
        positional: Vec::new(),
        data_dir: None,
        model: None,
        language: None,
        roots: Vec::new(),
        after: None,
        format: None,
        output: None,
        node: None,
        lan_model: None,
        fallback_local: false,
        host: None,
        port: None,
        name: None,
        force: false,
        timeout_secs: None,
    };
    if let Some(first) = iter.next() {
        if first == "--help" || first == "-h" || first == "help" {
            return Err(USAGE.to_owned());
        }
        // "mode" 也是一级命令，但它的动作是位置参数而不是二级词。
        if first == "models" || first == "jobs" || first == "lan" {
            parsed.group = first;
            parsed.action = iter.next();
        } else {
            parsed.group = first;
        }
    } else {
        return Err(USAGE.to_owned());
    }
    while let Some(arg) = iter.next() {
        let mut take = |name: &str| -> Result<String, String> {
            iter.next().ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--help" | "-h" => return Err(USAGE.to_owned()),
            "--json" => {}
            "--data-dir" => parsed.data_dir = Some(PathBuf::from(take("--data-dir")?)),
            "--model" => parsed.model = Some(take("--model")?),
            "--language" => parsed.language = Some(take("--language")?),
            "--root" => parsed.roots.push(PathBuf::from(take("--root")?)),
            "--after" => parsed.after = Some(take("--after")?),
            "--format" => parsed.format = Some(take("--format")?),
            "--output" => parsed.output = Some(PathBuf::from(take("--output")?)),
            "--node" => parsed.node = Some(take("--node")?),
            "--lan-model" => parsed.lan_model = Some(take("--lan-model")?),
            "--fallback-local" => parsed.fallback_local = true,
            "--host" => parsed.host = Some(take("--host")?),
            "--name" => parsed.name = Some(take("--name")?),
            "--force" => parsed.force = true,
            "--port" => {
                let value = take("--port")?;
                parsed.port = Some(parse_port(&value)?);
            }
            "--timeout" => {
                let value = take("--timeout")?;
                parsed.timeout_secs = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("--timeout expects a number of seconds, got '{value}'"))?,
                );
            }
            other if other.starts_with('-') => {
                return Err(format!("Unknown option: {other}\n{USAGE}"))
            }
            other => parsed.positional.push(other.to_owned()),
        }
    }
    Ok(parsed)
}

fn parse_port(value: &str) -> Result<u16, String> {
    let port = value
        .parse::<u16>()
        .map_err(|_| format!("port must be a number between 1 and 65535, got '{value}'"))?;
    if port == 0 {
        return Err("port must be between 1 and 65535.".to_owned());
    }
    Ok(port)
}

/// 拆分 `host[:port]`：只有「单个冒号 + 后缀是合法端口」才拆，避免破坏 IPv6。
///
/// `[::1]:7878` 这种带方括号的写法也支持。
fn split_host_port(value: &str) -> Result<(String, Option<u16>), String> {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix('[') {
        // 方括号形式：[::1]:7878 / [fe80::1]
        let Some(close) = rest.find(']') else {
            return Err(format!("'{value}' has an opening '[' without a closing ']'."));
        };
        let host = rest[..close].to_owned();
        let tail = &rest[close + 1..];
        if tail.is_empty() {
            return Ok((host, None));
        }
        let Some(port) = tail.strip_prefix(':') else {
            return Err(format!("Unexpected text after ']': '{tail}'."));
        };
        return Ok((host, Some(parse_port(port)?)));
    }
    let mut parts = trimmed.split(':');
    let head = parts.next().unwrap_or_default();
    match (parts.next(), parts.next()) {
        (Some(port), None) if !port.is_empty() => Ok((head.to_owned(), Some(parse_port(port)?))),
        (None, None) => Ok((trimmed.to_owned(), None)),
        // 两个及以上冒号：当作 IPv6 字面量，整体当 host，不做猜测。
        _ => Ok((trimmed.to_owned(), None)),
    }
}

fn service_binary() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("LOCAL_TRANSCRIBE_SERVICE") {
        return Ok(PathBuf::from(path));
    }
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "无法定位服务进程目录。".to_owned())?;
    for candidate in ["voice-to-text-service", "voice-to-text-service.exe"] {
        let path = dir.join(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err("找不到 voice-to-text-service（可用 LOCAL_TRANSCRIBE_SERVICE 指定）。".to_owned())
}

/// 连接已在运行的服务；没有时按需拉起一个（单实例锁保证不会双开）。
fn ensure_client(data_dir: &Path, roots: &[PathBuf]) -> Result<ServiceClient, String> {
    if let Ok(client) = ServiceClient::connect(data_dir) {
        return Ok(client);
    }
    let binary = service_binary()?;
    let mut command = Command::new(binary);
    command
        .arg("serve")
        .arg("--data-dir")
        .arg(data_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for root in roots {
        command.arg("--allow-root").arg(root);
    }
    command
        .spawn()
        .map_err(|error| format!("无法启动本地服务：{error}"))?;
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match ServiceClient::connect(data_dir) {
            Ok(client) => return Ok(client),
            Err(error) => {
                if Instant::now() >= deadline {
                    return Err(format!("本地服务没有就绪：{error}"));
                }
                thread::sleep(Duration::from_millis(120));
            }
        }
    }
}

fn print_response(response: &Response) -> ExitCode {
    let payload = serde_json::to_value(response).unwrap_or(Value::Null);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).unwrap_or_default()
    );
    if response.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn finish(result: Result<Response, String>) -> ExitCode {
    match result {
        Ok(response) => print_response(&response),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(4)
        }
    }
}

/// LAN 的启停会轮询最多 60s；客户端默认 30s 读超时会在服务返回前断开。
const LAN_READ_TIMEOUT_SECS: u64 = 90;

/// `lan start [node]` / `lan stop [node]`：给了节点名走远端，没给走本机。
fn lan_start_stop(action: &str, parsed: &Parsed) -> Request {
    let start = action == "start";
    // 显式传空串视作「没传」，避免把空 selector 发给服务。
    let remote = parsed
        .positional
        .first()
        .map(|node| node.trim())
        .filter(|node| !node.is_empty());
    match remote {
        Some(node) if start => Request::LanStartRemote {
            node: node.to_owned(),
        },
        Some(node) => Request::LanStopRemote {
            node: node.to_owned(),
        },
        None if start => Request::LanStartLocal,
        None => Request::LanStopLocal,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let data_dir = parsed
        .data_dir
        .clone()
        .or_else(|| default_data_dir().ok())
        .unwrap_or_else(|| std::env::temp_dir().join("local-transcribe"));
    let action = parsed.action.clone().unwrap_or_default();

    // 提交任务时把文件所在目录作为授权根，供按需启动的服务使用。
    let mut roots = parsed.roots.clone();
    if parsed.group == "jobs" && action == "add" {
        if let Some(media) = parsed.positional.first() {
            if let Ok(canonical) = PathBuf::from(media).canonicalize() {
                if let Some(parent) = canonical.parent() {
                    roots.push(parent.to_path_buf());
                }
            }
        }
    }

    let mut client = match ensure_client(&data_dir, &roots) {
        Ok(client) => client,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(4);
        }
    };

    // LAN 命令可能要等轮询（启动最多 60s、远端停止 10s、扫描 60s），放宽读超时。
    if parsed.group == "lan" {
        client.set_read_timeout(LAN_READ_TIMEOUT_SECS);
    }

    let request = match (parsed.group.as_str(), action.as_str()) {
        ("status", _) => Request::Ping,
        ("models", "list") => Request::Models,
        ("models", "info") => match parsed.positional.first() {
            Some(id) => Request::SelectModel { id: id.clone() },
            None => {
                eprintln!("models info needs a model id\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        // `mode local|prefer-lan`：模式词是位置参数而不是二级动作。
        ("mode", _) => {
            let Some(mode) = parsed.positional.first().filter(|value| !value.trim().is_empty())
            else {
                eprintln!("mode needs 'local' or 'prefer-lan'\n{USAGE}");
                return ExitCode::from(2);
            };
            Request::LanMode { mode: mode.clone() }
        }
        ("lan", "nodes") | ("lan", "list") => Request::LanNodes,
        ("lan", "add") => {
            let (Some(name), Some(host)) = (parsed.positional.first(), parsed.positional.get(1))
            else {
                eprintln!("lan add needs a name and a host\n{USAGE}");
                return ExitCode::from(2);
            };
            let (host, embedded_port) = match split_host_port(host) {
                Ok(split) => split,
                Err(error) => {
                    eprintln!("{error}\n{USAGE}");
                    return ExitCode::from(2);
                }
            };
            // --port 优先于 host 里内联的端口。
            let port = parsed.port.or(embedded_port);
            Request::LanAdd {
                name: name.clone(),
                host,
                port,
                force: parsed.force,
            }
        }
        ("lan", "remove") => match parsed.positional.first() {
            Some(node) => Request::LanRemove { node: node.clone() },
            None => {
                eprintln!("lan remove needs a node id or name\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        ("lan", "enable") => match parsed.positional.first() {
            Some(node) => Request::LanEnable { node: node.clone() },
            None => {
                eprintln!("lan enable needs a node id or name\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        ("lan", "disable") => match parsed.positional.first() {
            Some(node) => Request::LanDisable { node: node.clone() },
            None => {
                eprintln!("lan disable needs a node id or name\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        ("lan", "set") => match parsed.positional.first() {
            Some(node) => Request::LanSet {
                node: node.clone(),
                host: parsed.host.clone(),
                port: parsed.port,
                model: parsed.model.clone(),
                name: parsed.name.clone(),
            },
            None => {
                eprintln!("lan set needs a node id or name\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        ("lan", "scan") => Request::LanScan {
            timeout_secs: parsed.timeout_secs,
        },
        ("lan", "start") | ("lan", "stop") => lan_start_stop(&action, &parsed),
        ("lan", "status") => Request::LanStatus,
        ("jobs", "add") => {
            let Some(media) = parsed.positional.first() else {
                eprintln!("jobs add needs a media path\n{USAGE}");
                return ExitCode::from(2);
            };
            if let Some(language) = parsed.language.clone() {
                let response = Response {
                    ok: false,
                    error: Some(voice_to_text_service::ErrorBody {
                        code: "unsupported".to_owned(),
                        message: format!(
                            "指定语言（{language}）尚未支持：当前由模型自动检测语言。"
                        ),
                    }),
                    service: None,
                    job: None,
                    runs: None,
                    data: None,
                };
                return print_response(&response);
            }
            Request::Submit {
                path: media.clone(),
                model_dir: None,
                model_id: parsed.model.clone(),
                owner: Some("cli".to_owned()),
                node: parsed.node.clone(),
                lan_model: parsed.lan_model.clone(),
                fallback_local: parsed.fallback_local,
            }
        }
        ("jobs", "list") => Request::Jobs,
        ("jobs", "show") => match parsed.positional.first() {
            Some(id) => Request::Status { job_id: id.clone() },
            None => {
                eprintln!("jobs show needs a job id\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        ("jobs", "cancel") => match parsed.positional.first() {
            Some(id) => Request::Cancel { job_id: id.clone() },
            None => {
                eprintln!("jobs cancel needs a job id\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        ("jobs", "retry") => match parsed.positional.first() {
            Some(id) => Request::Retry { job_id: id.clone() },
            None => {
                eprintln!("jobs retry needs a job id\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        ("jobs", "reorder") => match parsed.positional.first() {
            Some(id) => Request::Reorder {
                job_id: id.clone(),
                after: parsed.after.clone(),
            },
            None => {
                eprintln!("jobs reorder needs a job id\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        ("jobs", "remove") => match parsed.positional.first() {
            Some(id) => Request::Remove { job_id: id.clone() },
            None => {
                eprintln!("jobs remove needs a job id\n{USAGE}");
                return ExitCode::from(2);
            }
        },
        ("export", _) => {
            let Some(id) = parsed.positional.first() else {
                eprintln!("export needs a job id\n{USAGE}");
                return ExitCode::from(2);
            };
            let (Some(format), Some(output)) = (parsed.format.clone(), parsed.output.clone())
            else {
                eprintln!("export needs --format txt|json and --output PATH\n{USAGE}");
                return ExitCode::from(2);
            };
            Request::Export {
                job_id: id.clone(),
                path: output.to_string_lossy().into_owned(),
                format,
            }
        }
        _ => {
            eprintln!("Unknown command: {} {}\n{USAGE}", parsed.group, action);
            return ExitCode::from(2);
        }
    };

    finish(client.request(&request))
}
