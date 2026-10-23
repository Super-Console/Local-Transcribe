# 模型下载与转写质量（标点、退化重复、切块）

本文记录 2026-10-23 引入的三项能力与其实测数据。所有数字均来自本机实测，方法见 §5。

## 1. 模型下载（模型页）

- **清单**（`apps/desktop/src-tauri/src/model_download.rs`）：Paraformer 中文、SenseVoice
  small、Whisper small（int8）、Qwen3-ASR 0.6B（int8）、FireRedASR2（int8）与
  CT-Transformer 中文标点模型。清单只包含 `transcription-core` 已实现解码器布局的
  模型族；「能下载」等于「能用」。
- **来源**：Hugging Face 官方仓库逐文件下载；直连失败时自动回退 `hf-mirror.com`。
- **安装**：先下载到 `models/.downloads/<目录>`，用 `inspect_model_directory` 校验后
  交给模型库导入（首个模型自动设为默认）；标点模型安装到
  `models/punctuation-ct-transformer/model.onnx`。手动导入入口保留。
- **网络守护**：连接 15 s 超时、读取 45 s 无数据视为断流（另有 45 s 分块超时），
  界面显示可读原因并提示检查网络/代理后重试；失败的文件重新点击下载即可，
  已完整文件按大小跳过。
- **取消与进度**：支持取消下载；进度通过 `voice-to-text:model-download` 事件上报。

## 2. 标点后处理

- 使用 sherpa-onnx `OfflinePunctuation`（CT-Transformer zh-en），由
  `TranscriptionOptions.punctuation_model` 驱动；未下载标点模型时行为与旧版完全一致。
- 每段识别文本单独补标点，时间轴不变；模型调用失败时原样返回，不丢字。
- 实测：真实录音 351 s（Paraformer），标点 0 → **164 个**，对照软件同为 164 个量级。

## 3. 退化重复折叠

- 规则：连续同一字 ≥6 次保留 2 次；同一短语（1–6 字）连续 ≥4 次且总长 ≥12 字保留 2 次。
  正常口语（"对对对""好的好的"）不受影响。
- 实测：同一真实录音，FireRed 最长连续「有」**231 → 3**；文本 1685 → 1456 字，
  恰好等于去掉 231 连旁路后的长度，未误删其他内容。

## 4. 固定切块：30 s 与 60 s 对照

| 测试（TTS 标准答案） | 30 s CER | 60 s CER |
|---|---:|---:|
| 短（160 字 / 30.6 s） | 2.07% | **0.69%** |
| 中（293 字 / 57.0 s） | **0.75%** | 2.26% |
| 长（731 字 / 142.9 s） | **0.46%** | 1.67% |

结论：60 s 无一致收益 → **默认保持 30 s**。`LT_CHUNK_SECONDS` 仅作为测量开关存在，
不面向用户暴露。VAD 切分（Silero）暂不实施：现有数据不支持其必要性，若真实录音
持续显示边界集中损失再评估。

## 5. 测量方法

- 标准答案：小米 MiMo TTS（Token Plan，TTS 免费）生成 3 段中文音频（160/293/731 字），
  参考文本与音频一一对应；用 `crates/transcription-core/tests/quality_live.rs`
  （`--ignored`，env 驱动）转写，再用字符级 Levenshtein 计算 CER（去标点、去空白）。
- 对照：真实录音 `与王硕的通话.m4a`（351 s）与用户既有对照稿。
- 边界说明：TTS 为清晰单人语音，不代表会议嘈杂场景；真实录音一侧没有人工听辨地面
  真值，因此「谁更准」仍属未知，不据 CER 断言模型优劣。

## 6. 待验证

- 模型页的界面操作（下载进度、取消、断网提示）尚未在 GUI 中人工点击验证——
  验证时 macOS 处于锁屏状态，CUA 无法驱动界面；后端代码路径已通过真实网络测试
  （`downloads_a_small_catalog_file_with_progress`，忽略式）。
