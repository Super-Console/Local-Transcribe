# 阅读器交互修复与胶囊统一（2026-10-24）

依据：《交互修复与胶囊统一任务书 V1.0》；沿用阅读器胶囊化任务中未被新要求替代的约束。未修改任何任务书。按 Timmy 指示不做 CUA 验收，平台 GUI 截图与最终验收交由 Timmy。

## 当前事实与证据范围

- 技术栈为现有 Tauri 2 / Svelte 5，普通 CSS + CSS custom properties，没有 Tailwind 或组件库；已有浅深色 token。本轮没有安装依赖。
- 旧的复制 DOM/CSS harness 不是应用行为测试，不能据此声称导航、聚焦或 WKWebView 已验收。本轮新增测试会由 Vite 编译并挂载项目真实的 `Reader.svelte`、`HistoryView.svelte`、`AppToolbar.svelte`，使用现有 Chrome headless 执行；不是 CUA，也不是原生 WKWebView 验收。
- 未重做掩膜移除的视觉因果实验；历史对照不完整，不写为通过。

## P0 优先修复与先红后绿

### 首页、查找的命中与点击

- `Reader.svelte` 的 `.reader-wrap` 为 `pointer-events: none`。现在 `.reader-bar` 明确 `pointer-events: auto; z-index: 3`，渐隐层为 `pointer-events: none; z-index: 2`。
- 对按钮装饰子元素设置 `pointer-events: none`，使中心坐标直接返回 button，不只是内部 span/svg。
- 真实组件测试里受控设回 bar 的旧值 `none`，中心命中 fixture 而非按钮；移除此内联值后命中真实按钮。此为受控复现，不冒充旧安装版本的现场采样。
- `tests/browser/reader-interaction.ts` 对实际命中的节点调用 `.click()`，首页回调两次均被调用；查找两次均展开，且 `document.activeElement` 等于输入框；Esc 关闭后再次打开通过。不只是检查元素存在。
- 这是组件回调/焦点测试；整个桌面应用回首页路由仍待 Timmy 手工验收。

### 下拉菜单不透明

- 真实组件测试首次失败：背景 alpha 已为 1，但菜单整体 `opacity` 为 0。`svelte/transition` 的 `scale` 默认包含 opacity 动画；因此“实心背景”本身不能保证整个菜单始终不透明。
- 移除菜单的入场 scale/退场 fade，不再让内容层整体透明；不影响全局其它动画。
- `.menu` 使用不透明 `--panel`、1px `--hairline`、12px 圆角、`--shadow-panel`，无 backdrop-filter；深色底使用 `--bg`。toolbar 的 stacking context 提升至 z-index 10，菜单内部为 40，高于阅读器控件/渐隐层。
- hover 为 `--hover`、选中为 `--selected`。浅色 `--panel` 对齐任务要求的 `#fffefb`。
- 修复后 getComputedStyle：浅色 `backgroundColor = rgb(255, 254, 251)`、深色 `rgb(30, 30, 32)`；两者 `opacity = 1`、`backdropFilter = none`。

### 顶栏文字、调试标签与滚动条

- `Reader.svelte` 中临时 TOP/MID 调试标签已移除；本轮 grep 检查 `apps/desktop/src`，未发现 `class="dbg"`、`blur-test`、`>TOP<`、`atTop ? "TOP"`。
- `.reader-scroll` 从 `top: 40px` 开始，轨道不再进入 0–40px 顶栏。顶部渐隐仍从容器 y=0 开始、高 56px，正文 < fade < bar；scrollTop <= 2 时隐藏。未使用 mask-image。
- 在组件测试中实际滚动后检查 fade 显示，并核对 scrollport 顶部不高于 bar 底部。
- 保留滚动/恢复逻辑；ResizeObserver 与 scroll 事件测量状态。未改成自绘滚动条。

## document.elementFromPoint 实测输出

环境：1000×800 headless Chromium，真实 Svelte 组件、合成文字；以下坐标仅属于本次 fixture，不能套用到原截图。

