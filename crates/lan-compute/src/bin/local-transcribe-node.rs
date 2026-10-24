//! `local-transcribe-node`: headless LAN compute node.
//!
//! ```text
//! local-transcribe-node --name <显示名> --data-dir <目录> --listen <IPv4:端口>
//!     [--netmask <掩码>] [--compute on|off] [--role gpu|cpu|auto]
//!     [--models-dir <应用数据目录>] [--model-dir <模型目录>]
//!     [--selected-model <selected-model.json>] [--media-root <目录>]
//!     [--parent-pid <pid>]
//!
//! local-transcribe-node --from-settings
//! ```
//!
//! `--from-settings` 是 `lan start`（CLI/MCP 远程与本机拉起）的固定入口：从
//! 应用数据目录的 `lan-settings.json` 读 `host_name`/`host_address`/
//! `host_port`、空闲卸载与模型目录，自己解析网卡掩码，不要求其余参数；显式
//! 传入的参数优先。
//!
//! User-enabled trusted-LAN node; no pairing credentials. The listen address
//! may be any non-loopback IPv4 of this machine (Wi-Fi, Ethernet, VPN,
//! Tailscale) or `0.0.0.0` for every interface. A normal LAN mask keeps the
//! same-subnet filter; /31 and /32 point-to-point links do not filter, and
//! `0.0.0.0` is unscoped.

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use voice_to_text_core::{inspect_model_directory, LocalModel, ModelLibrary};
use voice_to_text_lan::node::{NodeConfig, NodeServer, RunningNode};

const USAGE: &str = "local-transcribe-node - Local Transcribe LAN compute node

USAGE:
    local-transcribe-node --name <显示名> --data-dir <目录> --listen <IPv4:端口> [OPTIONS]
    local-transcribe-node --from-settings

OPTIONS:
    --name <显示名>               Display name announced over mDNS (required)
    --data-dir <目录>             Task storage directory (required)
    --listen <IP:PORT>            Any non-loopback IPv4, or 0.0.0.0 for all interfaces
                                  (required; no default). 127.0.0.1 is for tests.
    --netmask <掩码>              Subnet mask. Required for normal LAN addresses;
                                  optional for 0.0.0.0, loopback and Tailscale 100.64/10.
                                  /31 and /32 point-to-point links filter no peers.
    --compute on|off              Transcribe locally (default off; 'on' needs a model)
    --role gpu|cpu|auto           Compute role advertised to peers (default auto)
    --models-dir <应用数据目录>   Serve every model under <目录>/models
    --model-dir <模型目录>        Serve one model; takes precedence over --models-dir
    --selected-model <文件>       selected-model.json; that model is served first
    --media-root <目录>           Only paths inside this root may be written by collect
    --retention-file <文件>       Terminal-task cleanup policy JSON (optional)
    --parent-pid <pid>            Exit when the parent process disappears
    --idle-unload <秒>            Free the loaded model after this idle time;
                                  one of 20 / 120 / 600 / 1800 / 7200 (default 120)
    --from-settings               Read name, listen address, models dir and idle
                                  unload from the app's lan-settings.json;
                                  explicit flags above still win
    -h, --help                    Show this help";

const STATE_FILE: &str = "lan-node.json";
const PARENT_POLL: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct Args {
    name: String,
    data_dir: PathBuf,
    listen: SocketAddr,
    netmask: Option<Ipv4Addr>,
    compute: bool,
    role: String,
    model_dir: Option<PathBuf>,
    models_dir: Option<PathBuf>,
    selected_model: Option<PathBuf>,
    media_root: Option<PathBuf>,
    parent_pid: Option<u32>,
    idle_unload: u64,
    retention_file: Option<PathBuf>,
    /// `--from-settings` 模式：节点需在 SSH 断开后继续存活，因此忽略 SIGHUP
    /// 并把 stdio 重定向到日志（等价 nohup），见 [`detach_from_terminal`]。
    from_settings: bool,
}

