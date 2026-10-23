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

独立探针 crate（未入仓库工作区，避免影响 CI）：子命令 `list`（进程枚举）、
`capture [exclude] <pid> <seconds> <out.wav>`（按 PID 捕获、下混 mono i16 WAV、
检测目标进程退出）、`capdev`（经典默认端点回环对照）、`sysvol`（端点音量/静音/3 角色端点）。

- ✅ 在 Tims-Gaming-PC 真机 `cargo build --release` 与 `cargo test` 均通过
  （windows 0.61.3 + windows-core 0.61.2）。编译迭代中核实的 API 事实已全部并入 §2。
- 探针源码现位于 Mac 临时目录与 PC `C:\Users\TGP\lt-loopback-probe\`；
  其生产等价实现即 §4 的 `recording_windows.rs`，后续以仓库代码为准。

### 3.1 运行时验证（2026-10-23 真机实测）

**T02 精准隔离 — ✅ 已验证**。SSH 上下文双 ffplay 并行：目标 PID 播 440 Hz，另一进程
播 880 Hz（均 60 s 单发音文件，裸 `-nodisp -autoexit`），捕获后离线 Goertzel：

| 采样 | G(440 Hz) | G(880 Hz) | 峰值 |
|---|---:|---:|---:|
| `devmix2.wav`（默认端点混音，对照） | 2894.7 | 2895.0 | 5762/32767 |
| `cap440.wav`（目标 PID 进程 tap） | **2894.2** | **0.010** | 3028/32767 |

端点混音含两音（对照成立），进程 tap 只含目标 440 Hz、他应用 880 Hz 泄漏≈0——
**「精准到应用的隔离」核心红线达成**（同一时刻两声源均存活）。

**T08 目标退出 — ✅ 已验证（探针级，TDD 红→绿）**。捕获 10 s，进行到 4 s 时强杀目标；
`kill10.wav` 分秒分析：t=0–4 s 满幅 440 Hz（G≈2800–2894），t=4–10 s 纯净静音
（peak=1 即 dither），全程 9.95 s 正常封口 RC=0，stderr 出现
`警告: 目标进程 PID … 已退出，余下时段为静音。`。首次实跑该警告**未**触发——根因
`process_alive` 仅持 `PROCESS_QUERY_LIMITED_INFORMATION` 打开句柄，而
`WaitForSingleObject` 要求 `SYNCHRONIZE`（0x00100000），缺它同步恒 `WAIT_FAILED`，
进程对象被他人句柄持有（PID 仍可打开）时永判「存活」。按 TDD 于真机先写聚焦测试复现红
（`目标已退出但句柄仍被持有时必须报告不存活`），补 `SYNCHRONIZE` 后绿；测试同步移植生产
端 `recording_windows.rs`（`mod tests`，随 Windows 目标编译）。

**T03 麦克风 — ⏳ 未在本阶段验证**（任务书既定：推迟至 Phase C/D 应用级 cpal 路径）。

**执行上下文说明**：以上绿数据取自 SSH 直跑上下文（`powershell -File`；`qwinsta` 显示
查询进程位于 services 会话 0）。同探针二进制在 `schtasks /IT` 任务上下文中
`GetDefaultAudioEndpoint` 报 `找不到元素 (0x80070490)`、捕获仅得静音 dither；该上下文中
ffplay「存活但无声」（疑 SDL 落 dummy 驱动，未逐层隔离证实）。会话事实与影响见 §5。

### 3.2 拦截证据（已核实事实，SAC 关闭前的历史记录）

真机运行未签名新构建 exe 时被 Device Guard 拦截（「未能满足企业签名级别要求」）：

- CodeIntegrity/Operational 事件 3033：13:34:52（lt-loopback-probe.exe）、13:44:00（sac-probe.exe 重建产物）。
- `Win32_DeviceGuard`：`CodeIntegrityPolicyEnforcementStatus = 2`（强制）；
  CiTool 策略列表：`VerifiedAndReputableDesktop` **IsEnforced: true**，另新增
  `VerifiedAndReputableDesktopFlightSupplemental`（IsEnforced: true）。
- 时间线：PC 于 **12:41:39 重启**（LastBootUpTime）。重启前（任务书一测试期）未签名本地构建
  与 CI 产物均可运行（GUI E2E、LAN 节点实测为证）；重启后所有**新**未签名 exe 一律被拦。
  早晨的 `local-transcribe-node.exe` 因重启前已获放行判定，现仍可运行——按文件判定，非按签名类型。
- 交互会话计划任务（`schtasks /IT`，TGP 会话）同样被拦——拦截为策略级，与会话无关。

### 3.3 结论与待办

**2026-10-23：Timmy 已本人关闭 Smart App Control**，未签名本地构建恢复可运行
（此后探针与测试 exe 均正常执行，§3.1 绿数据即关闭后取得——此为实证）。
§3.2 保留关闭前的原始取证。剩余待办：T03 麦克风（Phase C/D 应用级）、
生产端代码在活跃交互会话的真机运行（Phase C/D）。

### 3.4 真机故障链与修复（按发现顺序，逐项已修）

1. **激活 `0x80070002`（找不到文件）**：设备路径曾误用字面量
   `__DEFAULT_LOOPBACK_DEVICE_ID`（不存在），且 PROPVARIANT 以 reinterpret_cast 置
   `vt=1` 被解释为 VT_NULL。修复：MS 官方样例 `ApplicationLoopback` 同款——
   `VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK`（`VAD\Process_Loopback`）+ VT_BLOB 容器
   （`vt=65`、`blob.cbSize/pBlobData`，`#[repr(C)] struct PropVariantBlob`）。
