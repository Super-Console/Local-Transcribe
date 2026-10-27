//! LAN 设置与本机/远端节点启停，供 CLI 与 MCP 复用的薄客户端语义。
//!
//! 所有对 `lan-settings.json` 的读写都走共享的 `voice_to_text_lan::settings`
//! （与 GUI 同一份实现），因此 fs2 文件锁串行化 GUI 与 CLI/MCP 的并发写。
//!
//! host 是任意字符串（IP / `.local` / tailnet 名 / 任意 DNS），连接时直接拼
//! `http://{host}:{port}`，不做 IP 解析。远程启停走 SSH 固定命令模板。

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
// Arc/Mutex 在 unix 的 stderr 捕获和 Windows 的同名 stub 里都用得到。
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use voice_to_text_lan::client::{discover_with_scan, LanClient, DEFAULT_NODE_PORT};
use voice_to_text_lan::settings::{
    assign_missing_ids, find_node, is_safe_node_binary, is_safe_ssh_target, read, update, LanMode,
    LanNodeConfig, SETTINGS_FILE,
};

use crate::Response;

/// `add` / `set` 探测 `/api/v1/capabilities` 的超时。
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// `status` 逐节点探测健康的超时（短于 add，只为给出可达性）。
const STATUS_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
/// 启动后轮询能力的上限（任务书 §8 测试 10 的「不挂住」由 SSH BatchMode 保证）。
const START_POLL_TIMEOUT: Duration = Duration::from_secs(60);
/// 停止后等待进程退出的上限；超过则报告「仍在运行」。
const STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// 应用数据目录下的 `lan-settings.json`。
fn settings_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join(SETTINGS_FILE)
}

/// 节点状态文件 `<data-dir>/lan-host/lan-node.json`（`--from-settings` 写入处）。
fn node_state_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("lan-host").join("lan-node.json")
}

/// 节点日志 `<data-dir>/lan-host/lan-node.log`（`--from-settings` 的 stdio 落点）。
fn node_log_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("lan-host").join("lan-node.log")
}

/// 探测一个节点的能力；成功返回 capabilities JSON，失败返回可读原因。
///
/// `host` 直接拼进 `http://{host}:{port}`，不做 IP 解析（任务书 §2）。
/// 把「监听所有网卡」的写法映射成本机可连的地址。
///
/// 节点的 `host_address` 可能是 `0.0.0.0`（对本机探测时 reqwest 连不上它），
/// 因此 add / set / start 轮询都走这一处映射，行为一致。
fn probe_host(host: &str) -> &str {
    if host.trim() == "0.0.0.0" {
        "127.0.0.1"
    } else {
        host
    }
}

fn probe_capabilities(host: &str, port: u16, timeout: Duration) -> Result<Value, String> {
    let client = LanClient::new(probe_host(host).to_owned(), port);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Cannot start the network probe: {error}"))?;
    // 探测本身要在 async 里做超时，但错误要落回 String（LanError 不实现
    // From<String>），所以超时/失败都在 async 里折叠成 String。
    runtime.block_on(async {
        match tokio::time::timeout(timeout, client.capabilities()).await {
            Ok(Ok(capabilities)) => Ok(serde_json::to_value(&capabilities).unwrap_or(Value::Null)),
            Ok(Err(error)) => Err(format!(
                "The node at {host}:{port} is unreachable: {error}."
            )),
            Err(_) => Err(format!("The node at {host}:{port} did not answer in time.")),
        }
    })
}

/// 从设置文件读出某个节点（按 id 或名称），用于启停。
///
/// 错误直接就是可返回给 CLI/MCP 的 `Response`（与其他 LAN 处理函数一致），
/// 这里不为了 clippy 的尺寸阈值再包一层 Box。
#[allow(clippy::result_large_err)]
fn load_node(data_dir: &Path, selector: &str) -> Result<LanNodeConfig, Response> {
    let settings = read(&settings_path(data_dir));
    find_node(&settings, selector)
        .cloned()
        .map_err(|error| Response::failure("node_not_found", error))
}

/// 校验 `add` / `set` 的 host：非空、无 scheme、可安全进入 SSH 模板。
///
/// 拒绝 `://`（如 `ssh://x`，测试 7）与 shell 元字符/空白/控制字符（如
/// `x; rm -rf /`，测试 11）——这些字段会进入 SSH 命令模板，必须先挡住。
fn validate_add_host(host: &str) -> Result<(), String> {
    if host.trim().is_empty() {
        return Err("A LAN node needs a host name or address.".to_owned());
    }
    if host.contains("://") {
        return Err(format!(
            "'{host}' contains a URL scheme; give a plain host (IP, .local, tailnet or DNS name), not a URL."
        ));
    }
    if !is_safe_ssh_target(host) {
        return Err(format!(
            "'{host}' contains characters that are not allowed in a host (whitespace, control or shell metacharacters)."
        ));
    }
    Ok(())
}

/// 读取并解析本机节点状态文件里的 pid；文件缺失或无 pid 返回 None。
fn read_local_pid(data_dir: &Path) -> Option<u32> {
    let bytes = fs::read(node_state_path(data_dir)).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    value
        .get("pid")
        .and_then(Value::as_u64)
        .map(|pid| pid as u32)
}

/// 本机节点是否仍在运行（pid 存活）。
fn local_node_running(data_dir: &Path) -> bool {
    read_local_pid(data_dir).is_some_and(pid_alive)
}