/// Written to `<data-dir>/lan-node.json` so a second launch can detect a live
/// node instead of silently competing for the same storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct NodeState {
    pid: u32,
    name: String,
    address: String,
    mask: String,
    subnet_filter: String,
    port: u16,
    version: String,
    started_at_unix_ms: u64,
    served_models: Vec<String>,
}

fn main() {
    // 并发写设置的测试会重新执行本二进制，在这里分叉出去；正常运行不受影响。
    voice_to_text_lan::settings::settings_test_helper();

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

    // `--from-settings` 常经 SSH 拉起：断开连接后必须继续存活，因此先忽略
    // SIGHUP 并把 stdio 落到日志，避免远端 shell 退出时管道写入失败带崩节点。
    if args.from_settings {
        detach_from_terminal(&args.data_dir);
    }

    if let Err(error) = voice_to_text_core::set_idle_unload(args.idle_unload) {
        eprintln!("error: {error}");
        std::process::exit(2);
    }

    if let Err(error) = refuse_if_another_node_runs(&args.data_dir) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }

    let models = match collect_models(&args) {
        Ok(models) => models,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };
    if args.compute && models.is_empty() {
        eprintln!(
            "error: --compute on needs at least one usable model (--models-dir or --model-dir)"
        );
        std::process::exit(1);
    }

    let config = NodeConfig {
        name: args.name.clone(),
        data_dir: args.data_dir.clone(),
        listen: args.listen,
        compute_enabled: args.compute,
        role: args.role.clone(),
        model_dir: None,
        media_root: args.media_root.clone(),
        retention_policy: args
            .retention_file
            .as_deref()
            .and_then(voice_to_text_lan::retention::read_policy),
    };

    let served_models = models
        .iter()
        .map(|model| model.id.clone())
        .collect::<Vec<_>>();
    let node = match start_node(config, args.netmask, models) {
        Ok(node) => node,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };

    let state = NodeState {
        pid: std::process::id(),
        name: args.name.clone(),
        address: args.listen.ip().to_string(),
        mask: effective_mask(args.listen, args.netmask).to_string(),
        subnet_filter: subnet_filter_label(args.listen, args.netmask).to_owned(),
        port: node.port(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        started_at_unix_ms: now_ms(),
        served_models,
    };
    if let Err(error) = write_state(&args.data_dir, &state) {
        eprintln!(
            "error: cannot write {}: {error}",
            state_path(&args.data_dir).display()
        );
        node.shutdown();
        node.wait();
        std::process::exit(1);
    }

    println!(
        "node \"{}\" listening on {}:{} (compute: {})",
        args.name,
        args.listen.ip(),
        node.port(),
        if args.compute { "on" } else { "off" }
    );

    let stop = Arc::new(AtomicBool::new(false));
    install_ctrl_c(stop.clone());
    if let Some(parent) = args.parent_pid {
        install_parent_watchdog(parent, stop.clone());
    }
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(200));
    }

    eprintln!("shutting down…");
    node.shutdown();
    node.wait();
    remove_state(&args.data_dir);
}

/// Start with subnet scoping for private LAN addresses, without scoping for
/// loopback (127.0.0.1), which is only reachable from this machine.
fn start_node(
    config: NodeConfig,
    netmask: Option<Ipv4Addr>,
    models: Vec<LocalModel>,
) -> Result<RunningNode, String> {
    let IpAddr::V4(address) = config.listen.ip() else {
        return Err("--listen must be an IPv4 address.".to_owned());
    };
    // Loopback and wildcard binds are not scoped: loopback is test-only, and
    // 0.0.0.0 explicitly means every interface.
    if address.is_loopback() || address.is_unspecified() {
        return NodeServer::start_with_models(config, models).map_err(|error| error.to_string());
    }
    let mask = effective_mask(config.listen, netmask);
    NodeServer::start_on_lan_with_models(config, mask, models).map_err(|error| error.to_string())
}

/// Mask used for scoping. A missing mask is only accepted for point-to-point
/// Tailscale addresses (100.64.0.0/10), which behave as /32.
fn effective_mask(_listen: SocketAddr, netmask: Option<Ipv4Addr>) -> Ipv4Addr {
    netmask.unwrap_or(Ipv4Addr::new(255, 255, 255, 255))
}

