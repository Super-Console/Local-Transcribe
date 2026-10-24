//! 局域网计算设置的唯一读写入口（GUI / CLI / service / MCP 共用一份实现）。
//!
//! 设计要点：
//! - `host` 是任意字符串（IP / `.local` / tailnet 名 / 任意 DNS），不做 IP 解析。
//! - 文件不存在 → 返回默认值；损坏内容 → 备份成 `.corrupt-<时间戳>` 后回落默认值，
//!   绝不静默覆盖用户数据。
//! - 写入走「临时文件 → 原子改名 → 回读校验」，并用 `fs2` 文件锁串行化
//!   跨进程的读-改-写，避免 GUI 与 CLI 并发写丢更新。

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

/// 设置文件名（应用数据目录下）。
pub const SETTINGS_FILE: &str = "lan-settings.json";

/// 派发模式。`prefer_lan` 时新任务自动发往已启用且在线的节点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanMode {
    #[serde(alias = "prefer_local")]
    LocalOnly,
    PreferLan,
}

impl LanMode {
    /// 解析 CLI/MCP 传入的字符串（`local` / `prefer-lan`，也接受 snake_case）。
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "local" | "local_only" | "prefer_local" => Ok(LanMode::LocalOnly),
            "prefer_lan" | "lan" => Ok(LanMode::PreferLan),
            other => Err(format!(
                "Unknown LAN mode '{other}'; expected 'local' or 'prefer-lan'."
            )),
        }
    }
}

/// 一个已保存的节点配置。
///
/// `host` 接受任意主机名或地址；连接时直接拼 `http://{host}:{port}`。
/// `ssh` 与 `node_binary` 仅用于远程启停（走 SSH），缺省时分别回落到
/// `host` 与按 sidecar 路径推断的默认值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanNodeConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    /// Remote model ID, independent of the client's installed models.
    #[serde(default)]
    pub model_id: Option<String>,
    pub enabled: bool,
    /// 远程启停的 SSH 目标（`user@host` 或 `~/.ssh/config` 里的别名）；缺省用 `host`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<String>,
    /// 远程节点二进制路径；缺省按 sidecar 路径推断。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_binary: Option<String>,
    /// 添加时抓取到的能力快照（name / protocol_version / compute_enabled / models）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<serde_json::Value>,
}

fn default_idle_unload() -> u64 {
    voice_to_text_core::DEFAULT_IDLE_UNLOAD_SECONDS
}

fn default_true() -> bool {
    true
}

fn default_host_name() -> String {
    "Local Transcribe".to_owned()
}

fn default_host_port() -> u16 {
    7878
}

/// 局域网计算设置：模式、节点列表、后台形态与两个空闲卸载档位。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanSettings {
    pub mode: LanMode,
    pub nodes: Vec<LanNodeConfig>,
    /// 界面进程的模型空闲卸载（秒）；闭集合，非法值读入时回落 120。
    #[serde(default = "default_idle_unload")]
    pub idle_unload_gui: u64,
    /// 节点子进程的空闲卸载（秒），与界面各存各的。
    #[serde(default = "default_idle_unload")]
    pub idle_unload_node: u64,
    /// 上次退出时共享是否开启；启动时据此恢复（网卡失效则不强启）。
    #[serde(default)]
    pub share_on_launch: bool,
    /// 首次开启共享的知情同意只弹一次。
    #[serde(default)]
    pub lan_consent_acknowledged: bool,
    /// 关闭窗口后继续在后台共享；共享未开启时不生效。
    #[serde(default = "default_true")]
    pub keep_running_in_background: bool,
    /// 上次共享使用的名称/网卡/端口；启动恢复时据此重开，缺一不可。
    #[serde(default = "default_host_name")]
    pub host_name: String,
    #[serde(default)]
    pub host_address: String,
    #[serde(default = "default_host_port")]
    pub host_port: u16,
}

