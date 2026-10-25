# Local Transcribe

**English version: [README.md](README.md)。**

Local Transcribe 是本地优先的桌面应用：把用户选择的音频/视频文件在**本机**转写为文字。一份 Rust 核心同时服务三个平等的界面——桌面 GUI、命令行、本地 MCP 服务。

macOS 与 Windows 构建发布在 [Releases](https://github.com/TimmySheep/Local-Transcribe/releases)。视觉设计仍在迭代；产品名已确定。


## 产品边界

- 本地文件 → 设备端转写 → 导出 TXT 或 JSON。
- 源媒体文件只读，绝不上传。
- 不含实时听写/实时字幕、云端 ASR 或翻译。1.4.0-beta.1 起提供可选的本地语言模型摘要与提问；该功能在设备端运行，仍属实验阶段。
- 1.0 起支持**本地录音**：在应用内录麦克风和/或指定会议应用，录完本地转写；音频不上传任何地方。
- MCP 仅走本地 stdio；无 TCP/HTTP 监听、无任意文件访问、无 shell 执行。
- 模型从应用内经过验证的清单下载；转写完全在本地离线完成，不上传音频。

## 架构

```
apps/desktop/                 Tauri 2 + Svelte 桌面应用（macOS 优先）
crates/transcription-core/    音频解码、模型布局校验、sherpa-onnx 推理、
                              可取消的流式转写、模型库、任务/结果存储
crates/transcription-service/ 本地共享服务（macOS/Linux 为 Unix socket，
                              Windows 为仅回环 TCP），GUI / CLI / MCP 共用
crates/transcription-cli/     `local-transcribe` 命令行客户端
crates/transcription-mcp/     本地 stdio 的 MCP 服务（13 个工具）
crates/lan-compute/           局域网 Compute V1.1：任务存储、节点服务、客户端、mDNS
                              （`local-transcribe-node` 程序）
```

三个界面连接同一个文件存储的共享服务实例：GUI 里开始的任务，CLI 和 MCP 都能看到，反之亦然。

## 桌面应用

- **录音**（1.0）：在应用内直接录麦克风和/或指定应用（Core Audio 进程音频捕获），两条音轨分别转写、各自成记录；不安装任何虚拟声卡驱动。
- **首页**把转写与历史合并：导入磁贴 + 按日期分组的转写卡片；打开记录即在阅读器中查看正文。
- **搜索**：顶栏搜索同时匹配文件名与转写正文（本地 JSON，不区分大小写）并展示命中片段；阅读器内另有本稿搜索，带匹配计数与高亮。
- **模型选择器**：顶栏下拉列出已下载模型、标注默认项，且**只有模型真正驻留内存时才显示绿点**（按需加载、同模型复用、空闲 120 秒卸载）。
- **存储**设置展示所有应用目录（模型库、历史结果、任务记录、应用数据），并提供按时间（1/3/5/7/30 天）或按容量（1/5/10 GB）的自动清理，默认关闭；删除一律进废纸篓。
- **界面语言**：简体中文、English、Español、Français、Deutsch（默认跟随 macOS）。
- **本地语言模型（1.4.0-beta.1，实验性）**：用户可单独下载 Qwen3 GGUF 模型，在答案卡中总结转写或提问。模型和转写内容留在本机，不使用云服务；安装包不含模型权重。内存数据为估算，模型回答可能不准确，请对照原文核验。许可证见[模型许可证说明](docs/model-licenses.md)。

## 局域网计算（协议 V1.1）

可选、默认关闭：把转写放到局域网内的另一台电脑上执行。

- **Node**（`local-transcribe-node`）提供本地 HTTP API（`/api/v1`）：分块上传（支持断点续传）、串行计算队列、结果/收集/导出 API、mDNS 发现（`_localtranscribe._tcp.local.`）。不需要配对或认证。
- **Client**（桌面 GUI「局域网」视图）：扫描或手动添加节点，获取节点模型列表并选择转写模型，带进度与续传地发送文件、跟踪任务并取回结果；客户端不必安装远端模型。
- **计算模式**：仅本机 / 局域网优先。旧的“本机优先”设置按“仅本机”读取。仅本机不自动外发（仍可在局域网页手动发送）；局域网优先使用已启用节点上选定的模型；节点不可用或执行/传输/结果获取失败时在本机重新转写，需要已安装并选定本地模型。主动取消不会重跑；回退沿用同一任务 ID 和历史条目。失联时只能尽力取消远端，不能保证远端立即停算。
- **共享本机算力（macOS）**：在「局域网」开启“允许其他设备使用本机算力”。名称、网卡与端口会被记住，下次启动时若该网卡仍存在会自动恢复共享；网卡消失则不启动节点，并提示重新选择（**绝不**静默回退到 `0.0.0.0`）。可选任意非回环 IPv4（Wi-Fi、以太网、USB、VPN、Tailscale）。共享已安装模型库和已选的外部模型，客户端可以自由选择。接收的媒体保存在应用数据目录的 `lan-host`，可在设置里启用自动清理（默认关闭、只清终态任务、最短 1 天、移入废纸篓）。关窗在「关闭窗口后继续在后台共享」开启时不会停节点；**退出**应用时若有任务在进行会先确认。
- **网络范围**：共享可绑定任意非回环 IPv4（Wi-Fi、以太网、USB、VPN、Tailscale），也可显式选择 `0.0.0.0` 监听全部接口。对端过滤按掩码：`/1`–`/30` 仅同子网；`/31`、`/32` 点对点链路（Tailscale）不做过滤；`0.0.0.0` 不做过滤。界面会写明当前属于哪一种。传输为 HTTP，不配置公网访问、端口映射，也不修改防火墙。
- **内存**：节点模型与本机模型分属两个进程，大模型同时驻留时会各占一份内存。M2 同一秒采样：节点峰值 1.72 GiB、本机峰值 0.91 GiB、同刻合计峰值 2.16 GiB。M3（debug）在 120 秒后两边都打了卸载日志，但 260 秒时 RSS 仍有节点 872 MB、本机 484 MB；GUI 空闲 42 MB。卸载是缓存释放，不保证 RSS 立即回落。
- **Tailscale / VPN**：节点可以在 tailnet 地址上监听，但**不会**在该地址注册 mDNS（多播不跨 tailnet），因此 tailnet 节点不会出现在局域网扫描结果里，需要手动填写地址与端口。后续手机版将通过 Tailscale 连接本机节点；这不改变「无鉴权、仅在私有网络内使用」的边界。
- **节点进程**：开启共享会启动随包分发的 `local-transcribe-node` 子进程，GUI 自身不监听局域网端口。**本机没有运行 GUI 时节点仍可工作**：手动运行同一个二进制即可，数据目录与协议完全一致。停止共享或确认退出会结束子进程。有任务进行中时，退出前会先确认。关窗后是否继续共享由「关闭窗口后继续在后台共享」决定（仅在共享开启时生效），从菜单栏图标可重新打开窗口或退出。笔记本合盖或睡眠时节点不可达，客户端约 5 次轮询（约 10 秒）后报失联，这是预期行为。
- **无头节点**：`local-transcribe-node --name 名称 --data-dir 目录 --listen 地址:端口 [--netmask 掩码] [--compute on --models-dir 应用数据目录]` 可在无 GUI 时运行同一节点。`--listen` 必填；普通局域网地址必须给 `--netmask`，`0.0.0.0`、回环与 Tailscale `100.64/10` 可省略。`--parent-pid` 让节点随父进程退出；`--idle-unload` 只接受 20/120/600/1800/7200 秒。
- **数据与日志位置**：应用数据在 `~/Library/Application Support/com.timmy.voicetotextapp`（模型、历史、任务、`lan-host` 接收媒体、设置）；节点日志在 `~/Library/Logs/com.timmy.voicetotextapp/lan-node.log`（5 MB 轮转，保留 1 份）。
- **卸载残留**：把应用拖进废纸篓不会删除上述两个目录，不再需要数据时请手动清理。
- Windows 安装包由 `windows-2022` CI 构建；设计评审见 `docs/architecture/windows-lan-review.md`。

## 支持的模型

模型页只提供经过实测的清单（3 个准确优先 + 3 个速度优先 + 中文标点），不再接受任意导入。
清单里的模型在 Apple Silicon 上用同一套中文测试音频完成过 CER 对照，结果见
`docs/model-download-and-quality.md`。

| 模型族 | 目录布局 | 实测 CER |
|---|---|---|
| X-ASR Zipformer（transducer） | `encoder*` + `decoder*` + `joiner*` + `tokens.txt` | 0.84% |
| FireRedASR2 (AED) | `encoder*.onnx` + `decoder*.onnx` + `tokens.txt` | 1.29% |
| Qwen3-ASR 0.6B | `conv_frontend.onnx` + encoder/decoder + `tokenizer/` | 1.80% |
| SenseVoice small（2025） | `model.onnx` + `tokens.txt` | 1.67% |
| Paraformer zh | `model.onnx` + `tokens.txt` | 1.74% |
| Zipformer CTC small zh | `model.onnx` + `tokens.txt` | 2.57% |

推理引擎为 sherpa-onnx 1.13.8（CPU）。

## 三个界面

### 桌面应用

Tauri 2 + Svelte。拖放或文件选择、增量转写显示、任务队列、历史记录、模型库、浅色/深色主题。视觉设计仍在迭代；本文只承诺行为。阅读器工具行与应用顶栏的两条玻璃是纯 CSS；在 `<html>` 上设 `data-glass="off"` 可关闭，系统「降低透明度」会自动等同关闭。

### 命令行

```sh
local-transcribe status
local-transcribe models list
local-transcribe jobs add <媒体文件> [--model ID] [--root 目录]
local-transcribe jobs list | show | cancel | retry | remove <job-id>
local-transcribe export <job-id> --format txt|json --output 输出路径

local-transcribe lan list | nodes
local-transcribe lan add <名称> <主机>[:端口] [--force]
local-transcribe lan remove <id|名称>
local-transcribe lan enable|disable <id|名称>
local-transcribe lan set <id|名称> [--host H] [--port P] [--model ID] [--name N]
local-transcribe lan scan [--timeout 8]
local-transcribe lan start [<id|名称>] | stop [<id|名称>]
local-transcribe lan status
local-transcribe mode local|prefer-lan
```

所有命令输出 JSON。退出码：`0` 成功；`1` 服务返回错误（例如 `lan stop` 时本机没有
运行中的节点：`not_running`）；`2` 参数错误；`4` 连不上本地服务。

`lan add` 的 host 是任意字符串——IP、`.local`、tailnet 名或 DNS 名——直接拼成
`http://{host}:{port}`，不做 IP 解析。节点暂时不可达时报可读错误；加 `--force` 仍可保存。

`lan start <节点>` / `lan stop <节点>` 走**系统自带 SSH**（`BatchMode=yes`，不会停在密码
提示）在另一台 Mac 上启停节点，使用你已有的免密登录；不安装任何守护进程、LaunchAgent、
计划任务或开机自启，远端命令固定为 `<node_binary> --from-settings`。节点的可选 `ssh`
字段按**发起端**填写：在 Air 上 `ssh pro` 指 Pro，在 Pro 上 `ssh air` 指 Air，不要假设
两端存在同一组别名。不带 id 的 `lan start` 在本机拉起节点，与 GUI 的共享开关共用设置。

### MCP

`voice-to-text-mcp` 通过本地 stdio 说话 MCP 协议，恰好暴露十三个工具：`transcribe_file`、`get_job_status`、`get_job_result`、`cancel_job`、`retry_job`、`list_jobs`、`list_models`、`export_result`、`list_nodes`、`add_node`、`update_node`、`remove_node`、`node_status`。文件访问必须显式传 `--allow-root 目录`；不传任何 root 时全部文件读取被拒绝。`add_node` 是唯一能引入新地址的入口，其余节点工具只作用于 `lan-settings.json` 里已有的节点；任何工具都不接受命令、路径或 URL scheme。详见 [`docs/mcp.md`](docs/mcp.md)。

## 从源码构建（macOS，Apple Silicon）

```sh
# 桌面应用（未签名的本地构建）
cd apps/desktop && npm install && npm run tauri -- build --bundles app --no-sign

# CLI + 服务 + MCP + 局域网 Node
cargo build --release -p voice-to-text-cli -p voice-to-text-service -p voice-to-text-mcp -p voice-to-text-lan
```

需要 Rust stable + Node.js 24。测试：`cargo test --workspace`，以及 `apps/desktop` 下的 `npm run check`。

Windows：workspace 可编译 `x86_64-pc-windows-msvc`（由 CI 在 `windows-2022` 真实验证，同时构建 MSI 并附带 `local-transcribe-node.exe` / `local-transcribe.exe` / `voice-to-text-service.exe` 产物）。Windows 端应用内录音支持麦克风；按应用内录仅 macOS（Core Audio 进程内录）。

## 验证状态

- Rust workspace、CLI、服务与 MCP 在 Apple Silicon（macOS）上构建并通过测试。
- macOS 上已用真实模型完成端到端转写验证（见上表），CLI 与 GUI 均已走通。
- MCP 协议、工具面与 `--allow-root` 文件访问防线已对真实服务实测。
- Windows：完整管线（fmt、测试、严格 clippy、release 构建、npm check、MSI）已在真实 `windows-2022` CI runner 全绿，且本机对 `x86_64-pc-windows-msvc` 做交叉核对。真机安装/启动验收记录见 `docs/testing/`，运行时结论以该记录实际验证的范围为限。（历史阻塞：Smart App Control 拦截的是"在测试主机上编译"；CI 编译与 MSI 安装是另一条路径。）
- 局域网：自动化测试覆盖任务状态机、持久化/崩溃恢复、无认证的私有网段访问、断点续传上传、收集 API 越界拒绝、带 sha256 清单的 ZIP 导出、CLI/MCP 节点管理与共享设置文件锁。跨设备运行时验证记录见 `docs/testing/`。
- 发布签名/公证未解决（macOS 为 ad-hoc、Windows 未签名；SmartScreen 首次运行警告适用）。界面视觉仍可能调整。

完整里程碑日志（已核实事实与提议/阻塞分开记录）见 [`PROJECT_STATUS.md`](PROJECT_STATUS.md)；可复现的核查步骤见 [`docs/technical-validation.md`](docs/technical-validation.md)。

## 许可证

代码采用 MIT 许可证 —— 见 [LICENSE](LICENSE)。

"Local Transcribe" 名称与应用图标不在该许可证范围内，仍归作者所有。

第三方组件保留各自许可证，见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。