/// Peer filtering reported in the state file: `same_subnet` for /1–/30,
/// `none` for point-to-point masks and unscoped binds.
fn subnet_filter_label(listen: SocketAddr, netmask: Option<Ipv4Addr>) -> &'static str {
    if listen.ip().is_unspecified() {
        return "none";
    }
    if u32::from(effective_mask(listen, netmask)).leading_ones() >= 31 {
        "none"
    } else {
        "same_subnet"
    }
}

#[derive(Debug)]
enum ParseOutcome {
    Help,
    Error(String),
}

fn parse_args<I: Iterator<Item = String>>(mut arguments: I) -> Result<Args, ParseOutcome> {
    let mut name = None;
    let mut data_dir = None;
    let mut listen = None;
    let mut netmask = None;
    let mut compute: Option<bool> = None;
    let mut role: Option<String> = None;
    let mut model_dir = None;
    let mut models_dir = None;
    let mut selected_model = None;
    let mut media_root = None;
    let mut parent_pid = None;
    let mut idle_unload: Option<u64> = None;
    let mut retention_file = None;
    let mut from_settings = false;

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
            "--listen" => listen = Some(take_value("--listen")?),
            "--netmask" => {
                let value = take_value("--netmask")?;
                netmask = Some(value.parse::<Ipv4Addr>().map_err(|error| {
                    ParseOutcome::Error(format!("invalid --netmask '{value}': {error}"))
                })?);
            }
            "--compute" => {
                compute = Some(match take_value("--compute")?.as_str() {
                    "on" => true,
                    "off" => false,
                    other => {
                        return Err(ParseOutcome::Error(format!(
                            "--compute must be 'on' or 'off', got '{other}'"
                        )))
                    }
                })
            }
            "--role" => {
                let value = take_value("--role")?;
                if !matches!(value.as_str(), "gpu" | "cpu" | "auto") {
                    return Err(ParseOutcome::Error(format!(
                        "--role must be 'gpu', 'cpu' or 'auto', got '{value}'"
                    )));
                }
                role = Some(value);
            }
            "--model-dir" => model_dir = Some(PathBuf::from(take_value("--model-dir")?)),
            "--models-dir" => models_dir = Some(PathBuf::from(take_value("--models-dir")?)),
            "--selected-model" => {
                selected_model = Some(PathBuf::from(take_value("--selected-model")?))
            }
            "--media-root" => media_root = Some(PathBuf::from(take_value("--media-root")?)),
            "--retention-file" => {
                retention_file = Some(PathBuf::from(take_value("--retention-file")?))
            }
            "--idle-unload" => {
                let value = take_value("--idle-unload")?;
                let seconds = value.parse::<u64>().map_err(|error| {
                    ParseOutcome::Error(format!("invalid --idle-unload '{value}': {error}"))
                })?;
                if !voice_to_text_core::IDLE_UNLOAD_CHOICES.contains(&seconds) {
                    return Err(ParseOutcome::Error(format!(
                        "--idle-unload must be one of {:?} seconds; 'never unload' is not supported",
                        voice_to_text_core::IDLE_UNLOAD_CHOICES
                    )));
                }
                idle_unload = Some(seconds);
            }
            "--parent-pid" => {
                let value = take_value("--parent-pid")?;
                parent_pid = Some(value.parse::<u32>().map_err(|error| {
                    ParseOutcome::Error(format!("invalid --parent-pid '{value}': {error}"))
                })?);
            }
            "--from-settings" => from_settings = true,
            other => {
                return Err(ParseOutcome::Error(format!("unknown argument '{other}'")));
            }
        }
    }

    if from_settings {
        // `--from-settings`: read the same `lan-settings.json` the app writes,
        // so `lan start` (locally or over SSH) starts an identical node with
        // one fixed command. Explicit flags still win.
        let root = voice_to_text_lan::settings::default_data_dir().map_err(ParseOutcome::Error)?;
        let settings = voice_to_text_lan::settings::read(
            &root.join(voice_to_text_lan::settings::SETTINGS_FILE),
        );
        if name.is_none() {
            name = Some(settings.host_name.clone());
        }
        if data_dir.is_none() {
            data_dir = Some(root.join("lan-host"));
        }
        if listen.is_none() {
            if settings.host_address.trim().is_empty() {
                return Err(ParseOutcome::Error(
                    "The node has no sharing interface yet; choose one in the app first."
                        .to_owned(),
                ));
            }
            listen = Some(format!("{}:{}", settings.host_address, settings.host_port));
        }
        if models_dir.is_none() {
            models_dir = Some(root.clone());
        }
        if selected_model.is_none() {
            selected_model = Some(root.join("selected-model.json"));
        }
        if retention_file.is_none() {
            retention_file = Some(root.join("lan-host-retention.json"));
        }
        if compute.is_none() {
            compute = Some(true);
        }
        if role.is_none() {
            role = Some("cpu".to_owned());
        }
        if idle_unload.is_none() {
            idle_unload = Some(settings.idle_unload_node);
        }
        // The subnet mask comes from the connected interfaces, mirroring the
        // app's own start path; wildcard and loopback binds need none.
        if netmask.is_none() {
            let address = listen
                .as_deref()
                .and_then(|raw| raw.parse::<SocketAddr>().ok())
                .and_then(|socket| match socket.ip() {
                    IpAddr::V4(ip) => Some(ip),
                    IpAddr::V6(_) => None,
                })
                .filter(|ip| !ip.is_unspecified() && !ip.is_loopback());
            if let Some(address) = address {
                match interface_mask(address) {
                    Some(mask) => netmask = Some(mask),
                    None if is_tailscale_cgnat(address) => {}
                    None => {
                        return Err(ParseOutcome::Error(format!(
                            "No connected interface has the address {address}; connect it or pick another address in the app."
                        )))
                    }
                }
            }
        }
    }

    let name = name.ok_or_else(|| ParseOutcome::Error("--name is required".to_owned()))?;
    let data_dir =
        data_dir.ok_or_else(|| ParseOutcome::Error("--data-dir is required".to_owned()))?;
    let raw_listen = listen.ok_or_else(|| {
        ParseOutcome::Error(
            "--listen is required, e.g. --listen 192.168.1.10:7878 (0.0.0.0 is not allowed)"
                .to_owned(),
        )
    })?;
    let listen = raw_listen.parse::<SocketAddr>().map_err(|error| {
        ParseOutcome::Error(format!("invalid --listen '{raw_listen}': {error}"))
    })?;
    validate_listen(listen, netmask)?;

    Ok(Args {
        name,
        data_dir,
        listen,
        netmask,
        compute: compute.unwrap_or(false),
        role: role.unwrap_or_else(|| "auto".to_owned()),
        model_dir,
        models_dir,
        selected_model,
        media_root,
        parent_pid,
        idle_unload: idle_unload.unwrap_or(voice_to_text_core::DEFAULT_IDLE_UNLOAD_SECONDS),
        retention_file,
        from_settings,
    })
}