```text
受控旧值：bar.pointerEvents = none
document.elementFromPoint(174, 36) -> <div id="fixture" ...>

修复后的实际样式：bar.pointerEvents = auto，fade.pointerEvents = none
document.elementFromPoint(174, 36) -> <button class="back svelte-…">…</button>
document.elementFromPoint(250.5234375, 36) -> <button class="find-toggle svelte-…">…</button>
document.elementFromPoint(500, 36) -> <div class="reader-bar svelte-…">…</div>
两轮按钮采样结果相同；两轮回调/输入焦点断言通过。
```

**原截图「…ON」「TOP」「thumb」三处命中输出：未采集。** 本次上下文没有原截图可校准坐标，旧调试构建也未保留现场；不得用上面的按钮/空白点输出冒充这三处。TOP 的调试标签来源由历史源代码变更确认；无法据此断定同屏其它字符的原始命中。此项为证据限制，非已验收。

## P1–P3（P0 组件测试通过后继续）

- 圆角：主内容 `.main`、首页/转写 `.content` 使用 `--radius` 并裁剪溢出；首页列表、reader shell/scrollport 同用 12px token；查看最新维持 999px 全圆角。原生窗口外角仍由系统管理。
- 导出：TXT/JSON（及同排复制按钮）统一 30px、999px、左右 10px、12px 字号/图标、次要底色、细边、上沿高光与 blur18；hover 使用既有 .78 token。覆盖 snippet scoped CSS，避免旧 26px/6px 规则反压。
- Transcribe 的 reader actions 去掉原 16px margin-top，间距 8px，避免导出控件下沉。History 直接继承 bar 的 8px gap。
- 搜索/月亮为 30×30 正圆，模型为 30px 文字胶囊；同底/边/高光/blur、hover/active/focus，模型名长文本省略且可收缩。
- 原生滚动条：`.content`/`.reader-scroll` 同时使用 scrollbar-width/color 和 WebKit 8px/透明轨道/4px thumb/2px 透明边；深色 thumb 为白色 .16，未隐藏或自绘。
- 样式新增测试先失败于导出旧高度/圆角/缺少 blur、右上三个控件缺少 blur；实现后真实组件计算样式断言通过。

## 验证与产物

- `npm run check`：0 errors / 0 warnings。
- `npm test`：38 passed / 0 failed / 0 skipped（含一个真实浏览器组件测试，内部多项行为与计算样式断言；其余很多仍为源码断言，不混称端到端）。
- `npm run build`：通过。
- Rust 业务代码未在本轮修改，本轮未重新声称全量 Rust 测试已执行。
- macOS bundle 正在构建，结果与安装情况另行追加；版本未升级，未提交、推送、打 tag 或发布。
- 测试启动的 Vite 服务与 Chrome 均随测试结束关闭。临时 Chrome profile 留存，未自动删除/移入废纸篓；清理必须另外列清单并获准。

## ⛔ 待 Timmy 人工验收（不计作未实现功能）

1. 阅读器：首页回列表、重新进详情；查找展开并聚焦，各点两次。
2. 打开模型菜单：浅深色都不应看到正文笔画穿透。
3. 长文上下滚动：0–40px 无正文/TOP；thumb 在头栏下方；滚顶第一段完整，滚后 fade 出现。
4. 首页/阅读器四角与查看最新浮层各看 2× 局部，无直角背景溢出。
5. TXT/JSON 与右上三控件同族；长模型名不撑破布局；键盘焦点清晰；深色无白斑。
6. macOS 与 Windows 原生滚动条实际细淡/透明轨道/无箭头。本轮未在 Windows 执行或安装，不能写 Windows 已通过。
7. 正常/关闭玻璃对照、降低透明度、长文滚动性能仍待人工；不声称截图或20分钟性能验收通过。

## 最终重建与安装回读（2026-10-24 01:45，本机时间）

