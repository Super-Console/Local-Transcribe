# Local Transcribe — 阅读器：查找、渐隐与功能层材质任务书 V1.1

> **合并说明**：本文件合并了原《阅读器查找与渐隐任务书 V1.0》与《液态玻璃功能层任务书 V1.0》。
> 那两份作废，**只读这一份**。行为要求没有删减，只是去掉了重复与互相冲突的部分。
> **执行前提**：排在其它界面任务之后，不并行（同样改 `Reader.svelte` / `AppToolbar.svelte` / `app.css` / i18n）。
> **不得修改其它任务书。**

**一句话目标**：查找收进左侧工具行（⌘F 才展开）、正文两端渐隐、功能层两个条上有真正成立的玻璃——而玻璃要成立，正文必须从条下面穿过去。

---

## 1. 结构改造（先做这一节，后面三项都依赖它）

**现状**：面包屑条、标题、元信息、正文依次排布；条在最上面，正文永远不会经过它，玻璃没有可折射的内容。

**改成**

```
┌──────────────────────────────────────┐
│ ‹ 首页  查找 ⌘F        ↓TXT  ↓JSON   │ ← sticky 玻璃条（约 40px）
├──────────────────────────────────────┤  ← 条的 ::after：滚动边缘渐隐
│  光纤背后无敌的工程.mp4               │
│  X-ASR Zipformer · 33:05 · 23:10     │
│  正文……（滚动后从玻璃条下方穿过）       │
└──────────────────────────────────────┘
```

- 滚动容器包含「玻璃条 + 标题 + 元信息 + 正文」；玻璃条在其中 `position: sticky; top: 0`，`z-index` 高于正文
- **标题与元信息会随滚动从条下面滑过**，这是预期行为。不要为了留住标题再加一层固定标题
- 条高与现在的面包屑行一致（约 40px），不得因为加材质变高
- 滚动容器顶部留出与条等高的 padding，初始状态第一段正文不被遮

---

## 2. 查找控件：从右侧移到左侧，改成「动作」而不是常驻控件

**现状**：200px 阔的圆角框飘在元信息行右侧；它既不属于标题也不属于工具行，且与顶栏放大镜形状几乎一样。

**位置**

```
‹ 首页     查找 ⌘F                              ↓TXT  ↓JSON
```

- 放在**面包屑条左侧**，紧跟「‹ 首页」；纯文字按钮，无边框无底色，hover 文字加深
- 收起状态**不出现输入框**，因此同一屏只有一个放大镜（顶栏那个）
- 点开 / ⌘F 后在**标题与正文之间**插入一条左对齐输入条，最大宽度 320px

**输入条**

```
🔍 [会议            ]  3 / 12   ↑  ↓   ×
```

- 计数 `当前 / 总数`；无匹配显示 `0 / 0`，不报错
- `↑` `↓` 上一个 / 下一个；`Enter` = 下一个，`Shift+Enter` = 上一个
- `×` 或 `Esc` 关闭：清空关键词与高亮，焦点回到正文容器
- 命中用 `--selected`，**当前命中**用 `--accent` 描边或加深，二者必须可区分
- 打开时焦点在输入框；关闭后焦点回正文，不得留在隐藏元素里

**不改**：查找只作用于当前这一篇；不跨文件、不改后端、不加新命令。

---

## 3. 返回按钮：尺寸修好，并给它玻璃质感

**现状**：`‹ 首页` 是一行 11.5px 的文字链接，命中区约 40×14，高度只有标准的三分之一。

**尺寸（HIG 桌面标准）**

- 高 28–32px，图标 12–14px，文字 12.5–13px，左右内边距 10–12px
- 命中区撑满整个按钮；左侧与正文起始位置对齐；与右侧 `TXT / JSON` 同一水平线

**玻璃质感**

```css
background: color-mix(in srgb, var(--glass) 82%, transparent);
border: 1px solid var(--glass-border);
box-shadow: inset 0 1px 0 var(--glass-highlight), var(--glass-shadow);
backdrop-filter: blur(12px) saturate(1.2);
-webkit-backdrop-filter: blur(12px) saturate(1.2);
```

- 只用 `app.css` 里已有的 `--glass*` token，不新造数值
- hover：底色略升、文字由 `--text-2` 变 `--text`；active 轻微内缩 1px
- 必须保留描边——纯色底上玻璃与背景的差值很小，没描边边缘会糊

**注意**：它已经坐在玻璃条上，**不要**再给它自己叠一层玻璃亮面，否则变成玻璃叠玻璃。按钮只需要「可点的边界」，不必二次玻璃化。

---

## 4. 两个搜索的文案分工（不合并）

- 顶栏搜全部：`search.placeholder` 五语言改为「搜索所有转写」类表述
- 阅读器搜本篇：`reader.searchPlaceholder` 五语言改为「在本篇中查找」类表述
- 只改文案，不加模式切换、不做结果分组

---

## 5. 「查看最新」不再压住正文

- 贴右下角，距容器边缘 12px
- 容器 `pointer-events: none`，按钮 `auto`
- 它下面那一层由第 6 节的底部渐隐承担，文字从按钮下面淡出，不是被白块截断
- 位置变化不得影响「跟随最新」的自动滚动逻辑

---

## 6. 渐隐：分工与规则

**顶部不再由滚动容器负责**。玻璃条就在容器顶部，如果对容器加顶部 mask，会把条本身一起淡掉。

