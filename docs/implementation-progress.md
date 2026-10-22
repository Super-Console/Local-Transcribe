# Windows Early Beta + LAN Compute V1.1 实施进度

任务书：`taskbook/Local_Transcribe_Windows_LAN_Early_Beta_无人值守开发任务书.md`。
本文件为滚动进度账本：每完成一个阶段追加一节，区分「已验证事实 / 已实现待验证 / 阻塞 / 推测」。

## Phase 0 · 现状调查与技术评审（2026-10-23 完成）

- 评审文档：`docs/architecture/windows-lan-review.md`。
- 已验证事实：
  - 仓库私有、无 LICENSE；CI（windows-2022）管线完整但最近 8 连红，首败于 `cargo fmt --check`（recording.rs 未格式化），Windows 编译从未被 CI 真正验证。
  - `transcription-service` 硬编码 `std::os::unix`（UnixListener/UnixStream/PermissionsExt），Windows 无法编译；客户端统一走 `ServiceClient::connect`，改造可收敛在 service 一处。
  - `apps/desktop/src-tauri/Cargo.toml` 通用 `[dependencies]` 含 4 个 macOS-only crate（core-foundation、objc2-core-audio、objc2-core-foundation、objc2-core-audio-types）需门控。
  - recording.rs 平台分叉已完成（非 macOS 有 stub），麦克风路径跨平台。
- 环境准备（依赖安装记录）：
  - 检查：`rustup target list --installed` 仅有 `aarch64-apple-darwin`；
  - 安装：`rustup target add x86_64-pc-windows-msvc`（用途：本地 cargo check 交叉验证 Windows 编译，避免每轮等 CI；版本随工具链 rustc 1.97.1）。
- Windows PC 唤醒：HA `pc_wake` 实体交叉验证（`button.wake_on_lan_REDACTED-MAC`，与 pc_shutdown/pc_sleep 同指 REDACTED-USER@REDACTED-TAILSCALE-IP）；因宿主无 HA API token，改为同宿主发送同 MAC/同广播的 WoL 魔术包（等效该按钮动作）。PC 已唤醒，SSH 登录成功（hostname `REDACTED-HOST`）。

## Phase 1 · Windows 构建兼容（2026-10-23 完成代码层，CI 实证见下）

已实现并本地验证：

1. **rustfmt 修复**：`cargo fmt --all` 清掉 CI 首败点（recording.rs 手写代码未格式化），`cargo fmt --all -- --check` 通过。
2. **service 跨平台 IPC**：`crates/transcription-service/src/lib.rs` 引入 `LocalStream`/`LocalListener` 类型别名与 `bind_local`/`connect_local`：macOS/Linux 保持 Unix socket（0600）不变；Windows 用仅回环 TCP（127.0.0.1 随机端口，写入 `service.port`），fs2 单实例锁与换行 JSON 协议不变。新增公共 `endpoint_file()`（客户端/测试判断服务存活用）。**该做法为任务书允许的两种 Windows 本地 IPC 之一（loopback TCP），MCP 纯 stdio 产品边界不受影响。**
3. **desktop 依赖门控**：`core-foundation`、`objc2-core-audio`、`objc2-core-foundation`、`objc2-core-audio-types` 移入 `[target.'cfg(target_os = "macos")'.dependencies]`；recording.rs 的 `File/BufWriter/Path` import 加 macOS 门控。
4. **测试门控**：`tests/service_ipc.rs` 的 socket mode 断言与 /tmp 临时目录按平台门控（Windows 用系统 temp + 数据文件夹 ACL），`wait_for_socket` 改用 `endpoint_file`。
5. **clippy 清零**（CI 用 `-D warnings`）：修掉 3 处旧代码问题（`unnecessary_cast`、`collapsible_if`×2，改为 let-chain）+ unused import。
6. **Windows 状态显示**：`service_status` 的 endpoint 路径改用 `endpoint_file`（Windows 显示 port 文件而非不存在的 socket）。

本地验证结果（已验证事实）：

