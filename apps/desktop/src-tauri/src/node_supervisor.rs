//! Supervises the `local-transcribe-node` sidecar.
//!
//! Responsibilities only: locate the binary, spawn it with arguments and the
//! parent pid, detect exit, and stop it. The GUI never hosts `NodeServer`.
//!
//! The child's stdout/stderr go straight to a log file (never a pipe), so a
//! chatty node can never block on a full pipe buffer.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use serde::Deserialize;

use crate::lan_host::HostStatus;

/// How long the child may take before it must have written its state file.
const READY_TIMEOUT: Duration = Duration::from_secs(12);
/// Grace period for the child to remove its own state file on SIGINT.
const STOP_TIMEOUT: Duration = Duration::from_secs(4);
const LOG_LIMIT_BYTES: u64 = 5 * 1024 * 1024;

const STATE_FILE: &str = "lan-node.json";
const LOG_FILE: &str = "lan-node.log";

#[derive(Debug, Deserialize)]
struct NodeStateFile {
    pid: u32,
    name: String,
    address: String,
    mask: String,
    subnet_filter: String,
    port: u16,
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    started_at_unix_ms: u64,
    served_models: Vec<String>,
}

pub struct NodeSupervisor {
    child: Option<Child>,
    state_path: PathBuf,
    status: HostStatus,
}

impl NodeSupervisor {
    pub fn start(
        root: &Path,
        log_dir: &Path,
        name: &str,
        address: &str,
        port: u16,
        mask: Ipv4Addr,
        idle_unload: u64,
    ) -> Result<Self, String> {
        let binary = node_binary()?;
        let data_dir = root.join("lan-host");
        fs::create_dir_all(&data_dir)
            .map_err(|error| format!("Cannot prepare node storage: {error}"))?;
        fs::create_dir_all(log_dir).map_err(|error| format!("Cannot prepare logs: {error}"))?;
        let log_path = log_dir.join(LOG_FILE);
        rotate_log(&log_path)?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|error| format!("Cannot open the node log: {error}"))?;

        let mut command = Command::new(&binary);
        command
            .arg("--name")
            .arg(name)
            .arg("--data-dir")
            .arg(&data_dir)
            .arg("--listen")
            .arg(format!("{address}:{port}"))
            .arg("--compute")
            .arg("on")
            .arg("--role")
            .arg("cpu")
            .arg("--models-dir")
            .arg(root)
            .arg("--selected-model")
            .arg(root.join("selected-model.json"))
            .arg("--parent-pid")
            .arg(std::process::id().to_string())
            .arg("--idle-unload")
            .arg(idle_unload.to_string())
            .arg("--retention-file")
            .arg(root.join("lan-host-retention.json"))
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone().map_err(|error| {
                format!("Cannot redirect node output: {error}")
            })?))
            .stderr(Stdio::from(log));
        if mask != Ipv4Addr::UNSPECIFIED {
            command.arg("--netmask").arg(mask.to_string());
        }

        let mut child = command.spawn().map_err(|error| {
            format!(
                "Cannot start the node process ({}): {error}",
                binary.display()
            )
        })?;
        let state_path = data_dir.join(STATE_FILE);
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(state)) => {
                    let log = tail(&log_path);
                    if log.to_lowercase().contains("address already in use") {
                        return Err(format!(
                            "端口 {port} 已被占用。请用 `lsof -nP -iTCP:{port}` 检查占用进程，或换一个端口。\n{log}"
                        ));
                    }
                    return Err(format!("The node stopped during startup ({state}).\n{log}"));
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    return Err(format!("Cannot read the node process state: {error}"));
                }
            }
            if let Some(state) = read_state(&state_path)
                && state.pid == child.id()
            {
                let status = status_from(&state);
                return Ok(Self {
                    child: Some(child),
                    state_path,
                    status,
                });
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "The node did not report readiness within {}s.\n{}",
                    READY_TIMEOUT.as_secs(),
                    tail(&log_path)
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn status(&mut self) -> Result<HostStatus, String> {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => self.child = None,
                Ok(None) => {}
                Err(error) => return Err(format!("Cannot read the node process state: {error}")),
            }
        }
        if self.child.is_none() {
            return Ok(HostStatus::default());
        }
        if let Some(state) = read_state(&self.state_path) {
            self.status = status_from(&state);
        }
        Ok(self.status.clone())
    }

    pub fn stop(&mut self) -> Result<HostStatus, String> {
        if let Some(mut child) = self.child.take() {
            // SIGINT lets the child delete its own state file and withdraw mDNS.
            #[cfg(unix)]
            unsafe {
                libc::kill(child.id() as libc::pid_t, libc::SIGINT);
            }
            let deadline = Instant::now() + STOP_TIMEOUT;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {
                        if Instant::now() >= deadline {
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
        }
        retire_state(&self.state_path);
        Ok(HostStatus::default())
    }
}

impl Drop for NodeSupervisor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn status_from(state: &NodeStateFile) -> HostStatus {
    HostStatus {
        supported: cfg!(target_os = "macos"),
        running: true,
        name: state.name.clone(),
        address: state.address.clone(),
        port: state.port,
        model: state.served_models.join(" · "),
        mask: state.mask.clone(),
        subnet_filter: state.subnet_filter.clone(),
        notice: None,
    }
}

fn read_state(path: &Path) -> Option<NodeStateFile> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// The child normally deletes its state file on SIGINT. If it was killed hard,
/// move the leftover to the Trash instead of deleting it (project rule).
fn retire_state(path: &Path) {
    if !path.exists() {
        return;
    }
    if crate::move_to_trash(path).is_err() {
        eprintln!(
            "warning: stale node state left at {} (could not move it to the Trash)",
            path.display()
        );
    }
}

/// Move a log that outgrew the cap to `.1`, keeping exactly one generation.
fn rotate_log(path: &Path) -> Result<(), String> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() <= LOG_LIMIT_BYTES {
        return Ok(());
    }
    let previous = path.with_file_name(format!("{LOG_FILE}.1"));
    if previous.exists() {
        let _ = crate::move_to_trash(&previous);
    }
    fs::rename(path, &previous).map_err(|error| format!("Cannot rotate the node log: {error}"))
}