/// `--from-settings` 模式下把节点与启动它的终端（通常是 SSH 会话）解绑：
/// 忽略 SIGHUP，stdin 指向 /dev/null，stdout/stderr 追加到
/// `<data_dir>/lan-node.log`。这样远端 shell 退出后节点继续监听，
/// 且 `lan stop` 之前的真实失败原因仍留在日志里可查。
#[cfg(unix)]
fn detach_from_terminal(data_dir: &std::path::Path) {
    use std::os::unix::io::AsRawFd as _;
    // SAFETY: 三个调用都只操作本进程的信号表、会话与描述符，参数均为有效常量/文件描述符。
    unsafe {
        // setsid 脱离 sshd 会话：此后 sshd 对旧会话进程组的 SIGHUP→SIGKILL 升级
        // 清理不再波及本节点，节点可独立存活到 `lan stop`。
        libc::setsid();
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    let log_path = data_dir.join("lan-node.log");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);
    let devnull = std::fs::OpenOptions::new().read(true).open("/dev/null");
    if let (Ok(log), Ok(devnull)) = (log, devnull) {
        // SAFETY: 文件描述符来自刚打开的有效文件，dup2 失败仅表示重定向未生效，
        // 不影响后续运行（最坏情况是日志仍写到原来的 stderr）。
        unsafe {
            libc::dup2(devnull.as_raw_fd(), 0);
            libc::dup2(log.as_raw_fd(), 1);
            libc::dup2(log.as_raw_fd(), 2);
        }
    }
}

