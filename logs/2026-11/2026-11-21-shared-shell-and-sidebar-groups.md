# 2026-11-21 共享窗口外壳 AppShell 与侧栏分组参数化（供 Cloud Transcribe 复用）

## 背景

Cloud Transcribe 的 UI 验收要求两个 App 外观一致。四条整改中三条落在 Local-Transcribe
共享源：AppSidebar 参数化（分组 + items）、窗口外壳抽成 `lib/components/AppShell.svelte`、
红绿灯避让分平台。Timmy 明确要求修改本仓库并跑 Local 回归；这是对既往「共享源只读」
约束的显式覆盖（决策已记录在 Cloud Transcribe 的 PROJECT_STATUS）。

## 改动

- `apps/desktop/src/lib/components/AppShell.svelte`（新增）：窗口外壳结构 + CSS + 拖拽。
  `.window-header`（52px）/ `.sidebar-header`（padding-left 80px）/ `.header-tools` /
  `.workspace` / `.main` 与 `windows-sidebar-up` 全套规则从 `routes/+page.svelte` 原样迁入；
  `headerPointerDown` 改名 `startWindowDrag` 从 `<script module>` 导出，侧栏拖拽条复用同一入口。
- 平台标记：启动时 `document.documentElement.dataset.os = currentPlatform()`（UA 判定，
  仅 macos/windows 两个交付目标）；CSS 用 `:global([data-os="windows"]) .sidebar-header
  { padding-left: 10px; }`。macOS 保留 80px；未引入负 margin 或高度补偿。
  （裸写 `[data-os="windows"] .sidebar-header` 会被 Svelte scoping 判为 unused 剪掉，
  构建产物实测缺失，故祖先选择器用 `:global()` 包裹。）
- `apps/desktop/src/lib/components/AppSidebar.svelte`：参数化为 `groups`（每组
  `{ title, items: [{ view, icon, label }] }`）；每组标题仍渲染 `<div class="section">`；
  第 2 组起沿用原 24px 组间距。Local 调用方传原有 5 项（工作区：home/record/lan；
  管理：model/settings），样式与 token 不变。
- `apps/desktop/src/routes/+page.svelte`：改用 AppShell（toolbar/sidebar/overlay 三个
  snippet + children）；删除页面内外壳 CSS 与 `headerPointerDown`；`sidebarGroups`
  用 `$derived` 传入翻译后的分组文案。

## 验证（TDD）

- 先改 `tests/sidebar-shell.test.mjs` 到新契约（AppShell 断言、data-os 断言、分组断言），
  确认失败（红：仅 sidebar-shell.test.mjs fail，130 pass）；实现后全绿。
- `npm test`：143 passed / 0 failed / 0 skipped。
- `npm run check`：0 errors / 1 warning（既有 WhatsNewCard `state_referenced_locally`，
  非本次引入）。
- `npm run build`：成功；产物 CSS 含 `[data-os=windows] .sidebar-header.svelte-*{padding-left:10px}`。
- Rust 未改动，未跑 cargo 测试。

## 未做 / 限制

- 未发版、未改版本号（非 release）。
- macOS GUI 同状态对比截图在 Cloud Transcribe 侧任务中进行；Windows 验收截图受
  GitHub Actions 计费故障阻塞（详见 Cloud Transcribe 记录）。
