# Local Transcribe — 顶栏材质修正任务书 V1.0

> **性质**：补丁任务书（第八份）。修一件事：玻璃顶栏现在**看起来没生效**。
> **覆盖范围**：本文件覆盖《阅读器：查找、渐隐与功能层材质任务书 V1.1》第 3 节（返回按钮玻璃）、第 6 节（渐隐实现方式）、第 7 节（材质参数）。其余条款不变。
> **不得修改其它任务书。**

---

## 1. 现状与判断

**症状**（Timmy 实际观察）：滚动时正文那行字穿过顶栏，能清楚读出字，和「首页」「查找」「TXT」叠在一起。

**两个原因，第二个是主因**

1. 有效不透明度太低：`color-mix(in srgb, var(--glass) 82%, transparent)`，而 `--glass` 是 `rgba(255,255,255,.52)` → 实际约 **0.43**，背后 57% 的字透上来。
2. **`backdrop-filter` 很可能根本没生效**：`.reader-bar` 是 `.reader-scroll` 的子元素，而后者带 `mask-image`。祖先带 mask / filter / transform / opacity<1 时，`backdrop-filter` 的采样会被切断，表现就是「设了等于没设」。

判定依据不是猜的：如果 14px 模糊真的生效，条下的中文字应该已经不可辨认。实际能读出字形 → 模糊没起作用。

---

## 2. 第一步：先做 5 分钟验证（不要直接改参数）

按顺序做，把结果写进日志：

1. 开发者工具里临时删掉 `.reader-scroll` 的 `mask-image` 与 `-webkit-mask-image`
2. 刷新，滚动一次，观察条下文字是否变糊
3. **变糊了** → 坐实是 mask 打断采样，按第 3 节改结构
4. **还是清晰的** → 问题在别处（父级 transform/opacity、层级、或 WebView 不支持），记录下来再改

这一条必须做，不许跳过直接调数值——否则调完还是没效果，而且会以为"参数不够"。

---

## 3. 结构修正：bar 与 scroller 做兄弟，彻底不用 mask

**目标结构**

```
.reader-shell                       ← 无 mask / filter / transform / opacity
  ├ .reader-bar                     ← position: absolute; top:0; 玻璃在这里
  └ .reader-scroll                  ← 唯一的滚动容器，无 mask
      └ 正文
  └ .fade-bottom                    ← 底部渐隐覆盖层（pointer-events: none）
```

**要求**

- `.reader-bar` 不再是被 mask 容器的子元素；它与滚动容器同级，覆盖在其上方
- **删除所有 `mask-image`**，包括 `.reader-scroll` 的底部渐隐和 `.at-bottom` / `.short` 变体
- 底部渐隐改由独立的 `.fade-bottom` 覆盖层实现：`pointer-events: none`，高 24px（到底时 8px，不可滚动时 8px），渐变**颜色必须取正文背景的实际计算值**（不要猜；深色主题用对应色）
- 顶栏下沿渐隐同样用覆盖层（高 12–16px），`scrollTop <= 2` 时隐藏
- 正文顶部留出与 bar 等高的 padding，初始状态第一段不被遮
- 滚动逻辑（`at-top` / `at-bottom`）继续存在，只是从控制 mask 改为控制渐隐层的显隐

**为什么不用 mask**：mask 是这一整类问题（backdrop-filter 失效、遮罩把玻璃条一起淡掉、选中与打印异常）的根源。覆盖层方案没有这些副作用，代价只是渐变色必须和背景一致。

---

## 4. 参数（浅色 / 深色分别给）

**先确认底色再填数值**：阅读器正文坐在 `--panel`（`#fffefb`）上，应用顶栏坐在 `--bg`（`#faf9f5`）上。两者不同，不要照抄同一个 rgba。落地时用 `getComputedStyle` 读一遍实际背景色，再按 0.74–0.76 的 alpha 算。

**浅色 · 阅读器条**（底为 `--panel`）

