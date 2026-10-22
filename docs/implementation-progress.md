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
- 代理备忘：sherpa-onnx-sys 下载脚本不支持 SOCKS，构建时需 `env -u ALL_PROXY`（保留 HTTP(S)_PROXY）。

待实证（Phase 5）：GitHub Actions windows-2022 真实构建 MSI + 真机安装启动。
