# Local Transcribe — 跨平台移植、发布与两台 Mac 换装任务书 V1.0

> **性质**：收口任务（第十四份）。把今晚所有改动搬上 Windows、去掉 Windows 的菜单栏、发布双平台版本，并在两台 Mac 上换成新版、清掉旧副本。
> **不得修改其它任务书。**

---

## 0. 顺序（严格按这个来）

```
1 版本号统一 → 2 Windows 移除菜单栏 → 3 Windows 真机验证
→ 4 推送并等 CI 出 MSI → 5 本地出 dmg → 6 创建 GitHub release
→ 7 从 release 资产装两台 Mac → 8 清理旧副本 → 9 最终校验 → 10 关机
```

先发布、后安装。这样两台 Mac 装的就是**已发布的那一份**，不会出现「装的和发的不一样」。

**无人值守（重要）**：Timmy 已去睡觉，全程不要等他确认。

- 遇到需要判断的地方，按「不破坏现有数据、不留半成品」的最保守做法自行决定，并把决定写进日志
- 只有涉及不可逆破坏（删除用户数据、改仓库可见性、force push、动 HA 配置）才停下
- 若 Windows 端还有未完成项：**把它唤醒，做完，再关掉**——最终必须处于关机状态
- 报告写进日志与对话即可，不需要等回复

---

## 1. 版本号

当前三处都是 `1.1.0-beta.5`，统一改成 **`1.2.0-beta.1`**：

- `apps/desktop/package.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/tauri.conf.json`（`version` 与 `windows.wix.version` → `1.2.0.1`）
- README 双语中出现的版本号
- 改完 `grep -r "1.1.0-beta.5"` 必须为空
- 版本一致性测试必须通过

---

## 2. Windows：去掉菜单栏

**现状**：`lib.rs` 里 `.menu(|app| ...)` 无条件生效，Windows 上会出现原生菜单栏（File / Edit / View / Help）。

**要求**

- 菜单与 `on_menu_event` **只在 macOS 编译**（`#[cfg(target_os = "macos")]`），Windows 上完全不创建菜单
- 不得为了去掉菜单而删除 macOS 的「打开…」：macOS 侧行为保持不变
- Windows 上打开文件的入口是界面里的「导入文件」按钮，确认它仍在、可用
- 去掉菜单后窗口内容区应上移，检查首屏没有被裁切

**断言**

- `grep -n "menu(" apps/desktop/src-tauri/src/lib.rs` 的结果里，菜单构建处于 `#[cfg(target_os = "macos")]` 之下
- Windows 构建产物启动后，窗口顶部没有 File/Edit/View 条（需人工确认，见第 7 节）

---

## 3. Windows 移植：需要逐项确认的地方

界面是同一套 Svelte，大部分自动跟随。真正需要注意的只有四处：

| 项 | 要求 |
| --- | --- |
| 字体 | `--font-display` 目前是 `"New York", ui-serif, "Songti SC", Georgia, serif`。Windows 没有前两者，会落到 Georgia（中文落到宋体）。**在字体栈里补 `"SimSun"`、`"Microsoft YaHei"` 作为中文回退**，其余不动 |
| 玻璃 | `backdrop-filter` 在 WebView2 可用。确认两个胶囊与顶栏不是纯黑/纯白（退化）；若退化只允许把底色不透明度提到 92% |
| 托盘图标 | 已按平台分文件（`tray-windows.png`，白色）。确认 Windows 上开的是**非 template** 分支，且任务栏里可见 |
| 路径 | 数据目录走 `app_local_data_dir`（`%LOCALAPPDATA%`），不要落到 Roaming |

**禁止**：为 Windows 单独写一套布局或分支样式；除上述字体栈外，不得出现平台判断。

---

## 4. Windows 真机验证（SAC 已由 Timmy 关闭）

主机：`ssh win`（Tims-Gaming-PC，100.94.223.63）。

1. 先确认拦截真的没了：`cargo fmt --check`、`cargo test --workspace`、`cargo clippy -- -D warnings`
   - 历史上失败原因是 Code Integrity 3033/3077 拦 build script；若仍失败，记录事件号，不要绕过
2. 从 CI 取 MSI 安装，记录安装路径与版本
3. 启动后确认（客观项）：
   - 进程存活，窗口标题正确
   - 顶部**没有**菜单栏
   - `Get-NetTCPConnection -LocalPort 7878` 的持有者是 `local-transcribe-node.exe`（开启共享后）
