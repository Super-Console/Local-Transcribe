# 模型清单、下载与转写质量（标点、退化重复、切块）

本文记录 2026-10-23 的模型重选与三项后处理能力。所有数字均为本机实测，方法见 §6。

## 1. 模型清单（模型页）

- 清单只包含 `transcription-core` 已实现解码器布局、且在本机完成过中文实测的模型；
  「能下载」等于「能用」。
- 分组：**准确优先** 3 个、**速度优先** 3 个，另有中文标点辅助模型。
- **不再接受任意导入**：模型页没有导入按钮，CLI / MCP 也不再提供 import。
  用户只能从这份清单下载；删除仍走系统回收站。
- 来源：Hugging Face 官方仓库逐文件下载；直连失败时自动回退 `hf-mirror.com`。

| 分组 | id | 模型 | 大小 | 实测 CER | 速度 |
|---|---|---|---:|---:|---:|
| 准确 | `x-asr-zh-en` | X-ASR Zipformer（transducer，zh/en） | 176 MB | **0.84%** | 38.9× |
| 准确 | `fire-red-asr2` | FireRedASR2 AED（int8，zh/en） | 1.23 GB | 1.29% | 1.6× |
| 准确 | `qwen3-asr-0.6b` | Qwen3-ASR 0.6B（int8） | 987 MB | 1.80% | 2.8× |
| 速度 | `sensevoice-small` | SenseVoice small（2025-09-09，int8） | 237 MB | 1.67% | 12.6× |
| 速度 | `paraformer-zh` | Paraformer zh（2023-09-14，int8） | 243 MB | 1.74% | 15.9× |
| 速度 | `zipformer-ctc-small-zh` | Zipformer CTC small zh（int8） | 63 MB | 2.57% | **60.7×** |
| 辅助 | `punctuation-zh-en` | CT-Transformer zh/en 标点 | 294 MB | — | — |

速度 = 116 秒音频的转写耗时倍率（CPU、2 线程，含模型已加载）。

## 2. 实测过程与取舍

- 测试音频：小米 MiMo TTS（Token Plan 月付，TTS 免费）生成 6 段中文，
  27.4 / 39.4 / 45.8 / 53.3 / 65.6 / 116.2 秒，覆盖 30 s 与 60 s 切块边界、
  数字/专名/英文缩写混杂的难点稿；标准答案即送进 TTS 的原文。
- 指标：字符级 CER，去掉空白与标点，阿拉伯数字按中文读法归一，避免书写差异。
- 淘汰（未进清单）：

| 候选 | CER | 速度 | 结论 |
|---|---:|---:|---|
| Whisper small（原清单） | 26.06% | 3.3× | 中文不可用，移除 |
| Dolphin base CTC（104 MB） | 6.82% | 27.8× | 明显落后，不采用 |
| TeleSpeech CTC（341 MB） | 10.49% | 10.0× | 每段开头丢字，不采用 |
| FireRedASR2 CTC（776 MB） | 3.22% | 2.3× | 同家族 AED 更准，CTC 导出不更快 |
| FunASR Nano（1.01 GB） | 59.65% | — | 导出 KV 上限 512，30 s 块被截断并幻觉，不可用 |
| Paraformer zh 2025-10-07 | 2.83% | 17.8× | 比 2023 版差，保留 2023 版 |

- Whisper small 的移除与「增量」原则不冲突：用户明确要求「不要准确率很差的模型」，
  26% 的 CER 属于不可用级别。
- FireRed 的退化重复由 §4 的折叠兜底；标点由 §3 的后处理补齐，两者都作用于全部模型。

## 3. 标点后处理

- 使用 sherpa-onnx `OfflinePunctuation`（CT-Transformer zh-en），由
  `TranscriptionOptions.punctuation_model` 驱动；未下载标点模型时行为与旧版完全一致。
- 每段识别文本单独补标点，时间轴不变；模型调用失败时原样返回，不丢字。
- 实测：真实录音 351 s（Paraformer），标点 0 → **164 个**。

## 4. 退化重复折叠

- 规则：连续同一字 ≥6 次保留 2 次；同一短语（1–6 字）连续 ≥4 次且总长 ≥12 字保留 2 次。
  正常口语（"对对对""好的好的"）不受影响。
- 实测：同一真实录音，FireRed 最长连续「有」**231 → 3**；文本 1685 → 1456 字。