/// Last lines of the child log, used to surface real startup failures.
fn tail(path: &Path) -> String {
    const MAX_BYTES: u64 = 16 * 1024;
    const MAX_LINES: usize = 12;
    let Ok(mut file) = fs::File::open(path) else {
        return "(no node log yet)".to_owned();
    };
    let len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let start = len.saturating_sub(MAX_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return "(cannot read the node log)".to_owned();
    }
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    let lines = text.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(MAX_LINES);
    lines[start..].join("\n")
}

fn node_binary() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("VOICE_TO_TEXT_NODE_BINARY") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "VOICE_TO_TEXT_NODE_BINARY does not point to a file: {}",
            path.display()
        ));
    }
    let exe = std::env::current_exe().map_err(|error| format!("Cannot locate the app: {error}"))?;
    let directory = exe
        .parent()
        .ok_or_else(|| "Cannot locate the app directory.".to_owned())?;
    let candidate = directory.join(if cfg!(windows) {
        "local-transcribe-node.exe"
    } else {
        "local-transcribe-node"
    });
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(format!(
            "The node sidecar is missing: {}. Reinstall the app or rebuild with the sidecar.",
            candidate.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{NodeStateFile, read_state, rotate_log, tail};
    use std::io::Write;

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "node-supervisor-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn tail_returns_the_last_lines_of_the_child_log() {
        let root = scratch("tail");
        let path = root.join("lan-node.log");
        let mut file = std::fs::File::create(&path).unwrap();
        for index in 0..40 {
            writeln!(file, "line {index}").unwrap();
        }
        drop(file);
        let text = tail(&path);
        assert!(text.contains("line 39"));
        assert!(!text.contains("line 1\n"));
        assert_eq!(text.lines().count(), 12);
    }

    #[test]
    fn rotation_keeps_one_previous_generation() {
        let root = scratch("rotate");
        let path = root.join("lan-node.log");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(6 * 1024 * 1024).unwrap();
        drop(file);
        rotate_log(&path).unwrap();
        let previous = root.join("lan-node.log.1");
        assert!(previous.exists());
        assert!(!path.exists());
        std::fs::write(&path, b"fresh").unwrap();
        rotate_log(&path).unwrap();
        assert_eq!(std::fs::metadata(&previous).unwrap().len(), 6 * 1024 * 1024);
    }

    #[test]
    fn small_logs_are_not_rotated() {
        let root = scratch("small");
        let path = root.join("lan-node.log");
        std::fs::write(&path, b"small").unwrap();
        rotate_log(&path).unwrap();
        assert!(path.exists());
        assert!(!root.join("lan-node.log.1").exists());
    }

    #[test]
    fn state_file_is_parsed_with_filter_fields() {
        let root = scratch("state");
        let path = root.join("lan-node.json");
        std::fs::write(
            &path,
            br#"{
              "pid": 42, "name": "n", "address": "100.64.0.1",
              "mask": "255.255.255.255", "subnet_filter": "none",
              "port": 7878, "version": "1.2.0-beta.1",
              "started_at_unix_ms": 1, "served_models": ["m1"]
            }"#,
        )
        .unwrap();
        let state: NodeStateFile = read_state(&path).unwrap();
        assert_eq!(state.port, 7878);
        assert_eq!(state.subnet_filter, "none");
    }
}

