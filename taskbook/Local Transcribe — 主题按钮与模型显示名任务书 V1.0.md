# Local Transcribe — 主题按钮与模型显示名任务书 V1.0

> **文件性质**：独立小任务书（第五份增补）。只改视觉与显示文案，**不改任何转写、局域网、服务行为**。
> **执行前提**：排在主任务书之后（不与其并行）；与「界面三处调整任务书」无强制先后，但**必须遵守第 3 节的唯一来源规则**。
> **不得修改其它任务书。**

---

## 1. 月亮按钮没有遮罩

**现象**：顶栏左侧搜索按钮是一个 32×32 的圆形浅底，右侧月亮按钮只有图标本身，没有那层底。

**原因**（已核实）

- `.search`（搜索）：`width/height: 32px`、`border-radius: 16px`、`background: var(--fill)` —— 常态就有底
- `.icon-btn`（月亮）：`border-radius: 8px`、`background: transparent`，只有 `:hover` 才出现 `var(--fill)`

**要求**

- 把月亮按钮做成与搜索按钮一致的视觉：32×32、`border-radius: 16px`、常态 `background: var(--fill)`
- 悬停时略深一点（例如 `color-mix(in srgb, var(--foreground) 9%, transparent)` 或既有更深色阶），点击/聚焦时保持可见焦点环
- 只改 `.icon-btn.theme`，不要改其它可能的 `.icon-btn` 用途
- 浅色与深色主题都要看一次：深色下 `var(--fill)` 是否仍然可见，不可变成"没有底"

**验收**：截图对照——月亮与搜索两个圆形按钮直径相同、圆心同高、底色调一致；悬停与聚焦都有可见反馈；深色主题同样成立。

---

## 2. 模型名去掉 `sherpa-onnx-`，直接显示人类可读名

**现象**：模型在界面上显示的是文件夹名，例如：

- 工具栏当前模型：`sherpa-onnx-paraformer-zh-2...`
- 首页模型菜单：`sherpa-onnx-qwen3-asr-0.6B-int8-2026...`
- 历史卡片：`parformer-sherpa-onnx-paraoformer-zh-2023-09-14`（再多一层家族前缀）

**根因**（已核实，不要靠猜）

- `crates/transcription-core/src/asr.rs`：`display_name = 文件夹名`；`id = "{family_prefix}-{文件夹名}"`
- 历史卡片直接打印 `modelId`（`HistoryView.svelte:100/153/189`），所以那串长名就是 id

**目标显示名**

| 值形态 | 显示为 |
| --- | --- |
| `sherpa-onnx-x-asr-zipformer-transducer-...` | `X-ASR Zipformer` |
| `sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26` | `FireRedASR2 (int8)` |
| `sherpa-onnx-qwen3-asr-0.6B-int8-2026-...` | `Qwen3-ASR 0.6B (int8)` |
| `sherpa-onnx-sensevoice-small-...` | `SenseVoice small（2025）` |
| `sherpa-onnx-paraformer-zh-2023-09-14` | `Paraformer · 2023-09-14` |
| `sherpa-onnx-zipformer-ctc-small-zh-...` | `Zipformer CTC small（体积最小）` |
| `...ct-transformer-...punctuation...` | `CT-Transformer punctuation` |
| `lan-node`、其它未知值 | 原样显示，不要硬编 |

映射来源就是已有的模型清单（`model_download.rs` 的 catalog），**不要新建第二份名单**。

**实现方式（关键约束）**

- **不改** `crates/` 里的 `display_name` 与 `id`。它们会写进历史记录、跨进程传递，改了会污染已有数据。
- 新建前端模块 `apps/desktop/src/lib/model-names.svelte.ts`：
  - 首次调用时拉一次 `model_catalog`（Tauri 命令，已存在）并缓存
  - 导出 `prettyName(value: string): string`，解析顺序：
    1. **命中清单**：`value === entry.directory` 或 `value.endsWith("-" + entry.directory)` → 返回清单里的名字（去掉语言后缀；本项目已有/将有 `languages` 字段时，语言不进主标题）
    2. **回退清理**：去掉开头的家族前缀（若形如 `xxx-` 且紧跟 `sherpa-onnx-`）、去掉 `sherpa-onnx-`、去掉结尾的 `-YYYY-MM-DD`
    3. **都不匹配**：原样返回，不得返回空串
  - 解析必须对 `id`（带家族前缀）与 `display_name`（纯文件夹名）**两种输入都成立**
- 所有展示点统一调用它，不许各处自己写一套字符串处理。

**必须替换的展示点（逐处核对，全部走 `prettyName`）**

1. `AppToolbar.svelte:140-141` 当前模型按钮文字
2. `AppToolbar.svelte:172` 模型菜单-本机条目
3. `AppToolbar.svelte:200` 模型菜单-远端节点条目（远端模型名也按同一规则清理）
4. `SettingsView.svelte:188` 设置页「当前模型」
5. `TranscribeView.svelte:189` 转写页底部「模型」
6. `ModelView.svelte:129` 已安装列表标题
7. `HistoryView.svelte:100 / 153 / 189` 历史条目与详情里的模型名
8. `+page.svelte:119` 远端标签 `节点 · 模型`

**不改的地方**：`selected-model.json` 里存的值、历史 JSON 里存的 `model_id`、局域网协议里的 `display_name` 字段——这些都保持原样，只在渲染时转换。

---

## 3. 与「界面三处调整任务书」的唯一来源规则

那份任务书第 3 节也涉及模型标题与副标题。为了不出现两套名字逻辑：

- **`prettyName` 是模型显示名的唯一来源。**
- 若「界面三处调整」**尚未执行**：它第 3 节里"新增 `catalog_name` 字段"的部分**改为复用本任务的 `prettyName`**；它仍然负责把语言挪到副标题。
- 若**已经执行**：把它的 `catalog_name` 取值改为调用 `prettyName`，或让 `prettyName` 内部复用它已有的字段——两者保留一个，删掉另一个，不许并存。
- 两份都不得在组件里直接拼 `family · size` 之外的名字字符串。

---

## 4. 测试

- 新增解析器单测（前端）：7 条清单各断言一次；两种输入形态（`id` 带家族前缀 / `display_name` 纯文件夹名）都要覆盖；未知值（`lan-node`）原样返回；空串不崩溃。
- 新增 1 条渲染断言：历史条目不再出现 `sherpa-onnx` 字样。
- 既有测试全绿：`npm test`、`npm run check` 0/0、`npm run build`、`cargo test --workspace`（本任务不改 Rust，跑一遍确认没被误伤）。

---

## 5. 验收

- 浅色 + 深色各截图一次：顶栏两个圆形按钮外观一致
- 首页模型菜单、工具栏当前模型、历史两张卡片、模型页已安装列表——四处都看不到 `sherpa-onnx`
- 未知模型（如 `lan-node`）仍能正常显示，不显示成空白或 `undefined`
- 切换一次语言，模型名不受影响（模型名是专有名词，不进 i18n）

---

## 6. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sed -n '110,130p' apps/desktop/src/lib/components/AppToolbar.svelte && \
  sed -n '230,255p' apps/desktop/src/lib/components/AppToolbar.svelte && \
  sed -n '95,105p;150,160p;185,195p' apps/desktop/src/lib/components/HistoryView.svelte && \
  sed -n '515,540p' crates/transcription-core/src/asr.rs
```
