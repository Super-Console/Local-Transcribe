# VoiceToTextApp

> 临时工作名。产品名（界面中当前候选：**Local Transcribe**）与最终视觉设计**有意保持未定**，由 Timmy 决定后才会锁定。

**English version: [README.md](README.md)。**

本地优先的桌面转写工具：把用户选择的音频/视频文件在**本机**转写为文字。一份 Rust 核心同时服务三个平等的界面——桌面 GUI、命令行、本地 MCP 服务。

## 产品边界（V1，已冻结）

- 本地文件 → 设备端转写 → 导出 TXT 或 JSON。
- 源媒体文件只读，绝不上传。
- 不含实时听写/实时字幕、云端 ASR、翻译、摘要或笔记生成。
- 1.0 起支持**本地录音**：在应用内录麦克风和/或指定会议应用，录完本地转写；音频不上传任何地方。
- MCP 仅走本地 stdio；无 TCP/HTTP 监听、无任意文件访问、无 shell 执行。
- 模型权重需用户显式导入；已导入的模型完全离线转写。

## 架构

```
apps/desktop/                 Tauri 2 + Svelte 桌面应用（macOS 优先）
crates/transcription-core/    音频解码、模型布局校验、sherpa-onnx 推理、
                              可取消的流式转写、模型库、任务/结果存储
crates/transcription-service/ 本地共享服务（macOS/Linux 为 Unix socket，
                              Windows 为仅回环 TCP），GUI / CLI / MCP 共用
crates/transcription-cli/     `local-transcribe` 命令行客户端
crates/transcription-mcp/     本地 stdio 的 MCP 服务（9 个工具）
crates/lan-compute/           局域网 Compute V1.1：任务存储、节点服务、客户端、mDNS
                              （`local-transcribe-node` 程序）
```

三个界面连接同一个文件存储的共享服务实例：GUI 里开始的任务，CLI 和 MCP 都能看到，反之亦然。

## 桌面 GUI（1.0.0）

- **录音**（1.0）：在应用内直接录麦克风和/或指定应用（Core Audio 进程音频捕获），两条音轨分别转写、各自成记录；不安装任何虚拟声卡驱动。
- **首页**把转写与历史合并：导入磁贴 + 按日期分组的转写卡片；打开记录即在阅读器中查看正文。
- **搜索**：顶栏搜索同时匹配文件名与转写正文（本地 JSON，不区分大小写）并展示命中片段；阅读器内另有本稿搜索，带匹配计数与高亮。
- **模型选择器**：顶栏下拉列出已导入模型、标注默认项，且**只有模型真正驻留内存时才显示绿点**（按需加载、同模型复用、空闲 120 秒卸载）。
- **存储**设置展示所有应用目录（模型库、历史结果、任务记录、应用数据），并提供按时间（1/3/5/7/30 天）或按容量（1/5/10 GB）的自动清理，默认关闭；删除一律进废纸篓。
- **界面语言**：简体中文、English、Español、Français、Deutsch（默认跟随 macOS）。

## 局域网 Compute V1.1（Early Beta）

可选、默认关闭：把转写放到局域网内的另一台电脑上执行。

- **Node**（`local-transcribe-node`）提供本地 HTTP API（`/api/v1`，Bearer 令牌认证）：分块上传（支持断点续传）、串行计算队列、结果/收集/导出 API、mDNS 发现（`_localtranscribe._tcp.local.`）。
- **Client**（桌面 GUI「局域网」视图）：发现节点、令牌配对、带进度与续传的发送、九态任务状态跟踪（init → file_transferring → queued → transcribing → post_processing → completed / failed / cancelled）、结果取回。
- **计算模式**：仅本机 / 局域网优先。旧的“本机优先”设置按“仅本机”读取。仅本机不自动外发（仍可在局域网页手动发送）；局域网优先选择已认证且启用计算的节点，节点不可用或执行/传输/结果获取失败时在本机重新转写，需要已安装并选定本地模型。主动取消不会重跑；回退沿用同一任务 ID 和历史条目。失联时只能尽力取消远端，不能保证远端立即停算。
- **共享本机算力（macOS）**：在「局域网」开启“允许其他设备使用本机算力”。默认关闭、退出后不自动重启；选择私有 IPv4 Wi-Fi / 以太网接口、名称与端口，使用开启时的默认模型。仅监听该接口地址，并拒绝该子网之外的客户端；凭令牌访问，不配置公网访问或修改防火墙。令牌默认隐藏，保存在应用数据目录 `lan-host/pairing-token`（0600）；接收的媒体/任务也存于 `lan-host`，不自动清理。关闭共享会停止监听并中止节点计算。更换模型需重新开启共享。
- **网络边界**：当前 LAN 协议为未加密 HTTP，配对令牌不是传输加密；只在可信网络使用。应用不配置端口转发，但无法控制用户自行设置的路由器 NAT / 反向代理；不要对公网转发此端口。
- Windows Early Beta 冻结设计稿见 `design/windows-early-beta.html`，设计评审见 `docs/architecture/windows-lan-review.md`。

