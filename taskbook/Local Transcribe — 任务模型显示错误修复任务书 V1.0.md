# Local Transcribe — 任务模型显示错误修复任务书 V1.0

> **性质**：Bug 修复（第二十份）。只修"转写结果页把模型显示成未选择"。
> **不得修改其它任务书。**

---

## 1. 问题

**复现**：在 MacBook Air 上选一个**远端节点上的模型**转写一段录音 → 转写完成、文本正常 → 但结果页的模型一栏显示「未选择」。

**根因**（已核实，不是猜测）：

`apps/desktop/src/lib/components/TranscribeView.svelte:174`

```svelte
<span>{t("transcribe.model")} <b>{prettyName(model?.displayName) || t("common.notSelected")}</b></span>
```

这里的 `model` 来自 `+page.svelte:69` 的 `let model = $state<ModelSummary | null>(null)`，它是**当前选中的本机模型**，不是"这次任务用的模型"。

远程转写时，用户选的是远端模型，本机选择为空 → `model` 为 `null` → 显示「未选择」。

**这是把两个概念混成了一个**：

| 概念 | 含义 | 来源 |
| --- | --- | --- |
| 当前选中的模型 | 用户接下来要用的 | `selected-model.json` |
| **本次任务用的模型** | 这条结果是怎么来的 | 任务的 `model_id` / 转写记录的 `model_id` |

结果页显示的是后者，却取了前者的值。

---

## 2. 修法

**显示优先级（从高到低）**

1. 该任务的 `model_id`（任务状态里记录的那个；远程任务即节点上的模型 id）
2. 该转写记录（`transcript.model_id`）里的值
3. 都取不到 → 显示 **`未知模型`**（新增 key `common.unknownModel`）

**绝不允许**在没有任务上下文时回退到"当前选中模型"——那会让结果页显示一个与这条结果无关的模型。

**绝不允许**在任务**已完成**时显示「未选择」。「未选择」只应出现在"还没开始、也没选模型"的空状态。

**展示名称**：走已有的 `prettyName()`。远端模型 id 通常是文件夹名形式（`transducer-sherpa-onnx-x-asr-zipformer-...`），`prettyName` 已能清理成 `X-ASR Zipformer`；如果它在本地清单里找不到，就退化为清理后的名字，**不要显示原始长串**。

---

## 3. 适用范围（逐处核对，别只修一处）

| 位置 | 要显示什么 |
| --- | --- |
| `TranscribeView.svelte:174` 结果页模型 | 本次任务的 `model_id` |
| 结果页/阅读器的元信息行（模型 · 时长 · 字数） | 同上 |
| 历史卡片上的模型名 | 同上（若目前读的是 `model`，一并改） |
| 导出 TXT / JSON 的元数据 | 同上（导出里的 model 字段必须是这条记录的，不是当前选中的） |
| 顶栏模型选择器 | **保持现状**（它就是"当前选中"） |
| 设置页「默认模型」 | **保持现状**（它是设置项） |

判断标准：**凡是描述"这条结果"的地方，用记录的 model_id；凡是描述"接下来用什么"的地方，用选中模型。**

---

## 4. 文案

新增：

| key | zh | en |
| --- | --- | --- |
| `common.unknownModel` | 未知模型 | Unknown model |

`common.notSelected`（未选择）保留，但只用于"尚未选择模型、也还没有任务"的空状态。

---

## 5. 断言（只做能自动验的）

1. 构造一条 `transcript.model_id = "transducer-sherpa-onnx-x-asr-zipformer-zh-en-int8-2026-06-03"`、且当前选中模型为 `null` 的记录：结果页显示 `X-ASR Zipformer`，**不含「未选择」**
2. 同上但 `model_id` 也是空：显示 `未知模型`，不含「未选择」
3. 本机转写（有选中模型）行为不变：显示该模型名
4. 历史卡片：远程任务显示远端模型名，不是「未选择」
5. 导出 JSON 里的 model 字段等于该记录的 `model_id`，不是当前选中的模型
6. 顶栏与设置页展示的仍是"当前选中"，未受影响

---

## 6. 不做

- 不改模型选择与派发逻辑
- 不改 `selected-model.json` 的写入
- 不改顶栏与设置页的"当前选中"语义
- 不评价观感、不截图

---

## 7. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sed -n '168,180p' apps/desktop/src/lib/components/TranscribeView.svelte && \
  grep -n "let model" apps/desktop/src/routes/+page.svelte && \
  grep -rn "model_id" crates/transcription-core/src/lib.rs | head -5 && \
  grep -n "model" apps/desktop/src/routes/+page.svelte | grep -n "export\|history" | head -5
```