## 5. 固定切块：30 s 与 60 s 对照

| 测试（TTS 标准答案） | 30 s CER | 60 s CER |
|---|---:|---:|
| 短（160 字 / 30.6 s） | 2.07% | **0.69%** |
| 中（293 字 / 57.0 s） | **0.75%** | 2.26% |
| 长（731 字 / 142.9 s） | **0.46%** | 1.67% |

结论：60 s 无一致收益 → **默认保持 30 s**。`LT_CHUNK_SECONDS` 仅作为测量开关存在。
Whisper 类模型单流上限 30 s，30 s 切块同时是安全边界。

## 6. 测量方法

- 标准答案：MiMo TTS 生成的 6 段音频与原文一一对应；用
  `crates/transcription-core/examples/transcribe_eval.rs` 逐模型转写，
  再用字符级 Levenshtein 计算 CER（去标点、去空白、数字按读法归一）。
- 每段音频都短于或跨过切块边界，避免「只测短句」掩盖边界问题。
- 边界说明：TTS 为清晰单人语音，不代表会议嘈杂场景；真实录音一侧没有人工听辨的
  逐字真值，因此不据 CER 断言绝对准确率。

## 7. 待验证

- 模型页的界面操作（下载进度、取消、断网提示）尚未在 GUI 中人工点击验证；
  后端下载路径已通过真实网络测试（忽略式）。
- 新一轮清单尚未在 Windows 上重跑 CER；Windows 包只做构建与启动级验证。

## 8. 第五轮模型实测（2026-11-15，本机 macOS）

本轮使用 `~/lt-model-eval/corpus/` 中已有的 6 段自拟中文稿与 `say` 合成语音，不重新合成。语音为 `Flo (中文（中国大陆）)`，WAV 格式均为 16 kHz、单声道、16-bit PCM。自建语料不同于 2026-10 的 MiMo TTS 语料；**本节 CER、RTF 与历史章节数字不可横比**。

| 语料 | WAV 实际时长 | 采样率/声道 | 内容摘要 |
|---|---:|---|---|
| 01-short | 23.441813 s | 16 kHz mono | 上师大校园网、Wi-Fi 7、H.264、金额和电话 |
| 02-39sec | 41.824313 s | 16 kHz mono | 纪录片工作坊、上师大、4K、日期与电话 |
| 03-46sec | 48.162063 s | 16 kHz mono | Tesla Model Y、费用、H.264、PCM |
| 04-53sec | 52.422750 s | 16 kHz mono | H.264/H.265/ProRes、SSD、帧率与音频参数 |
| 05-66sec | 70.789938 s | 16 kHz mono | 校园出行计数、地铁站、4K、WAV 与日期 |
| 06-long-116sec | 138.763000 s | 16 kHz mono | 胶片相机纪录片、专名、数字、Sony/Panasonic、格式与项目编号 |

目标时长约 27/39/46/53/66/116 s；表格列实际回读时长，不将偏差伪装成目标值。总音频时长 375.403877 s。模型使用同一 sherpa-onnx 1.13.8、CPU、2 线程；每模型先运行 `00-warm-*` 六条预热，再运行 `10-*` 六条计时。RTF = 6 条实测音频时长总和 ÷ 模型加载后 6 条转写耗时总和（越大越快）。CER 用 `~/lt-model-eval/runs/score.py`：去空白/标点、阿拉伯数字按中文读法归一，再作字符级 Levenshtein。

