# Local Transcribe — 阅读器顶栏胶囊化任务书 V1.0

> **性质**：补丁任务书（第九份）。改一件事：**取消整条玻璃，只让「首页」「查找」两个小胶囊带玻璃，用顶部渐隐代替那条。**
> **覆盖范围**：本文件覆盖以下条款，其余全部不变：
> - 《Local Transcribe — 顶栏材质修正任务书 V1.0》第 3、4、5 节（整条玻璃、参数、按钮实心化）
> - 《阅读器：查找、渐隐与功能层材质任务书 V1.1》第 3 节（返回按钮玻璃配方）、第 6 节（顶部渐隐归属）、第 7 节（整体条材质）
> **不得修改其它任务书。**
> **执行前提**：排在其它界面任务之后，不并行。

---

## 1. 目标形态

```
┌────────────────────────────────────────┐
│  (  ‹ 首页 )  ( 查找 ⌘F )      ↓TXT  ↓JSON  │  ← 无通栏底；只有两个胶囊有材质
│  ░░░░░░░░░ 顶部渐隐带（仅滚动后出现）░░░░░░░░░  │
│  正文……（滚到顶部带内淡出）                  │
└────────────────────────────────────────┘
```

- **整条背景彻底消失**：透明、无底色、无边框、无分割线、无圆角
- **「首页」是玻璃胶囊**，**「查找 ⌘F」是更淡的玻璃胶囊**（同排，一主一次）
- **TXT / JSON 保持纯文字**，hover 时给极淡底，不常驻底色
- 正文滚到顶部带内**淡出**，不与控件相撞

---

## 2. 结构

```
.reader-shell
  ├ .reader-fade      ← 顶部渐隐层，position:absolute; top:0; height:56px;
  │                      pointer-events:none; z-index:2
  ├ .reader-bar       ← 保留为布局行（height:40px），但**完全透明**；z-index:3
  │    ├ 返回胶囊
  │    ├ 查找胶囊
  │    └ TXT / JSON（纯文字）
  └ .reader-scroll    ← 滚动容器，无 mask；底部渐隐仍由覆盖层负责
```

**硬性要求**

- `.reader-bar` **不得**再有任何 `background` / `border-bottom` / `box-shadow` / `backdrop-filter`
- 玻璃只出现在两个胶囊上
- `.reader-fade` 必须 `pointer-events: none`，否则会拦住文字选中
- `.reader-scroll` 继续不用 `mask-image`（这是上一轮已修正的结论，保持）
- 正文顶部 padding 保留，初始状态第一段不被遮

---

## 3. 「首页」胶囊（主）

| 项 | 值 |
| --- | --- |
| 形状 | `border-radius: 999px`，高 30px，左右内边距 12px |
| 底色 | `rgba(255, 254, 251, 0.84)`（深色见第 6 节） |
| 模糊 | `blur(18px) saturate(1.1)` + `-webkit-` 前缀 |
| 边界 | `1px solid rgba(20, 20, 19, 0.07)` |
| 高光 | `inset 0 1px 0 rgba(255,255,255,.6)` |
| 内容 | 图标 `‹` 12–14px + 「首页」12.5–13px，`--text-2` |
| 交互 | hover：底色升到 .90；active：内缩 1px；聚焦：2px `--accent` 外环 |

## 4. 「查找 ⌘F」胶囊（次）

同样的形状与高度，但**明显轻一档**，形成主次：

| 项 | 值 |
| --- | --- |
| 底色 | `rgba(255, 254, 251, 0.66)` |
| 边界 | `1px solid rgba(20, 20, 19, 0.05)` |
| 模糊 | 同 18px |
| 内容 | 「查找」+ `⌘F`（`⌘F` 用 mono 10px、`--text-3`） |
| 交互 | hover 底色升到 .78；展开查找输入条时，此胶囊保持按下态（底色 .82、内缩 1px） |

> 两个胶囊之间间距 8px。它们是一组，不是两排。

## 5. TXT / JSON

- 纯文字，`--text-3`，12px
- hover：加 `background: var(--hover)`，圆角 6px，**只在 hover 出现**
- 不放胶囊、不加边框

---

## 6. 顶部渐隐带（替代那条的视觉作用）

**目的**：让正文靠近顶部时淡出，使纯文字的 TXT / JSON 不与正文重叠。

- 高度 56px；`position: absolute; top: 0; left: 0; right: 0`
- 渐变：从**正文实际背景色**（浅色 `#fffefb`）到透明；深色用 `#1e1e20`
- `pointer-events: none`
- **只在滚动后出现**：`scrollTop <= 2` 时完全隐藏（否则会洗掉第一段的顶部）
- 不加 transition（滚动时出现动画会抖），用阈值切换

**深色参数**

| 项 | 浅色 | 深色 |
| --- | --- | --- |
| 首页胶囊底 | `rgba(255,254,251,.84)` | `rgba(38,38,40,.82)` |
| 首页胶囊边 | `rgba(20,20,19,.07)` | `rgba(255,255,255,.08)` |
| 查找胶囊底 | `rgba(255,254,251,.66)` | `rgba(38,38,40,.62)` |
| 顶部渐隐起色 | `#fffefb` | `#1e1e20` |

深色下必须单独确认：胶囊不要变成一块发亮的白斑（.82 的深色底在深色背景上会显亮，必要时降到 .75）。

---

## 7. 回滚与无障碍（不能丢）

- `<html data-glass="off">`：两个胶囊改为实心 `--fill` + `1px --hairline`，**无** `backdrop-filter`；顶部渐隐保留（它不是玻璃，是遮罩）
- `prefers-reduced-transparency: reduce`：等同 `off`
- `prefers-contrast: more`：胶囊描边用 `--hairline-2`，文字用 `--text`
- 键盘焦点必须在两个胶囊上可见（`--accent` 外环），不得依赖玻璃表达可聚焦

---

## 8. 验收（硬标准）

1. **顶部没有矩形边界**：截图里看不出任何通栏的边、底色或分割线；只有两个胶囊有形状
2. 两个胶囊尺寸一致（高 30px），间距 8px，圆心同高
3. 滚动后正文在顶部 56px 范围内淡出，**不会和「查找」「TXT」叠字**
4. 滚回最顶时，渐隐带消失，第一段完整可见
5. 玻璃对照：同一位置截两张（正常 / 临时把胶囊的 `backdrop-filter` 设 `none`），胶囊区域必须有可见差异——证明模糊真的在跑
6. `getComputedStyle` 断言：
   - `.reader-bar` 的 `background-color` 为透明、`backdrop-filter` 为 `none`
   - 两个胶囊有 `backdrop-filter`
   - `.reader-scroll` 无 `mask-image`
7. 深浅色各一组、macOS 与 Windows 各一组
8. 长文档滚动一次，记录是否掉帧

---

## 9. 测试

1. 顶部渐隐：`scrollTop=0` 隐藏、中段出现
2. 胶囊在 `data-glass="off"` 时无 `backdrop-filter`
3. 两个胶囊高度一致、圆角为全圆
4. `.reader-bar` 无背景、无边框、无阴影
5. TXT / JSON hover 才有底色
6. 焦点环在两个胶囊上可见（键盘 Tab 走一遍）

---

## 10. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sed -n '240,300p' apps/desktop/src/lib/components/Reader.svelte && \
  sed -n '370,470p' apps/desktop/src/lib/components/Reader.svelte && \
  grep -n "reader-bar\|reader-fade" apps/desktop/src/lib/components/Reader.svelte | head
```