/// 把 pid 标记为存活（`kill(pid, 0)` 探测，不实际发信号）。
#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // SAFETY: kill 以 0 信号只做存活探测；返回 -1 且 errno==EPERM 也算存在。
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    false
}

/// 向本机 pid 发 SIGINT（不用 SIGKILL，任务书 §7.1）。
///
/// 节点只在 SIGINT 时走干净退出（自己删状态文件、撤回 mDNS 记录）；与 GUI 的
/// `NodeSupervisor::stop` 完全一致，避免两条停止路径表现不同。
#[cfg(unix)]
fn terminate_pid(pid: u32) -> Result<(), String> {
    // SAFETY: kill 发 SIGINT 给已解析出的 u32 pid。
    let result = unsafe { libc::kill(pid as i32, libc::SIGINT) };
    if result == 0 {
        Ok(())
    } else {
        Err(format!("Could not ask process {pid} to stop."))
    }
}

#[cfg(not(unix))]
fn terminate_pid(_pid: u32) -> Result<(), String> {
    Err("Stopping the local node is only supported on macOS.".to_owned())
}

/// 把残留的节点状态文件移入废纸篓（本项目的删除约定：永不永久删除）。
///
/// 节点正常收到 SIGINT 时会自己删掉状态文件；只有被硬杀时才会留下残留，此时
/// 由这里收尾。与 GUI 的 `retire_state` 语义一致。
fn retire_state_file(data_dir: &Path) {
    let path = node_state_path(data_dir);
    if !path.exists() {
        return;
    }
    match voice_to_text_core::move_to_trash(&path) {
        Ok(()) => {}
        Err(error) => eprintln!(
            "warning: stale node state left at {} ({error})",
            path.display()
        ),
    }
}

