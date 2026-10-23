# Local Transcribe — 界面三处调整任务书 V1.0

> **文件性质**：独立小任务书（第四份增补）。只改界面文案与列表结构，**不改任何转写、局域网、服务行为**。
> **执行前提**：排在主任务书之后执行，**不与其并行**。开始前必须确认主任务书已交付（见第 6 节）。
> **不得修改其它任务书；涉及文件**：`LanView.svelte`、`AppSidebar.svelte`、`ModelView.svelte`、`model_download.rs`、`lib.rs`、`types.ts`、五个 i18n 语言文件。

---

## 1. 删掉「临时 UI（视觉设计未定稿）」

**位置**

- `apps/desktop/src/lib/components/LanView.svelte:420`：`<span class="temp">{t("lan.tempUi")}</span>`
- 同文件样式区约 744 行：`.temp` 规则（删前确认只被这个 span 使用）
- i18n key `lan.tempUi`，五个文件全有，**必须一起删**：`de.ts` / `en.ts` / `es.ts` / `fr.ts` / `zh.ts`（只删中文会导致 key 一致性检查失败）

**要求**

- 删掉后标题行只剩 `{t("lan.title")}`，右侧不留空白占位
- 五语言逐一切换，该文案都不得出现

---

## 2. 删掉侧栏的「Local Transcribe」

**位置**

- `apps/desktop/src/lib/components/AppSidebar.svelte:19-20`：整个 `.brand` 块（`div` + `span.label`）
- 同文件样式区 114–123 行：`.brand` 规则

**要求**

- 删掉后侧栏顶部直接是第一个分组标题「工作区」
- 原 `.brand` 有 `padding: 2px 10px 22px`，删掉后首个分组会贴顶：**必须在 nav 容器上补回 12–16px 的顶部间距**，并在 900×620 与最小 640×460 两个尺寸目视确认，不允许贴边或跳动
- **不动**：窗口标题、`tauri.conf.json` 的 `productName`、README 中的产品名、设置页里的任何产品名文字
- 本项只删侧栏这一处展示，不代表产品名已定稿

---

## 3. 模型列表：语言从主标题移到副标题

**目标**

下载列表：

```
X-ASR Zipformer                    ← 主标题
X-ASR · zh/en · 168 MB             ← 副标题
```

已安装列表：

```
FireRedASR2 (int8)                 ← 主标题（清单里的名字）
FireRedASR · zh/en · 1.15 GB · 已安装，未加载   ← 副标题
```

**数据结构改动**

1. `apps/desktop/src-tauri/src/model_download.rs` 的 `CatalogEntry` 新增 `languages: &'static str`，并从 `name` 里把 ` · 语言` 后缀去掉。
2. `CatalogState` 新增 `languages: String`，随现有字段一起序列化。
3. `apps/desktop/src/lib/types.ts` 同步新增 `languages`。
4. `lib.rs` 的 `library_models`：`LibraryModelSummary` **新增** `catalog_name: Option<String>` 与 `languages: Option<String>`。
   - **不要改 `display_name` 本身**——它被首页模型菜单、历史等共用，改了会扩大影响面。
   - 匹配方式：模型文件夹名与 catalog 的 `directory` **完全相等**（大小写敏感）才算命中；命中不了就是 `None`。
5. `ModelView.svelte`：
   - 下载列表：`{entry.name}` 作标题；副标题顺序为 `family · languages · size`，语言为空时**连同分隔点一起省略**
   - 已安装列表：标题用 `catalogName ?? displayName`；副标题顺序为 `family · languages · size · 已安装，未加载`
   - 分隔符统一用 ` · `（空格 + U+00B7 + 空格），与现有 `span.sep` 一致；**不得**用全角句点、顿号或斜杠作分隔

**七条清单的拆分值（逐条核对，不许自己发明）**

| 原 name | 新标题 | languages |
| --- | --- | --- |
| `X-ASR Zipformer · zh/en` | `X-ASR Zipformer` | `zh/en` |
| `FireRedASR2 (int8) · zh/en` | `FireRedASR2 (int8)` | `zh/en` |
| `Qwen3-ASR 0.6B (int8)` | `Qwen3-ASR 0.6B (int8)` | **留空**（原本没写语言，不许补） |
| `SenseVoice small · zh/en/ja/ko/yue（2025）` | `SenseVoice small（2025）` | `zh/en/ja/ko/yue` |
| `Paraformer zh · 2023-09-14` | `Paraformer · 2023-09-14` | `zh` |
| `Zipformer CTC small · zh（体积最小）` | `Zipformer CTC small（体积最小）` | `zh` |
| `CT-Transformer punctuation · zh/en` | `CT-Transformer punctuation` | `zh/en` |

注意：括号里的补充说明（年份、体积）**留在标题**，不要挪到副标题；只有语言移动。

---

## 4. 明确不允许顺带做的

- 不改任何转写、局域网、模型下载逻辑
- 不改模型页其它分组的顺序、间距、按钮文案
- 不把 `Qwen3-ASR` 的语言"补全"成猜测值
- 不改产品名、包名、窗口标题、README

---

## 5. 测试与验证

- `npm run check` 0/0，`npm run build` 通过
- `npm test` 现有用例全绿；新增断言须先红后绿。**至少新增 1 条**：渲染模型列表后，主标题中不得出现 `zh` / `en` 这类语言串，副标题中必须出现
- Rust：`cargo test --workspace` 全绿，catalog 的既有 4 个用例必须仍然通过（id 唯一、字节数匹配、目录名与 family 匹配、3 质量 + 3 速度）
- 目视三处：局域网页标题行、侧栏顶部、模型页（下载 + 已安装两段），各截一张图对照本文档第 1–3 节
- 五语言各切一次，确认副标题顺序不变、空语言不产生连续两个分隔点

---

## 6. 执行前提（本任务按排队执行，不是并行）

**前置条件（硬性）**：主任务书（`Local Transcribe — 无头节点与菜单栏常驻任务书 V1.0.md`）及其三份增补**已交付**，即：

- `PROJECT_STATUS.md` 里有对应的交付记录
- 主任务书要求的那份开发日志已存在（`logs/2026-10/<日期>-headless-node-and-tray.md`）
- 交接说明里没有还在进行的构建或测试

**开工前先确认，不满足就不要开始**：读一遍 `PROJECT_STATUS.md` 末尾与那份日志末尾，确认主任务已收尾；若发现主任务仍在进行（日志还在追加、或它明确写了未完成阶段），**停下并报告「等主任务交付后再做」**，不要为了赶进度提前动手。

原因很具体：本任务与主任务书在 `LanView.svelte` 与 i18n 上有文件重叠，并行执行会互相覆盖，并且会把半成品编进对方的构建里，产生无法解释的假失败。

开始后按本文档第 1→3 节顺序做，不要顺带重构其它部分。

---

## 7. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sed -n '415,425p' apps/desktop/src/lib/components/LanView.svelte && \
  sed -n '14,24p' apps/desktop/src/lib/components/AppSidebar.svelte && \
  sed -n '55,70p;124,142p' apps/desktop/src/lib/components/ModelView.svelte && \
  grep -n "pub struct CatalogEntry" -A 12 apps/desktop/src-tauri/src/model_download.rs
```