2. **`GetMixFormat` → `E_NOTIMPL (0x80004001)`**：进程回环激活的 `IAudioClient` 不支持
   查询混音格式（真机实测）。修复：不查询、直接给定固定格式 + `AUTOCONVERTPCM`。
3. **`Initialize` → `0x88890021`（AUDCLNT_E_FORMAT_NOT_SUPPORTED）**：缺
   `AUDCLNT_STREAMFLAGS_LOOPBACK`。修复：对齐 MS 样例——PCM16 44100/2ch、
   `LOOPBACK | EVENTCALLBACK | AUTOCONVERTPCM`、duration 0、`CreateEventW` +
   `SetEventHandle`（探针需 `Win32_Security`、`Win32_Media_Audio_Endpoints` 特性；
   生产侧需 `Win32_Security`）。阶段中曾试 f32/48k/2s 配置：Initialize 成功但三源捕获
   全零，与会话上下文问题叠加、未单独归因；最终以样例同款配置取得实测绿为准。
4. **音源脚本坑**：ffplay `-stream_loop -1` 立即退出；改 60 s 单发音文件
   （`ffmpeg -f lavfi -i sine=…` 生成）+ 裸 `-nodisp -autoexit`。PowerShell 任务上下文
   中 `System.Timers.Timer` 行为异常，改 `System.Media.SoundPlayer.PlayLooping()`。
5. **`/IT` 任务上下文异常**：同二进制任务上下文端点枚举 `0x80070490`、捕获全零，
   SSH 上下文正常；t02d→t02e 之间零代码改动，属执行上下文差异（见 §5 限制 1）。
6. **`process_alive` 缺 `SYNCHRONIZE`**：见本节 T08 段（红→绿已修，测试守护）。

## 4. Phase B 实现（已交付，配置已对齐真机验证值）

- 新增 `apps/desktop/src-tauri/src/recording_windows.rs`（`#[cfg(windows)]` 模块）：
  - `run_loopback`：按 PID 捕获目标进程树输出音频。
    - 采样格式：固定 PCM16 / 44100 Hz / 2 ch（MS ApplicationLoopback 样例同款、
      真机实测绿的配置；`SampleKind::I16` 活跃，`F32` 保留为未启用的回退分支）。
      `GetMixFormat` 在进程回环下不可用（E_NOTIMPL），不猜格式、直接给定（§3.4-2）。
    - 输出：mono i16 WAV（44100 Hz），与麦克风轨文件规格一致。
    - 流初始化：`LOOPBACK | EVENTCALLBACK | AUTOCONVERTPCM` + duration 0 +
      `CreateEventW`/`SetEventHandle`；读取循环 10 ms 轮询（探针同款、实测绿）。
    - 电平：50 ms 窗口峰值，写入共享 `TrackLevel` 槽。
    - 目标进程退出（T08）：`process_alive` 以 `QUERY_LIMITED_INFORMATION | SYNCHRONIZE`
      打开句柄（缺 SYNCHRONIZE 的教训与红→绿测试见 §3.1）；检测到后在错误槽写入
      「目标应用已退出，正在继续录制静音；可停止后转写已录内容。」，继续录制静音，
      停止时正常封口——已录数据完整可恢复（探针级实测绿，生产端待 Phase C/D 实测）。
    - 写盘/缓冲错误（T12）：如实写入错误槽，不伪报成功。
  - `visible_apps`：枚举带可见窗口的进程（`EnumWindows` + ToolHelp 进程名，去 `.exe`），
    与 macOS `runningApplications` 的可用性语义对齐；System/Idle 进程排除。
- `recording.rs`：`spawn_app`/`list_apps` 增加 `cfg(windows)` 实现，
  其余非 macOS 平台保留占位（文案改为「仅支持 macOS 与 Windows」）；
  `store_level` 提为 `pub(crate)` 供 Windows 模块复用。共享类型与命令契约**零改动**。
- `Cargo.toml`：`[target.'cfg(windows)'.dependencies]` 增加 `windows 0.61`（特性清单含
  `Win32_Security` 等，见文件）与 `windows-core 0.61`。