/// 取日志文件末尾若干行，用于启动失败时给出真实原因。
fn log_tail(path: &Path, lines: usize) -> String {
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

/// 组装一条 SSH 命令：`BatchMode=yes` 保证不挂密码提示，`ConnectTimeout` 保证
/// 快速失败。目标与远端命令都经安全校验后作为独立 argv 传入，不经本地 shell。
fn ssh_command(target: &str, remote: &str) -> Command {
    let mut command = Command::new("ssh");
    command
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg(target)
        .arg(remote);
    command
}

/// 远程 `lan-node.json` 的固定路径（远端机器用应用默认数据目录）。
const REMOTE_STATE: &str =
    "\"$HOME/Library/Application Support/com.timmy.voicetotextapp/lan-host/lan-node.json\"";
/// 远程节点日志的固定路径。
const REMOTE_LOG: &str =
    "\"$HOME/Library/Application Support/com.timmy.voicetotextapp/lan-host/lan-node.log\"";

// ---------------------------------------------------------------------------
// Settings mutations
// ---------------------------------------------------------------------------

/// `lan add <name> <host>[:port]`：探测能力后保存，附能力快照。
pub fn add(data_dir: &Path, name: &str, host: &str, port: Option<u16>, force: bool) -> Response {
    let name = name.trim();
    if name.is_empty() {
        return Response::failure("invalid_name", "A LAN node needs a name.");
    }
    if let Err(error) = validate_add_host(host) {
        return Response::failure("invalid_host", error);
    }
    let host = host.trim();
    let port = port.unwrap_or(DEFAULT_NODE_PORT);

    let name = name.to_owned();
    let host_owned = host.to_owned();
    let probe = probe_capabilities(host, port, PROBE_TIMEOUT);
    let capabilities = match &probe {
        Ok(capabilities) => Some(capabilities.clone()),
        Err(error) => {
            if !force {
                // 不可达时给出可读原因并提示 --force；并点出 ssh 别名按发起端填写。
                return Response::failure(
                    "unreachable",
                    format!(
                        "{error} Add --force to save it anyway. Note: the optional 'ssh' field (used only for remote start/stop) is filled per initiating machine — do not assume the same ssh aliases exist on both ends."
                    ),
                );
            }
            None
        }
    };
    let snapshot = capabilities.clone();
    let reachable = probe.is_ok();

    let outcome = update(&settings_path(data_dir), |settings| {
        let model_id = capabilities
            .as_ref()
            .and_then(|value| value.get("model_id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        settings.nodes.push(LanNodeConfig {
            id: String::new(),
            name: name.clone(),
            host: host_owned.clone(),
            port,
            model_id,
            enabled: true,
            ssh: None,
            node_binary: None,
            capabilities,
        });
        assign_missing_ids(settings);
        // 直接返回刚保存的那条记录（含分配好的 id），不再按 name+host 反查。
        settings
            .nodes
            .last()
            .cloned()
            .ok_or_else(|| "The node could not be saved.".to_owned())
    });
    match outcome {
        Ok(node) => Response::ok_data(json!({
            "node": node,
            "reachable": reachable,
            "capabilities": snapshot,
        })),
        Err(error) => Response::failure("save_failed", error),
    }
}

/// `lan remove <id|name>`：仅从本地列表移除。
pub fn remove(data_dir: &Path, selector: &str) -> Response {
    let outcome = update(&settings_path(data_dir), |settings| {
        let id = find_node_mut(settings, selector)?.id.clone();
        let index = settings
            .nodes
            .iter()
            .position(|node| node.id == id)
            .ok_or_else(|| format!("No saved LAN node matches '{selector}'."))?;
        settings.nodes.remove(index);
        Ok(())
    });
    match outcome {
        Ok(()) => Response::ok_data(json!({ "removed": selector })),
        Err(error) => Response::failure("remove_failed", error),
    }
}

/// `lan enable|disable <id|name>`：切换启用状态。
fn set_enabled(data_dir: &Path, selector: &str, enabled: bool) -> Response {
    let outcome = update(&settings_path(data_dir), |settings| {
        let node = find_node_mut(settings, selector)?;
        node.enabled = enabled;
        Ok(())
    });
    match outcome {
        Ok(()) => Response::ok_data(json!({ "node": selector, "enabled": enabled })),
        Err(error) => Response::failure("update_failed", error),
    }
}

pub fn enable(data_dir: &Path, selector: &str) -> Response {
    set_enabled(data_dir, selector, true)
}

pub fn disable(data_dir: &Path, selector: &str) -> Response {
    set_enabled(data_dir, selector, false)
}

/// `lan set <id|name> [--host H] [--port P] [--model ID] [--name N]`。
///
/// 改 host/port 会重新探测能力：可达则更新快照，不可达则清空快照并给警告，
/// 但仍视为成功（用户明确改了地址，节点暂时不可达不是错误）。
pub fn set(
    data_dir: &Path,
    selector: &str,
    host: Option<&str>,
    port: Option<u16>,
    model: Option<&str>,
    name: Option<&str>,
) -> Response {
    if let Some(host) = host {
        if let Err(error) = validate_add_host(host) {
            return Response::failure("invalid_host", error);
        }
    }
    let mut warning: Option<String> = None;
    // 只有真的改了 host/port 才重新探测；仅改 model/name 时不必联网。
    let reprobe = host.is_some() || port.is_some();
    let mut node_id = String::new();
    let mut new_endpoint: Option<(String, u16)> = None;
    let outcome = update(&settings_path(data_dir), |settings| {
        let node = find_node_mut(settings, selector)?;
        if let Some(host) = host {
            node.host = host.trim().to_owned();
        }
        if let Some(port) = port {
            node.port = port;
        }
        if let Some(model) = model {
            node.model_id = if model.is_empty() {
                None
            } else {
                Some(model.to_owned())
            };
        }
        if let Some(name) = name {
            node.name = name.trim().to_owned();
        }
        // 用 id 而不是 selector 回查：改名后旧 selector 已经对不上了。
        node_id = node.id.clone();
        if reprobe {
            new_endpoint = Some((node.host.clone(), node.port));
        }
        Ok(())
    });
    if let Err(error) = outcome {
        return Response::failure("update_failed", error);
    }

    if let Some((host, port)) = new_endpoint {
        match probe_capabilities(&host, port, PROBE_TIMEOUT) {
            Ok(capabilities) => {
                let _ = update(&settings_path(data_dir), |settings| {
                    if let Ok(node) = find_node_mut(settings, &node_id) {
                        node.capabilities = Some(capabilities);
                    }
                    Ok(())
                });
            }
            Err(error) => {
                warning = Some(format!(
                    "{error} The saved address was updated, but its capability snapshot was cleared."
                ));
                let _ = update(&settings_path(data_dir), |settings| {
                    if let Ok(node) = find_node_mut(settings, &node_id) {
                        node.capabilities = None;
                    }
                    Ok(())
                });
            }
        }
    }

    let settings = read(&settings_path(data_dir));
    let node = find_node(&settings, &node_id).cloned().ok();
    Response::ok_data(json!({ "node": node, "warning": warning }))
}

/// `lan scan [--timeout 8]`：mDNS + 子网 TCP 发现，只返回结果，绝不写设置。
pub fn scan(data_dir: &Path, timeout_secs: Option<u64>) -> Response {
    let timeout = Duration::from_secs(timeout_secs.unwrap_or(8).clamp(1, 60));
    // 与 GUI 一致：已保存节点的自定义端口也要纳入扫描（否则改过端口的节点扫不到）。
    let settings = read(&settings_path(data_dir));
    let extra_ports: Vec<u16> = settings.nodes.iter().map(|node| node.port).collect();
    let found = discover_with_scan(timeout, &extra_ports);
    let listed = found
        .into_iter()
        .map(|info| {
            json!({
                "name": info.name,
                "host": info.host,
                "port": info.port,
                "proto_version": info.proto_version,
                "compute_enabled": info.compute_enabled,
                "role": info.role,
                "model_id": info.model_id,
            })
        })
        .collect::<Vec<_>>();
    Response::ok_data(json!({ "nodes": listed }))
}

/// `mode local|prefer-lan`：设置计算方式（解析逻辑与 GUI 共享 `LanMode::parse`）。
pub fn mode(data_dir: &Path, mode: &str) -> Response {
    let parsed = match LanMode::parse(mode) {
        Ok(parsed) => parsed,
        Err(error) => return Response::failure("invalid_mode", error),
    };
    let outcome = update(&settings_path(data_dir), |settings| {
        settings.mode = parsed;
        Ok(())
    });
    match outcome {
        Ok(()) => {
            let value = match parsed {
                LanMode::LocalOnly => "local",
                LanMode::PreferLan => "prefer-lan",
            };
            Response::ok_data(json!({ "mode": value }))
        }
        Err(error) => Response::failure("save_failed", error),
    }
}

/// 可变借用定位节点（`update` 闭包内用）。
///
/// 语义与共享的 `find_node` 一致：先按 id 精确匹配，再按名称唯一匹配；名称
/// 重复时报错而不是默默取第一条（否则并发添加同名节点时会改错对象）。
fn find_node_mut<'a>(
    settings: &'a mut voice_to_text_lan::settings::LanSettings,
    selector: &str,
) -> Result<&'a mut LanNodeConfig, String> {
    let position = match settings.nodes.iter().position(|node| node.id == selector) {
        Some(position) => position,
        None => {
            let mut matches = settings
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| node.name == selector)
                .map(|(index, _)| index);
            let first = matches.next();
            match (first, matches.next()) {
                (Some(position), None) => position,
                (Some(_), Some(_)) => {
                    return Err(format!(
                        "More than one saved LAN node is named '{selector}'; use its id."
                    ));
                }
                (None, _) => {
                    return Err(format!("No saved LAN node matches '{selector}'."));
                }
            }
        }
    };
    Ok(&mut settings.nodes[position])
}