## 支持的模型族

导入的模型目录按磁盘布局严格校验，不会自动下载任何东西。

| 模型族 | 目录布局 | 验证情况 |
|---|---|---|
| Paraformer | `model.onnx` + `tokens.txt` | ✓ 真实转写 |
| SenseVoice | `model.onnx` + `tokens.txt` | 仅布局校验 |
| Whisper | `encoder*.onnx` + `decoder*.onnx` + `tokens.txt` | 仅布局校验 |
| Qwen3-ASR | `conv_frontend.onnx` + encoder/decoder + `tokenizer/` | ✓ 真实转写 |
| FireRedASR (v1/v2 AED) | `encoder*.onnx` + `decoder*.onnx` + `tokens.txt` | ✓ 真实转写 |

已在 Apple Silicon 上用 sherpa-onnx 官方 int8 预转换包完成真实验证：**Qwen3-ASR 0.6B**、**FireRedASR2-AED 中英双语**、**Paraformer-zh**，覆盖中英混合语音与四川话、河南话方言样本。推理引擎为 sherpa-onnx 1.13.8（CPU）。

## 三个界面

### 桌面应用

Tauri 2 + Svelte。拖放或文件选择、增量转写显示、任务队列、历史记录、模型库、浅色/深色主题。当前界面是功能完整的临时设计，不代表最终视觉定稿。

### 命令行

```sh
local-transcribe status
local-transcribe models list
local-transcribe models import <模型目录>
local-transcribe jobs add <媒体文件> [--model ID] [--root 目录]
local-transcribe jobs list | show | cancel | retry | remove <job-id>
local-transcribe export <job-id> --format txt|json --output 输出路径
```

所有命令输出 JSON。共享服务按需自动启动。

### MCP

`voice-to-text-mcp` 通过本地 stdio 说话 MCP 协议，恰好暴露九个工具：`transcribe_file`、`get_job_status`、`get_job_result`、`cancel_job`、`retry_job`、`list_jobs`、`list_models`、`import_model`、`export_result`。文件访问必须显式传 `--allow-root 目录`；不传任何 root 时全部文件读取被拒绝。详见 [`docs/mcp.md`](docs/mcp.md)。

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
- 局域网 Compute V1.1：23 项自动化测试覆盖任务状态机、持久化/崩溃恢复、令牌认证、断点续传上传、收集 API 越界拒绝、带 sha256 清单的 ZIP 导出。跨设备运行时验证记录见 `docs/testing/`。
- 发布签名/公证未解决（Early Beta 不签名；Windows SmartScreen 首次运行警告适用）。产品名与最终 macOS 视觉设计保持开放决策。

完整里程碑日志（已核实事实与提议/阻塞分开记录）见 [`PROJECT_STATUS.md`](PROJECT_STATUS.md)；可复现的核查步骤见 [`docs/technical-validation.md`](docs/technical-validation.md)。

## 仓库说明

- `GPT聊天记录/`（原始对话存档）是私有只读参考材料，已被 Git 忽略，绝不 stage、移动或编辑。
- 模型权重、媒体文件与生成的转写结果均被 Git 忽略。
- 开发日志位于 `logs/`（一个事件一份文档，只追加）。

## 许可证

MIT —— 见 [LICENSE](LICENSE)。第三方组件保留各自许可证。
