# Windows Early Beta + LAN Compute V1.1 无人值守开发（任务书一）

日期：2026-10-23（夜间无人值守）。执行：OpenCode 会话（Claude Opus 4.7 路由）。
任务书：Local_Transcribe_Windows_LAN_Early_Beta 无人值守开发任务书。
本文按「事件」组织，后续同事件追加到末尾。只记录已验证事实与实际执行，不写未验证结论。

## Phase 0 · 现状调查（输出：docs/architecture/windows-lan-review.md）

- 仓库 `github.com/TimmySheep/VoiceToTextApp`（当时 PRIVATE、无 LICENSE）；CI `windows-validation.yml` 管线完整但最近 8 连红，首败于 `cargo fmt --check`（recording.rs 未格式化），**Windows 编译从未被 CI 真正验证过**。
- 关键雷区（代码核实）：`transcription-service` 无条件 `use std::os::unix`（Windows 无法编译）；desktop Cargo.toml 通用 [dependencies] 含 4 个 macOS-only crate；测试 service_ipc.rs 含 Unix 专属断言。
- Windows PC 唤醒：HA `pc_wake`（`button.wake_on_lan_*`，MAC 经 pc_shutdown/pc_sleep 交叉验证同指一台机）；因宿主无 HA API token（secrets.yaml 仅占位符），改为 HA 同宿主发送同 MAC/同广播 WoL 魔术包（等效该按钮动作，已在评审文档记录偏离）。PC 唤醒成功，hostname `REDACTED-HOST` 交叉验证通过。

## Phase 1 · Windows 构建链打通（fa449c7）

- service 跨平台 IPC：macOS/Linux 保持 Unix socket 0600；Windows 用仅回环 TCP + `service.port` 文件（fs2 单实例锁、换行 JSON 协议不变）——任务书允许的本地 IPC 之一。客户端统一 `ServiceClient::connect`，改造收敛在 service 一处。
- desktop 依赖门控（core-foundation / objc2-core-audio* 移入 macOS target）；recording.rs 平台 import 门控（麦克风路径 cpal 跨平台保留，应用音频非 macOS stub）；测试按平台门控。
- clippy `-D warnings` 双目标清零（修 3 处旧代码 + unused import）；rustfmt 修复。
- 本地验证：`cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` 零告警；macOS 42 测试通过。
- **CI（windows-2022 真实 runner）完整管线首次全绿**（25 分钟：fmt/test/clippy/release/npm/MSI/产物暂存）。
- 环境安装记录：`rustup target add x86_64-pc-windows-msvc`（本地交叉核对用）、`brew install llvm`（llvm-rc，交叉编译 Windows 资源用）。均已先查后装。

## Phase 2/3 · LAN Compute V1.1（2d034df + 78cb9d2）

- 新 crate `crates/lan-compute`（`voice-to-text-lan`）+ `local-transcribe-node` 程序：九态任务状态机、任务持久化/崩溃恢复、axum `/api/v1`（分块上传+断点续传、Bearer 恒定时间比较、collect 防路径穿越、ZIP+sha256 清单）、mDNS `_localtranscribe._tcp.local.`、token 自动生成（0600）。23 测试。
- GUI Client 集成：`LanView.svelte` + `lan.rs` 命令层（设置持久化/发现/4 MiB 分块提交/进度事件/结果回写历史）、`start_transcription` 优先局域网自动派发（不可达回落本机）、i18n 104 键×5 语言、6 个 TDD 单测。workspace 45 测试全绿、双平台 clippy 零告警、svelte-check 零告警。
- **跨设备真机实测（Mac → Windows Node）8 步全绿**：401 认证、capabilities（platform:windows/paraformer/cpu）、创建任务、分片上传+断点续传逐值核对、**Windows CPU 真实转写 queued→completed**（8.67 s 音频出稿）、结果取回、collect 导出。mDNS 真机广播日志确认。
- Windows 真机安装：MSI 静默安装返回码 0；交互会话 GUI 启动（PID 4836、窗口标题 `Local Transcribe`）；真实截图 `docs/testing/screenshots/windows-11-app-window.png`。headless（SSH session 0）启动 GUI 退出属 WebView2 会话限制，已记录为测试方法学备忘。

## Phase 4/5 · 文档与发布线

- 冻结灰度设计稿 `design/windows-early-beta.html`（§0-§9 全板块：文案/尺寸/状态/交互/转场）；MIT LICENSE；README 双语更新；测试报告 `docs/testing/windows-early-beta-test-report.md`（严格区分已验证/待验证/不支持）；版本定格 `1.1.0-beta.1`。
- Gate G4 敏感信息扫描：git 历史无凭据类泄漏；内网 IP/主机名/SSH 用户/MAC/真名（1 处）定脱敏规则，公开前 git-filter-repo 全历史脱敏（`brew install git-filter-repo`，先查后装）。
- 真机档案：Win11 Pro build 26300 x64、AMD RX 6600（无 NVIDIA，compute 角色 cpu）、WebView2 155、SSH 管理员权限、C: 剩余 ~362 GB。

## 排障备忘（实证）

- sherpa-onnx-sys 构建脚本不支持 SOCKS 代理：构建需 `env -u ALL_PROXY`（保留 HTTP(S)_PROXY）。
- Mac curl 访问 Tailscale 地址须 `--noproxy '*'`，否则被 Clash 127.0.0.1:7890 吞掉（表现为 52 空回包/超时，极具误导性）。
- Windows 里 SSH 会话内 `Start-Process` 起的进程随会话回收；需长期驻留用 WMI `Win32_Process.Create`（或交互计划任务）。GUI 必须在交互会话跑（WebView2）。
- 模型目录族识别依赖原始文件夹名（如 `sherpa-onnx-paraformer-zh-2023-09-14`），改名会拒绝并给出可读错误（校验行为正确）。
- 测试用防火墙限端口入站规则 `LT-EarlyBeta-Test node 7878` 为临时规则，收尾删除。

（后续追加：GUI 层跨设备实测、历史脱敏、公开、GitHub Release。）
