# Local Transcribe 首次引导 OOBE 实现与验证

日期：2026-10-26
对象：`TimmySheep/Local-Transcribe` 桌面端，独立功能分支 `openchamber/oobe-onboarding`
执行者：OpenCode
状态：代码未提交、未发布；最终视觉稿仍待 Timmy 确认

## 实施

- 增加 OOBE 状态持久化、硬件/数据卷空间检测、Tauri 命令及首次启动/设置页重跑入口；状态文件以临时文件写入并回读校验，Windows 用 `MoveFileExW` 替换已存在文件。
- 增加临时五步 Svelte 引导、Windows 权限步跳过、五种语言文案、设备与磁盘约束下的 ASR/LLM 建议、权限显式请求、真实数据/模型/日志路径、跳过和完成流程。
- 模型默认偏好显示“未下载”状态，下载其他 ASR 模型不会覆盖一个尚未安装的显式偏好；本地转写在偏好模型缺失时明确提示，但已可用的 LAN 节点仍可接收任务。
- 补齐完成页“上一步”；更新模型页、设置页与 browser probe fixture 的新属性。
- 未修改任何任务书；UI 仅为功能验证，不代表最终设计。

## 验证

- `cargo test --workspace --offline --quiet`：通过；被忽略项均明确依赖本地模型、媒体样例或人工质量测量。
- `cargo clippy --workspace --all-targets --offline -- -D warnings`：通过。
- `cargo fmt --all -- --check`、`git diff --check`：通过。
- `apps/desktop/tests/onboarding.test.mjs`：10/10 通过；OOBE 相关 4 个 Svelte 文件使用已有编译器解析，0 条编译警告。
- `npm test`：113/115 通过；`reader-interaction.test.mjs` 与 `reader-spec.test.mjs` 在导入阶段因 worktree 缺少 Vite 而未启动。`npm run check` 同样受缺失前端依赖阻塞；尝试复用原 checkout 的 `node_modules` 时，发现当前 `vitefu` 不提供该 worktree 所用插件要求的 `findClosestPkgJsonPath` 导出。没有安装依赖。
- 未进行 macOS TCC 弹窗实机验收；未在真实 Windows 主机或 Windows CI 上验证该分支，Windows OOBE 验收仍阻塞。没有执行截图或视觉验收。

## 记录约束

- `logs/README.md` 在本功能 worktree 与原 checkout 中均不存在；未擅自创建该说明文件。项目 `AGENTS.md`、OOBE 任务书及项目状态已阅读。
- 原 checkout 的未提交网站、移动端与文档工作未混入本分支；没有创建 Git commit。

## 合并与用户验收补记

- 用户明确授权合并后，创建 `c57156f`（OOBE）与 `9238a51`（JSDoc 类型），fast-forward 至 main；未覆盖既有网站、移动端改动。
- 原 checkout 已有可复用前端依赖，无新增安装。补齐类型后 `npm run check` 0 errors / 0 warnings；`npm test` 115/115。首次与 check 并行运行有浏览器 target closed，分开重跑全通过。
- Timmy 明确表示在 MacBook Pro 测试本版本，没有问题，接受功能实现。此记录来自用户验收，不冒充 AI 观察的系统权限弹窗/Windows 实机验收。
- 两次 Tauri dev 构建成功，但启动后退出，未出现可确认的新窗口；观察到的 PID 49546 是已安装应用，不是新 debug binary，不将其截图计为新版本证据。
