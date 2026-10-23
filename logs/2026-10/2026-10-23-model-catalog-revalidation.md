# 模型清单重选与导入移除（1.1.0-beta.5）

日期：2026-10-23（深夜）
对象：VoiceToTextApp 桌面端 / transcription-core

## 目标

1. 以中文能力为主，补 3 个准确率最强的模型、3 个速度优先模型；
2. 用小米 MiMo TTS（Token Plan 月付，TTS 免费）生成多段不同长度的中文音频，
   在 MacBook Pro 上逐模型转写并与标准答案对照；
3. 能修的问题就修，修不动就不支持；
4. 移除任意模型导入的按钮与逻辑，只允许清单内的模型；
5. 同步到 Windows，本机编译，两个平台一起发 GitHub。

## 候选与结果

测试音频：27.4 / 39.4 / 45.8 / 53.3 / 65.6 / 116.2 秒，覆盖 30 s、60 s 切块边界，
含数字、专名、英文缩写与同音词。CER 为字符级，去标点空白、数字按中文读法归一。

| 候选 | 布局 | CER | 116 s 耗时 | 结论 |
|---|---|---:|---:|---|
| X-ASR Zipformer transducer zh-en 2026-06 | enc/dec/joiner | 0.84% | 3.0 s | 采用（准确 + 速度双榜首） |
| FireRedASR2 AED 2026-02 | enc/dec | 1.29% | 71.1 s | 保留（原清单） |
| SenseVoice 2025-09-09 | 单图 | 1.67% | 9.2 s | 采用（替换 2024 导出） |
| Paraformer zh 2023-09-14 | 单图 | 1.74% | 7.3 s | 保留（原清单） |
| Qwen3-ASR 0.6B 2026-03 | frontend/enc/dec | 1.80% | 41.3 s | 保留（原清单） |
| Zipformer CTC small zh 2025-07 | 单图 | 2.57% | 1.9 s | 采用（最快、体积最小） |
| Paraformer zh 2025-10-07 | 单图 | 2.83% | 6.5 s | 不采用（比 2023 版差） |
| FireRedASR2 CTC 2026-02 | 单图 | 3.22% | 51.0 s | 不采用（不更快、更不准） |
| Dolphin base CTC 2025-04 | 单图 | 6.82% | 4.2 s | 不采用 |
| TeleSpeech CTC 2024-06 | 单图 | 10.49% | 11.6 s | 不采用（每段开头丢字） |
| Whisper small | enc/dec | 26.06% | 35.5 s | 移除（中文不可用） |
| FunASR Nano 2025-12 | emb/adaptor/llm | 59.65% | — | 不采用（KV 上限 512，30 s 块截断并幻觉） |

失败模式的验证：
- FunASR Nano 在 30 s 块上报 `Context_len (530) exceeds KV capacity (512)`，输出只剩几十字；
  降到 20 s 块后仍出现大段幻觉（59.65%），判断为导出限制，不做特殊切块迁就。
- TeleSpeech 每段开头固定丢字，属于 CTC 导出/分块问题，未找到无副作用的后处理修复。
- Whisper small 的错误是系统性漏字与替换（26%），无法用后处理修复。

## 代码改动

- `crates/transcription-core/src/asr.rs`：新增 `FireRedCtc`、`ZipformerCtc`、`Dolphin`、
  `TeleSpeech`、`FunAsrNano`、`Transducer` 六个模型族；新增 `find_containing`
  （带训练后缀的 encoder/decoder/joiner）与 `find_tokenizer_dir`；配置分别接到
  `fire_red_asr_ctc`、`zipformer_ctc`、`dolphin`、`telespeech_ctc`、`funasr_nano`、
  `transducer`；布局检测单图族按目录名区分。
- 新增布局测试：FunASR Nano、带后缀的 transducer、四个单图 CTC 族、未知单图拒绝；
  原「transducer 必须被拒绝」的用例改为「识别为 Transducer」。
- `apps/desktop/src-tauri/src/model_download.rs`：清单改为 3 准确 + 3 速度 + 标点，
  新增 `CatalogTier`；新增清单一致性/分组用例；文件大小与 HF 上 Content-Length 逐一核对。
- `apps/desktop/src/lib/components/ModelView.svelte`：按 tier 分组展示；删除导入按钮，
  空状态改为「从上方清单下载」。
- 移除导入链路：`lib.rs` 的 `import_model` 命令、CLI `models import`、
  MCP `import_model`（9 → 8 工具）、service 的 `Request::ImportModel`。
- 五语言文案：删除 `model.import`、`model.chooseFolder`、`dialog.importModelTitle`，
  更新空状态文案，新增三个 tier 标题。
- 版本统一 `1.1.0-beta.5`。

## 验证

- `cargo test --workspace`（模型/下载相关的真实网络用例保持 ignored）：见收尾记录。
- `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`。
- `npm test`、`npm run check`、`npm run build`。
- 新增 `examples/transcribe_eval.rs` 作为可复现的批量评测入口。

## 未完成 / 风险

- GUI 下载（进度、取消、断网）仍未人工点验；后端下载路径与文件大小已核对。
- Windows 端未重跑 CER；Windows 包只验证构建与启动链路。
- 真实会议/电话录音没有逐字真值，CER 结论只代表清晰 TTS 单人语音。
