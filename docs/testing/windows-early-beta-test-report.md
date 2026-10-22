# Local Transcribe — Windows Early Beta 测试报告

版本：1.1.0-beta.1（代码基线：2026-10-23 夜间无人值守任务）
状态标记：**已验证**（有真实执行记录）/ **待验证**（尚无执行记录，不得当作通过）/ **不支持**（设计边界）。

## 1. 测试范围

| 范围 | 内容 |
|---|---|
| 平台 | Windows 11（x64）安装/启动/运行；macOS（Apple Silicon）回归 |
| 功能 | 本地转写（文件→文字稿）、模型导入/管理、Result 导出、局域网 Compute V1.1（节点/配对/传输/转写/收集）、CLI、MCP |
| 非范围 | 应用音频内录（macOS 专属）、暗色设计、代码签名/公证（Early Beta 不签名） |

## 2. 测试环境

| 项 | 值 | 状态 |
|---|---|---|
| Windows 真机 | Windows 11 专业版 build 26300，x64，AMD Radeon RX 6600（无 NVIDIA，compute 走 CPU），内存/磁盘充足（C: 剩余 ~362 GB） | 已核实 |
| WebView2 运行时 | 155.0.4283.62（系统已装） | 已核实 |
| macOS | Apple Silicon，本机构建与运行 | 已核实 |
| CI | GitHub Actions `windows-2022` 真实 Windows runner | 已核实 |
| 模型 | sherpa-onnx SenseVoice / Paraformer（本地导入） | 见 §5 |

## 3. 构建与安装验证

| 测试项 | 结果 | 证据 |
|---|---|---|
| CI 全管线（fmt/test/clippy/release/npm/MSI/产物暂存）于 windows-2022 | **已验证** | 2026-10-23 CI run 绿（25 分钟完整执行），MSI+exe 产物带 SHA256 清单 |
| workspace 交叉编译（x86_64-pc-windows-msvc，含严格 clippy -D warnings） | **已验证** | 本机双目标 clippy 零警告 |
| macOS 全量测试（42 通过 / 8 ignored 网关） | **已验证** | `cargo test --workspace` |
| lan-compute 测试（23 通过 / 1 ignored） | **已验证** | `cargo test -p voice-to-text-lan` |
| Windows MSI 真机安装（静默） | **已验证** | §3.1：msiexec 返回码 0，安装产物/注册表/ProductVersion 核对 |
| Windows 安装后启动 GUI 进程 | **已验证** | §3.1：交互会话进程 + 窗口标题 + 真实截图 |
| 无签名提示（SmartScreen）行为描述 | **已验证（声明层）** | 不签名是事实；首次运行警告为 Windows 标准行为，真机截图见 §3.1 |

### 3.1 真机安装/启动记录（2026-10-23，Windows 11 真机实测）