// ---------------------------------------------------------------------------
// Local node start / stop
// ---------------------------------------------------------------------------

/// 本机节点启动：以 `--from-settings` 拉起，轮询能力确认，失败给真实原因。
pub fn start_local(data_dir: &Path) -> Response {
    let settings = read(&settings_path(data_dir));
    if settings.host_address.trim().is_empty() {
        return Response::failure(
            "no_interface",
            "Choose a currently connected network interface in the app before starting the local node.",
        );
    }
    if local_node_running(data_dir) {
        return Response::failure("already_running", "The local node is already running.");
    }

    let binary = match node_binary() {
        Ok(binary) => binary,
        Err(error) => return Response::failure("node_binary", error),
    };

    // 确保日志目录存在，并清空上次日志，便于读出本次真实失败原因。
    if let Some(parent) = node_log_path(data_dir).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::remove_file(node_log_path(data_dir));

    let log = match fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(node_log_path(data_dir))
    {
        Ok(log) => log,
        Err(error) => {
            return Response::failure("log", format!("Cannot open the node log: {error}"))
        }
    };
    let log_err = match log.try_clone() {
        Ok(log_err) => log_err,
        Err(error) => {
            return Response::failure("log", format!("Cannot open the node log: {error}"))
        }
    };

    let mut child = match Command::new(&binary)
        .arg("--from-settings")
        // 让节点读到与本服务一致的数据目录里的 lan-settings.json。
        .env("VOICE_TO_TEXT_APP_DATA_DIR", data_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return Response::failure("start_failed", format!("Could not start the node: {error}"));
        }
    };

    let result = poll_until_ready(&settings.host_address, settings.host_port, &mut child);
    match result {
        Ok(()) => {
            // 节点已 setsid 脱离，本服务不持有其生命周期；用 reaper 线程回收僵尸。
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            let pid = read_local_pid(data_dir);
            // 与 GUI 的共享开关对齐：下次启动按此恢复共享。
            set_share_on_launch(data_dir, true);
            Response::ok_data(json!({
                "running": true,
                "pid": pid,
                "address": settings.host_address,
                "port": settings.host_port,
            }))
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let tail = log_tail(&node_log_path(data_dir), 20);
            let detail = if tail.is_empty() {
                error
            } else {
                format!("{error}\n{tail}")
            };
            Response::failure("start_failed", detail)
        }
    }
}

/// 轮询本机节点能力；期间子进程提前退出视为失败。
fn poll_until_ready(host: &str, port: u16, child: &mut Child) -> Result<(), String> {
    let deadline = Instant::now() + START_POLL_TIMEOUT;
    let client = LanClient::new(probe_host(host).to_owned(), port);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Cannot start the network probe: {error}"))?;
    while Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("The node exited during startup ({status})."));
        }
        let ready = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), client.health())
                .await
                .map(|result| result.unwrap_or(false))
                .unwrap_or(false)
        });
        if ready {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    Err(format!(
        "The node did not answer on {host}:{port} within {}s.",
        START_POLL_TIMEOUT.as_secs()
    ))
}

/// 本机节点停止：SIGINT 后等待；成功清状态，超时报告仍在运行。
///
/// 语义与 GUI 的共享开关一致：停止成功后写 `share_on_launch=false`，这样下次
/// 启动 GUI 不会把刚被 CLI 停掉的节点又拉起来。停止失败时不动这个标志。
pub fn stop_local(data_dir: &Path) -> Response {
    let Some(pid) = read_local_pid(data_dir) else {
        // 测试 12：不存在的 pid 报告「节点未在运行」，退出码 1，stdout 只有 JSON。
        return Response::failure("not_running", "节点未在运行");
    };
    if !pid_alive(pid) {
        // pid 已死但状态文件还在（上次被硬杀）：清掉残留，同样报告未在运行。
        retire_state_file(data_dir);
        return Response::failure("not_running", "节点未在运行");
    }
    if let Err(error) = terminate_pid(pid) {
        return Response::failure("stop_failed", error);
    }
    let deadline = Instant::now() + STOP_WAIT_TIMEOUT;
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            // 节点 SIGINT 后通常已自己删了状态文件；留下的残留移入废纸篓。
            retire_state_file(data_dir);
            set_share_on_launch(data_dir, false);
            return Response::ok_data(json!({ "running": false, "pid": pid }));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    // 10s 后仍在运行：不动 share_on_launch（共享事实上还开着），只如实报告。
    Response::ok_data(json!({
        "running": true,
        "pid": pid,
        "warning": "The node is still running 10s after being asked to stop; please stop it manually.",
    }))
}

