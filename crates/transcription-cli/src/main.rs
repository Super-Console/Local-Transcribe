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
  local-transcribe lan nodes [--data-dir DIR]
  local-transcribe jobs add <media> [--model ID] [--language LANG] [--root DIR] [--data-dir DIR]
      [--node NODE] [--lan-model ID] [--fallback-local]
  local-transcribe jobs list [--data-dir DIR]
  local-transcribe jobs show <job-id> [--data-dir DIR]
  local-transcribe jobs cancel <job-id> [--data-dir DIR]
  local-transcribe jobs retry <job-id> [--data-dir DIR]
  local-transcribe jobs reorder <job-id> [--after ID] [--data-dir DIR]
  local-transcribe jobs remove <job-id> [--data-dir DIR]
  local-transcribe export <job-id> --format txt|json --output PATH [--data-dir DIR]

All commands print JSON. The service is started on demand and shared with the app.";

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
    };
    if let Some(first) = iter.next() {
        if first == "--help" || first == "-h" || first == "help" {
            return Err(USAGE.to_owned());
        }
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
            other if other.starts_with('-') => {
                return Err(format!("Unknown option: {other}\n{USAGE}"))
            }
            other => parsed.positional.push(other.to_owned()),
        }
    }
    Ok(parsed)
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

    let request = match (parsed.group.as_str(), action.as_str()) {
        ("status", _) => Request::Ping,
        ("models", "list") => Request::Models,
        ("lan", "nodes") => Request::LanNodes,
        ("models", "info") => match parsed.positional.first() {
            Some(id) => Request::SelectModel { id: id.clone() },
            None => {
                eprintln!("models info needs a model id\n{USAGE}");
                return ExitCode::from(2);
            }
        },
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