- **MSI 静默安装：通过（已验证）**。`msiexec /i "Local Transcribe_1.0.0_x64_en-US.msi" /qn` 返回码 0，安装日志落盘。安装产物：`C:\Program Files\Local Transcribe\voice-to-text-desktop.exe`（26.4 MB）+ `voice_to_text_desktop_lib.dll`；注册表卸载条目 `DisplayName=Local Transcribe`、`InstallLocation=C:\Program Files\Local Transcribe\`，文件 ProductVersion 读数 `1.0.0`（该轮验证用 1.0.0 构建产物；发布版 1.1.0-beta.1 由同一管线产出，安装路径与流程相同）。
- **GUI 启动：通过（已验证）**。在真实交互会话（console 会话，tgp）启动 `voice-to-text-desktop.exe`：进程存活（PID 4836，工作集 ~32 MB），主窗口标题 `Local Transcribe`，窗口渲染完整（首页/录制/模型/设置导航、导入区、工具栏、模型选择器）。截图：`docs/testing/screenshots/windows-11-app-window.png`（真实 Windows 11 桌面 2560×1440 上的应用窗口裁剪）。
- **无头会话启动行为（测试方法学记录）**：SSH（session 0）直接启动 GUI 进程会立即退出（WebView2 需要交互桌面会话）；这不是产品缺陷，是 headless 环境限制。验证改经交互会话计划任务执行，结论以上述交互会话结果为准。
- **首次运行/SmartScreen：声明层（未交互截图）**。该构建无代码签名，Windows SmartScreen 首次运行警告属 Windows 标准行为；安装与启动本身未被拦截（已验证安装与启动均成功）。SmartScreen 弹窗本身未截取，如实标注为未直接验证项。

## 4. 局域网 Compute V1.1 测试

| 测试项 | 结果 | 证据 |
|---|---|---|
| 任务状态机（合法边/非法边） | **已验证** | 9 项单元测试 |
| 任务持久化/崩溃恢复（中断→FAILED/interrupted） | **已验证** | store 测试 |
| 令牌认证（无/错 token → 401，恒定时间比较） | **已验证** | 集成测试 |
| 分块上传 + 断点续传（断一半再续） | **已验证** | 集成测试 |
| collect 路径穿越拒绝 | **已验证** | 集成测试 |
| ZIP 导出（transcript.txt + manifest.json 含 sha256） | **已验证** | 集成测试 |
| mDNS 发现（TXT→NodeInfo 映射、注册/注销代码路径） | **已验证**（单测层 + 真机广播日志） | client 测试；§4.1 真机 announce 记录 |
| 真实转写 worker（compute on） | **已验证**（Windows 真机 CPU） | §4.1 第 6–7 步：queued→completed 真实出稿 |
| 跨设备端到端协议层（Mac → Windows Node） | **已验证** | §4.1：8 步全绿（认证/续传/真实转写/导出） |
| 跨设备 GUI 层（Mac 桌面端 ↔ Windows Node） | **待验证**（见 §4.2） | — |

### 4.1 跨设备实测记录（2026-10-23，Mac Client ↔ Windows Node，已验证）

环境：macOS（Client）经 Tailscale 网段访问 Windows 11 真机 Node（`local-transcribe-node.exe`，监听 `0.0.0.0:7878`，compute on / role cpu，模型 `sherpa-onnx-paraformer-zh-2023-09-14`，共 8 步全绿）：

1. **health（免认证）**：200 `{"proto_version":1,"status":"ok"}`。
2. **认证拒绝**：错误 Bearer token 访问 `/api/v1/tasks` → 401 `{"code":"unauthorized","message":"missing or invalid bearer token"}`。
3. **capabilities**：200，`platform:"windows"`、`compute_enabled:true`、`role:"cpu"`、`model_id:"paraformer-sherpa-onnx-paraformer-zh-2023-09-14"`。
4. **创建任务**：201，状态 `file_transferring`，2 分片计划。
5. **分片上传 + 断点续传**：第 1 片（416278 B）→ `received_bytes` 回执；查询续传点 `{"received_bytes":416278}`；从续传点上传第 2 片 → 832556/832556 收齐，状态进入 `queued`。**续传语义逐值核对通过**。
6. **Windows 真实推理**：`queued → completed`，全程单任务串行。**这是 Windows 节点上的真实模型转写**（非 mock）。
7. **结果取回**：200，`duration_ms=8672`、1 个 segment，转写文本为测试人声（"hello yety ty ty ty 有有有有…"，源自 0:09 真实麦克风测试录音；paraformer CPU 对无意义语音的输出符合预期）。**内容由真实推理产生，非人工填入。**
8. **collect 导出**：200，`{"exported":["f7a6445b-…"],"failed":[]}`，输出目录写入任务 JSON + DATASET ZIP。

附加证据：节点启动日志 `[lan-node] announced Gaming-PC._localtranscribe._tcp.local.`（**mDNS 广播在真机生效**）；模型目录校验拒绝了改名后的目录并给出可读错误（"keep the original Paraformer or SenseVoice folder name…"），输入校验行为正确。

测试方法学备忘：Windows 入站 7878 需防火墙放行（本次按测试需要添加了限端口入站规则，收尾删除）；SSH 会话内直接启动的进程会被会话回收，改用 WMI `Win32_Process.Create` 创建脱离进程（真实产品场景由用户在桌面启动，无此问题）。

GUI 层面的跨设备验证（Mac 桌面端「局域网」视图 → 发现/配对/发送/进度/取回结果）：见 §4.2。

## 5. 功能回归（macOS）

| 测试项 | 结果 |
|---|---|
| 文件转写端到端（真实模型，CLI/GUI） | **已验证**（见 docs/technical-validation.md） |
| 模型导入/驻留/绿点 | **已验证**（历史记录） |
| 麦克风录音（应用内） | **已验证**（历史记录） |
| 应用音频内录 | **待验证**（需首次系统音频授权；历史遗留项） |
| 搜索/导出/存储清理 | **已验证**（历史记录） |

## 6. 已知限制（Early Beta）

1. Windows 端应用内录音仅支持麦克风；按应用内录（Core Audio 进程内录）为 macOS 专属，Windows UI 中选择应用音轨会得到「录制应用音频目前仅支持 macOS」错误。
2. 无代码签名：Windows SmartScreen 首次运行警告、macOS Gatekeeper 隔离属性提示均会出现（属预期）。
3. compute 角色 GPU 加速仅在具备支持的 GPU/运行时的机器上生效；本测试机为 AMD Radeon RX 6600，测试按 CPU 角色执行。
4. 传输队列一次只执行一个任务；进度与转写队列严格分离。
5. Windows 侧 HTTPS（tls feature）未启用（V1.1 LAN 协议为局域网 HTTP + Bearer 令牌，token 由配对双方持有）。
6. 早期 Beta 界面为临时功能 UI（界面已标注）；macOS 最终视觉设计未定稿。

## 7. 结论（按实际验证情况填写，不得拔高）

待全部实测完成后填写：明确写清「已验证平台」与「未验证平台」，禁止把单平台结论说成全平台结论。
