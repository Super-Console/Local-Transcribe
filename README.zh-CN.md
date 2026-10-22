# VoiceToTextApp

> 临时工作名。产品名（界面中当前候选：**Local Transcribe**）与最终视觉设计**有意保持未定**，由 Timmy 决定后才会锁定。

**English version: [README.md](README.md)。**

本地优先的桌面转写工具：把用户选择的音频/视频文件在**本机**转写为文字。一份 Rust 核心同时服务三个平等的界面——桌面 GUI、命令行、本地 MCP 服务。

## 产品边界（V1，已冻结）

- 本地文件 → 设备端转写 → 导出 TXT 或 JSON。
- 源媒体文件只读，绝不上传。
- V1 不含实时麦克风听写、云端 ASR、翻译、摘要或笔记生成。
- MCP 仅走本地 stdio；无 TCP/HTTP 监听、无任意文件访问、无 shell 执行。
- 模型权重需用户显式导入；已导入的模型完全离线转写。

## 架构

```
apps/desktop/                 Tauri 2 + Svelte 桌面应用（macOS 优先）
crates/transcription-core/    音频解码、模型布局校验、sherpa-onnx 推理、
                              可取消的流式转写、模型库、任务/结果存储
crates/transcription-service/ 本地共享服务（Unix socket），GUI / CLI / MCP 共用
crates/transcription-cli/     `local-transcribe` 命令行客户端
crates/transcription-mcp/     本地 stdio 的 MCP 服务（9 个工具）
```

三个界面连接同一个文件存储的共享服务实例：GUI 里开始的任务，CLI 和 MCP 都能看到，反之亦然。

## 桌面 GUI（0.2.0）

- **首页**把转写与历史合并：导入磁贴 + 按日期分组的转写卡片；打开记录即在阅读器中查看正文。
- **搜索**：顶栏搜索同时匹配文件名与转写正文（本地 JSON，不区分大小写）并展示命中片段；阅读器内另有本稿搜索，带匹配计数与高亮。
- **模型选择器**：顶栏下拉列出已导入模型、标注默认项，且**只有模型真正驻留内存时才显示绿点**（按需加载、同模型复用、空闲 120 秒卸载）。
- **存储**设置展示所有应用目录（模型库、历史结果、任务记录、应用数据），并提供按时间（1/3/5/7/30 天）或按容量（1/5/10 GB）的自动清理，默认关闭；删除一律进废纸篓。
- **界面语言**：简体中文、English、Español、Français、Deutsch（默认跟随 macOS）。

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

# CLI + 服务 + MCP
cargo build --release -p voice-to-text-cli -p voice-to-text-service -p voice-to-text-mcp
```

需要 Rust stable + Node.js 24。测试：`cargo test --workspace`，以及 `apps/desktop` 下的 `npm run check`。

## 验证状态

- Rust workspace、CLI、服务与 MCP 在 Apple Silicon（macOS）上构建并通过测试。
- macOS 上已用真实模型完成端到端转写验证（见上表），CLI 与 GUI 均已走通。
- MCP 协议、工具面与 `--allow-root` 文件访问防线已对真实服务实测。
- 仓库带有 Windows CI 工作流（`.github/workflows/windows-validation.yml`），但真实的 Windows 运行时/界面验收仍**受阻**：测试主机的 Smart App Control 拒绝 Cargo 生成的构建脚本可执行文件（DeviceGuard 代码完整性事件 3033/3077）。不声称任何 Windows Rust 构建成功。
- 发布签名/公证未解决。产品名与最终视觉设计保持开放决策。

完整里程碑日志（已核实事实与提议/阻塞分开记录）见 [`PROJECT_STATUS.md`](PROJECT_STATUS.md)；可复现的核查步骤见 [`docs/technical-validation.md`](docs/technical-validation.md)。

## 仓库说明

- `GPT聊天记录/`（原始对话存档）是私有只读参考材料，已被 Git 忽略，绝不 stage、移动或编辑。
- 模型权重、媒体文件与生成的转写结果均被 Git 忽略。
- 开发日志位于 `logs/`（一个事件一份文档，只追加）。

## 许可证

尚未选择许可证；在添加之前默认保留所有权利。
