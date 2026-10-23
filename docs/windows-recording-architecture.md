# Windows 精准应用录音架构（Application Loopback）

任务书：《Local Transcribe — Windows 精准应用录音与多音轨开发任务书》。
分支：`windows-recording`。本文对应任务书 Phase A（可行性验证与架构确认）与 Phase B（Windows 后端实现）。

**状态总览（2026-10-23）**

| 事项 | 状态 |
| --- | --- |
| 现状审计 | ✅ 已完成（见 §1） |
| Windows API 调研 | ✅ 已完成并对照 windows 0.61.3 注册表源码逐符号核实（见 §2） |
| 可行性原型编译 | ✅ 真机（Tims-Gaming-PC）编译通过 |
| 可行性原型运行（隔离/麦克风/应用关闭实测） | ⛔ 被 Smart App Control 阻塞，待用户操作后补测（见 §3.3） |
| Phase B 后端代码 | ✅ 已实现，双目标 `cargo check`/`clippy -D warnings` 等价检查清零 |
| Phase B 运行时验证 | ⛔ 同上，待补 |

## 1. 现状审计

- 共享契约（本任务不得改动）：`apps/desktop/src-tauri/src/recording.rs` 的
  `TrackSpec{kind:"input"|"app", target, label}`、`RecordedTrack(s)`、`TrackLevel`、
  `RecordingStatus`、`AudioApp`，以及 `start/stop/status` Tauri 命令。
  每轨独立 WAV，mono i16。
- 分支基点（`9d9b473`，任务书一发布版 1.1.0-beta.1）上的 Windows 状态：
  - `spawn_app`（应用音频）在非 macOS 平台为占位实现，返回「仅支持 macOS」错误。
  - `list_apps` 在非 macOS 平台返回空列表。
  - 麦克风路径基于 cpal，Windows（WASAPI 后端）可编译，设备枚举与采集为跨平台代码。
- 真机环境（均实测）：
  - Windows PC build **26300**（≥ 19041，满足 Application Loopback 前提）；
    音频栈正常（Realtek/AMD/USB 设备在位，Audiosrv 运行）。
  - 本机工具链：cargo 1.99.1、MSVC 2022、SDK 10.0.26100.0。
  - 测试音源已就位：`C:\Users\TGP\rec-probe\t440.wav`、`t880.wav`（48 kHz 立体声 10 s）。

## 2. API 调研（全部对照 windows 0.61.3 注册表源码核实）

- 按进程捕获走 **Application Loopback**：
  `ActivateAudioInterfaceAsync(PCWSTR "__DEFAULT_LOOPBACK_DEVICE_ID", &IAudioClient::IID, Some(pv), handler)`，
  其中 `pv` 为 `*const AUDIOCLIENT_ACTIVATION_PARAMS as *const PROPVARIANT`
  （结构体指针按 ABI 直接作激活参数传入，与微软 ApplicationLoopbackAudio 示例一致）；
  `ActivationType = AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`，
  `ProcessLoopbackMode = PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`（捕获整个进程树输出）。
- 完成回调接口真实名称为 `IActivateAudioInterfaceCompletionHandler`，
  方法签名 `ActivateCompleted(Ref<'_, IActivateAudioInterfaceAsyncOperation>)`；
  `#[implement]` 宏要求为宏生成的 `Xxx_Impl` 类型实现 trait（`windows-implement` 0.60.2 源码确认），
  且 **`windows-core` 必须作为直接依赖**（代码生成引用 `windows_core::` 路径）。
- 回调槽只发信号；接口结果由录制线程调 `GetActivateResult(*mut HRESULT, *mut Option<IUnknown>)`
  取回后 `.cast::<IAudioClient>()`。
- 采集侧：`IAudioClient::GetMixFormat` → `Initialize(SHARED, AUTOCONVERTPCM|SRC_DEFAULT_QUALITY, …)`
  → `GetService::<IAudioCaptureClient>` → `GetNextPacketSize()/GetBuffer()/ReleaseBuffer()`。
- 进程工具：ToolHelp（`CreateToolhelp32Snapshot/Process32FirstW/Process32NextW`，
  位于 `Win32::System::Diagnostics::ToolHelp`）；窗口枚举 `EnumWindows/GetWindowThreadProcessId/IsWindowVisible`
  （`HWND` 位于 `Win32::Foundation`）；进程存活 `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)+WaitForSingleObject`。
- 版本判定（验收 T01）：`RtlGetVersion`（`Wdk::System::SystemServices`）
  判 build < 19041 时直接给出准确提示，不依赖激活失败的间接报错。
- 语言/结构细节：crate 需 edition 2024（let-chains）；`WAVEFORMATEX` 为 packed 结构，
  字段必须先按值拷出再使用；`GetNextPacketSize` 无参数、返回 `Result<u32>`。

## 3. 可行性原型（lt-loopback-probe）

独立探针 crate（未入仓库工作区，避免影响 CI）：子命令 `list`（进程枚举）与
`capture <pid> <seconds> <out.wav>`（按 PID 捕获、下混 mono i16 WAV、检测目标进程退出）。

- ✅ 在 Tims-Gaming-PC 真机 `cargo build --release` 编译通过（windows 0.61.3 + windows-core 0.61.2）。
  编译迭代中核实的 API 事实已全部并入 §2。