4. 人工项（Timmy 看）：托盘白色图标是否可见、玻璃是否正常、字体是否难看

---

## 5. CI 与发布资产

1. 推送 `main`，等 `.github/workflows/windows-validation.yml` 全绿（用 `gh run watch` 确认，不要只看"像绿"）
2. 下载 artifact：MSI + 辅助 exe，记录 SHA-256
3. 本地 `npm run bundle:mac`，产出 `.app` 与 dmg，确认 bundle 内含 sidecar 与 `icon.icns`
4. 记录 dmg 的 SHA-256

---

## 6. GitHub release

- 仓库：`TimmySheep/VoiceToTextApp`（public，main）。不改可见性、不 force push
- tag：`1.2.0-beta.1`；release 标题沿用 `Local Transcribe 1.2.0-beta.1 — macOS + Windows`；**Pre-release**
- 资产：dmg、msi、`SHA256SUMS`。**不附模型权重、不附个人录音、不附裸 exe**
- Release notes 双语，必须包含：
  1. 本轮变化：无头节点与菜单栏常驻、CLI/MCP 局域网派发、Tailscale 支持、界面体系（胶囊/渐隐/材质）、按钮三档、新应用图标
  2. Windows 本轮验证到哪一步（真机安装、启动、无菜单栏、托盘、节点端口），以及一句：**未签名应用在他人强制开启 SAC 的机器上仍会「可安装不可运行」**
  3. macOS 为 ad-hoc 签名、未公证
  4. 两机局域网端到端是否已通过（未通过就写未通过）
  5. 升级说明：节点是独立进程；共享状态会跨启动记住；接收媒体清理默认关闭

**禁止**：伪造签名/公证/运行验证；把 blocked 写成 verified。

---

## 7. 两台 Mac 换装（MBP 本机 + Air 经 SSH）

目标：两台都装 release 里的同一份，旧版进废纸篓。

### 7.1 MBP（本机）

1. 从 release 下载 dmg，挂载，取出 `.app`
2. 备份现有 `~/Applications/Local Transcribe.app` 到 `~/Applications/Local Transcribe Backups/before-<时间戳>/`
3. 正常退出旧实例（不要 `kill -9`），确认进程消失、7878 释放
4. 安装新的，安装前后可执行文件 SHA-256 与 release 记录一致
5. 从安装路径启动，确认新 pid 与可执行路径

### 7.2 Air（`ssh airm1`）

同 7.1 的 2–5 步。传输用 `scp` 把 dmg 或 `.app` 送过去，安装后**在 Air 上重算 SHA-256** 与 MBP 上一致。

---

## 8. 清理旧副本（消除 Spotlight 噪音）

**规则**：一律 `mv` 到废纸篓，禁止 `rm`；不得清空废纸篓本身。

### 8.1 MBP

已知残留（先确认再动）：

- `target/release/bundle/macos/VoiceToTextApp.app` ← **旧产品名残留，Spotlight 会索引到它**
- `target/release/bundle/macos/Local Transcribe.app`（构建产物，非安装副本）
- `~/Applications/Local Transcribe Backups/` 下的多份旧备份（目前 2 份，约 90 MB）

处理：

- 旧产品名 bundle 与构建产物 → 移入 `~/.Trash/local-transcribe-stale-<时间戳>/`
- 备份目录 **全部移入废纸篓，不保留回滚点**（Timmy 明确要求）
- 移完执行 `mdfind -name "Local Transcribe.app"`，结果**应只剩 `~/Applications/Local Transcribe.app` 一条**

> 回滚不再依赖本地备份：任何时候都能从 GitHub release 重新下载同一版本。若将来需要回滚点，由 Timmy 另行说明。

### 8.2 Air

同样处理：`~/Applications/` 下的备份目录**全部移入废纸篓**；构建目录里的旧 bundle 一并移走。

### 8.3 明确不做

**不把 `target/` 加进 Spotlight 隐私排除。** Timmy 明确表示不做，不要顺手改系统设置。

---

## 9. 最终校验