| 位置 | 顶部（由条承担） | 底部（由容器承担） |
| --- | --- | --- |
| 最顶（`scrollTop <= 2`） | 不显示 | 24px |
| 中间 | 条的 `::after` 渐隐 12–16px | 24px |
| 最底（距底 `<= 2`） | 条的 `::after` 渐隐 | 8px |
| 不满一屏（不可滚动） | 不显示 | 8px |

**实现**

- 条的 `::after`：从玻璃底色渐到透明，高 12–16px，`scrollTop <= 2` 时隐藏
- 容器底部用 `mask-image: linear-gradient(...)`（含 `-webkit-` 前缀），**不要**盖一层渐变色
- **两者不得叠加**：容器上不许再加顶部 mask
- 不加 transition 动画；只在跨越阈值时切 class，不在滚动事件里每帧改样式
- 遮罩不得影响文字选中与复制

---

## 7. 材质：只上两个条

**允许**

1. 阅读器面包屑条
2. 应用顶栏（搜索 / 月亮 / 模型选择）

**禁止**：侧栏、历史卡片、模型条目、任务列表、正文容器、标题、元信息、任何按钮组、输入框内部、弹窗。

理由：内容层用玻璃会破坏层级；玻璃用多了会分散对内容的注意。

**配方（两条共用同一套）**

```css
background: color-mix(in srgb, var(--glass) 82%, transparent);
border-bottom: 1px solid var(--glass-border);
box-shadow: inset 0 1px 0 var(--glass-highlight);
backdrop-filter: blur(14px) saturate(1.25);
-webkit-backdrop-filter: blur(14px) saturate(1.25);
```

- 模糊半径 **≤ 16px**
- 条下面不得再叠实心底色
- 条上的文字与图标用 `--text-2`，不给条上文字上色
- 深色主题要单独确认：`--glass` 为 `rgba(255,255,255,.09)`，`--glass-border` 需调到「可见但不过亮」

**明确不做**：窗口级 vibrancy（`transparent: true` + `NSVisualEffectView`）。会影响 resize、深色模式与截屏一致性，且 Windows 无对应能力，两平台会长得不一样。本轮只做 CSS 层可跨平台一致的材质。

---

## 8. 跨平台

- `backdrop-filter` 在 WKWebView 与 WebView2 都支持，但**必须在两个平台各截一次图**，确认：
  1. 条不是纯黑或纯白（WebView2 在父级透明组合下可能退化成不透明）
  2. 滚动时确实看得出下方文字被模糊
  3. 文字对比度在两个平台都够
- **不写平台分支**；若 WebView2 退化，唯一允许的兜底是把玻璃底不透明度提到 92% 并记录

---

## 9. 性能

- 模糊只加在两条 40px 高的条上，**不得**加在滚动容器或正文
- 不在滚动事件里每帧写 style
- 用 ≥20 分钟的转写文档滚动一次，记录是否掉帧；有掉帧就把模糊降到 10px 并写下结论

---

## 10. 回滚（必须做）

- 根元素 `<html>` 使用 `data-glass="on" | "off"`，默认 `on`
- `[data-glass="off"]`：两条去掉 `backdrop-filter` 与半透明底，改实心 `--fill` + 1px `--hairline`，**布局完全不变**
- 关闭后视觉上应与改造前基本一致（允许 hairline 级差异）
- `prefers-reduced-transparency: reduce` 自动等同 `off`
- `prefers-contrast: more` 时描边用 `--hairline-2`、文字用 `--text`
- 开关写进 README 一句话；**不做设置页开关**
- 若后续提交，这项改造单独一个 commit，便于 `git revert`

---

## 11. 测试

1. 条高与改造前一致（±2px）
2. `⌘F` 打开并聚焦；`Esc` 关闭、清空、焦点回正文
3. 无匹配 → `0 / 0`，不抛错
4. 高亮数量与正文实际出现次数一致；当前命中与普通命中可区分
5. 返回按钮命中区实测高度 ≥ 28px（`getBoundingClientRect` 断言，不是目测）
6. `data-glass="off"` 时无 `backdrop-filter`，底色为实心 `--fill`；开 `prefers-reduced-transparency` 亦然
7. 滚动到顶：条下沿无灰影；中段：有渐隐且正文被模糊；到底：底部渐隐降为 8px
8. 不可滚动时：两端都不出现深渐隐
9. 侧栏、卡片、列表、弹窗**没有** `backdrop-filter`（`getComputedStyle` 断言）
10. 五语言 key 齐全（`search.placeholder`、`reader.searchPlaceholder`）

---

## 12. 验收

- 四张截图：滚动到顶 / 中段 / 到底 / `data-glass="off"` 对比
- macOS 与 Windows 各一组，能看出条下方文字被模糊
- 深浅色主题各一次；渐隐边缘不得出现色带或与背景不一致的方块
- 返回按钮：高 28–32px、有上沿高光与 1px 描边、hover 有非模糊的可见变化
- 长文档滚动一次，记录帧率或「是否有可见卡顿」
- 口径：返回按钮在纯色底上应读作「磨砂小标签」，**不要**拿宣传图里的折射效果做验收标准

---

## 13. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sed -n '1,32p' apps/desktop/src/app.css && \
  sed -n '120,175p' apps/desktop/src/lib/components/Reader.svelte && \
  sed -n '195,270p' apps/desktop/src/lib/components/Reader.svelte && \
  sed -n '60,120p' apps/desktop/src/lib/components/AppToolbar.svelte && \
  grep -n "glass" apps/desktop/src/app.css
```