/// 写 `share_on_launch`，让下次启动时 GUI 恢复共享与否与 CLI 的启停保持一致。
///
/// 写失败不致命：启停本身已经成功，这里只影响下次启动的自动恢复。
fn set_share_on_launch(data_dir: &Path, value: bool) {
    let path = settings_path(data_dir);
    if let Err(error) = update(&path, |settings| {
        settings.share_on_launch = value;
        Ok(())
    }) {
        eprintln!("warning: could not persist share_on_launch={value}: {error}");
    }
}

// ---------------------------------------------------------------------------
// Remote node start / stop over SSH
// ---------------------------------------------------------------------------

/// 远程节点启动：SSH 固定命令模板拉起，轮询能力确认，失败给远端日志真实原因。
pub fn start_remote(data_dir: &Path, selector: &str) -> Response {
    let node = match load_node(data_dir, selector) {
        Ok(node) => node,
        Err(response) => return response,
    };
    let target = node.ssh.clone().unwrap_or_else(|| node.host.clone());
    if !is_safe_ssh_target(&target) {
        return Response::failure(
            "unsafe_target",
            format!("The ssh target '{target}' contains characters that are not allowed."),
        );
    }
    let binary = node
        .node_binary
        .clone()
        .unwrap_or_else(default_remote_binary);
    if !is_safe_node_binary(&binary) {
        return Response::failure(
            "unsafe_binary",
            format!("The node binary path '{binary}' contains characters that are not allowed."),
        );
    }

    let remote = format!("'{binary}' --from-settings");
    let mut command = ssh_command(&target, &remote);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Response::failure(
                "ssh_failed",
                format!("Could not run ssh for '{target}': {error}."),
            );
        }
    };

    // 捕获远端 ssh stderr（节点把 stdio 落到远端日志，故这里多半是 ssh 自身的错）。
    // 非 unix 的 capture_stderr 是空 stub，保持调用点只有一处。
    let stderr_lines = capture_stderr(&mut child);

    match poll_remote_ready(&node.host, node.port, &mut child) {
        Ok(()) => {
            // 节点已 setsid + 忽略 SIGHUP + stdio 落日志，独立于本 ssh 会话存活。
            let _ = child.kill();
            let _ = child.wait();
            let pid = remote_pid(&target);
            Response::ok_data(json!({
                "running": true,
                "node": node.name,
                "pid": pid,
                "address": node.host,
                "port": node.port,
            }))
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = stderr_text(&stderr_lines);
            let remote_log = remote_log_tail(&target);
            let detail = if !remote_log.is_empty() {
                format!("{error}\n{remote_log}")
            } else if !stderr.is_empty() {
                format!("{error}\n{stderr}")
            } else {
                error
            };
            Response::failure("start_failed", detail)
        }
    }
}