impl Default for LanSettings {
    fn default() -> Self {
        Self {
            mode: LanMode::LocalOnly,
            nodes: Vec::new(),
            idle_unload_gui: default_idle_unload(),
            idle_unload_node: default_idle_unload(),
            share_on_launch: false,
            lan_consent_acknowledged: false,
            keep_running_in_background: true,
            host_name: default_host_name(),
            host_address: String::new(),
            host_port: default_host_port(),
        }
    }
}

impl LanSettings {
    /// 非法或历史遗留的档位只回落自己那一项，互不影响。
    fn normalize_idle(&mut self) {
        if !voice_to_text_core::IDLE_UNLOAD_CHOICES.contains(&self.idle_unload_gui) {
            self.idle_unload_gui = default_idle_unload();
        }
        if !voice_to_text_core::IDLE_UNLOAD_CHOICES.contains(&self.idle_unload_node) {
            self.idle_unload_node = default_idle_unload();
        }
    }
}

/// SSH 目标（`user@host` / 别名）或节点二进制路径是否安全。
///
/// 这两个字段会进入 SSH 命令模板。SSH 目标作为独立 argv 传给 `ssh`（不经本地
/// shell），但仍拒绝以 `-` 开头（防选项注入）与任何控制字符。节点路径会被单
/// 引号包进远端命令串，因此拒绝单引号本身与会破坏远端 shell 语法的元字符。
fn is_safe_remote_field(value: &str, allow_spaces: bool) -> bool {
    if value.is_empty() || value.starts_with('-') {
        return false;
    }
    !value.chars().any(|c| {
        c.is_control()
            || matches!(c, '\'' | '"' | ';' | '|' | '&' | '$' | '`' | '<' | '>' | '(' | ')')
            || (!allow_spaces && c.is_whitespace())
    })
}

/// 校验 SSH 目标字段（不允许空格）。
pub fn is_safe_ssh_target(value: &str) -> bool {
    is_safe_remote_field(value, false)
}

/// 校验节点二进制路径字段（允许空格，如 `Local Transcribe.app`）。
pub fn is_safe_node_binary(value: &str) -> bool {
    is_safe_remote_field(value, true)
}

/// 校验设置：节点必须有 id/名称/主机/端口，id 不重复，远程字段需安全。
pub fn validate(settings: &LanSettings) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for node in &settings.nodes {
        if node.id.trim().is_empty() {
            return Err("A LAN node is missing its identifier.".to_owned());
        }
        if !ids.insert(node.id.clone()) {
            return Err("Two LAN nodes share the same identifier.".to_owned());
        }
        if node.name.trim().is_empty() {
            return Err("A LAN node needs a name.".to_owned());
        }
        if node.host.trim().is_empty() {
            return Err("A LAN node needs a host name or address.".to_owned());
        }
        if node.port == 0 {
            return Err("A LAN node needs a port between 1 and 65535.".to_owned());
        }
        if let Some(ssh) = &node.ssh {
            if !is_safe_ssh_target(ssh) {
                return Err(format!(
                    "Node '{}' has an unsafe SSH target; only plain 'user@host' or an alias is allowed.",
                    node.name
                ));
            }
        }
        if let Some(binary) = &node.node_binary {
            if !is_safe_node_binary(binary) {
                return Err(format!(
                    "Node '{}' has an unsafe node binary path.",
                    node.name
                ));
            }
        }
    }
    for seconds in [settings.idle_unload_gui, settings.idle_unload_node] {
        if !voice_to_text_core::IDLE_UNLOAD_CHOICES.contains(&seconds) {
            return Err(format!(
                "Model unload must be one of {:?} seconds.",
                voice_to_text_core::IDLE_UNLOAD_CHOICES
            ));
        }
    }
    Ok(())
}

/// 给空 id 的节点派发 UUID v4（前端新建节点时填空 id 即可）。
pub fn assign_missing_ids(settings: &mut LanSettings) {
    for node in &mut settings.nodes {
        if node.id.trim().is_empty() {
            node.id = uuid::Uuid::new_v4().to_string();
        }
    }
}