#[cfg(not(unix))]
fn detach_from_terminal(_data_dir: &std::path::Path) {}

/// Subnet mask of the connected interface holding `address`, from getifaddrs.
#[cfg(unix)]
fn interface_mask(address: Ipv4Addr) -> Option<Ipv4Addr> {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs owns the list until freeifaddrs.
    unsafe {
        if libc::getifaddrs(&mut head) != 0 {
            return None;
        }
        let mut found = None;
        let mut cursor = head;
        while !cursor.is_null() {
            let entry = &*cursor;
            if !entry.ifa_addr.is_null()
                && (*entry.ifa_addr).sa_family as i32 == libc::AF_INET
                && entry.ifa_flags & libc::IFF_UP as u32 != 0
            {
                let candidate = Ipv4Addr::from(
                    (*(entry.ifa_addr as *const libc::sockaddr_in))
                        .sin_addr
                        .s_addr
                        .to_ne_bytes(),
                );
                if candidate == address && !entry.ifa_netmask.is_null() {
                    found = Some(Ipv4Addr::from(
                        (*(entry.ifa_netmask as *const libc::sockaddr_in))
                            .sin_addr
                            .s_addr
                            .to_ne_bytes(),
                    ));
                    break;
                }
            }
            cursor = entry.ifa_next;
        }
        libc::freeifaddrs(head);
        found
    }
}

#[cfg(not(unix))]
fn interface_mask(_address: Ipv4Addr) -> Option<Ipv4Addr> {
    None
}

/// The listen address is the substitute for authentication: it must be a
/// private IPv4 on this machine, and a LAN address needs its subnet mask so
/// `LanScope` can reject peers outside it. 0.0.0.0 and public addresses are
/// rejected outright.
fn validate_listen(listen: SocketAddr, netmask: Option<Ipv4Addr>) -> Result<(), ParseOutcome> {
    let IpAddr::V4(address) = listen.ip() else {
        return Err(ParseOutcome::Error(
            "--listen must be an IPv4 address.".to_owned(),
        ));
    };
    // Loopback is test-only; 0.0.0.0 is the user's explicit "all interfaces"
    // choice and is never inferred.
    if address.is_loopback() || address.is_unspecified() {
        return Ok(());
    }
    match netmask {
        Some(mask) => {
            if mask == Ipv4Addr::UNSPECIFIED || !is_contiguous_mask(mask) {
                return Err(ParseOutcome::Error(format!(
                    "--netmask {mask} is not a contiguous subnet mask."
                )));
            }
            Ok(())
        }
        None if is_tailscale_cgnat(address) => Ok(()),
        None => Err(ParseOutcome::Error(
            "--netmask is required for this address: it scopes the node to one subnet, which is what replaces authentication here. Only 0.0.0.0, loopback and Tailscale 100.64/10 may omit it."
                .to_owned(),
        )),
    }
}