/// 远程节点停止：读远端 `lan-node.json` 的 pid → `kill -INT`（不 -9），等待退出。
///
/// 与本机/GUI 一致发 SIGINT，让远端节点自己删状态文件并撤回 mDNS；存活与否用
/// `kill -0` 判断，不依赖状态文件是否还在。
pub fn stop_remote(data_dir: &Path, selector: &str) -> Response {
    let node = match load_node(data_dir, selector) {
        Ok(node) => node,
        Err(response) => return response,
    };
    let target = node.ssh.clone().unwrap_or_else(|| node.host.clone());
    if !is_safe_ssh_target(&target) {
        return Response::failure(
            "unsafe_target",
            format!("The ssh target '{target}' contains characters that are not allowed."),
        );
    }
    let Some(pid) = remote_pid(&target) else {
        return Response::failure("not_running", "节点未在运行");
    };
    if !remote_pid_alive(&target, pid) {
        return Response::failure("not_running", "节点未在运行");
    }
    let kill = format!("kill -INT {pid}");
    let _ = ssh_command(&target, &kill)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let deadline = Instant::now() + STOP_WAIT_TIMEOUT;
    while Instant::now() < deadline {
        if !remote_still_running(&target, pid) {
            return Response::ok_data(json!({ "running": false, "pid": pid }));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Response::ok_data(json!({
        "running": true,
        "pid": pid,
        "warning": "The remote node is still running 10s after being asked to stop; please stop it manually.",
    }))
}

/// `lan status`：本机节点状态 + 各已保存节点可达性。
pub fn status(data_dir: &Path) -> Response {
    let pid = read_local_pid(data_dir);
    let local_running = pid.is_some_and(pid_alive);
    let settings = read(&settings_path(data_dir));
    // 运行时只建一次；建不起来就把所有节点判为不可达，不让 status 整体失败。
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let nodes = settings
        .nodes
        .iter()
        .map(|node| {
            let reachable = match &runtime {
                Ok(runtime) => runtime.block_on(async {
                    let client = LanClient::new(probe_host(&node.host).to_owned(), node.port);
                    // status 只回答「在不在」，用更短的超时，避免 N 个节点串行累加。
                    tokio::time::timeout(STATUS_PROBE_TIMEOUT, client.health())
                        .await
                        .map(|result| result.unwrap_or(false))
                        .unwrap_or(false)
                }),
                Err(_) => false,
            };
            json!({
                "id": node.id,
                "name": node.name,
                "host": node.host,
                "port": node.port,
                "enabled": node.enabled,
                "reachable": reachable,
            })
        })
        .collect::<Vec<_>>();
    Response::ok_data(json!({
        "local": {
            "running": local_running,
            "pid": pid,
            "address": settings.host_address,
            "port": settings.host_port,
            "name": settings.host_name,
        },
        "nodes": nodes,
    }))
}

// ---------------------------------------------------------------------------
// Single-node probe (MCP `node_status`)
// ---------------------------------------------------------------------------

/// 探测一个**已保存**节点：可达性、协议版本、模型列表。
///
/// 只接受 `lan-settings.json` 里已存在的节点（id 或唯一名称），不接受任意地址
/// ——否则 MCP 就变成了一个任意内网探测器（任务书 §6 硬性约束）。
pub fn node_probe(data_dir: &Path, selector: &str) -> Response {
    let settings = read(&settings_path(data_dir));
    let node = match find_node(&settings, selector) {
        Ok(node) => node.clone(),
        Err(error) => return Response::failure("node_not_found", error),
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return Response::failure("service_error", error.to_string()),
    };
    let client = LanClient::new(probe_host(&node.host).to_owned(), node.port);
    let capabilities = runtime
        .block_on(async { tokio::time::timeout(PROBE_TIMEOUT, client.capabilities()).await });
    match capabilities {
        Ok(Ok(capabilities)) => {
            // 可达时顺带取模型列表；取不到不致命（能力信息已经在手）。
            let models = runtime
                .block_on(async { tokio::time::timeout(PROBE_TIMEOUT, client.models()).await })
                .ok()
                .and_then(Result::ok)
                .map(|models| {
                    models
                        .into_iter()
                        .map(|model| json!({ "id": model.id, "display_name": model.display_name }))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Response::ok_data(json!({
                "id": node.id,
                "name": node.name,
                "host": node.host,
                "port": node.port,
                "enabled": node.enabled,
                "reachable": true,
                "protocol_version": capabilities.proto_version,
                "compute_enabled": capabilities.compute_enabled,
                "model_id": capabilities.model_id,
                "models": models,
            }))
        }
        Ok(Err(error)) => Response::ok_data(json!({
            "id": node.id,
            "name": node.name,
            "host": node.host,
            "port": node.port,
            "enabled": node.enabled,
            "reachable": false,
            "error": error.to_string(),
        })),
        Err(_) => Response::ok_data(json!({
            "id": node.id,
            "name": node.name,
            "host": node.host,
            "port": node.port,
            "enabled": node.enabled,
            "reachable": false,
            "error": format!("The node at {}:{} did not answer in time.", node.host, node.port),
        })),
    }
}

// ---------------------------------------------------------------------------
// SSH plumbing
// ---------------------------------------------------------------------------

/// 从远端 `lan-node.json` 读 pid（固定路径模板，无用户输入）。
fn remote_pid(target: &str) -> Option<u32> {
    let command = format!("cat {REMOTE_STATE}");
    let output = ssh_command(target, &command)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value: Value = serde_json::from_slice(&output.stdout).ok()?;
    value
        .get("pid")
        .and_then(Value::as_u64)
        .map(|pid| pid as u32)
}

/// 远端某 pid 是否仍在运行（`kill -0`；pid 来自远端状态文件的 JSON 数字，非用户字符串）。
fn remote_pid_alive(target: &str, pid: u32) -> bool {
    let command = format!("kill -0 {pid} 2>/dev/null; echo $?");
    let Ok(output) = ssh_command(target, &command).stderr(Stdio::null()).output() else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout).trim() == "0"
}

/// 远端节点是否仍在运行：优先 `kill -0`。
///
/// 只靠状态文件会误判——节点收到 SIGINT 后会自己删掉状态文件，此时 `cat` 失败
/// 并不代表进程已退出；反过来进程被硬杀时状态文件可能残留。两种情况都要覆盖。
fn remote_still_running(target: &str, pid: u32) -> bool {
    remote_pid_alive(target, pid)
}

/// 读远端节点日志末尾（固定路径模板），用于启动失败给真实原因。
fn remote_log_tail(target: &str) -> String {
    let command = format!("tail -n 20 {REMOTE_LOG}");
    let Ok(output) = ssh_command(target, &command).stderr(Stdio::null()).output() else {
        return String::new();
    };
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// 远程节点的缺省二进制路径（两台 Mac 装法一致时可用；否则用 node_binary 字段覆盖）。
fn default_remote_binary() -> String {
    if cfg!(windows) {
        "C:/Program Files/Local Transcribe/local-transcribe-node.exe".to_owned()
    } else {
        "/Applications/Local Transcribe.app/Contents/MacOS/local-transcribe-node".to_owned()
    }
}

/// 本机节点二进制：`VOICE_TO_TEXT_NODE_BINARY` 覆盖，否则取可执行文件同目录。
fn node_binary() -> Result<std::path::PathBuf, String> {
    if let Some(path) = std::env::var_os("VOICE_TO_TEXT_NODE_BINARY") {
        let path = std::path::PathBuf::from(path);
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
            "The node binary was not found next to the app: {}",
            candidate.display()
        ))
    }
}

/// 捕获 ssh 子进程 stderr 到共享缓冲（节点已把 stdio 落远端日志，这里主要是 ssh 自身错误）。
#[cfg(unix)]
fn capture_stderr(child: &mut Child) -> Arc<Mutex<Vec<String>>> {
    let lines = Arc::new(Mutex::new(Vec::new()));
    if let Some(stderr) = child.stderr.take() {
        let sink = Arc::clone(&lines);
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let mut guard = sink.lock().unwrap();
                if guard.len() < 20 {
                    guard.push(line);
                }
            }
        });
    }
    lines
}