/// 按保存顺序返回启用中的节点（派发与探测只看这些）。
pub fn enabled_nodes(settings: &LanSettings) -> Vec<LanNodeConfig> {
    settings
        .nodes
        .iter()
        .filter(|node| node.enabled)
        .cloned()
        .collect()
}

/// 读设置：文件不存在给默认值；损坏内容备份成 `.corrupt-<时间戳>` 后回落默认值。
///
/// 备份用改名（不复制），因此损坏数据不会被后续写入静默覆盖，原路径也干净。
pub fn read(path: &Path) -> LanSettings {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return LanSettings::default(),
    };
    match serde_json::from_slice::<LanSettings>(&bytes) {
        Ok(mut settings) => {
            settings.normalize_idle();
            settings
        }
        Err(_) => {
            backup_corrupt(path);
            LanSettings::default()
        }
    }
}

/// 把损坏的设置文件改名备份；失败也继续回落默认值，不阻塞读取。
fn backup_corrupt(path: &Path) {
    let Some(timestamp) = unix_ms() else {
        return;
    };
    let file_name = match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => return,
    };
    let backup = path.with_file_name(format!("{file_name}.corrupt-{timestamp}"));
    let _ = fs::rename(path, backup);
}

fn unix_ms() -> Option<u128> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis())
}

/// 打开与设置文件同名的 `.lock` 文件并取独占锁；跨进程串行化读-改-写。
fn lock(path: &Path) -> Result<fs::File, String> {
    let lock_path = path.with_extension("json.lock");
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("Cannot open the LAN settings lock: {error}"))?;
    file.lock_exclusive()
        .map_err(|error| format!("Cannot lock the LAN settings: {error}"))?;
    Ok(file)
}

/// 不加锁的原子写 + 回读校验（供 [`write`] 与 [`update`] 在持锁时调用）。
fn write_unlocked(path: &Path, settings: &LanSettings) -> Result<(), String> {
    validate(settings)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| "The app's local data folder is unavailable.".to_owned())?;
    fs::create_dir_all(parent).map_err(|_| "Could not prepare the LAN settings.".to_owned())?;
    let temporary = parent.join(format!("{SETTINGS_FILE}.tmp"));
    let contents =
        serde_json::to_vec_pretty(settings).map_err(|_| "Could not encode the LAN settings.".to_owned())?;
    {
        let mut file = fs::File::create(&temporary)
            .map_err(|_| "Could not save the LAN settings.".to_owned())?;
        file.write_all(&contents)
            .map_err(|_| "Could not save the LAN settings.".to_owned())?;
        let _ = file.sync_all();
    }
    fs::rename(&temporary, path).map_err(|_| "Could not save the LAN settings.".to_owned())?;
    let saved = fs::read(path).map_err(|_| "Could not verify the LAN settings.".to_owned())?;
    if saved != contents {
        return Err("The saved LAN settings did not match the chosen settings.".to_owned());
    }
    Ok(())
}

/// 加锁原子写整份设置（校验失败不落盘）。
pub fn write(path: &Path, settings: &LanSettings) -> Result<(), String> {
    let _guard = lock(path)?;
    write_unlocked(path, settings)
}

/// 加锁的「读-改-写」：持锁期间读出设置，交给 `mutate` 修改，校验后原子写回。
///
/// 返回 `mutate` 的结果；`mutate` 报错则不落盘。
pub fn update<T>(
    path: &Path,
    mutate: impl FnOnce(&mut LanSettings) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = lock(path)?;
    let mut settings = read(path);
    let outcome = mutate(&mut settings)?;
    write_unlocked(path, &settings)?;
    Ok(outcome)
}