/// Tailscale hands out 100.64.0.0/10 (CGNAT) addresses on its utun link.
fn is_tailscale_cgnat(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

/// True when the mask's bits are ones followed by zeros (e.g. 255.255.255.0).
fn is_contiguous_mask(mask: Ipv4Addr) -> bool {
    let inverted = !u32::from(mask);
    inverted & inverted.wrapping_add(1) == 0
}

/// Build the served model list: library order, the saved selection first
/// (when present in the library), then `--model-dir` first and de-duplicated.
fn collect_models(args: &Args) -> Result<Vec<LocalModel>, String> {
    let mut models = match &args.models_dir {
        Some(directory) => ModelLibrary::new(directory)
            .list()?
            .into_iter()
            .map(|entry| entry.model)
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };
    if let Some(path) = &args.selected_model {
        if let Some(selected) = read_selected_model_file(path) {
            if let Some(position) = models.iter().position(|model| model.id == selected.id) {
                let model = models.remove(position);
                models.insert(0, model);
            }
        }
    }
    if let Some(directory) = &args.model_dir {
        let model = inspect_model_directory(directory)?;
        models.retain(|entry| entry.id != model.id);
        models.insert(0, model);
    }
    Ok(models)
}

/// A missing or unreadable selection is not fatal: the library order stands.
fn read_selected_model_file(path: &Path) -> Option<LocalModel> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    serde_json::from_slice::<LocalModel>(&bytes).ok()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn state_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STATE_FILE)
}

fn write_state(data_dir: &Path, state: &NodeState) -> Result<(), String> {
    let path = state_path(data_dir);
    let temporary = data_dir.join(format!(".{STATE_FILE}.{}.tmp", std::process::id()));
    let payload = serde_json::to_vec_pretty(state).map_err(|error| error.to_string())?;
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&payload)
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    std::fs::rename(&temporary, &path).map_err(|error| error.to_string())
}

fn remove_state(data_dir: &Path) {
    let path = state_path(data_dir);
    if path.exists() {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) => eprintln!("warning: cannot remove {}: {error}", path.display()),
        }
    }
}

fn refuse_if_another_node_runs(data_dir: &Path) -> Result<(), String> {
    let path = state_path(data_dir);
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(());
    };
    let Ok(state) = serde_json::from_slice::<NodeState>(&bytes) else {
        // A corrupt or stale record is replaced after a successful start.
        return Ok(());
    };
    if pid_alive(state.pid) {
        return Err(format!(
            "another node is already running in this data directory (pid {}, port {})",
            state.pid, state.port
        ));
    }
    Ok(())
}

/// Best-effort liveness probe used for stale-state detection and the parent
/// watchdog. Never used to send signals.
fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        // SAFETY: signal 0 only performs the permission/existence check.
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut code) != 0;
            CloseHandle(handle);
            ok && code == STILL_ACTIVE as u32
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        true
    }
}

fn install_ctrl_c(stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("warning: cannot build signal runtime ({error}); SIGINT will terminate");
                return;
            }
        };
        runtime.block_on(async {
            if tokio::signal::ctrl_c().await.is_ok() {
                stop.store(true, Ordering::SeqCst);
            }
        });
    });
}

