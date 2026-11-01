# 移除文件访问白名单 + 首页历史自动刷新（1.7.0）

日期：2026-11-01
执行：OpenCode（DeepSeek V4.1 Flash），工作树 `local-transcribe-170-release`

## 决定与范围

Timmy 明确要求：

1. 彻底删除文件访问白名单，默认允许读取任意路径（含写入导出），不再保留设置项、CLI 命令或 `access.json`。
2. 修复首页历史列表不热更新的问题。

跨账户隔离（Windows 不同本地用户互相拒绝）由 Timmy 明确放弃，不再作为发布门槛。

## 代码变更

- 删除 `crates/transcription-service/src/access.rs`（原 454 行）与
  `apps/desktop/src-tauri/src/access_settings.rs`（原 246 行）。两份文件未永久删除，
  已移入 `~/.Trash/Local-Transcribe-code-removal-20261101-194749/`。
- `crates/transcription-service/src/lib.rs`：移除 `AccessRoots` / `SetRoots` / `AddRoot` /
  `RemoveRoot` / `AuthorizeInput` 请求变体、`RootsState`（约 160 行，含
  `check_authorized_with`）、`ServerOptions::allow_roots`、`canonical_roots`、两份白名单测试模块；
  `SubmitAuthorized` / `ExportAuthorized` 去掉 `temporary_roots`；
  `authorize_input` 只保留「绝对路径 + 存在 + 是普通文件 + 能识别媒体容器」的校验。
- `crates/transcription-service/src/sidecar.rs`：`render_mcp_config` 不再接受或输出目录参数。
- `crates/transcription-service/src/main.rs`：`serve` 去掉 `--allow-root`。
- `crates/transcription-cli/src/main.rs`：删除 `roots` 子命令、`--root` / `--allow-root`
  参数、提交任务时自动把文件所在目录当临时授权根的逻辑。
- `crates/transcription-mcp/src/main.rs`：删除 `allowed_roots` 字段、argv 根与相关校验，
  只做路径规范化后转交服务。
- `apps/desktop/src-tauri/src/lib.rs`：`start_transcription` 直接 `InputFile::inspect`，
  不再经服务授权；注销三个 `access_*` 命令。
- 前端：`SettingsView.svelte` 删除「文件访问」分组与相关状态和样式；`TranscribeView.svelte`
  删除「添加该文件夹」一键修复；`+page.svelte` 删除 `blockedPath` / `isAccessError` /
  `access_status` 初始化；`types.ts` 删除 `AccessStatus` / `AccessRoot`；五个语言包各删除
  12 条 `settings.access*` 与 `transcribe.accessFix*` 文案。
- 新增 `apps/desktop/src-tauri/src/history_watch.rs`：每 1.5 秒比较 `transcripts/` 目录签名
  （最新 mtime 纳秒、文件数、总字节），变化时 `emit("local-transcribe:history-changed")`；
  首页监听该事件后重读 `list_history`。附带 4 个单元测试。
- 测试：`tests/service_ipc.rs` 把白名单用例改为「任意绝对路径可用、相对路径仍拒绝」，
  删除热更新/迁移/并发加根三个用例；`tests/mcp_config.rs` 改为断言配置不带参数；
  前端 `access-and-updates.test.mjs` 改为验证白名单已消失并验证历史刷新事件接线。

## 文档与文案

`README.md`、`README.zh-CN.md`、`docs/mcp.md`、`PROJECT_STATUS.md` 同步改写。
官网 `design/website/changelog.html`（中英）与 `updates.json` 的 1.7.0 说明改为
「去掉白名单 + 历史自动刷新 + 启动检查更新」。历史日志 `logs/2026-10/`、`logs/2026-11/` 未改动。

## 验证

- `cargo fmt --all -- --check`：通过。
- `cargo test --offline --workspace`：274 passed / 0 failed / 11 ignored。
- `cargo clippy --offline --workspace --all-targets -- -D warnings`：通过。
- `npm run check`：0 errors / 0 warnings；`npm test`：128 passed。
- `npm run bundle:mac`（`CARGO_PROFILE_RELEASE_STRIP=none` + `-lclang_rt.osx`）成功，
  产物 1.7.0 / `net.timhuang.localtranscribe`，`codesign --verify --deep --strict` 通过（ad-hoc）。
- MBP：旧安装移入 `~/.Trash/Local-Transcribe-old-install-20261101-200326/`，新包安装并启动；
  设置页确认不再有「文件访问」分组，MCP 配置为 `"args": []`。
- 热更新实测：应用停留在首页时写入一份合成 transcript JSON，约 1.5 秒内出现在历史列表；
  移走后同样自动消失（未重启应用）。探针文件已移入
  `~/.Trash/Local-Transcribe-test-artifacts-20261101-200408/`。
- Air：旧安装移入 Air 自己的 `~/.Trash/Local-Transcribe-old-install-20261101-200423/`，
  新包安装并启动，版本/标识/五个可执行文件与 MBP 一致。
- 两个入口目录残留扫描：`/Applications` 与 `~/Applications` 只剩新版一个 `.app`。

## 未完成

- Windows 包需要从这批提交后的源码重新构建（本机先前那份 MSI 来自改动前的快照，已过时）。
- GitHub 提交/推送/CI/Release 与官网部署。
- 升级兼容性、带真实模型的 GUI 转写。
- Apple Developer ID 签名、公证、上架（按 Timmy 决定延期）。