/// 按 id（优先）或名称（精确匹配）定位一个已保存节点。
pub fn find_node<'a>(
    settings: &'a LanSettings,
    selector: &str,
) -> Result<&'a LanNodeConfig, String> {
    if let Some(node) = settings.nodes.iter().find(|node| node.id == selector) {
        return Ok(node);
    }
    let mut matches = settings.nodes.iter().filter(|node| node.name == selector);
    let first = matches.next();
    match (first, matches.next()) {
        (Some(node), None) => Ok(node),
        (Some(_), Some(_)) => Err(format!(
            "More than one saved LAN node is named '{selector}'; use its id."
        )),
        (None, _) => Err(format!("No saved LAN node matches '{selector}'.")),
    }
}

/// 应用数据目录（`lan-settings.json` 所在处）。`VOICE_TO_TEXT_APP_DATA_DIR`
/// 可覆盖（测试与便携部署用），否则按平台默认路径。
pub fn default_data_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("VOICE_TO_TEXT_APP_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library/Application Support/com.timmy.voicetotextapp"))
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|local| local.join("com.timmy.voicetotextapp"))
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .map(|path| path.join("com.timmy.voicetotextapp"))
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".local/share/com.timmy.voicetotextapp"))
            })
            .ok_or_else(|| "Could not locate the local app data folder.".to_owned())
    }
}

/// 并发测试用的子进程入口：对同一份设置做一次「加节点 + 改自己那条」。
///
/// 只有设置了环境变量才会真正做事；普通 `cargo test` 运行时它是 no-op，
/// 因此不会干扰测试本身的执行。两个这样的进程同时跑，就能验证 fs2 文件锁
/// 是否真的串行化了跨进程的读-改-写。
#[doc(hidden)]
pub fn settings_test_helper() {
    let Ok(dir) = std::env::var("VOICE_TO_TEXT_LAN_SETTINGS_HOLDER") else {
        return;
    };
    let index = std::env::var("VOICE_TO_TEXT_LAN_SETTINGS_INDEX").unwrap_or_default();
    let path = PathBuf::from(dir).join(SETTINGS_FILE);
    update(&path, |settings| {
        let name = format!("child-{index}");
        // 改自己那条，别碰别人的：丢更新会表现为某条的字段回退。
        if let Some(me) = settings.nodes.iter_mut().find(|node| node.name == name) {
            me.port = 10_000 + index.parse::<u16>().unwrap_or(0);
        }
        settings.nodes.push(node_for_test(&name, &format!("{name}.local")));
        assign_missing_ids(settings);
        Ok(())
    })
    .expect("the helper process could not write the settings");
    // 写完就退出：这个进程只承担测试里的一次写入，不能继续当节点跑起来。
    std::process::exit(0);
}