- 探针源码现位于 Mac 临时目录与 PC `C:\Users\TGP\lt-loopback-probe\`；
  其生产等价实现即 §4 的 `recording_windows.rs`，后续以仓库代码为准。

### 3.1 原计划的运行时验证（待补）

- T02 隔离：ffplay 播 t440.wav（目标 PID）+ ffplay 播 t880.wav（干扰），捕获目标 PID，
  离线频谱分析应只见 440 Hz、无 880 Hz。
- T03 麦克风：cpal 麦克风轨与 loopback 应用轨同时采集，两条独立 WAV。
- T08 应用关闭：捕获中结束目标进程，确认继续输出静音且已录数据封口可恢复。

### 3.2 拦截证据（已核实事实）

真机运行未签名新构建 exe 时被 Device Guard 拦截（「未能满足企业签名级别要求」）：

- CodeIntegrity/Operational 事件 3033：13:34:52（lt-loopback-probe.exe）、13:44:00（sac-probe.exe 重建产物）。
- `Win32_DeviceGuard`：`CodeIntegrityPolicyEnforcementStatus = 2`（强制）；
  CiTool 策略列表：`VerifiedAndReputableDesktop` **IsEnforced: true**，另新增
  `VerifiedAndReputableDesktopFlightSupplemental`（IsEnforced: true）。
- 时间线：PC 于 **12:41:39 重启**（LastBootUpTime）。重启前（任务书一测试期）未签名本地构建
  与 CI 产物均可运行（GUI E2E、LAN 节点实测为证）；重启后所有**新**未签名 exe 一律被拦。
  早晨的 `local-transcribe-node.exe` 因重启前已获放行判定，现仍可运行——按文件判定，非按签名类型。
- 交互会话计划任务（`schtasks /IT`，TGP 控制台会话）同样被拦——拦截为策略级，与会话无关。

### 3.3 结论与待办

Smart App Control 强制模式会拦截所有新构建（本地与 CI 产物同样未签名）。关闭 SAC 属
机器级安全设置，须用户本人在「Windows 安全中心 → 应用和浏览器控制 → 智能应用控制 → 关闭」
操作（已发通知，等待操作）。SAC 关闭后按 §3.1 补齐全部运行时证据并更新本文档。

## 4. Phase B 实现（已交付，运行时待验证）

- 新增 `apps/desktop/src-tauri/src/recording_windows.rs`（`#[cfg(windows)]` 模块）：
  - `run_loopback`：按 PID 捕获目标进程树输出音频。
    - 采样格式：混音格式为 float32（含 EXTENSIBLE 包装）或 PCM16 时支持；
      其他格式如实报错（不猜格式）。
    - 输出：mono i16 WAV，采样率取系统混音引擎格式（通常 48 kHz），与麦克风轨文件规格一致。
    - 电平：50 ms 窗口峰值，写入共享 `TrackLevel` 槽。
    - 目标进程退出（T08）：检测到后在错误槽写入「目标应用已退出，正在继续录制静音；
      可停止后转写已录内容。」，继续录制静音，停止时正常封口——已录数据完整可恢复。
    - 写盘/缓冲错误（T12）：如实写入错误槽，不伪报成功。
  - `visible_apps`：枚举带可见窗口的进程（`EnumWindows` + ToolHelp 进程名，去 `.exe`），
    与 macOS `runningApplications` 的可用性语义对齐；System/Idle 进程排除。
- `recording.rs`：`spawn_app`/`list_apps` 增加 `cfg(windows)` 实现，
  其余非 macOS 平台保留占位（文案改为「仅支持 macOS 与 Windows」）；
  `store_level` 提为 `pub(crate)` 供 Windows 模块复用。共享类型与命令契约**零改动**。
- `Cargo.toml`：`[target.'cfg(windows)'.dependencies]` 增加 `windows 0.61`（所需特性清单见文件）
  与 `windows-core 0.61`。
- 验证（Mac 交叉检查，不链接、不执行产物）：
  - `cargo check --target x86_64-pc-windows-msvc -p voice-to-text-desktop` ✅
  - `cargo clippy --target x86_64-pc-windows-msvc -p voice-to-text-desktop` 0 警告 ✅
  - macOS 目标 `cargo check`/`clippy` 回归 ✅（录制 macOS 路径未触碰）
  - 构建环境备注：Mac 上交叉检查需 `llvm-rc`（brew llvm，已在 PATH）与
    `http(s)_proxy` 指向本机代理（sherpa-onnx-sys 下载器）；`ALL_PROXY` 须 unset（isahc 误解析空值）。

## 5. 已知限制与边界（如实记录）

1. **运行时未验证**：SAC 阻塞期间，Application Loopback 的隔离性、静音包行为、
   目标退出语义均未在真机跑通；§4 代码仅完成类型级验证。T02/T03/T08 等实测通过前，
   不得宣称 Windows 应用录音可用。
2. `list_apps` 仅列带可见窗口的进程：无窗口的音频进程（如 `ffplay -nodisp`、后台服务）
   不出现在列表；但按契约 `TrackSpec.target` 接受任意 PID 字符串，API 调用不受限。
3. 麦克风轨运行时拔插的错误回传：cpal 流错误回调目前仅打日志（沿用现状），
   「UI 明确状态」在 Phase D 真测中验证，不足则补。
4. 混音格式仅支持 float32/PCM16；其他格式报错并请求反馈，不静默转换。
5. `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` 会捕获目标进程树的全部子进程输出
   （如浏览器的 GPU/实用进程），与「精确到应用」的用户语义一致，但不等于单一 PID。
6. SAC 拦截期间，Phase D 的安装器测试（T13）同样受阻——MSI 产物亦为未签名新文件。

## 6. 后续路径

1. 用户关闭 SAC → 补 §3.1 三项原型实测（探针）→ 更新 §3 证据。
2. Phase C：RecordView 集成（Windows 应用选择、逐轨状态显示含「目标应用已退出」）、
   录后直接转写联动（T10）。
3. Phase D：T01–T14 真机测试矩阵、回归（T14）、安装器（T13）、测试报告与双语 README 更新。