1. `mdfind -name "Local Transcribe.app"` 在 MBP 与 Air 上各只剩安装的那一条
2. 两台的可执行文件 SHA-256 相同，且等于 release 里记录的 dmg 内文件
3. 两台都能从安装路径启动，版本号显示为 `1.2.0-beta.1`
4. release 页面上的资产齐全（dmg / msi / SHA256SUMS）
5. CI 绿色链接、四个 SHA-256 一起写进日志

---

## 10. 最后一步：关闭 Windows（通过 Home Assistant）

**前置条件（全部满足才关）**

- Windows 上的工作已全部完成并已回报：MSI 安装、顶部无菜单栏、托盘白色图标、7878 归 `local-transcribe-node.exe`、版本号为 `1.2.0-beta.1`
- CI 已全绿，MSI 与 dmg 已下载到本机并记录了 SHA-256（关机后 CI 仍能跑，但资产必须在手里）
- **不要等 Timmy 确认**——他已在睡觉。缺什么就自己补，补齐再关

**若关机时发现还有未完成项（无人值守自行处理）**

1. 用已有路径唤醒（沿用 `logs/2026-10/2026-10-23-Windows精准应用录音无人值守开发.md` 记录的同一方式：在 HA 宿主上发同 MAC 同广播魔术包；若有可用 token 则调 `script.pc_wake`）
2. 唤醒后用 hostname 交叉验证确实是这台 PC，再继续干活
3. 干完、确认无待办，重新执行本节的关机流程
4. 允许这个「唤醒 → 干活 → 再关」循环发生，但**最终状态必须是关机**

**凭据现实（不要猜）**

历史记录 `logs/2026-10/2026-10-23-Windows精准应用录音无人值守开发.md` 已载明：

- Home Assistant 跑在局域网宿主上（Docker 容器 `homeassistant:2026.8.3`）
- **Mac 上没有 HA API token**（`secrets.yaml` 只有占位符）
- 当时的做法是：在 HA 宿主上执行与被按按钮等效的动作，并在日志里记录这处偏离

所以本轮按这个顺序：

1. `ssh` 到 HA 宿主（用已确认可达的那个别名），**只读**核对 `scripts.yaml` 里 `pc_shutdown` 的定义：目标 Tailscale IP、hostname，确认它指向的是这台 Windows PC，不是别的机器
2. 若能拿到可用 token → 调用 `script.pc_shutdown`，并记录该实体的 `last_triggered`
3. 若没有 token → 在宿主上执行与 `pc_shutdown` **完全相同**的命令，日志注明「等效于 `script.pc_shutdown`，原因：无 API token」
4. **禁止**：伪造 HA 调用记录、把 token 写进日志或提交、猜实体 ID、动 HA 的配置文件

**验证（两条独立证据，缺一不可）**

- `ssh win` 在 60 秒内超时
- `tailscale status` 中该节点变为 `offline`（记录 last seen）

只拿到一条就写「可能已关，证据不全」，不要写成已确认。

**最终状态确认（必须）**

全部工作结束后，无论中途唤醒过几次，收尾时都要再确认一次：

- `tailscale status` 该节点为 `offline`
- `ssh win` 超时
- 日志里最后一条写「Windows 已关闭，时间戳，两条证据如下」

如果最后发现它又被唤醒了（例如循环没收敛），必须再关一次，不能留开机状态过夜。

**失败处理**

- 最多重试 3 次，每次间隔 30 秒
- 仍失败：停下报告，让 Timmy 手动关。**不要**改用强杀、断网、拔电之类的手段

**关机前先落盘**

日志必须在关机动作**之前**写完并回读——关掉之后就写不了了。日志里至少包含：调用方式（token / 等效命令）、目标主机、时间戳、两条验证证据。

**不做**

- 不关 Mac、不重启 Ubuntu 服务器
- 不改 HA 任何配置

---

## 11. 不做

- 不做移动端
- 不改局域网协议、不改 MCP/CLI 接口
- 不改 macOS 最低系统版本
- 不评价界面观感（Timmy 看）
- 不删除任何文件（只移废纸篓）
- 临时文件不自行清理，需要时先列清单

---

## 12. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  grep -rn "1\.1\.0-beta\.5" --include=*.json --include=*.toml --include=*.md . | head && \
  grep -n "menu(" apps/desktop/src-tauri/src/lib.rs | head -3 && \
  ls -d target/release/bundle/macos/*.app 2>/dev/null && \
  ls ~/Applications/"Local Transcribe Backups" 2>/dev/null
```
