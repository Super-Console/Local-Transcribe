# 把 model-download-quality 分支的模型下载功能合并回当前构建（2026-10-23）

## 背景

Timmy 反馈「模型页面里可以自由下载做好的模型」这一页不见了。查明原因不是代码被删：该功能在另一个 worktree `VoiceToTextApp-mq` 的分支 `model-download-quality` 上（4 个提交：一键下载、进度回调、文档、beta.4 发布），而最近几次安装的构建都来自 `main`，所以应用里没有这一页。分支与 `main` 的合并基就是 `main` 的 tip（2fc8ed2），没有分叉。

## 处理方式

不提交 Git（遵守项目规则），把分支内容并入当前工作区，逐文件处理：

- 直接取分支版本（本工作区未改过）：`model_download.rs`、`ModelView.svelte`、`transcription-core` 的 `asr.rs`/`lib.rs`/`punctuation.rs`/`tests/quality_live.rs`、`sidebar-shell.test.mjs`、`tauri.conf.json`、`src-tauri/Cargo.toml`、`package-lock.json`、`Cargo.lock`、`docs/model-download-and-quality.md`、分支日志。
- 手工合并（本工作区已有未提交改动）：`lib.rs`（注册 `model_download` 模块、三个命令、`read/save_selected_model` 改 `pub(crate)`、转写改成 `transcribe_file_streaming_with_options` 并接入标点模型）、`+page.svelte`（catalog 状态、下载/取消/进度函数、事件监听、ModelView 属性）、`types.ts`（`CatalogModel`、`ModelDownloadEvent`）、五语言各 8 个 `model.*` 文案、`package.json` 版本号。
- 补充：分支本身**没有在挂载或进入模型页时调用 `refreshCatalog()`**，只在下载完成后刷新——照搬会看到空白列表。本工作区补上 `onMount` 与 `navigate("model")` 两处调用。
- 版本统一 `1.1.0-beta.4`（package.json / tauri.conf.json / src-tauri Cargo.toml / Cargo.lock / package-lock）。

## 验证

- `cargo test --workspace --offline`：29 组全 ok，0 failed（真实下载/推理用例按原样 ignored）。
- 严格 Clippy（`-D warnings`）、`cargo fmt --check` 通过。
- `npm test` 17/17（含 beta.4 版本同步用例）、`svelte-check` 0/0、`npm run build` 通过。
- 打包 `1.1.0-beta.4`，26.17 MiB，签名标识 `com.timmy.voicetotextapp`；两台均已换装（备份 `before-model-download-20261023-2020`）。
- 未验证：真实下载（Hugging Face / hf-mirror 回退）与下载后推理质量，留给 Timmy 验收。

## 追加：删除下载区说明文案并换装两台（同日 20:45）

- 按 Timmy 要求删除模型页「以上模型来自官方 Hugging Face 仓库…」这段说明：移除 `ModelView.svelte` 中的引用与五语言的 `model.downloadHint`。
- 验证：`npm test` 17/17、`svelte-check` 0/0 通过；重新打包 `1.1.0-beta.4`，可执行文件 `231fb48c…bc9dbc`，签名标识 `com.timmy.voicetotextapp`。
- 两台换装：先把当前版本（`ecd77e14…2f966`）移入 `Local Transcribe Backups/before-hint-removal-20261023-2045/`，再 `ditto` 新包并核对哈希一致；随后把其余 6 个旧备份目录整体移入 `~/.Trash/local-transcribe-old-backups-<时间戳>/`（仅 `mv`，未用 `rm`，未清空废纸篓）。每台只保留本次这一份回滚点。

## 追加：已下载模型的删除入口（同日 21:15）

- 问题：下载目录里安装完成后只显示「已下载」，没有删除按钮；模型库列表里当前默认模型也不显示删除（后端还会直接拒绝删除默认模型），刚下好的模型往往正好是默认，于是看起来「删不掉」。
- 后端：
  - `model_download::installed_path` / `delete_installed` / `delete_installed_to`：ASR 条目走 `ModelLibrary::delete_to`（移入回收站），标点模型走本模块的 `move_any_to_trash`；未安装时返回「这个模型还没有安装。」
  - 新命令 `delete_catalog_model(id)`：若被删模型正是当前默认，先清空选择再删，避免下次启动报「模型文件已变化」。
  - `delete_library_model` 不再拒绝删除默认模型，改为先清空选择。
  - `read_selected_model` 把空文件视为「没有默认模型」；`clear_selected_model` 以写空文件的方式清除，不删除任何文件。
- 前端：模型页下载列表对已安装条目增加「删除」按钮（带确认框，复用 `dialog.deleteModelMessage`）；模型库列表的删除按钮改为始终显示（含默认模型）。
- TDD：新增 3 个 Rust 用例（已安装 ASR 删除后进入回收站、未安装时报错、标点模型删除）与 1 个前端用例，先红后绿。
- 验证：`cargo test --workspace` 29 组全 ok（含新增 3 项）、严格 Clippy、fmt、`npm test` 18/18、`svelte-check` 0/0 通过；打包 `1.1.0-beta.4`，可执行文件 `72d39998…377af`，两台均已换装（备份 `before-model-delete-20261023-2115`）。
