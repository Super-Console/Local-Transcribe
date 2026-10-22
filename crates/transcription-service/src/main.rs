//! `voice-to-text-service` — local service plus a minimal client for P0 checks.
//!
//! Machine-readable JSON is always printed to stdout; diagnostics go to stderr.
//!
//! Exit codes:
//!   0 success
//!   1 request or runtime failure
//!   2 usage error
//!   3 another service instance is already running

use std::path::PathBuf;
use std::process::ExitCode;

use voice_to_text_service::{
    default_data_dir, run_server, Request, Response, ServerOptions, ServerStartError, ServiceClient,
};

fn usage() -> &'static str {
    "Usage:\n  voice-to-text-service serve [--data-dir DIR] [--allow-root DIR]...\n  \
     voice-to-text-service ping [--data-dir DIR]\n  \
     voice-to-text-service submit <absolute-media-path> [--model-dir DIR] [--data-dir DIR]\n  \
     voice-to-text-service status <job-id> [--data-dir DIR]\n  \
     voice-to-text-service cancel <job-id> [--data-dir DIR]\n  \
     voice-to-text-service runs [--data-dir DIR]"
}

struct Parsed {
    command: String,
    positionals: Vec<String>,
    data_dir: Option<PathBuf>,
    model_dir: Option<PathBuf>,
    allow_roots: Vec<PathBuf>,
}

fn parse(args: Vec<String>) -> Result<Parsed, String> {
    let mut iter = args.into_iter();
    let command = iter.next().ok_or_else(|| usage().to_owned())?;
    let mut parsed = Parsed {
        command,
        positionals: Vec::new(),
        data_dir: None,
        model_dir: None,
        allow_roots: Vec::new(),
    };
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--data-dir" => {
                let value = iter.next().ok_or("--data-dir needs a value")?;
                parsed.data_dir = Some(PathBuf::from(value));
            }
            "--model-dir" => {
                let value = iter.next().ok_or("--model-dir needs a value")?;
                parsed.model_dir = Some(PathBuf::from(value));
            }
            "--allow-root" => {
                let value = iter.next().ok_or("--allow-root needs a value")?;
                parsed.allow_roots.push(PathBuf::from(value));
            }
            "--help" | "-h" => return Err(usage().to_owned()),
            other if other.starts_with('-') => {
                return Err(format!("Unknown option: {other}\n{}", usage()));
            }
            other => parsed.positionals.push(other.to_owned()),
        }
    }
    Ok(parsed)
}

fn print_response(response: &Response) -> ExitCode {
    match serde_json::to_string_pretty(response) {
        Ok(json) => println!("{json}"),
        Err(error) => eprintln!("could not encode response: {error}"),
    }
    if response.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
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
    let data_dir = match parsed.data_dir {
        Some(dir) => dir,
        None => match default_data_dir() {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        },
    };

    match parsed.command.as_str() {
        "serve" => match run_server(ServerOptions {
            data_dir,
            allow_roots: parsed.allow_roots,
        }) {
            Ok(()) => ExitCode::SUCCESS,
            Err(ServerStartError::AlreadyRunning(message)) => {
                eprintln!("{message}");
                ExitCode::from(3)
            }
            Err(ServerStartError::Other(message)) => {
                eprintln!("{message}");
                ExitCode::FAILURE
            }
        },
        "ping" | "runs" => {
            let mut client = match ServiceClient::connect(&data_dir) {
                Ok(client) => client,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };
            let request = if parsed.command == "ping" {
                Request::Ping
            } else {
                Request::Runs
            };
            match client.request(&request) {
                Ok(response) => print_response(&response),
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        "submit" => {
            let Some(path) = parsed.positionals.first() else {
                eprintln!("submit needs an absolute media path\n{}", usage());
                return ExitCode::from(2);
            };
            let mut client = match ServiceClient::connect(&data_dir) {
                Ok(client) => client,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };
            let request = Request::Submit {
                path: path.clone(),
                model_dir: parsed
                    .model_dir
                    .map(|dir| dir.to_string_lossy().into_owned()),
                model_id: None,
                owner: Some("cli".to_owned()),
            };
            match client.request(&request) {
                Ok(response) => print_response(&response),
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        "status" | "cancel" => {
            let Some(job_id) = parsed.positionals.first() else {
                eprintln!("{} needs a job id\n{}", parsed.command, usage());
                return ExitCode::from(2);
            };
            let mut client = match ServiceClient::connect(&data_dir) {
                Ok(client) => client,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };
            let request = if parsed.command == "status" {
                Request::Status {
                    job_id: job_id.clone(),
                }
            } else {
                Request::Cancel {
                    job_id: job_id.clone(),
                }
            };
            match client.request(&request) {
                Ok(response) => print_response(&response),
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        other => {
            eprintln!("Unknown command: {other}\n{}", usage());
            ExitCode::from(2)
        }
    }
}