- 补足降低透明度 media query 的 specificity，避免 Svelte scoped 样式覆盖其关闭 blur 的规则；显式关闭玻璃的真实组件测试也确认 capsule blur 为 none，顶部 fade 仍在。
- 最终源码重新执行 `npm run check && npm test && npm run bundle:mac`：0 errors/0 warnings；38 passed/0 failed/0 skipped；release bundle 通过，约 46.10 MiB。不是此前中断的构建产物。
- `.app` 与 node sidecar 均为 ad-hoc 签名，identifier 仍为 `com.timmy.voicetotextapp`；不是 Developer ID 签名或公证。
- 安装前备份：`~/Applications/LocalTranscribe-backups/before-reader-interaction-20261024-014506/Local Transcribe.app`。未删除任何旧包或清理临时目录。
- 正常退出旧应用后通过 ditto 换装 `~/Applications/Local Transcribe.app`，未强制 kill；重开后读到 GUI PID 92562。
- 构建包及安装包都通过 `codesign --verify --deep --strict`，两个二进制 SHA-256 均逐个相同：
  - desktop：`092dca3664b38f1031aa6429d2405620e7628e946be98e076ada85c1e8d8089a`
  - node：`e536446b71f3f764d15f19451c6731d668394679f6b19051d809de0b18a97e8a`
- 版本仍为 1.1.0-beta.5；本轮未重新安装 Windows、未做 CUA 验收、未发布。上述人工清单仍待 Timmy。

## 两份任务书的源码自查与补漏（同日后续，不作观感判断）

- 范围仅阅读器胶囊化、交互修复与胶囊统一两份；未改任务书。逐条表在当前会话，不新增总结报告文件。
- 新增 `tests/reader-spec.test.mjs` / `tests/browser/reader-spec-probe.ts`，由现有 Chrome DevTools Protocol 读取真实组件计算样式与几何，并模拟 reduce/contrast 媒体条件；无新增依赖、无 CUA。
- 先红后绿修复：fade 的起色与实际正文底色不同（正文 scrollport 现明确使用 `--reader-fade`）；off/reduce 用了 panel 而非 fill；contrast 被 scoped CSS 覆盖；两端渐隐子层缺圆角；首页实际 `.list` 漏 scrollbar-width/WebKit 规则。
- reader bar 局部的 `--fill` 现在引用实心 token，浅色 `rgb(242,239,230)`，深色 `rgb(61,61,64)`；深色是原白色 .09 填充叠在 panel 上的实心值，不改变应用其它位置的 --fill。实测 off 与 reduce 的两个胶囊无 blur，fade 保留；contrast 文字为 text，边为 hairline-2。
- 长模型名还会把箭头从12px压至0.703125px（窄处0.375px）；新增失败断言后用 `flex:none` 修复。修后箭头12px，窄toolbar内模型右边界402≤420、286≤304，文本省略保留。
- 计算实测：content/shell/scroll圆角12px，latest999px；top fade圆角12/12/0/0，bottom fade为0/0/12/12；实际滚动列表及reader均thin。fade在scrollTop 0/2时opacity0，在3/160时opacity1。
- 当前真实History组件命中：JSON中心 `(824.359375,38) → BUTTON.act`；当前滚动边缘点 `(856,78) → DIV.file`；上方同一横坐标 `(856,38) → BUTTON.act`；TOP元素查询不存在。这些不是原异常三处的历史输出，原三点仍未采集，不能补造。原生thumb不是可单独query的DOM节点。
- 规格冲突保留：前份TXT/JSON默认透明与后份P2默认`.66`不能同时满足，继续采用后份胶囊规格；后份通用hover“+.06”与明确`.66→.78`也不相符，保留明确配方，不自行改动任务书。
- 最终 `npm run check` 0 errors/0 warnings，`npm test` 39 passed/0 failed/0 skipped，`npm run build`通过。原始测试输出保留在本会话临时目录 `reader-spec-before.log`、`reader-spec-arrow-before.log`、`reader-spec-final-tests.log`；未自动清理。
- 本轮仅修源码与前端产物，未重打/换装.app，未重启应用，未执行Windows测试，未提交或发布；之前安装包不含本段补漏。
