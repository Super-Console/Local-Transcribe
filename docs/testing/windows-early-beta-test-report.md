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
| 跨设备 GUI 层（Mac 桌面端 ↔ Windows Node） | **部分验证**（配对/测试连接/状态展示已验证并修复 2 个真 bug；发送→结果闭环待验证，见 §4.2） | §4.2 |

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

### 4.2 GUI 层跨设备实测记录（2026-10-23，Mac 桌面端 GUI 驱动真实 Windows Node）

测试方式：在 macOS 端运行打包后的 `Local Transcribe.app`（1.1.0-beta.1 构建），通过 GUI 实际操作「局域网」视图（添加节点 → 保存 → 测试连接 → 状态/能力/存储展示），节点为 §4.1 同一 Windows 真机（CPU 角色 + Paraformer 模型）。GUI 由自动化工具按截图坐标驱动，每步回读截图核对。

**已验证（本轮 GUI 实测中发现并修复 3 个真实缺陷；前 2 个含 TDD 回归测试，第 3 个为能力白名单配置修复并经 GUI 实测验证）：**

| 环节 | 结果 |
|---|---|
| 添加节点（表单填写/校验/保存） | **已验证**：名称/主机/端口/令牌校验生效，保存后卡片即时出现 |
| 配置持久化 | **已验证**：节点配置跨应用重启与重装保留 |
| 测试连接的状态展示 | **已验证**：可达性/延迟/能力/存储分区展示（修复后） |
| 探测错误的真实呈现 | **已验证**：错误原文（如 401）显示在视图内，不再被吞没 |

**发现的缺陷（均已定位根因、TDD 修复、回归转绿）：**

1. **代理吞局域网（真 bug，跨平台影响）**：LAN client（`crates/lan-compute`）默认读取环境 `HTTP(S)_PROXY`/`ALL_PROXY`，在 Clash 等代理环境下把 100.x/192.168.x 私网地址送进代理导致「节点离线」；同机 `curl --noproxy` 恒通。修复：`LanClient::new` 显式 `.no_proxy()`（LAN 流量一律直连），回归测试 `crates/lan-compute/tests/proxy_policy.rs`（死代理环境下仍须直连成功）先红后绿。
2. **状态覆写误报离线（真 bug，GUI 状态逻辑）**：原 `loadStatus` 在存储探测（需鉴权）失败时走 catch，把已成功的 `reachable: true` 覆写成 `false`——令牌错误被误报为「节点离线」。修复：探测语义下沉 Rust 纯函数 `assemble_status`（可达性只由 health 探测决定，其余探测独立、失败进 `warning`），前端改为单次原子调用；3 个 TDD 单测（storage 失败不翻转可达性 / health 失败标不可达 / 全通过）先红后绿。

**测试自身的问题（非产品缺陷，如实记录）**：首次写入节点令牌时，自动化打字工具回报「送达 22/28 字符」不准确（实际 28 字符全部送达），补打后缀导致令牌多出 6 字符（34 字符）→ 节点 401。教训：键盘输入必须回读截图核对字数后再提交；后续测试改用短令牌降低输入风险。

3. **对话框权限缺失（真 bug，影响删除类操作）**：Tauri v2 插件命令受能力白名单约束，`capabilities/default.json` 只放行了 `dialog:allow-open`/`allow-save`，漏放 `dialog:allow-confirm`——「删除节点」「删除模型」两处确认框被静默拒绝（无对话框、无报错、操作无反应）。修复：补 `dialog:allow-confirm` 权限；GUI 实测（修复后构建）：点测试节点「删除」→ 原生确认框弹出「确定要移除节点 "LocalTest" 吗？」→ 确认后节点卡片消失，链路完整。

**发送文件完整闭环（2026-10-23 GUI 实测，已验证）**：选中真实 Windows 节点 →「发送文件」→ macOS 原生文件选择器（Cmd+Shift+G 路径导航）选取本机合成测试音频 `e2e-test.wav`（16 kHz 中文语音，171 KB）→ 任务入队（排队中，进度 1 MB/1 MB）→ 节点转写完成（状态流转 排队中 → 已完成）→「获取结果」→「节点结果」面板显示文字稿「你好这是本地转写端到端测试很高兴认识你」（与合成语音内容一致，Paraformer 模型）→「保存到历史」→ 提示「已保存到历史记录」→ 首页历史列表出现 `e2e-test.wav`（engine 标记 `lan-node`，时长 0:05）。全链路每步均回读截图核对，无一步靠推断。

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

**已验证（真实执行，有记录）：**

1. **macOS 13+（Apple Silicon）**：应用安装/启动、本地文件转写、模型导入/驻留、麦克风录音、搜索/导出、局域网 GUI 全链路（添加节点、测试连接、状态/能力/存储展示、发送文件 → 排队 → 节点转写 → 取回结果 → 保存到历史）。
2. **Windows 11 x64**：MSI 安装/启动（真机）；`local-transcribe-node` 节点程序在真机运行，作为局域网计算节点完成真实音频转写（Paraformer，CPU 角色），鉴权、存储探测、整理回收接口均实测。
3. **跨设备局域网计算（Mac Client ↔ Windows Node，真实两台设备）**：配对、Bearer 鉴权、状态/能力/存储探测、文件上传、任务队列、转写执行、结果取回、保存入历史——CLI 层（§4.1）与 GUI 层（§4.2）双重实测通过。
4. **代理环境兼容**：Clash 系统代理开启时 LAN 流量直连不受影响（`.no_proxy()` 修复 + 回归测试）。

**未验证 / 不在本轮范围（如实声明）：**

1. Windows 端应用内录音与多音轨（未实现，见 §6.1；已立项后续任务）。
2. mDNS 自动发现（本轮用手动配对，未实测扫描发现）。
3. GPU compute 角色加速（测试机 AMD Radeon RX 6600，按 CPU 角色执行）。
4. macOS 应用音频内录（历史遗留待验证项，需首次系统音频授权）。
5. iOS / Android（未实现，已立项）。
6. AI BYOK 功能（未实现，已立项）。

**修复缺陷汇总（均经真实复现 → 定位 → 修复 → 复测）**：代理吞局域网（跨平台）、状态覆写误报离线（GUI 逻辑）、`dialog:allow-confirm` 权限缺失（删除类操作静默失败）。另修正 MSI 版本号格式问题（`bundle.windows.wix.version` 覆盖，Tauri MSI 不接受非数字语义化版本）。

**总体判断**：Windows Early Beta 1.1.0-beta.1 的核心承诺——本地转写 + 跨设备局域网计算——在真实 Mac + 真实 Windows 双机环境完成端到端验证，可以按 Early Beta 发布；上述未验证项不构成发布阻塞，但也不得宣称已完成。