#[cfg(test)]
mod sidecar_tests {
    use super::*;
    use std::net::TcpStream;

    fn fixture_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "sidecar-real-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let model = root.join("models/sherpa-onnx-paraformer-zh-fixture");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("model.int8.onnx"), b"fixture").unwrap();
        std::fs::write(model.join("tokens.txt"), b"a 1\n").unwrap();
        std::fs::create_dir_all(root.join("logs")).unwrap();
        root
    }

    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    /// Real start/stop against the built sidecar. Run with:
    /// `VOICE_TO_TEXT_NODE_BINARY=target/debug/local-transcribe-node \
    ///  cargo test -p voice-to-text-desktop -- --ignored sidecar`
    #[test]
    #[ignore = "spawns the real node sidecar; set VOICE_TO_TEXT_NODE_BINARY"]
    fn starts_and_stops_the_real_sidecar() {
        let root = fixture_root();
        let port = free_port();
        let mut supervisor = NodeSupervisor::start(
            &root,
            &root.join("logs"),
            "test-node",
            "127.0.0.1",
            port,
            Ipv4Addr::new(255, 255, 255, 255),
            120,
        )
        .expect("node starts");
        assert!(supervisor.status().unwrap().running);
        assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());
        let state: NodeStateFile =
            read_state(&root.join("lan-host/lan-node.json")).expect("state file");
        assert_eq!(state.subnet_filter, "none");
        let _child_pid = state.pid;

        supervisor.stop().unwrap();
        assert!(!root.join("lan-host/lan-node.json").exists());
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
        // The child must be gone, not merely disconnected.
        #[cfg(unix)]
        unsafe {
            assert_ne!(libc::kill(_child_pid as libc::pid_t, 0), 0);
        }
    }
}

#[cfg(test)]
mod sidecar_failure_tests {
    use super::*;

    fn fixture_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "sidecar-fail-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let model = root.join("models/sherpa-onnx-paraformer-zh-fixture");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("model.int8.onnx"), b"fixture").unwrap();
        std::fs::write(model.join("tokens.txt"), b"a 1\n").unwrap();
        std::fs::create_dir_all(root.join("logs")).unwrap();
        root
    }

    #[test]
    #[ignore = "spawns the real node sidecar; set VOICE_TO_TEXT_NODE_BINARY"]
    fn an_occupied_port_surfaces_the_child_error() {
        let root = fixture_root();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let error = NodeSupervisor::start(
            &root,
            &root.join("logs"),
            "conflict",
            "127.0.0.1",
            port,
            Ipv4Addr::new(255, 255, 255, 255),
            120,
        )
        .err()
        .expect("start must fail while the port is taken");
        assert!(
            error.contains("已被占用") && error.contains("lsof"),
            "{error}"
        );
    }

    #[test]
    #[ignore = "spawns the real node sidecar; set VOICE_TO_TEXT_NODE_BINARY"]
    fn a_killed_child_is_reported_as_stopped() {
        let root = fixture_root();
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut supervisor = NodeSupervisor::start(
            &root,
            &root.join("logs"),
            "killed",
            "127.0.0.1",
            port,
            Ipv4Addr::new(255, 255, 255, 255),
            120,
        )
        .unwrap();
        assert!(supervisor.status().unwrap().running);
        let _pid = read_state(&root.join("lan-host/lan-node.json"))
            .expect("state file")
            .pid;
        #[cfg(unix)]
        unsafe {
            libc::kill(_pid as libc::pid_t, libc::SIGKILL);
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(!supervisor.status().unwrap().running);
        supervisor.stop().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn child_output_of_two_megabytes_does_not_block() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let root = std::env::temp_dir().join(format!(
            "node-supervisor-big-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("node.log");
        let log = std::fs::File::create(&path).unwrap();
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("head -c 2500000 /dev/zero | tr '\\0' x")
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap();
        let started = Instant::now();
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if started.elapsed() > Duration::from_secs(5) {
                let _ = child.kill();
                panic!("child blocked while writing 2.5 MB to a file handle");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(std::fs::metadata(&path).unwrap().len() >= 2_000_000);
    }
}
