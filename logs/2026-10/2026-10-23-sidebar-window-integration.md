# Sidebar 与窗口顶部融合迭代 / MBP 更新

## 授权与边界

2026-10-23，Timmy 提供 TranscribeX 与现有应用截图，授权修改 sidebar；随后明确要求在 MBP 更新并打开验收。保留暖色主题与现有导航，不扩展产品功能，不新增依赖、不提交 Git。产品名称与最终视觉仍待确认。

项目 `AGENTS.md` 与任务书/架构/技术验证/当前状态已阅读；`logs/` 可读取，`logs/README.md` 不存在。

## 修改

- `AppSidebar.svelte`：204px 宽、16px 品牌、13px / 32px 导航，管理入口改为跟随主导航；分组使用次级文字色；侧栏可滚动；恢复键盘 focus-visible。
- `+page.svelte`：52px 共享顶栏，独立于可折叠导航的圆形开关；恢复快捷键和原持久化键；顶栏与侧栏背景/边界对齐；保留五个入口。
- `AppToolbar.svelte`：空白处可拖动窗口。
- Tauri：Overlay 原生标题栏，红绿灯位置 x16 / y28；只新增窗口拖动权限，无新网络/文件权限。旧 Rust accessory 保持关闭，本轮用前端按钮而非两个按钮并存。
- `tests/sidebar-shell.test.mjs` 与 `npm test`：配置/结构回归，不把源码断言称为端到端测试。

## 验证

1. 结构回归在实现前 1 通过 / 3 预期失败；实现后 4/4 通过。
2. `npm run check`：0 errors / 0 warnings；`npm run build`：成功。
3. 用已有 Chrome headless、Node 内置 WebSocket/CDP 做独立浏览器运行验证，未安装依赖。900×620 / 640×620 无页面水平溢出、侧栏与标题栏对应区域均 204px；五个入口保留；收起/展开、inert、刷新后记忆、⌘\\ 均通过。浅深色截图位于批准的 OpenCode scratch 目录。
4. Tauri 首次传 `--offline` 被 CLI 拒绝，改用 `CARGO_NET_OFFLINE=true`；首次构建在 120 秒超时，未计为成功。随后长超时构建成功；真机发现红绿灯纵向略高后补设位置并重新成功打包。最终命令：`CARGO_NET_OFFLINE=true npm run tauri -- build --bundles app --no-sign`。产物 `target/release/bundle/macos/Local Transcribe.app`，25.41 MiB，未签名/未公证。
5. `cargo test --workspace --offline`：在 LAN crate 前执行的 core / desktop 单元与集成测试均通过，真实模型测试 ignored；LAN 单元 16 通过 / 1 失败，后续 workspace 测试未继续。失败：`client::tests::resolved_service_txt_records_map_to_node_info`，`crates/lan-compute/src/client.rs:389`，`ParseIpAddr("invalid IP address syntax")`。本轮未修改 LAN 文件；不宣称全套通过，不越范围修复。

## 本机部署与回退

- 当前机器 hostname `tims-mbp`，arm64。
- 更新前检查应用首页，无进行中转写显示；通过正常 Quit 菜单退出并检查进程终止。
- 暂存复制 bundle，比较可执行文件哈希；旧应用改名保留，再把暂存应用移到正式位置。没有删除文件、没有移动到废纸篓，也未改模型/历史数据。
- 原版备份：`~/Applications/Local Transcribe Backups/before-sidebar-20261023-114123/Local Transcribe.app`。
- 首次对齐前中间版备份：`~/Applications/Local Transcribe Backups/before-traffic-alignment-20261023-114547/Local Transcribe.app`。
- 安装：`~/Applications/Local Transcribe.app`。
- 源与安装可执行文件 SHA-256：`a21cfcb748dc377fbeef7d23f8adef116c3062af1e3a01298e4e56f06fcaee9e`。
- CUA 按 bundle ID 启动误选备份路径，两次均经进程路径发现、正常退出，未当作新版成功。刷新 LaunchServices 注册没有解决该选择。最后在用户明确要求打开验收的授权下，以应用准确路径前台打开；进程 83189 路径核实为正式安装目录，窗口 33054，900×620。
- 最终原生截图：`/private/var/folders/p1/3pqx3j392m381v0g293m9rgc0000gn/T/opencode/sidebar-native-final.png`。真实 AX 点击验证隐藏/显示，最后展开侧栏、保留首页供用户验收。
- 回退需要退出应用并切换上述原版备份，备份已核对存在；本次未执行回退。Windows 未验证，真实转写未重测。

## 留存

临时浏览器 profile、截图、检查脚本与两份应用备份仍保留；须先向 Timmy 列出精确清单并征得同意后，才能按废纸篓规则清理。不清理共享构建缓存或废纸篓。未触碰任务过程中出现的其他未跟踪任务书。
