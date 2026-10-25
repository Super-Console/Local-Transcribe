# 模型许可证核实

核实日期：2026-10-24。方法：逐个读取 Hugging Face 模型主页的元数据
（`/api/models/<repo>` 的 `cardData.license`）与模型卡原文
（`/<repo>/raw/main/README.md` 的 YAML frontmatter），只记录页面上实际写的内容，
不根据“出自 sherpa-onnx 官方导出”推断许可证。

> 说明：sherpa-onnx 的**代码**是 Apache-2.0；这不等同于它导出的模型权重也是
> Apache-2.0。下面的结论只针对模型卡本身。

| 模型（清单名） | 仓库 | 模型卡标注 | 核实日期 | 备注 |
| --- | --- | --- | --- | --- |
| X-ASR Zipformer | https://huggingface.co/csukuangfj2/sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03 | 未标注 | 2026-10-24 | 模型卡只写“文件来自 Gilgamesh-J/X-ASR”，无 license 字段 |
| FireRedASR2 (int8) | https://huggingface.co/csukuangfj2/sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26 | 未标注 | 2026-10-24 | 无 license 字段 |
| Qwen3-ASR 0.6B (int8) | https://huggingface.co/csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25 | 未标注 | 2026-10-24 | 无 license 字段 |
| SenseVoice small（2025） | https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09 | 未标注 | 2026-10-24 | 无 license 字段 |
| Paraformer · 2023-09-14 | https://huggingface.co/csukuangfj/sherpa-onnx-paraformer-zh-2023-09-14 | Apache-2.0 | 2026-10-24 | 模型卡 frontmatter：`license: apache-2.0` |
| Zipformer CTC small（体积最小） | https://huggingface.co/csukuangfj/sherpa-onnx-zipformer-ctc-small-zh-int8-2025-07-16 | 未标注 | 2026-10-24 | 无 license 字段 |
| CT-Transformer punctuation | https://huggingface.co/csukuangfj/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12 | 未标注 | 2026-10-24 | 无 license 字段 |

## 结论与影响

- 7 个仓库中只有 Paraformer 明确标注 Apache-2.0；其余 6 个**未标注**。
- 没有发现“非商业”或其它限制性许可的文字；但“未标注”也**不代表**可以自由再分发。
  安装包本身不包含模型权重（应用只在用户操作时从 Hugging Face 下载到本机），
  因此当前不涉及把权重打进安装包；如果将来要随包分发或商用，需要先向上游确认授权。
- 本轮按要求**没有删除或修改**任何清单条目；是否继续提供这 6 个未标注模型，由
  Timmy 决定。

## 本地语言模型候选（2026-10-24）

只读核验 Hugging Face 官方 `Qwen` 仓库元数据及不可变 commit tree；文件体积与 SHA-256
取自该 revision 的 LFS 元数据，`license` 取自仓库 API 的 `cardData.license`，并回读了
仓库 `LICENSE`。下载前代码使用固定 revision、精确字节数和 SHA-256 校验。

| 档位 | 仓库 / 文件 | Revision | 大小（字节） | LFS SHA-256 | License |
| --- | --- | --- | ---: | --- | --- |
| 中文默认 | [Qwen/Qwen3-4B-GGUF](https://huggingface.co/Qwen/Qwen3-4B-GGUF) · `Qwen3-4B-Q4_K_M.gguf` | `bc640142c66e1fdd12af0bd68f40445458f3869b` | 2,497,280,256 | `7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5` | Apache-2.0 |
| 中文大模型 | [Qwen/Qwen3-8B-GGUF](https://huggingface.co/Qwen/Qwen3-8B-GGUF) · `Qwen3-8B-Q4_K_M.gguf` | `7c41481f57cb95916b40956ab2f0b139b296d974` | 5,027,783,488 | `d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785` | Apache-2.0 |
| 多语言轻量 | [Qwen/Qwen3-1.7B-GGUF](https://huggingface.co/Qwen/Qwen3-1.7B-GGUF) · `Qwen3-1.7B-Q8_0.gguf` | `90862c4b9d2787eaed51d12237eafdfe7c5f6077` | 1,834,426,016 | `061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a` | Apache-2.0 |

以上是**来源与元数据核实**，不等于模型质量或双平台运行验收。三档权重已按明确授权下载到本机应用数据目录用于测试；模型权重不在源代码仓库或安装包内。Qwen3-4B 仅有一次长文摘要完成记录，事实核验未通过；Qwen3-1.7B/8B 的运行表现尚未验证。目录中的内存读数是估算，不是逐台设备实测峰值。模型页不显示许可证行，但此处保留来源与许可核查记录。