```css
background: rgba(255, 254, 251, 0.76);
border-bottom: 1px solid rgba(20, 20, 19, 0.06);
box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.55);
backdrop-filter: blur(20px) saturate(1.1);
-webkit-backdrop-filter: blur(20px) saturate(1.1);
```

**浅色 · 应用顶栏**（底为 `--bg`）

```css
background: rgba(250, 249, 245, 0.76);
```

其余同上。

**深色**（两种底在当前实现里都落到 `--bg` 深色值 `#1e1e20`，落地后回读确认）

```css
background: rgba(30, 30, 32, 0.74);
border-bottom: 1px solid rgba(255, 255, 255, 0.07);
box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
backdrop-filter: blur(20px) saturate(1.1);
-webkit-backdrop-filter: blur(20px) saturate(1.1);
```

**参数红线**

- 不透明度低于 0.70 视为不合格（中文小字会和条上的字打架）
- 模糊低于 16px 视为不合格
- `saturate` 不超过 1.15（再高会把奶油底拉成脏黄）
- 条上文字用 `--text-2`，不给条上文字上色

---

## 5. 条上的小按钮：不要再叠玻璃

返回按钮现在是玻璃条上的玻璃按钮，等于玻璃叠玻璃。

- 返回按钮改为**实心浅底**：`background: var(--fill)` + `1px solid var(--hairline)`，hover 加深
- 尺寸仍按 V1.1 第 3 节：高 28–32px、命中区撑满、图标 12–14px、文字 12.5–13px
- 「查找 ⌘F」保持纯文字按钮，不加底
- 目的：**一个视觉层里只有一处玻璃**（那条），小控件靠边界表达可点击

---

## 6. 应用顶栏同样处理

`.toolbar` 当前用的是同一套 `color-mix` + `blur(14px)`。要求：

- 换成与阅读器条**完全相同的参数**（第 4 节）
- 检查它的祖先链有没有 mask / filter / transform / opacity<1；有就按第 3 节的方式挪开
- 顶栏下面就是滚动内容区，验证同样成立：滚动时条下文字不可辨认

---

## 7. 回滚与无障碍（不能丢）

- `<html data-glass="off">` 仍然有效：两条改实心 `--fill` + `--hairline`，**无** `backdrop-filter`
- `prefers-reduced-transparency: reduce` 自动等同 `off`
- `prefers-contrast: more` 时描边用 `--hairline-2`，文字用 `--text`
- 深色主题必须单独截图确认，不得只验浅色

---

## 8. 验收：只认一条硬标准

**截图放大看条下面第一行字：必须认不出写的是什么。**

- 认得出 → 不合格，继续调（先确认 blur 是否生效，再考虑加不透明度）
- 认不出，但能看出有东西在动 → 合格
- 完全看不出有东西在动（像实心板） → 太实，把不透明度降到 0.72–0.74 再试

其余验收项：

1. 条高与改造前一致（约 40px）
2. 条上文字与背后文字**不重叠**，可读
3. 条下沿有 12–16px 过渡带，滚到顶时消失
4. 底部渐隐仍在：中段 24px、到底 8px、不可滚动 8px
5. 正文第一段初始不被遮
6. 滚动到中段，条下字是雾、条上字是清
7. 深浅色各一张，Windows 与 macOS 各一张
8. `getComputedStyle` 断言：`.reader-bar` 有 `backdrop-filter`；`.reader-scroll` **不再有** `mask-image`
9. 视觉证明：连续截两张（一张正常、一张临时把 `backdrop-filter` 设 `none`），两张在条区域必须有可见差异——**这是证明模糊真的在跑的唯一办法**

---

## 9. 性能

- 模糊只在这两条上，不进滚动容器
- 长文档（≥20 分钟）滚动一次，记录是否掉帧；掉帧则降到 16px 并记录

---

## 10. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sed -n '374,440p' apps/desktop/src/lib/components/Reader.svelte && \
  sed -n '225,245p' apps/desktop/src/lib/components/AppToolbar.svelte && \
  sed -n '175,200p' apps/desktop/src/app.css
```
