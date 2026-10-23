# Local Transcribe — 按钮体系规范任务书 V1.0

> **性质**：样式规范与统一（第十三份）。**不是「把所有按钮都改成胶囊」**，而是把已有的胶囊收成三档，再按角色逐个落位。
> **不得修改其它任务书。**
> **执行前提**：排在其它界面任务之后，不并行。

---

## 1. 原则

现在应用里已经有两套表达，是对的，不要推翻：

- **页面级动作 = 胶囊**（扫描、开始录音、保存节点、查找、TXT）
- **行内动作 = 文字**（测试连接、编辑、删除、打开、复制路径）

问题只是**胶囊这一档规格不统一**：`开始录音` 是黑色实心、`保存节点` 是陶土实心、`扫描` 与 `已关闭·点击开启` 是描边，高度与内边距也各不相同。

**目标**：三档定义清楚，落点明确，每个区域最多一个实心主操作。

---

## 2. 三档规格

复用 `app.css` 已有的 `--capsule-*` 与 `--accent`，不新造颜色。

### 主操作 `.btn-primary`

| 项 | 值 |
| --- | --- |
| 高 / 圆角 | 30px / 999px |
| 内边距 | `0 14px` |
| 字号 / 字重 | 13px / 500 |
| 底 / 字色 | `var(--accent)` / `#fffefb` |
| hover | 底 `var(--accent-ink)` |
| active | 内缩 1px |
| focus | 2px 外环 `color-mix(in srgb, var(--accent) 35%, transparent)` |
| disabled | 底 `var(--fill)`、字 `var(--text-3)`、无外环 |

### 次操作 `.btn-secondary`

| 项 | 值 |
| --- | --- |
| 高 / 圆角 | 30px / 999px |
| 内边距 | `0 12px` |
| 字号 | 12.5px |
| 底 / 边 / 字 | `var(--capsule-find)` / `1px var(--capsule-border)` / `var(--text-2)` |
| 高光 | `inset 0 1px 0 var(--capsule-highlight)` |
| hover | 底 `var(--capsule-find-hover)`、字 `var(--text)` |
| active | 底 `var(--capsule-find-pressed)` + 内缩 1px |
| 图标 | 12–14px，与文字间距 6px |

### 行内动作 `.link-action`

| 项 | 值 |
| --- | --- |
| 形态 | 纯文字，无底无边 |
| 字号 / 字色 | 12.5px / `var(--text-2)` |
| hover | 字 `var(--text)`，可加 `--hover` 底 + 6px 圆角 |
| 危险变体 `.link-danger` | 字 `var(--red)`，hover 加深；**不做胶囊** |

### 危险确认 `.btn-danger`

只用于「清除全部应用数据」这类**不可撤销**的动作。

高 30 / 圆角 999 / `0 12px` / 边 `1px color-mix(in srgb, var(--red) 45%, transparent)` / 字 `var(--red)` / 底透明；hover 底 `color-mix(in srgb, var(--red) 8%, transparent)`。

---

## 3. 落点表（逐页逐控件，不许自由发挥）

### 录制页 `RecordView.svelte`

| 控件 | 改成 |
| --- | --- |
| `开始录音` / `停止` | **主操作**（现在是黑色实心，换陶土实心；录音中的红点保留） |
| `添加音轨` | **次操作**（去掉虚线框，用标准描边胶囊） |
| `刷新来源` | 行内动作（不变） |

### 局域网页 `LanView.svelte`

| 控件 | 改成 |
| --- | --- |
| `已关闭 · 点击开启` | 次操作（规格对齐） |
| `扫描` | 次操作（带图标） |
| 权限横幅里的两个按钮 | 次操作 |
| `保存节点` | 主操作（已是陶土实心，规格对齐） |
| `测试连接` / `编辑节点` / `停用` | 行内动作 |
| `删除` | 行内动作 **危险色** |

### 模型页 `ModelView.svelte`

| 控件 | 改成 |
| --- | --- |
| `下载` | **次操作**（本页唯一改动） |
| `已下载`（状态） | 不是按钮，保留灰字 |
| `设为默认` | 行内动作 |
| `当前默认`（状态） | 保留灰字 |
| `删除` | 行内动作 **危险色** |

> 这一页只改一个控件。列表每行放两个胶囊会变成按钮墙，明确不做。

### 设置页 `SettingsView.svelte`

| 控件 | 改成 |
| --- | --- |
| 主题分段（跟随系统/浅色/深色） | 保持，已符合 |
| `打开` / `复制路径` / `管理模型` | 行内动作，**不改** |
| `清除！` | **危险确认胶囊** |
| 所有下拉框 | **不改**（表单控件不是按钮） |

### 阅读器与转录页

| 控件 | 改成 |
| --- | --- |
| `首页` / `查找` / `TXT` / `JSON` | 已是次操作，规格对齐即可 |
| `查看最新` | 次操作（规格对齐） |
| 查找输入条内的 `↑ ↓ ×` | 行内动作，不改 |

---

## 4. 硬规则（必须能被测试发现）

1. **每个可视区域最多一个主操作**（实心胶囊）。超过就说明档位用错。
2. **列表行内不得出现胶囊**——除该行的主操作列（模型页的「下载」是唯一例外）。
3. **删除永远不用实心红**，只用行内危险色。
4. 行内动作里不放图标；胶囊里的图标 12–14px。
5. 不改任何布局、文案、交互逻辑、确认流程。

---

## 5. 实现方式

- 在 `app.css` 里新增四个类：`.btn-primary` / `.btn-secondary` / `.btn-danger` / `.link-action`（含 `.link-danger` 变体），三态完整
- 组件里把 `class="btn"`、`class="record-btn"`、`class="add"`、`class="link"` 等**替换为新类，并删除各自重复的样式**——不要保留两套
- `.reader-bar button.act` 保留它自己的 `backdrop-filter`，其余属性改为复用次操作的值（允许局部覆盖，但只覆盖模糊与底色）
- 不要在组件里散写十六进制色值，一律走 token

---

## 6. 断言（只做能自动验的）

1. `.btn-primary`：`height = 30px`、`borderRadius = 999px`、背景色等于 `--accent`
2. `.btn-secondary`：`borderWidth = 1px`、背景不等于 `--accent`
3. `.btn-danger`：文字色等于 `--red`、背景透明
4. 每页 `.btn-primary` 数量 ≤ 1（逐页断言）
5. 模型列表每一行内胶囊数量 ≤ 1
6. 设置页 `.link-action` 的背景透明、且 `borderRadius ≠ 999px`
7. 组件里不再出现 `class="record-btn"`、`class="add"` 这类旧类名（`grep` 为空）
8. `npm run check` 0/0、`npm test` 全绿、`npm run build` 通过

---

## 7. 不做

- 不评价观感、不截图
- 不新增按钮、不调整文案、不改布局间距
- 不改表单控件（下拉、输入框、开关）
- 不动侧栏导航与顶栏的结构

---

## 8. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp/apps/desktop/src && \
  sed -n '20,50p' app.css && \
  grep -n "class=\"btn\|class=\"record-btn\|class=\"add\|class=\"link\"" lib/components/*.svelte | head -20
```