#[cfg(not(unix))]
fn capture_stderr(_child: &mut Child) -> Arc<Mutex<Vec<String>>> {
    Arc::new(Mutex::new(Vec::new()))
}

#[cfg(unix)]
fn stderr_text(lines: &Arc<Mutex<Vec<String>>>) -> String {
    lines.lock().unwrap().join("\n")
}

#[cfg(not(unix))]
fn stderr_text(_lines: &Arc<Mutex<Vec<String>>>) -> String {
    String::new()
}

/// 远程启动轮询：能力就绪即成功；ssh 提前退出视为失败。
fn poll_remote_ready(host: &str, port: u16, child: &mut Child) -> Result<(), String> {
    let deadline = Instant::now() + START_POLL_TIMEOUT;
    let client = LanClient::new(probe_host(host).to_owned(), port);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Cannot start the network probe: {error}"))?;
    while Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("The remote node exited during startup ({status})."));
        }
        let ready = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), client.health())
                .await
                .map(|result| result.unwrap_or(false))
                .unwrap_or(false)
        });
        if ready {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    Err(format!(
        "The remote node at {host}:{port} did not answer within {}s.",
        START_POLL_TIMEOUT.as_secs()
    ))
}

#[cfg(test)]
mod tests {
    //! 任务书 §8 的可自动验证部分（1、2、3、4、9、10、11、12）。
    //!
    //! 5/6 在 `voice-to-text-lan::settings`，7/8 在 MCP 的测试里，12 的退出码
    //! 由 `crates/transcription-cli/tests/lan_exit_codes.rs` 端到端验证。

    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    #[cfg(target_os = "macos")]
    use std::process::Stdio;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use serde_json::{json, Value};
    use voice_to_text_lan::settings::{read, LanSettings, SETTINGS_FILE};

    use super::{add, scan, start_remote, stop_local, stop_remote};

    /// 每个测试一个独立数据目录（同时作为 `lan-settings.json` 所在处）。
    fn scratch(name: &str) -> (PathBuf, PathBuf) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("lt-lan-svc-{name}-{}-{now}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SETTINGS_FILE);
        (dir, path)
    }