- 验证（Mac 交叉检查，不链接、不执行产物）：
  - `cargo check --target x86_64-pc-windows-msvc`（lib 与 `--tests`）✅
  - `cargo clippy --target x86_64-pc-windows-msvc --tests` 0 警告 ✅；`cargo fmt` ✅
  - macOS 目标 `cargo check`/`clippy` 回归 ✅（录制 macOS 路径未触碰）
  - **PC 真机整包链接与冒烟（2026-10-23 Phase C）**：Tims-Gaming-PC 本机 `cargo build`
    （debug）成功——全工作区首次真链接（`voice-to-text-desktop.exe` 46.7 MB）；本机
    git/node 24/npm/Rust+MSVC 均既有（事前核查，未新装任何工具）。sherpa-onnx-sys 从
    GitHub releases 下载预编译库在 PC 直连超时（10060），改由 Mac 代理下载官方
    v1.13.8 win-x64 静态库归档（14 个 .lib）经 LAN 传输，以 `SHERPA_ONNX_LIB_DIR`
    跳过下载。冒烟（SSH 上下文）：进程存活 10 s、WebView2 子进程 12 个（UI 真实渲染）、
    无 panic 输出、正常终止；收尾核查残留 6 个 WebView2 归属 session 1 的系统
    SearchHost（非冒烟遗留，保持未动）。
  - CI 预检（本机等价执行 CI 同款步骤）：`cargo fmt --all -- --check` 0 行；
    `cargo clippy --workspace --all-targets --locked -- -D warnings` exit 0；
    `cargo test --workspace --locked` exit 0。
  - 构建环境备注：Mac 上交叉检查需 `llvm-rc`（brew llvm，已在 PATH）与
    `http(s)_proxy` 指向本机代理（sherpa-onnx-sys 下载器）；`ALL_PROXY` 须 unset（isahc 误解析空值）。
  - **边界**：`mod tests` 在 Mac 只能类型检查（无法链接执行 Windows 测试二进制）；
    其红→绿在 PC 真机以探针同款代码完成（§3.1），生产端执行留待 Windows 主机/Phase D。

## 5. 已知限制与边界（如实记录）

1. **执行上下文与 TGP 会话（影响测试路径，非产品逻辑）**：`qwinsta` 实测（2026-10-23）：
   services 会话 0 = 断开（SSH 落此，音频链路可用）；**会话 1（TGP）= 断开**；
   console 会话 2 = 活动但无用户；`rdp-tcp` = 监听。`/IT` 任务向会话 1 派发时端点枚举失败
   （`0x80070490`）且捕获全零，SSH 上下文同二进制正常——关联明确（断开会话无可用默认
   端点），因果未在重连会话中单独复测。**产品真机验收须在 Timmy 活跃登录会话中进行**
   （Phase C/D 前提）；当前全部绿数据取自 SSH 上下文（同机、同混音引擎、同代码），
   交互会话实测待补。
2. **验证状态分层（如实标注）**：✅ 已验证（真机）：T02 隔离、T08 退出后继续录制与封口、
   激活/初始化故障链修复（§3.4，探针级）；生产端整包在 PC 链接成功并冒烟启动
   （进程存活、WebView2 渲染，SSH 上下文）。⏳ 未验证：T03 麦克风与录音音频实跑、
   活跃交互会话下的 UI/状态（切页、隐藏、队列）、T09–T14（Phase D）。
   **链接+冒烟 ≠ 音频链路可用**——在活跃会话实测通过前，不得宣称 Windows 应用录音
   整体可用。
3. `list_apps` 仅列带可见窗口的进程：无窗口的音频进程（如 `ffplay -nodisp`、后台服务）
   不出现在列表；但按契约 `TrackSpec.target` 接受任意 PID 字符串，API 调用不受限。
4. 麦克风轨运行时拔插的错误回传：cpal 流错误回调目前仅打日志（沿用现状），
   「UI 明确状态」在 Phase D 真测中验证，不足则补。
5. 采样格式固定 PCM16 44100（`F32` 分支保留未启用）；如遇不支持该固定格式的端点，
   如实报错，不静默转换。
6. `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` 会捕获目标进程树的全部子进程输出
   （如浏览器的 GPU/实用进程），与「精确到应用」的用户语义一致，但不等于单一 PID。
7. SAC 已由用户关闭（2026-10-23），Phase D 安装器测试（T13）不再受 SAC 拦截。

## 6. 后续路径

1. ~~用户关闭 SAC → 补 §3.1 原型实测~~（已完成，2026-10-23）；剩余运行时项
   （T03 麦克风、生产端真机、活跃交互会话）并入 Phase C/D。
2. Phase C：RecordView 集成（Windows 应用选择、逐轨状态显示含「目标应用已退出」）、
   录后直接转写联动（T10）。
3. Phase D：T01–T14 真机测试矩阵、回归（T14）、安装器（T13，由既有 CI
   `windows-validation.yml` 的 `workflow_dispatch` 在本分支 ref 构建 MSI 产物）、
   测试报告与双语 README 更新。
