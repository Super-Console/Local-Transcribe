# 2026-10-22 FireRedASR2-AED 与 Paraformer-zh 真实测试

## 任务来源

Timmy 给出两个 HuggingFace 仓库，要求下载并做真实测试：

- `https://huggingface.co/FireRedTeam/FireRedASR2-AED`
- `https://huggingface.co/funasr/paraformer-zh`

## 关键决策：下载 sherpa-onnx 官方预转换版，而非 HF 原始权重

两个 HF 仓库是 **PyTorch 原始格式**（safetensors/pt），本 App 的推理引擎是 sherpa-onnx，无法直接执行；自行转换需要额外工具链且不在 V1 边界内。核查 sherpa-onnx 1.3 官方文档后确认：

- sherpa-onnx 官方**就是从这两个仓库本体转换**并发布预转换包（int8 量化）：
  - FireRedASR2-AED → `sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26`（官方注明 "converted from ModelScope FireRedTeam/FireRedASR2-AED"）
  - paraformer-zh → `sherpa-onnx-paraformer-zh-2023-09-14`
- 因此下载预转换包 = 测试同一批模型，且能直接在本 App 引擎里跑。

下载来源 `github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/`（直链 HTTP 200，大小与官方文档一致：FireRed 838,589,068 B / Paraformer 234,051,698 B）。下载用 curl（本机无 hf CLI / git-lfs / aria2c，未安装新工具）。

## 兼容性核查结论

- sherpa-onnx Rust 绑定 1.13.8（Cargo.lock 锁定）**已含 `OfflineFireRedAsrModelConfig { encoder, decoder }`**，`fire_red_asr` 字段在 `OfflineModelConfig` 中存在——引擎侧无需升级。
- App 的 `detect_layout` 原本不识别 FireRed 布局：encoder+decoder+tokens.txt 会被当成"残缺 Whisper 导出"拒绝（FireRed 用字符级词表，不含 Whisper marker）。**需要加 FireRedAsr 族**。

## TDD 过程

1. **失败测试**：`tests/model_layouts.rs` 新增 `recognizes_a_fire_red_asr2_aed_layout`（目录名含 `sherpa-onnx-fire-red-asr2-…`，encoder.int8.onnx + decoder.int8.onnx + 字符级 tokens.txt）→ 编译失败（`ModelFamily::FireRedAsr` 不存在），符合预期。
2. **实现**（`crates/transcription-core/src/asr.rs`）：
   - `ModelFamily` 增加 `FireRedAsr`；`ModelFiles` 增加 `FireRedAsr { encoder, decoder, tokens }`。
   - `detect_layout`：encoder+decoder 分支里，**目录名含 `fire-red`/`firered` 时先识别为 FireRedAsr**（必须在 Whisper 词表检查之前，否则字符级词表会被误拒）。
   - `transcribe_file_streaming`：新增 `FireRedAsr` 配置分支 → `config.model_config.fire_red_asr`。
   - `language` 字段：FireRedASR 是中英自动检测，按"不编造语言"原则与 SenseVoice/Whisper/Qwen3 同列 `None`。
   - `SUPPORTED_LAYOUTS` 文案加入 FireRedASR（导入失败提示会列出）。
   - 三处 `family_label` 显示名映射（core 语义 + service lib.rs + desktop lib.rs 两处）加 `FireRedASR`。
3. **通过**：目标测试 9/9 通过；`cargo test --workspace` 全绿。

## 真实测试结果

导入（`local-transcribe models import`）：
- Paraformer：`family: Paraformer`，245,097,569 B ✓
- FireRedASR2：`family: FireRedASR`，1,236,614,460 B ✓（新族走通服务链路）

转写（真实人声 test_wavs，经共享服务 `jobs add --root` → export）：

| 音频 | 模型 | 结果 |
|---|---|---|
| 0.wav（10s 中英混读） | FireRedASR2-AED | `昨天是 MONDAY TODAY IS礼拜二 THE DAY AFTER TOMORROW是星期三`（与 sherpa-onnx 官方文档对该文件的基准输出逐字一致）|
| 3-sichuan.wav（四川话） | FireRedASR2-AED | `自己就是在那个在那个就是在情节里面就是感觉戏演得特别好就是好像很真实一样你知道吧` |
| 5-henan.wav（河南话） | FireRedASR2-AED | `它这个管一向都通到有时候都通到七八层楼高然后它这管一下就可以浇到那个那柱子上` |
| 0.wav（中文） | Paraformer-zh | `对我做了介绍啊那么我想说的是呢大家如果对我的研究感兴趣呢嗯` |
| 6-zh-en.wav（中英混合） | Paraformer-zh | `yesterday was 星期一 today is tuesday 明天是星期三` |

JSON 导出结构完整：`schemaVersion: "1"`、`engine: sherpa-onnx/1.13.8`、`durationMs: 10053`、`segments[]`（startMs/endMs/text）。`language` 字段按设计缺席（不编造）。

GUI 端到端：重装 App 后，模型页正确显示三个模型（FireRedASR · 1.15 GB、Paraformer · 234 MB、Qwen3-ASR · 954 MB）；FireRedASR 设为默认后历史页完整显示上述全部真实转写结果（时长/状态/文本正确）。测试后已把默认模型恢复为 Qwen3。

## 过程问题与处理

- `cargo test` 报"byte string 非 ASCII"：tokens.txt 样本含中文，改用 `"...".as_bytes()`。
- CLI 包名是 `voice-to-text-cli`（二进制名 `local-transcribe`），首次 `cargo build -p local-transcribe` 失败后修正。
- `models import` 无 `--root`/`--allow-root` 选项（只有 `jobs add` 有 `--root`）；由 import 拉起的服务不带 root，后续 `jobs add` 前需按既有规则重启服务。
- GUI 文件对话框（NSOpenPanel sheet）CUA 后台模式完全无法操作：AX 面向解析失败、截图纯黑、Escape/Esc 经 hotkey、press_key、osascript 均无效（osascript 无辅助功能权限）。GUI 引擎链路改由"设为默认 + 历史页"验证（与 CLI 共享同一服务与存储）。
- `jobs show` 只返回状态不含正文，正文需 `export`——这是已记录的既有行为，非本次回归。

## 影响与状态

- 新增模型族 FireRedASR（v1 AED 与 v2 AED 共用 encoder+decoder+tokens 布局，目录名含 fire-red 即可识别）。
- 模型库现有 3 个模型共 2.31 GB（应用内副本）。原始下载在 `.models/dl/`（已 gitignore），解压目录约 2.4 GB + 压缩包 1 GB。
- `PROJECT_STATUS.md` 已追加里程碑。