    /// 一个端口上没有任何监听者（先占住再释放，拿到一个刚空出来的端口）。
    fn closed_port() -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        port
    }

    /// 最小 HTTP 节点：`/api/v1/health`、`/capabilities`、`/models` 三个 GET。
    struct MockNode {
        port: u16,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl MockNode {
        fn start(capabilities: Value) -> Self {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let thread = std::thread::spawn(move || {
                while !thread_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = respond(stream, &capabilities);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            });
            // 等它真的能接连接再让调用方发请求：并行测试时启动线程可能被推迟。
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "the mock node never started listening"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            Self {
                port,
                stop,
                thread: Some(thread),
            }
        }
    }

    impl Drop for MockNode {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn respond(mut stream: TcpStream, capabilities: &Value) -> std::io::Result<()> {
        let mut buffer = [0_u8; 2048];
        let read = stream.read(&mut buffer)?;
        let request = String::from_utf8_lossy(&buffer[..read]).to_string();
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");
        let body = if path.ends_with("/capabilities") {
            capabilities.to_string()
        } else if path.ends_with("/models") {
            json!([{ "id": "model-a", "display_name": "Model A" }]).to_string()
        } else {
            json!({ "status": "ok" }).to_string()
        };
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    fn sample_capabilities() -> Value {
        json!({
            "proto_version": 7,
            "max_chunk_bytes": 4_194_304,
            "compute_enabled": true,
            "role": "cpu",
            "model_id": "model-a",
            "platform": "macos",
            "name": "Desk Node",
        })
    }

    fn write_node(path: &Path, id: &str, name: &str, host: &str, ssh: Option<&str>) {
        voice_to_text_lan::settings::write(
            path,
            &LanSettings {
                nodes: vec![voice_to_text_lan::settings::LanNodeConfig {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    host: host.to_owned(),
                    port: 7878,
                    model_id: None,
                    enabled: true,
                    ssh: ssh.map(str::to_owned),
                    node_binary: None,
                    capabilities: None,
                }],
                ..LanSettings::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn test1_an_unreachable_node_is_refused_unless_force_is_given() {
        let (dir, path) = scratch("unreachable");
        let port = closed_port();

        let refused = add(&dir, "desk", "127.0.0.1", Some(port), false);
        assert!(!refused.ok, "unreachable without --force must fail");
        let error = refused.error.unwrap();
        assert_eq!(error.code, "unreachable");
        assert!(
            error.message.contains("--force"),
            "the error must mention --force: {}",
            error.message
        );
        assert!(
            read(&path).nodes.is_empty(),
            "nothing is saved without --force"
        );

        let forced = add(&dir, "desk", "127.0.0.1", Some(port), true);
        assert!(forced.ok, "{:?}", forced.error);
        let data = forced.data.unwrap();
        assert_eq!(data["reachable"], false);
        assert!(data["capabilities"].is_null());
        let saved = read(&path);
        assert_eq!(saved.nodes.len(), 1);
        assert_eq!(saved.nodes[0].port, port);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test2_a_reachable_node_returns_the_capabilities_verbatim() {
        let capabilities = sample_capabilities();
        let node = MockNode::start(capabilities.clone());
        let (dir, path) = scratch("reachable");

        let response = add(&dir, "desk", "127.0.0.1", Some(node.port), false);
        assert!(response.ok, "{:?}", response.error);
        let data = response.data.unwrap();
        assert_eq!(data["reachable"], true);
        assert_eq!(data["capabilities"], capabilities);
        assert_eq!(data["node"]["model_id"], "model-a");

        let saved = read(&path);
        assert_eq!(saved.nodes.len(), 1);
        assert_eq!(saved.nodes[0].capabilities.as_ref(), Some(&capabilities));
        assert_eq!(saved.nodes[0].model_id.as_deref(), Some("model-a"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test3_a_mdns_local_name_is_used_as_a_plain_host() {
        // 用 dns-sd 注册一个指向 127.0.0.1 的 `.local` 名字，模拟真实 mDNS 主机名。
        // 直接把测试连到局域网 IP 会被 macOS 的本地网络权限挡住（测试二进制没有
        // 用户授权），loopback 不受这一限制，因此这里测的是"名字原样当 host"。
        let node = MockNode::start(sample_capabilities());
        let name = format!("lt-lan-test-{}", std::process::id());
        let host = format!("{name}.local");
        let mut publisher = std::process::Command::new("dns-sd")
            .args([
                "-P",
                &name,
                "_lt-lan-test._tcp",
                "local",
                &node.port.to_string(),
                &host,
                "127.0.0.1",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dns-sd is available on macOS");

        let resolves =
            || std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), node.port)).is_ok();
        let mut ready = false;
        for _ in 0..30 {
            if resolves() {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(ready, "the test mDNS name {host} never resolved");

        let (dir, _path) = scratch("mdns");
        let response = add(&dir, "desk", &host, Some(node.port), false);
        let _ = publisher.kill();
        let _ = publisher.wait();
        assert!(response.ok, "adding {host} failed: {:?}", response.error);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test4_an_invalid_host_is_refused_without_guessing() {
        let (dir, path) = scratch("invalid");
        for host in [
            "",
            "   ",
            "ssh://node",
            "http://node.local",
            "desk node",
            "x; rm -rf /",
            "$(id)",
            "node`whoami`",
        ] {
            let response = add(&dir, "desk", host, Some(7878), true);
            assert!(!response.ok, "host {host:?} must be refused");
            let error = response.error.unwrap();
            assert_eq!(error.code, "invalid_host", "host {host:?}");
        }
        assert!(
            !path.exists(),
            "a refused host must not create the settings file"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test9_scan_never_writes_the_settings_file() {
        let (dir, path) = scratch("scan");
        write_node(&path, "n1", "desk", "127.0.0.1", None);
        let before = std::fs::read(&path).unwrap();

        let response = scan(&dir, Some(1));
        assert!(response.ok, "{:?}", response.error);

        let after = std::fs::read(&path).unwrap();
        assert_eq!(before, after, "scan must not rewrite lan-settings.json");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// 依赖系统 `ssh`；Windows CI 不保证有 OpenSSH 客户端，故只在 unix 跑。
    #[cfg(unix)]
    #[test]
    fn test10_remote_start_fails_fast_without_passwordless_ssh() {
        let (dir, path) = scratch("ssh");
        // RFC 2606 保留域名，保证无法解析；BatchMode 使 ssh 不会停在密码提示。
        write_node(
            &path,
            "n1",
            "desk",
            "invalid.invalid",
            Some("invalid.invalid"),
        );

        let started = Instant::now();
        let response = start_remote(&dir, "n1");
        let elapsed = started.elapsed();

        assert!(!response.ok, "an unresolvable ssh target must fail");
        let error = response.error.unwrap();
        assert_eq!(error.code, "start_failed");
        assert!(
            !error.message.trim().is_empty(),
            "the failure needs a readable reason"
        );
        assert!(
            elapsed < Duration::from_secs(30),
            "ssh must fail fast instead of hanging on a password prompt ({elapsed:?})"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test11_injection_in_a_node_name_or_host_is_never_executed() {
        let (dir, path) = scratch("injection");
        let canary = dir.join("canary.txt");
        let payload = format!("127.0.0.1; touch {}", canary.display());

        // 直接写设置文件，绕过 add/update 的校验，模拟被手工编辑过的
        // lan-settings.json（服务端的二次校验才是这一条要验的东西）。
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({
                "mode": "local_only",
                "nodes": [{
                    "id": "n1",
                    "name": "desk; rm -rf /",
                    "host": payload,
                    "port": 7878,
                    "enabled": true,
                    "ssh": payload,
                    "node_binary": "$(touch /tmp/pwned)",
                }],
            }))
            .unwrap(),
        )
        .unwrap();

        for response in [start_remote(&dir, "n1"), stop_remote(&dir, "n1")] {
            assert!(!response.ok, "the unsafe target must be refused");
            let code = response.error.unwrap().code;
            assert!(
                code == "unsafe_target" || code == "unsafe_binary",
                "unexpected code {code}"
            );
        }
        assert!(!canary.exists(), "the injected command must never run");

        // add 也在落盘前挡住同样的片段。
        let rejected = add(&dir, "desk", &payload, Some(7878), true);
        assert!(!rejected.ok);
        assert_eq!(rejected.error.unwrap().code, "invalid_host");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test12_stop_local_without_a_pid_says_it_is_not_running() {
        let (dir, _path) = scratch("stop");
        let response = stop_local(&dir);
        assert!(!response.ok);
        let error = response.error.unwrap();
        assert_eq!(error.code, "not_running");
        assert_eq!(error.message, "节点未在运行");
        let _ = std::fs::remove_dir_all(dir);
    }
}
