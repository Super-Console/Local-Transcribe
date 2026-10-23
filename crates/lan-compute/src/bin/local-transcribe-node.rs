//! `local-transcribe-node`: foreground LAN compute node.
//!
//! ```text
//! local-transcribe-node --name <显示名> --data-dir <目录>
//!     [--listen 0.0.0.0:7878]
//!     [--compute on|off] [--role gpu|cpu|auto]
//!     [--model-dir <路径>] [--media-root <目录>]
//! ```
//!
//! User-enabled trusted-LAN node; no pairing credentials.

use std::path::PathBuf;

use voice_to_text_lan::node::{NodeConfig, NodeServer};

const USAGE: &str = "local-transcribe-node - Local Transcribe LAN compute node

USAGE:
    local-transcribe-node --name <显示名> --data-dir <目录> [OPTIONS]

OPTIONS:
    --name <显示名>          Display name announced over mDNS (required)
    --data-dir <目录>        Task storage directory (required)
    --listen <IP:PORT>       Bind address (default 0.0.0.0:7878; port 0 picks a free port)
    --compute on|off         Transcribe locally (default off; 'on' requires --model-dir)
    --role gpu|cpu|auto      Compute role advertised to peers (default auto)
    --model-dir <路径>       sherpa-onnx model directory used for transcription
    --media-root <目录>      Only paths inside this root may be written by collect
    -h, --help               Show this help";

struct Args {
    name: String,
    data_dir: PathBuf,
    listen: std::net::SocketAddr,
    compute: bool,
    role: String,
    model_dir: Option<PathBuf>,
    media_root: Option<PathBuf>,
}

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(ParseOutcome::Help) => {
            println!("{USAGE}");
            return;
        }
        Err(ParseOutcome::Error(message)) => {
            eprintln!("error: {message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };

    if let Err(error) = std::fs::create_dir_all(&args.data_dir) {
        eprintln!(
            "error: cannot create data directory {}: {error}",
            args.data_dir.display()
        );
        std::process::exit(1);
    }

    let config = NodeConfig {
        name: args.name.clone(),
        data_dir: args.data_dir.clone(),
        listen: args.listen,
        compute_enabled: args.compute,
        role: args.role.clone(),
        model_dir: args.model_dir.clone(),
        media_root: args.media_root.clone(),
    };

    let node = match NodeServer::start(config) {
        Ok(node) => node,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };
    println!(
        "node \"{}\" listening on {}:{} (compute: {})",
        args.name,
        args.listen.ip(),
        node.port(),
        if args.compute { "on" } else { "off" }
    );

    wait_for_ctrl_c();
    eprintln!("shutting down…");
    node.shutdown();
    node.wait();
}

enum ParseOutcome {
    Help,
    Error(String),
}

fn parse_args<I: Iterator<Item = String>>(mut arguments: I) -> Result<Args, ParseOutcome> {
    let mut name = None;
    let mut data_dir = None;
    let mut listen = "0.0.0.0:7878".to_owned();
    let mut compute = false;
    let mut role = "auto".to_owned();
    let mut model_dir = None;
    let mut media_root = None;

    while let Some(argument) = arguments.next() {
        let mut take_value = |flag: &str| {
            arguments
                .next()
                .ok_or_else(|| ParseOutcome::Error(format!("{flag} requires a value")))
        };
        match argument.as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--name" => name = Some(take_value("--name")?),
            "--data-dir" => data_dir = Some(PathBuf::from(take_value("--data-dir")?)),
            "--listen" => listen = take_value("--listen")?,
            "--compute" => {
                compute = match take_value("--compute")?.as_str() {
                    "on" => true,
                    "off" => false,
                    other => {
                        return Err(ParseOutcome::Error(format!(
                            "--compute must be 'on' or 'off', got '{other}'"
                        )))
                    }
                }
            }
            "--role" => {
                role = take_value("--role")?;
                if !matches!(role.as_str(), "gpu" | "cpu" | "auto") {
                    return Err(ParseOutcome::Error(format!(
                        "--role must be 'gpu', 'cpu' or 'auto', got '{role}'"
                    )));
                }
            }
            "--model-dir" => model_dir = Some(PathBuf::from(take_value("--model-dir")?)),
            "--media-root" => media_root = Some(PathBuf::from(take_value("--media-root")?)),
            other => {
                return Err(ParseOutcome::Error(format!("unknown argument '{other}'")));
            }
        }
    }

    let name = name.ok_or_else(|| ParseOutcome::Error("--name is required".to_owned()))?;
    let data_dir =
        data_dir.ok_or_else(|| ParseOutcome::Error("--data-dir is required".to_owned()))?;
    let listen = listen
        .parse::<std::net::SocketAddr>()
        .map_err(|error| ParseOutcome::Error(format!("invalid --listen '{listen}': {error}")))?;

    Ok(Args {
        name,
        data_dir,
        listen,
        compute,
        role,
        model_dir,
        media_root,
    })
}

fn wait_for_ctrl_c() {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("warning: cannot build signal runtime ({error}); waiting forever instead");
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
    };
    runtime.block_on(async {
        if tokio::signal::ctrl_c().await.is_err() {
            eprintln!("warning: Ctrl-C handler unavailable; waiting forever");
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        }
    });
}
