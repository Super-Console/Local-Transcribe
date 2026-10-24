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