/// Poll the parent pid; when it disappears, stop the node so a GUI crash does
/// not leave an orphan holding the port and the model in memory.
fn install_parent_watchdog(parent: u32, stop: Arc<AtomicBool>) {
    std::thread::Builder::new()
        .name("parent-watchdog".to_owned())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                if !pid_alive(parent) {
                    eprintln!("parent process {parent} is gone; stopping the node");
                    stop.store(true, Ordering::SeqCst);
                    return;
                }
                std::thread::sleep(PARENT_POLL);
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &[&str]) -> Result<Args, ParseOutcome> {
        parse_args(raw.iter().map(|value| (*value).to_owned()))
    }

    fn base(listen: &str, extra: &[&str]) -> Vec<String> {
        let mut args = vec![
            "--name".to_owned(),
            "test-node".to_owned(),
            "--data-dir".to_owned(),
            "/tmp/lan-node-test".to_owned(),
            "--listen".to_owned(),
            listen.to_owned(),
        ];
        args.extend(extra.iter().map(|value| (*value).to_owned()));
        args
    }

    fn parse_vec(args: Vec<String>) -> Result<Args, ParseOutcome> {
        parse_args(args.into_iter())
    }

    #[test]
    fn listen_is_required() {
        let error = parse(&["--name", "n", "--data-dir", "/tmp/x"]).unwrap_err();
        match error {
            ParseOutcome::Error(message) => assert!(message.contains("--listen is required")),
            ParseOutcome::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn wildcard_and_public_interface_addresses_are_allowed() {
        // Addendum V1.0: every non-loopback IPv4 interface, plus an explicit
        // "all interfaces" bind, is allowed. A missing netmask is fine for
        // 0.0.0.0 because no scope can apply.
        assert!(parse_vec(base("0.0.0.0:7878", &[])).is_ok());
        assert!(parse_vec(base("8.8.8.8:7878", &["--netmask", "255.255.255.0"])).is_ok());
    }

    #[test]
    fn tailscale_address_may_omit_the_netmask() {
        let args = parse_vec(base("100.64.0.1:7878", &[])).unwrap();
        assert_eq!(args.netmask, None);
        assert_eq!(subnet_filter_label(args.listen, args.netmask), "none");
    }

    #[test]
    fn wildcard_reports_no_subnet_filter() {
        let args = parse_vec(base("0.0.0.0:7878", &[])).unwrap();
        assert_eq!(subnet_filter_label(args.listen, args.netmask), "none");
        assert_eq!(
            effective_mask(args.listen, args.netmask).to_string(),
            "255.255.255.255"
        );
    }

    #[test]
    fn a_normal_lan_address_reports_the_same_subnet_filter() {
        let args = parse_vec(base("192.168.1.10:7878", &["--netmask", "255.255.255.0"])).unwrap();
        assert_eq!(
            subnet_filter_label(args.listen, args.netmask),
            "same_subnet"
        );
    }

    #[test]
    fn point_to_point_masks_report_no_filter() {
        for mask in ["255.255.255.255", "255.255.255.254"] {
            let args = parse_vec(base("100.64.0.1:7878", &["--netmask", mask])).unwrap();
            assert_eq!(
                subnet_filter_label(args.listen, args.netmask),
                "none",
                "{mask}"
            );
        }
    }

    #[test]
    fn lan_address_requires_a_netmask() {
        let error = parse_vec(base("192.168.1.10:7878", &[])).unwrap_err();
        match error {
            ParseOutcome::Error(message) => {
                assert!(message.contains("--netmask is required"), "{message}")
            }
            ParseOutcome::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn loopback_may_omit_the_netmask() {
        let args = parse_vec(base("127.0.0.1:7878", &[])).unwrap();
        assert!(args.netmask.is_none());
    }

    #[test]
    fn lan_address_with_netmask_is_accepted() {
        let args = parse_vec(base("192.168.1.10:7878", &["--netmask", "255.255.255.0"])).unwrap();
        assert_eq!(args.netmask, Some(Ipv4Addr::new(255, 255, 255, 0)));
    }

    #[test]
    fn non_contiguous_netmask_is_refused() {
        let error =
            parse_vec(base("192.168.1.10:7878", &["--netmask", "255.0.255.0"])).unwrap_err();
        match error {
            ParseOutcome::Error(message) => assert!(message.contains("contiguous"), "{message}"),
            ParseOutcome::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn idle_unload_defaults_to_two_minutes() {
        let args = parse_vec(base("127.0.0.1:7878", &[])).unwrap();
        assert_eq!(args.idle_unload, 120);
    }

    #[test]
    fn idle_unload_accepts_a_closed_value_and_rejects_others() {
        let args = parse_vec(base("127.0.0.1:7878", &["--idle-unload", "600"])).unwrap();
        assert_eq!(args.idle_unload, 600);
        for bad in ["0", "999999", "-1", "abc"] {
            let error = parse_vec(base("127.0.0.1:7878", &["--idle-unload", bad])).unwrap_err();
            match error {
                ParseOutcome::Error(message) => assert!(
                    message.contains("idle-unload"),
                    "unexpected message for {bad}: {message}"
                ),
                ParseOutcome::Help => panic!("unexpected help"),
            }
        }
    }

    #[test]
    fn parent_pid_must_be_a_number() {
        let error = parse_vec(base("127.0.0.1:7878", &["--parent-pid", "not-a-pid"])).unwrap_err();
        match error {
            ParseOutcome::Error(message) => assert!(message.contains("--parent-pid")),
            ParseOutcome::Help => panic!("unexpected help"),
        }
    }

    fn fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "lan-node-models-{}-{}-{}",
            std::process::id(),
            now_ms(),
            name
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn model_dir(root: &Path, name: &str) -> PathBuf {
        let directory = root.join("models").join(name);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("model.int8.onnx"), b"fixture").unwrap();
        std::fs::write(directory.join("tokens.txt"), b"a 1\n").unwrap();
        directory
    }

    #[test]
    fn library_scan_returns_every_model_and_selection_first() {
        let root = fixture("library");
        model_dir(&root, "sherpa-onnx-paraformer-zh-a");
        model_dir(&root, "sherpa-onnx-zipformer-ctc-zh-b");
        let selected =
            inspect_model_directory(root.join("models/sherpa-onnx-zipformer-ctc-zh-b")).unwrap();
        let selected_path = root.join("selected-model.json");
        std::fs::write(&selected_path, serde_json::to_vec(&selected).unwrap()).unwrap();

        let args = parse_vec(base(
            "127.0.0.1:7878",
            &[
                "--models-dir",
                root.to_str().unwrap(),
                "--selected-model",
                selected_path.to_str().unwrap(),
            ],
        ))
        .unwrap();
        let models = collect_models(&args).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, selected.id);
    }

    #[test]
    fn model_dir_wins_and_is_not_duplicated() {
        let root = fixture("precedence");
        let library_model = model_dir(&root, "sherpa-onnx-paraformer-zh-a");
        let standalone = model_dir(&root, "sherpa-onnx-zipformer-ctc-zh-b");
        let args = parse_vec(base(
            "127.0.0.1:7878",
            &[
                "--models-dir",
                root.to_str().unwrap(),
                "--model-dir",
                standalone.to_str().unwrap(),
            ],
        ))
        .unwrap();
        let models = collect_models(&args).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(
            models[0].id,
            inspect_model_directory(&standalone).unwrap().id
        );
        assert!(models
            .iter()
            .any(|model| model.id == inspect_model_directory(&library_model).unwrap().id));
    }

    #[test]
    fn empty_library_yields_no_models() {
        let root = fixture("empty");
        let args = parse_vec(base(
            "127.0.0.1:7878",
            &["--models-dir", root.to_str().unwrap()],
        ))
        .unwrap();
        assert!(collect_models(&args).unwrap().is_empty());
    }

    #[test]
    fn state_file_round_trips_with_owner_only_permissions() {
        let root = fixture("state");
        let state = NodeState {
            pid: std::process::id(),
            name: "n".into(),
            address: "192.168.1.10".into(),
            mask: "255.255.255.0".into(),
            subnet_filter: "same_subnet".into(),
            port: 7878,
            version: "1.2.0-beta.1".into(),
            started_at_unix_ms: 1,
            served_models: vec!["x".into()],
        };
        write_state(&root, &state).unwrap();
        let read: NodeState =
            serde_json::from_slice(&std::fs::read(state_path(&root)).unwrap()).unwrap();
        assert_eq!(read, state);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(state_path(&root))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        remove_state(&root);
        assert!(!state_path(&root).exists());
    }

    #[test]
    fn a_live_record_blocks_a_second_start_and_a_dead_one_does_not() {
        let root = fixture("stale");
        let live = NodeState {
            pid: std::process::id(),
            name: "n".into(),
            address: "127.0.0.1".into(),
            mask: "255.255.255.255".into(),
            subnet_filter: "none".into(),
            port: 7878,
            version: "1.2.0-beta.1".into(),
            started_at_unix_ms: 1,
            served_models: Vec::new(),
        };
        write_state(&root, &live).unwrap();
        assert!(refuse_if_another_node_runs(&root).is_err());

        let dead = NodeState {
            pid: 4_000_000,
            ..live
        };
        write_state(&root, &dead).unwrap();
        assert!(refuse_if_another_node_runs(&root).is_ok());
    }

    #[test]
    fn current_process_is_alive_and_an_unused_pid_is_not() {
        assert!(pid_alive(std::process::id()));
        assert!(!pid_alive(4_000_000));
        assert!(!pid_alive(0));
    }
}