| 模型 | 权重总字节 | 6 段 CER（01→06） | 整体 CER | RTF | 峰值 RSS | 6/6 非空 | 能力/纳入判断 |
|---|---:|---|---:|---:|---:|---:|---|
| X-ASR zh/en | 175,694,196 | 4.5977 / 10.6667 / 3.2609 / 10.8108 / 1.9305 / 6.8834% | 6.2680% | 38.91× | 918,355,968 B | 是 | 基线；识别器创建成功 |
| Zipformer CTC small zh | 62,680,226 | 11.4943 / 14.0000 / 11.4130 / 23.2432 / 5.0193 / 14.9140% | 13.4006% | 59.44× | 709,181,440 B | 是 | 基线；识别器创建成功 |
| Zipformer CTC zh 2025-07-03 | 367,342,902 | 10.3448 / 13.3333 / 10.3261 / 22.7027 / 5.7915 / 12.6195% | 12.3199% | 20.72× | 1,332,903,936 B | 是 | **纳入**：HTTP 200、目录识别/识别器成功、6 段通过、整体 CER <15% |
| FireRedASR2 | 1,234,657,933 | 4.5977 / 10.0000 / 2.7174 / 13.5135 / 2.3166 / 6.8834% | 6.5562% | 1.81× | 2,140,880,896 B | 是 | 基线；重跑完成。首次遗留日志只记录首条预热且没有异常原文，不能作为完整结果 |
| Qwen3-ASR 1.7B int8 | 2,404,222,421 | 4.5977 / 8.0000 / 4.8913 / 8.6486 / 40.1544 / 6.3098% | 12.8242% | 1.40× | 3,895,771,136 B | 是 | **纳入**：HTTP 200、目录识别/识别器成功、6 段通过、整体 CER <15%；比 X-ASR/FireRed 明显慢且 CER 更高，峰值约 3.90 GB |

RTF 与 CER 来自同一批 6 段，不以历史数据补齐。Zipformer 新版 CER 比 small 低 1.0807 个百分点，但 RTF 仅为 small 的约 34.9%；它不是速度升级。Qwen 1.7B 仅在本次自建语料中通过 15% 可用性线，不胜过 X-ASR，且峰值约 3.90 GB；模型页推荐语按此披露。两候选峰值均未超过 8 GB。

### 第五轮新增权重固定清单

所有下载字节为实测/官方树的精确字节；SHA-256 为实际下载文件计算值。Zipformer 仅打包模型推理需要的模型图、tokens 和 `bbpe.model`，不把 release archive 中的样例音频/README 纳入模型大小。候选的可下载 URL 固定到不可变 revision；各 URL 已实下核对 HTTP 200 与字节数。许可证只记录卡片原文声明，不推断权重授权范围。

| 模型/远端路径 | 字节 | SHA-256 | 来源 revision / 许可证 |
|---|---:|---|---|
| Zipformer `model.int8.onnx` | 367,074,356 | `e291b9c468b651e2697caa09bc684326c3addc6a019e78eb537cfd1a8248ca07` | [HF 仓库](https://huggingface.co/csukuangfj/sherpa-onnx-zipformer-ctc-zh-int8-2025-07-03/tree/824c678e9b80f301110e3ea8cee9819a7994c049) · `824c678e9b80f301110e3ea8cee9819a7994c049` · Apache-2.0 |
| Zipformer `tokens.txt` | 13,366 | `6fed8c6c248516f38e7faa19404b57413e8ce259f1cbc1fa4aebc86eac32fdfd` | 同上 |
| Zipformer `bbpe.model` | 255,180 | `503204e0690eff065e30d0e01898c9ab06d0e6dc376a741eb6846198f95b2f82` | 同上 |
| Qwen `model_1.7B/conv_frontend.onnx` | 48,080,441 | `fa894a4ba53da6a4238f2a6ca0b09362e505d39cecbd646051b033e2e8d7e2fb` | [ModelScope 仓库](https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/tree/cb045ad80b8970c9d411d463e5b78991a566596c) · `cb045ad80b8970c9d411d463e5b78991a566596c` · README `license: apache-2.0` |
| Qwen `model_1.7B/encoder.int8.onnx` | 314,222,162 | `436fbd910a0c8914851e5ac1354e807be9f283d08a5da728adaa609731c41469` | 同上 |
| Qwen `model_1.7B/decoder.int8.onnx` | 2,037,458,645 | `c43c853fa6e97d08365cb8a5502b360b595cd43c00dc60e4d8ca7cc18cad460b` | 同上 |
| Qwen `tokenizer/vocab.json` | 2,776,833 | `ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910` | 同上 |
| Qwen `tokenizer/merges.txt` | 1,671,853 | `8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5` | 同上 |
| Qwen `tokenizer/tokenizer_config.json` | 12,487 | `4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c` | 同上 |

权重留在 `~/lt-model-eval/models/`，语料和逐模型输出留在 `~/lt-model-eval/corpus/` 与 `~/lt-model-eval/runs/v5/`；它们均不进入 Git。六个 Qwen 下载文件合计 2,404,222,421 字节。候选内存估算以本机观测峰值 RSS 回填，并非跨设备保证。