/// 子进程要加的那条节点（保持 [`settings_test_helper`] 简单可读）。
fn node_for_test(name: &str, host: &str) -> LanNodeConfig {
    LanNodeConfig {
        id: String::new(),
        name: name.to_owned(),
        host: host.to_owned(),
        port: 7878,
        model_id: None,
        enabled: true,
        ssh: None,
        node_binary: None,
        capabilities: None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    /// 每个测试一个独立目录，互不干扰；返回 (目录, 设置文件路径)。
    fn scratch() -> (PathBuf, PathBuf) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lt-lan-settings-{now}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SETTINGS_FILE);
        (dir, path)
    }

    fn node(id: &str, name: &str, host: &str, port: u16) -> LanNodeConfig {
        LanNodeConfig {
            id: id.to_owned(),
            name: name.to_owned(),
            host: host.to_owned(),
            port,
            model_id: None,
            enabled: true,
            ssh: None,
            node_binary: None,
            capabilities: None,
        }
    }

    /// 设置文件缺失 → 默认值，且不产生任何文件（读取绝不创建副作用）。
    #[test]
    fn a_missing_file_reads_as_defaults_without_creating_one() {
        let (dir, path) = scratch();
        let settings = read(&path);
        assert_eq!(settings, LanSettings::default());
        assert!(!path.exists(), "reading must not create the settings file");
        let _ = fs::remove_dir_all(dir);
    }

    /// 任务书 §8 测试 6：损坏 JSON 被备份成 `.corrupt-*`，程序用默认值继续。
    #[test]
    fn corrupt_json_is_backed_up_and_reads_as_defaults() {
        let (dir, path) = scratch();
        fs::write(&path, b"{ this is not json").unwrap();

        let settings = read(&path);

        assert_eq!(settings, LanSettings::default(), "corrupt falls back");
        // 原路径已被改名让开，损坏内容一点没丢，也没有被默认值覆盖。
        assert!(
            !path.exists(),
            "the corrupt file must be renamed away, not overwritten"
        );
        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".corrupt-")
            })
            .collect();
        assert_eq!(backups.len(), 1, "exactly one corrupt backup");
        let bytes = fs::read(backups[0].path()).unwrap();
        assert_eq!(bytes, b"{ this is not json", "backup keeps the bytes");
        let _ = fs::remove_dir_all(dir);
    }

    /// 读-改-写往返：字段都存得住，lock 文件是独立的，不污染设置内容。
    #[test]
    fn update_round_trips_and_keeps_the_lock_file_separate() {
        let (dir, path) = scratch();
        update(&path, |settings| {
            settings.nodes.push(node("", "desk", "mbp.local", 7878));
            assign_missing_ids(settings);
            Ok(())
        })
        .unwrap();

        let saved = read(&path);
        assert_eq!(saved.nodes.len(), 1);
        assert!(!saved.nodes[0].id.trim().is_empty(), "id was assigned");
        assert!(path.exists());
        assert!(
            dir.join("lan-settings.json.lock").exists(),
            "the lock lives in its own file"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// 任务书 §8 测试 5（跨进程并发写）在 `tests/settings_concurrency.rs`：
    /// 那里用 `CARGO_BIN_EXE_local-transcribe-node` 启动子进程——节点二进制的
    /// main 会先走 `settings_test_helper()`。绝不能用 `current_exe()`：在单元
    /// 测试里那是测试二进制本身，子进程会重跑整个套件并再起两个子进程。
    ///
    /// 重名节点按名称选择时报错而不是默默取第一条，否则并发添加会改错对象。
    #[test]
    fn an_ambiguous_name_is_an_error_not_a_guess() {
        let mut settings = LanSettings::default();
        settings.nodes.push(node("a", "desk", "one.local", 1));
        settings.nodes.push(node("b", "desk", "two.local", 2));

        let error = find_node(&settings, "desk").unwrap_err();
        assert!(error.contains("More than one"), "{error}");
        // id 仍然精确可选。
        assert_eq!(find_node(&settings, "a").unwrap().host, "one.local");
        assert_eq!(find_node(&settings, "b").unwrap().host, "two.local");
    }

    /// 名称唯一时按名称选择照常工作（这是 CLI 的主要用法）。
    #[test]
    fn a_unique_name_still_selects_its_node() {
        let mut settings = LanSettings::default();
        settings.nodes.push(node("a", "desk", "one.local", 1));
        assert_eq!(find_node(&settings, "desk").unwrap().id, "a");
        assert!(find_node(&settings, "nope").is_err());
    }

    /// SSH 目标与节点路径会进命令模板：元字符、选项注入、控制字符一律拒绝。
    ///
    /// 任务书 §8 测试 11 用 `; rm -rf /` 与 `$(...)` 断言它们只能被当成普通
    /// 字符串拒绝，绝不可能被执行。
    #[test]
    fn remote_fields_reject_injection_fragments() {
        let nasty = [
            "; rm -rf /",
            "host; rm -rf /",
            "$(whoami)",
            "`whoami`",
            "a|b",
            "a&b",
            "a'b",
            "a\"b",
            "a<b",
            "a>b",
            "(rm)",
            "-oProxyCommand=/bin/sh",
            "",
        ];
        for value in nasty {
            assert!(
                !is_safe_ssh_target(value),
                "ssh target must reject {value:?}"
            );
        }
        // 控制字符（含换行）同样拒绝。
        assert!(!is_safe_ssh_target("host\nrm -rf /"));
        assert!(!is_safe_ssh_target("ho\tst"));

        // 路径允许空格（.app 目录名），但同样的元字符照样拒绝。
        assert!(is_safe_node_binary(
            "/Applications/Local Transcribe.app/Contents/MacOS/local-transcribe-node"
        ));
        assert!(!is_safe_node_binary("/bin/sh -c 'rm -rf /'"));
        assert!(!is_safe_node_binary("/bin/$(rm)"));
        assert!(!is_safe_node_binary("/bin/sh;rm"));

        // 合法形态照常通过。
        assert!(is_safe_ssh_target("mbp"));
        assert!(is_safe_ssh_target("tim@192.168.1.5"));
        assert!(is_safe_ssh_target("box.tail-scale.ts.net"));
    }

    /// 写入时校验拦下不安全的 ssh 字段（防止手工编辑的设置绕过）。
    #[test]
    fn writing_refuses_a_node_with_an_unsafe_ssh_field() {
        let mut settings = LanSettings::default();
        let mut bad = node("a", "desk", "one.local", 1);
        bad.ssh = Some("tim@host; rm -rf /".to_owned());
        settings.nodes.push(bad);

        let (dir, path) = scratch();
        let error = write(&path, &settings).unwrap_err();
        assert!(error.contains("unsafe SSH"), "{error}");
        assert!(!path.exists(), "nothing is written when validation fails");
        let _ = fs::remove_dir_all(dir);
    }

    /// 模式解析：连字符与下划线等价，未知值给出可读错误。
    #[test]
    fn mode_parsing_accepts_both_spellings() {
        assert_eq!(LanMode::parse("local").unwrap(), LanMode::LocalOnly);
        assert_eq!(LanMode::parse("Local").unwrap(), LanMode::LocalOnly);
        assert_eq!(LanMode::parse("local-only").unwrap(), LanMode::LocalOnly);
        assert_eq!(LanMode::parse("local_only").unwrap(), LanMode::LocalOnly);
        assert_eq!(LanMode::parse("prefer-lan").unwrap(), LanMode::PreferLan);
        assert_eq!(LanMode::parse("prefer_lan").unwrap(), LanMode::PreferLan);
        assert_eq!(LanMode::parse("lan").unwrap(), LanMode::PreferLan);
        let error = LanMode::parse("everything").unwrap_err();
        assert!(error.contains("prefer-lan"), "{error}");
    }

    /// 历史遗留的非法空闲档位只回落自己那一项，另一项不受牵连。
    #[test]
    fn an_invalid_idle_unload_falls_back_on_its_own() {
        // 3600 不在合法档位里；600 是合法的，必须原样保留。
        let json = r#"{"mode":"local_only","nodes":[],"idle_unload_gui":3600,"idle_unload_node":600}"#;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lt-lan-idle-{now}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SETTINGS_FILE);
        fs::write(&path, json).unwrap();

        let settings = read(&path);
        assert_eq!(
            settings.idle_unload_gui,
            voice_to_text_core::DEFAULT_IDLE_UNLOAD_SECONDS,
            "3600 is not a valid tier"
        );
        assert_eq!(settings.idle_unload_node, 600, "a valid tier is kept");
        let _ = fs::remove_dir_all(dir);
    }

    /// 只有启用中的节点参与派发与探测。
    #[test]
    fn only_enabled_nodes_are_dispatched() {
        let mut settings = LanSettings::default();
        let mut on = node("a", "desk", "one.local", 1);
        on.enabled = true;
        let mut off = node("b", "box", "two.local", 2);
        off.enabled = false;
        settings.nodes.push(on);
        settings.nodes.push(off);

        let enabled = enabled_nodes(&settings);
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, "a");
    }
}
