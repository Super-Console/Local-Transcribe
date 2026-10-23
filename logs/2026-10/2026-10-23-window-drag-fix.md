# macOS 窗口拖动修复（2026-10-23）

## 授权与范围

2026-10-23，Timmy 报告“窗口又拖不动了，这个问题一直都出现”，明确要求修复。本轮只修窗口拖动：不发布 release、未获新的提交授权前不 commit（发布链路另有进行中的确认）。项目 `AGENTS.md` 随会话载入已读；未改动产品边界与依赖。

## 诊断（先复现、再定根因）

- 本机运行中的安装版（PID 25695，1052×803）实测三处拖动：侧栏顶空白区、顶栏中部空白区、工具栏左缘——前台（真实指针）与后台投递两种方式下窗口 bounds 全部不变，确认完全拖不动，不只是命中区太小。
- 上游核查：Tauri 官方 issue `tauri-apps/tauri#9503`（至今 Open、`type: bug`）即“macOS 上 `titleBarStyle: Overlay` 窗口拖不动”，与本项目配置一致；2026 年多个项目引用该 issue 并以手动 `startDragging()` 兜底。本机依赖 `tauri 2.12.4`，其注入的 `drag.js` 仅在 mousedown 目标本身带 `data-tauri-drag-region` 时触发。
- 本地叠加缺陷：`header-tools` 的属性被子元素 `.toolbar` 整面覆盖而永不命中；`.spacer` 因 `align-items: center` 高度为 0 不可命中；工具栏左右 padding 无属性。双层问题叠加，用户几乎无处可拖。

## 修复（TDD）

1. 失败测试先行：改造 `sidebar toggle remains outside...` 的旧属性断言，新增 `window header starts dragging programmatically for non-interactive targets`（断言 header 显式 mousedown 处理器、`startDragging()`、`closest()` 放行判断、标记中不得再出现旧属性）；红灯确认 2 失败 / 5 通过后才实现。
2. `+page.svelte`：`<header class="window-header" role="presentation" onmousedown={headerPointerDown}>`；处理器排除可交互目标（`a/button/input/select/textarea/label/summary`、contenteditable、tabindex、button/link/tab/menuitem/checkbox/radio/switch/option/listbox/menu 等 role），其余目标 `getCurrentWindow().startDragging()`，非 Tauri 环境与权限异常静默降级；双击（detail>1）不触发。
3. 移除 `+page.svelte` 两处与 `AppToolbar.svelte` 一处 `data-tauri-drag-region`，避免新旧机制双重触发；断言按“标记内不许出现”表述，允许注释提及。
4. `role="presentation"` 同时消除了 svelte a11y 对静态元素 mousedown 的警告。

## 验证

- `npm test`：7/7；`npm run check`：0 errors / 0 warnings；`npm run build`：成功。
- release 打包成功：`target/release/bundle/macos/Local Transcribe.app`（26.02 MiB，`--no-sign`，未签名/未公证），可执行文件 SHA-256 `f67b94305bb2286e047c68ac12b56f49985a303a483f3672018530991f841d16`。
- 安装前检查：本地任务 16 completed / 4 failed、无进行中任务；`lan-host` 近 5 分钟无活动；⌘Q 正常退出。

## 本机安装与修复后实测

- 旧版（`4953c749…` LAN 构建）`mv` 至 `~/Applications/Local Transcribe Backups/before-dragfix-20261023-152953/Local Transcribe.app`（回读一致）；新包暂存校验后 `mv` 入位，安装后回读 `f67b9430…` 一致；空暂存目录 `mv` 至废纸篓。全程无 `rm`、无永久删除。
- 启动验证：PID 66332 可执行文件来自安装路径，近 3 分钟无崩溃报告。
- 修复后实测（与修复前同三点位）：侧栏顶空白、顶栏中部空白、工具栏左缘三处前台拖动，窗口均精确随指针位移（bounds `830,205 → 789,181 → 748,157 → 707,133`）；侧栏开合按钮点击两次正常（收起/展开）；窗口已 `set_window_frame` 归位原坐标 `(830,205,900,620)`。

## 已知问题与限制

- **发布链路修正点**：此前 `1.1.0-beta.3` 升版实际未进入提交 `48a327e`（`package.json`、`tauri.conf.json`、`Cargo.toml` 仍为 `1.1.0-beta.2`），但 commit message 声称已升版、tag `1.1.0-beta.3` 已指向该提交；发布继续前必须以真正的升版提交 + 移动 tag 修正，CI 已由 `48a327e` 触发。`48a327e` 之前的推送为 Timmy 方案 A（升 beta.3 全新发布）明确授权。
- 本轮拖动修复未提交 Git（等待发布/提交确认）。
- MBA 未安装本修复；MBA 端窗口拖动未验证。
- 应用重启后“允许其他设备使用本机算力”按设计回到默认关闭（重启前 Timmy 手动开启的状态随退出失效，需重新开启）；配对令牌文件未改动，本日志不记录任何令牌值。
- 未做签名/公证；版本号显示仍为 `1.1.0-beta.2`（升版未落地）。
