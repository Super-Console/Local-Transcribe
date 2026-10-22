# Windows Early Beta 与局域网 Compute V1.1 技术评审（Phase 0）

日期：2026-10-23。依据：`taskbook/Local_Transcribe_Windows_LAN_Early_Beta_无人值守开发任务书.md`。
本文只记录核实过的事实与有依据的判断；推测单独标注。

## 1. 仓库与发布现状

- 仓库：`github.com/TimmySheep/VoiceToTextApp`，当前 **PRIVATE**，默认分支 `main`，无 LICENSE（公开前必须补）。
- CI：`.github/workflows/windows-validation.yml`（windows-2022 runner）已具备完整管线：`cargo fmt --check` → `cargo test --workspace --locked` → `clippy -D warnings` → `cargo build --release` → `npm ci/check/build` → MSI 构建 → 产物校验（SHA256）→ 上传 artifact（7 天）。
- **实测：最近 8 次推送全部 CI failure**，首个失败步骤是 `cargo fmt --all -- --check`（`recording.rs` 手写代码未过 rustfmt）。CI 从未走到编译步骤，因此 Windows 编译可行性此前没有被真正验证过。
- 构建历史规模：`Cargo.toml`（workspace）+ `Cargo.lock`（约 1.2 MB）+ `apps/desktop/package-lock.json`（约 340 KB）都在版本控制内，可复现构建 ✓。

## 2. 技术栈与跨平台依赖审查

| 组件 | 依赖 | 跨平台性 | 结论 |
|---|---|---|---|
| transcription-core | symphonia, sherpa-onnx, fs2, serde | 基本跨平台（sherpa-onnx 官方提供 windows-msvc 预编译，待 CI 实证） | 可用 |
| transcription-service | **std::os::unix（UnixListener/UnixStream、PermissionsExt）** | **不可编译于 Windows** | 需改造 |
| transcription-cli / mcp | 经 `ServiceClient::connect` 连接 | 随 service 改造 | 可用 |
| desktop (tauri) | tauri 2, cpal, hound, core-foundation, objc2-core-audio* | cpal/hound 跨平台；**core-foundation、objc2-core-audio* 为 macOS 专属却写在通用 [dependencies]** | 需门控 |
| desktop recording.rs | cpal（麦克风，跨平台）+ Core Audio Process Tap（应用内录，仅 macOS） | 已按 cfg 分叉（非 macOS 有 stub） | 仅需依赖门控 |

关键发现（均已在代码中核实）：

1. `crates/transcription-service/src/lib.rs:21-22` 无条件 `use std::os::unix::...`，服务端监听与客户端连接都绑定 Unix socket —— Windows 下整个 workspace 编译失败。**这是 Windows 兼容的第一障碍。**
2. 所有客户端（GUI/CLI/MCP）统一走 `ServiceClient::connect(data_dir)`，IPC 改造可收敛在 service 一处。
3. `apps/desktop/src-tauri/Cargo.toml` 的通用 `[dependencies]` 含 `core-foundation`、`objc2-core-audio`、`objc2-core-core-foundation`、`objc2-core-audio-types` 四个 macOS-only crate，需移入 `[target.'cfg(target_os = "macos")'.dependencies]`（objc2/objc2-app-kit/objc2-foundation 已正确门控）。
4. `recording.rs` 平台分叉已完成：`spawn_app`/`list_apps` 非 macOS 有 stub，麦克风路径（cpal+hound）天然跨平台，Windows Early Beta 可保留麦克风录音，应用音频返回"仅支持 macOS"错误。
5. `transcription-core/jobs.rs` 已含 `cfg(windows)` 进程启动逻辑；service 的 `default_data_dir` 已含 Windows 路径分支。此前已有人做过部分 Windows 考量。

## 3. Windows 构建链与 Smart App Control