- `cargo check/clippy --workspace --all-targets -- -D warnings`：macOS host 目标零警告 ✓
- `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`：**Windows 目标零警告 ✓**（sherpa-onnx-sys 自动下载 win-x64 静态库成功，tauri-winres + llvm-rc 资源编译成功）
- `cargo test --workspace`（macOS）：42 通过 / 8 ignored（P0 网关测试需真实模型）/ 0 失败 ✓
- `cargo fmt --all -- --check` ✓

依赖安装记录（补充）：

- 检查：`which llvm-rc` 无、`brew list llvm` 无；
- 安装：`brew install llvm`（Homebrew，含 llvm-rc；用途：在 macOS 交叉编译 Windows 资源，等价 CI 的 rc.exe。本地验证用，不影响发布产物）。路径：`/opt/homebrew/opt/llvm/bin`。
- 检查：`which git-filter-repo` / brew / pip 均无；
- 安装：`brew install git-filter-repo`（2.47.0，纯 Python 361 KB；用途：公开前 git 历史隐私脱敏）。
- 代理备忘：sherpa-onnx-sys 下载脚本不支持 SOCKS，构建时需 `env -u ALL_PROXY`（保留 HTTP(S)_PROXY）；Mac 上 curl 访问 Tailscale 地址需 `--noproxy '*'`（Clash 代理会吞掉直连流量）。

待实证（Phase 5）：GitHub Actions windows-2022 真实构建 MSI + 真机安装启动。

## Phase 2/3 · LAN Compute V1.1（2026-10-23 完成核心 + GUI 集成）

- 核心库 `crates/lan-compute`（`voice-to-text-lan`）：九态任务状态机（非法迁移拒绝）、任务 JSON 持久化/崩溃恢复（中断任务→FAILED/interrupted）、axum HTTP API `/api/v1`（分块上传+断点续传、Bearer 令牌恒定时间比较、collect 防路径穿越、ZIP+sha256 清单导出）、mDNS 注册/发现（`_localtranscribe._tcp.local.`）、`local-transcribe-node` 程序（token 自动生成 0600、Ctrl-C 优雅退出）。23 测试全绿。
- GUI Client 集成（apps/desktop）：`LanView.svelte`（模式切换/节点管理/扫描配对/任务表九态/发送/取消/取回/收集，标注临时 UI）、`lan.rs` 命令层（设置持久化、发现、4 MiB 分块提交、`lan://progress` 事件、结果回写本地历史）、`start_transcription` 自动派发（优先局域网→可达节点，失败回落本机）、i18n 104 键×5 语言、6 个 TDD 单测。workspace 测试 45 项全绿，双平台 clippy 零告警，svelte-check 零告警。
- **跨设备真机实测（Mac → Windows Node，8 步全绿）**：health/401 认证/capabilities(`platform:"windows"`)/创建任务/分片上传+断点续传逐值核对/Windows CPU 真实 paraformer 转写（queued→completed，8.67 s 音频出稿）/结果取回/collect 导出。mDNS 真机广播日志确认。详见 `docs/testing/windows-early-beta-test-report.md` §4.1。
- Windows 真机安装验证：MSI 静默安装返回码 0、安装产物/注册表/ProductVersion 核对；交互会话 GUI 启动（PID 4836、窗口标题 `Local Transcribe`）+ 真实截图 `docs/testing/screenshots/windows-11-app-window.png`。

## Phase 4 · 测试与文档（2026-10-23）

- 测试报告：`docs/testing/windows-early-beta-test-report.md`（严格区分已验证/待验证/不支持，含真机实测记录）。
- README 双语更新；Windows Early Beta 冻结灰度设计稿 `design/windows-early-beta.html`；PROJECT_STATUS 里程碑追加。
- 版本定格 `1.1.0-beta.1`（tauri.conf/Cargo/package.json；MSI 数字版由 bundler 自动截取 1.1.0）。

## Phase 5 · 发布（进行中）

- Gate G4 敏感信息扫描：git 历史无凭据类泄漏；内网 IP/主机名/SSH 用户/MAC/真名 1 处 → 已定脱敏规则（temp/redact-rules.txt），公开前用 git-filter-repo 全历史脱敏后 force push。
- 待办：历史脱敏 → 仓库公开 → GitHub Release（Windows MSI + 程序 + macOS dmg + 校验和 + 双语 release notes）。