- 本机（macOS）不能产出 Windows 安装包；任务书允许的路径：**GitHub Actions windows-2022 真实编译出 MSI/NSIS** + **真实 Windows 机器（REDACTED-HOST）安装启动验证**。
- 历史阻塞：Smart App Control（DeviceGuard 3033/3077）曾在目标 PC 上拦截 Cargo 构建脚本执行。SAC 拦截的是"在该机上编译"，不影响"CI 编译 + 该机安装 MSI"路径；MSI 安装与 exe 启动是否被 SAC 拦截需真机实测（Early Beta 门槛项）。
- 证书：无可信代码签名证书；NSIS/MSI 均不签名。Windows SmartScreen 首次运行警告必须在说明中如实描述。
- 渲染层：Tauri 2 在 Windows 用 WebView2（Evergreen，MSI 引导安装器会带运行时安装策略）；macOS 用 WKWebView。UI 需真机截图，不能用 macOS 截图冒充。

## 4. 局域网 Compute V1.1 集成评审

- 现有 service 为纯本地 Unix socket、无网络监听；Compute Node 需**新增**网络面（HTTP API + mDNS 发现），不能塞进现有 service 端口语义里。方案：新增独立库 crate `crates/lan-compute` + 独立可执行 `local-transcribe-node`（Node 角色），GUI 作为 Client 集成。这样 MCP"无网络监听"的产品边界不受影响（MCP 仍纯 stdio）。
- 依赖选型（全部跨平台、社区维护活跃）：
  - `axum`（HTTP API，tokio 生态）；
  - `reqwest`（rustls，无 openssl，Client 侧传输）；
  - `mdns-sd`（纯 Rust mDNS/Zeroconf 实现，Windows/FreeBSD 均支持）；
  - `zip`（DATASET 包）；`sha2`（清单哈希）。
- 文件传输与任务状态机按任务书协议实现（INIT/…/COMPLETED/FAILED），断点续传用分片清单（"3/5 个分片"）。
- 传输/待传队列/收集 API 每次只执行一个；进度与转写队列严格分离。

## 5. GUI 设计稿（早期 Beta）与设计冻结

- 按任务书第一章：Windows **固定**早 Beta 功能设计稿（灰度工具风）；macOS 不改版、保持 Anthropic 纸质浅色主题。
- 设计稿以 HTML 形式提供（`design/windows-early-beta.html`），标注为冻结稿。
- macOS 截图来自真实运行；Windows 截图必须来自真机（生成后替换占位）。

## 6. 发布门槛（Gate）自查清单

| Gate | 要求 | 当前状态 |
|---|---|---|
| G1 构建可复现 | lock 文件齐 + CI 真实构建 | lock 齐 ✓；CI 当前红，Phase 1 修复中 |
| G2 验证边界 | 不把单平台验证说成全平台 | 报告分开写，遵守 |
| G3 资源真实性 | 安装包/截图/样例真实 | 待 Phase 5 |
| G4 权限与隐私 | 不泄密、公开前 secret scan | 待 Phase 5（gh secret scan + git 历史扫描） |
| G5 诚实发布 | 不用构建命令冒充用户安装验证 | 真机安装验证列入门槛 |

## 7. 真实验证条件盘点

- 目标 Windows PC：REDACTED-HOST（`win` / REDACTED-USER@REDACTED-TAILSCALE-IP，Tailscale），SSH 可达（公开 key，非密码）。
- 唤醒链路：HA `pc_wake`（`button.wake_on_lan_REDACTED-MAC`）经交叉验证归属该 PC。实际执行：HA REST API 无可用 token（宿主 `secrets.yaml` 仅占位符），改为在 HA 同一宿主（REDACTED-LAN-IP/24）直接向 `REDACTED-LAN-IP:9` 发送与 HA WoL 按钮**相同 MAC、相同广播**的魔术包（REDACTED-MAC），效果等同按该按钮。PC 已成功唤醒并 SSH 登录（hostname `REDACTED-HOST` 交叉验证通过）。
- 局域网跨设备测试条件：Mac（Client）与 PC（Node）同经 Tailscale/局域网互通，满足"同一路由器或虚拟局域网"要求。
