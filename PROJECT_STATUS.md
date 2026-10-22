# Project Status

## Current milestone

**Milestone 3 — shared task/result core and local MCP control plane.** The MCP executable is built and its stdio/control-plane behavior is tested on Apple Silicon. Real model inference remains unverified; this is not yet an end-to-end transcription product.

## Provisional UI refresh (2026-10-22)

- The desktop page now uses a single-column transcript workspace with compact top navigation; model selection and recent history remain available as secondary sections. The screen remains labeled `界面原型`; this is not approval of the final product name or visual design. No transcription behavior or backend contract was changed.
- Verified after the refresh: `npm run check` (0 errors, 0 warnings) and `npm run build` passed. The Vite preview was inspected at 1280×900 and at the configured 680×740 minimum window width; neither showed horizontal overflow. Representative light/dark text and action color pairs passed WCAG AA contrast checks.
- Not verified: the running Tauri window after reload. The Pro screen was locked during inspection, so CUA could capture its previous screen but could not read or operate the app. Browser preview cannot validate Tauri command integration. Real-model transcription remains unverified as stated above.

## Verified on Pro (2026-10-22)

- `cargo fmt --all -- --check` — passed.
- `cargo test --workspace` — 26 passed, 0 failed; one model-dependent local-recording test was intentionally ignored because no model weights/test recording are installed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo build -p voice-to-text-mcp` and `cargo build -p voice-to-text-desktop` — passed in debug mode; this is not a release bundle/signing check.
- `npm run check` and `npm run build` in `apps/desktop` — passed after adding `/opt/homebrew/bin` to the non-interactive SSH `PATH`.
- MCP process smoke test: initialize over stdio using protocol `2025-03-26`, `tools/list` returned exactly `cancel_job`, `get_job_result`, `get_job_status`, `list_models`, and `transcribe_file`; real `tools/call` requests exercised model listing and file-access/error behavior.
- File access: no `--allow-root` denies transcription; a canonical path outside an authorized root is rejected; a valid audio file inside the root reaches the clear “no model is selected” guard. No transcription job or fake transcript was created. Unit tests cover a symlink escape.
- Cross-process task test: a child process held a lease; the parent observed the active job, could not start a second task, and recovered the task as interrupted after the child exited.
- `apps/desktop/src/routes/+page.svelte` no longer contains the Chinese heading `模型信息`; the model-selection control remains.
- The official `sherpa-onnx` 1.13.8 macOS arm64 static-library archive was downloaded to scratch for dependency linking (20,965,936 bytes; local SHA-256 recorded in `docs/technical-validation.md`). This is not a model-weight download.

## MBA test installation (2026-10-22)

- Built the current working tree on Apple Silicon with `npm run tauri -- build --bundles app --no-sign`; Tauri produced `target/release/bundle/macos/VoiceToTextApp.app` (22.62 MiB). The build skipped signing; strict `codesign --verify --deep --strict` failed with `code has no resources but signature indicates they must be present`.
- Installed to the verified MacBook Air M1 (8 GB, macOS 27.0.1) at `~/Applications/VoiceToTextApp.app`; this target was absent before installation. Verified the executable exists and `open` resulted in a running `voice-to-text-desktop` process.
- This is a local test installation, not a signed/notarized release. GUI interaction and real-model transcription were not verified; no model weights are installed. The repository has no commits or remote, so this build represents the current working tree, not a confirmed upstream latest release.

## Windows validation attempt (2026-10-22T05:19:14Z)

- Toolchain install completed on `REDACTED-HOST` / `REDACTED-HOST\REDACTED-USER` (Windows 11 x64 build 26300): Visual Studio Build Tools 2022 17.14.42 (`isComplete=true`), MSVC VC Tools 14.44.35207 / compiler 19.44.35229.0, and Windows SDK 10.0.26100.0. Verified `Windows.h`, x64 `Kernel32.lib`, and `rc.exe`; WebView2 was already present.
- Installed/verified rustup 1.29.1 with stable `x86_64-pc-windows-msvc` (rustc/cargo 1.99.1) and Node.js 24.20.0/npm 11.19.0. Git was not installed because it is optional for this build. npm installed 61 desktop dependencies; the source `package-lock.json` SHA-256 matched Mac exactly.
- Passed: Windows `npm run check` (0 errors, 0 warnings), `npm run build`, and `cargo fmt --all -- --check`.
- Blocked: `cargo test --workspace` exited 101 before any tests ran. Code Integrity events 3033 and 3077 explicitly report that Cargo's generated `serde` build-script executable did not meet Enterprise signing-level requirements. DeviceGuard status values were read as `2`; no broader policy ownership/root cause was inferred. No exception or policy change was attempted.
- Not run: Windows Clippy, Rust release build, Tauri installer/bundle, runtime/UI acceptance, and real ASR. No model weights were present or downloaded. The first verification wrapper that printed Cargo help was invalid and is not counted as a pass; the corrected `cargo fmt` and actual failing test invocation are recorded separately.
- WinINet proxy returned to `ProxyEnable=0`, `ProxyServer=127.0.0.1:7897`; `ProxyOverride` was not changed. Reverse tunnel port 17890 has no listener. Scoped temp/WinGet search found no one-time installer package to move to Recycle Bin; shared npm/Visual Studio caches and partial Cargo target artifacts were retained.
- Shutdown was requested normally (no force-close) at `2026-10-22T05:17:02Z`. Independent post-shutdown evidence: SSH to `REDACTED-TAILSCALE-IP:22` timed out; Tailscale peer `tims-gaming-pc.tail1f3ba1.ts.net.` reported `Online=False` at `2026-10-22T05:20:00.1Z`.
- Detailed Windows-side record: `C:\\Users\\REDACTED-USER\\VoiceToTextApp\\windows-build.log`; Cargo failure output: `C:\\Users\\REDACTED-USER\\VoiceToTextApp\\cargo-test-workspace.log`. No successful Windows Rust build is claimed.

## Implemented, but not end-to-end validated

- The core contains model-folder inspection for Paraformer/SenseVoice, media decoding, a CPU `sherpa-onnx` inference path, chunk progress and cooperative cancellation.
- Tauri model selection and MCP use the same app-data location, selected-model record and file-backed task/result store. Cross-process locking/recovery is unit-tested, but an actual GUI-started model task was not exercised through MCP.
- `get_job_result` defaults to a bounded preview; full transcript delivery is opt-in.

## Current decisions

- Tauri 2 + Svelte remains the accepted desktop route; macOS first. Product name and final UI design remain open.
- MCP uses the official Rust SDK `rmcp = 3.6.0`, stdio only, with exactly five tools.
- MCP file access requires explicit repeated `--allow-root` paths; no root means deny. There is no arbitrary output path or shell tool.
- Task/result persistence currently uses JSON files and cross-process `fs2` locks; SQLite is deferred, not a user-approved permanent architecture decision.
- The original `GPT聊天记录/` material remains unchanged and Git-ignored. No Git commit or remote was created.

## Blockers / not verified

- No model weights were downloaded. Real audio/video inference, compatible model export, quality, chunk timestamps, cancellation during a real run, performance, offline run and model licensing are **not verified**. The ignored test `transcribes_a_local_recording_without_changing_it` needs an approved model and a short local recording.
- CLI is not implemented. Hermes or another MCP client has not been configured. A stdio command runs on the client's host; the process must run on the same Mac as the desktop app/media to share the default app-data path.
- Windows 11 x64 validation was started on `REDACTED-HOST` / user `REDACTED-USER` after explicit install authorization. Visual Studio Build Tools 17.14.42, MSVC 14.44.35207, Windows SDK 10.0.26100.0, Rust 1.99.1 MSVC target, and Node.js 24.20.0/npm 11.19.0 are installed and verified; WebView2 was pre-existing. Windows frontend `npm run check` (0 errors/warnings) and `npm run build`, plus `cargo fmt --all -- --check`, passed. The actual `cargo test --workspace` stopped before tests ran: Code Integrity events 3033/3077 blocked Cargo-generated `serde` build-script executable for not meeting Enterprise signing-level requirements (policy ID `0283ac0f-fff1-49ae-ada1-8a933130cad6`). No policy was modified or bypassed. Windows Clippy, release Rust builds, Tauri packaging/installer, and runtime/UI acceptance remain unverified. The temporary proxy was restored to WinINet `ProxyEnable=0`, `ProxyServer=127.0.0.1:7897`; the reverse tunnel was stopped. Normal shutdown was requested at `2026-10-22T05:17:02Z`; afterward SSH timed out and Tailscale reported the Windows peer `Online=False` at `2026-10-22T05:20:00.1Z`. See `docs/windows-build-log.md` for the installation and command details.
- A Tauri `.app` bundle/signing check was not rerun. Earlier strict `codesign --verify --deep --strict` failed with `code has no resources but signature indicates they must be present`; packaging/signing remains unresolved.
- Release build, installer, notarization, and real-device UX acceptance remain outstanding.

## Next gates

1. Before any model-weight download, Timmy must approve the exact model/source, license, size and verification plan.
2. With an approved model and lawful short sample, run a real Mac offline transcription and verify text/result/export/source-file immutability.
3. Decide whether/where to configure an MCP client, accounting for the Mac-versus-gateway host boundary.
4. Resolve the Windows Code Integrity block only through an administrator-approved signing/build path or another approved Windows builder; do not disable or bypass the active policy. Then run Windows Rust tests, Tauri packaging, and runtime/UI acceptance as separate checks.


## Windows Smart App Control root cause (2026-10-22T05:27:49Z)

- Read-only `CiTool -lp -json` reports the enforced, signed system policy `VerifiedAndReputableDesktop` (`0283ac0f-fff1-49ae-ada1-8a933130cad6`); the registry reports `VerifiedAndReputablePolicyState=1`. No policy values were changed.
- Correlated Code Integrity events 3033/3077 at `2026-10-22T05:12:25Z` identify Cargo launching `target\debug\build\serde-e329f3b96886e2fc\build-script-build.exe`, rejected by that policy. This matches Microsoft's official Rust-on-Windows setup note: SAC can block the unsigned Rust toolchain and build-script outputs; Developer Mode and locally trusted certificates are not exemptions. Microsoft Support says SAC has no per-app bypass.
- The latest installer command did not run installers: its guard correctly stopped at the already-existing `C:\Users\REDACTED-USER\VoiceToTextApp` project directory. A live verification confirms Git is now installed (`2.55.0.windows.5`); Build Tools/MSVC/SDK remain installed.
- No Windows Cargo test/build was rerun, and SAC was not disabled or bypassed. A compliant next route is another approved Windows build host; external CI/source upload needs explicit approval. If binaries must execute on SAC-protected Windows, a trusted public code-signing path may be needed for all relevant distributable binaries; it does not remove the build-script restriction on this machine.
- Sources: Microsoft Rust setup <https://learn.microsoft.com/en-us/windows/dev-environment/rust/setup>; Smart App Control FAQ <https://support.microsoft.com/en-us/windows/security/threat-malware-protection/smart-app-control-frequently-asked-questions>; signing guidance <https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control>.
- Windows-side readback: `C:\\Users\\REDACTED-USER\\VoiceToTextApp\\windows-build.log`.


## Latest Windows policy check and shutdown (2026-10-22T05:29:19Z)

- Confirmed Smart App Control enforcement and the exact Cargo build-script block using `CiTool`, registry state and Code Integrity events 3033/3077. The system policy stayed enabled and unchanged.
- The new installer command exited before installs because its guard found the existing VoiceToTextApp folder. The live check showed Git `2.55.0.windows.5` and the MSVC Build Tools already present.
- Verified WinINet proxy is off (`ProxyEnable=0`; original server retained), reverse tunnel listener count is zero, and no Cargo/winget/VS installer process remained.
- HA `script.pc_shutdown` was triggered and its `last_triggered` changed. After 35 seconds SSH timed out and Tailscale showed the Windows peer `Online=False` (`2026-10-22T05:30:00.1Z`), verifying shutdown.
- No Cargo tests or build were rerun; policy-compliant next step requires another approved Windows builder. External source upload was not performed.


## Late Windows validation process output correction (2026-10-22T05:30:37Z)

- The delayed output from the old PowerShell wrapper is now available. All four claimed steps (`cargo fmt`, `cargo test`, `cargo clippy`, `cargo build -p voice-to-text-mcp --release`) printed Cargo's generic usage/help text, yet were marked exit 0 by that wrapper. These are **not valid command executions or passes**; no formatting, tests, Clippy run, or release build can be inferred from this process.
- Earlier valid frontend checks (`npm run check`, `npm run build`) and the separate `cargo fmt --all -- --check` result remain distinct; the real workspace test previously exited 101 because SAC blocked the generated `serde` build script.
- The delayed process has therefore not changed the Windows build outcome. Current shutdown was independently rechecked: SSH times out and Tailscale reports `Online=False`. Windows is offline, so its `windows-native-validation.log` cannot be corrected or read back now; when next online, treat that file's four help-text sections as invalid and append this correction there.


## Windows Cargo error detail read from delayed process output (2026-10-22T05:31:49Z)

- A delayed full transcript of `cargo test --workspace` confirms Cargo compiled several dependencies and then failed at `serde v1.0.229` before any tests ran. Exit code was 101.
- Exact failure: Windows returned OS error 4551, `应用程序控制策略已阻止此文件` (“Application Control policy has blocked this file”) while Cargo tried to execute `target\debug\build\serde-e329f3b96886e2fc\build-script-build.exe`. This is direct evidence of the previously identified Code Integrity/Smart App Control block, not a crates.io download or source compilation failure.
- The same delayed transcript verifies `cargo fmt --all -- --check` exited 0. It does not show a successful test suite, Clippy run, or release build; those commands did not run in this process.
- Windows remains offline after the prior verified shutdown (SSH times out; Tailscale reports `Online=False`), so no remote log append/readback could be performed. The Windows-side `cargo-test-workspace.log` is preserved there; detailed transcript is captured in the session history.


## Separate delayed Cargo retry: crates.io index failure (2026-10-22T05:34:22Z)

- A later delayed Windows process transcript shows a separate command sequence: `cargo fmt --all -- --check` exited 0; the subsequent real `cargo test --workspace` stopped while `Updating crates.io index` and the SSH process exited 1. This attempt did **not** reach compilation or test execution. The transcript supplied no detailed Cargo network error, so the precise transport cause is unknown.
- This index-update failure is distinct from the earlier, fully captured `cargo test --workspace` exit 101, where Cargo compiled dependencies and Windows Code Integrity explicitly blocked the generated `serde` build script with OS error 4551. It neither replaces nor disproves that earlier confirmed SAC failure.
- The delayed process exited nonzero before Clippy or release build; no test/Clippy/release-build pass is established. Windows remains offline (Tailscale `Online=False`, SSH timeout), so the Windows-side `windows-native-validation-effective.log` cannot be read back or verified.


## Latest Windows retry status (2026-10-22T07:10:16Z)

Windows frontend check/build, Cargo formatting and workspace metadata all pass on `REDACTED-HOST`. The proxy successfully restored crates.io access. The actual Rust workspace test is still blocked before test execution: enforced Smart App Control rejected `parking_lot_core` and `crc32fast` build scripts with OS error 4551. No SAC change/bypass, external source upload, Rust release build, Tauri installer, or model inference occurred. The temporary tunnel was stopped, WinINet remained unchanged, and Windows is now shut down (SSH timeout plus Tailscale `Online=False`). Details and exact local log paths are in `docs/windows-build-log.md`; Windows remains not build-complete.


## Windows build verified (2026-10-22T09:24:38Z)

Windows CI is green end to end on `windows-2022`: Rust tests, strict Clippy, release build, frontend check/build, and the Tauri MSI all pass (runs 39749144534 and 39752222060). The installer and `voice-to-text-desktop.exe` were downloaded to `/Users/REDACTED/Downloads/VoiceToTextApp-Windows/` with SHA-256 matching the CI log. The Windows lock-contention bug (`ERROR_LOCK_VIOLATION` 33) was fixed. Real ASR inference remains unvalidated (no model weights); the installer is unsigned.

## UI 视觉重设计基线 V2.0（2026-10-22）

- 依据 `taskbook/Local-Transcribe-UI-V2.0-设计任务书.md` 产出 9 组 / 21 屏完整窗口设计，位于 `design/ui-v2/`；`index.html` 为可切换总览，`_build-designs.py` 为生成脚本（共享 token 与组件，不参与应用构建）。
- 设计基线：浅色 `#FBFBFD` 工作区 / `#F1F1F4` 侧栏 / 正文 `#1D1C1F` / 强调 `#0A7AFF`（仅选中、焦点、拖拽）/ 绿色仅表示模型已加载；深色 `#1C1C1E` / `#232325`。标题 15/600，正文 15/1.7，辅助 12.5，控件 30–32px，圆角 7–12px，阅读列宽 720px，侧栏 196px。
- 覆盖：转写空闲/进行中/完成、队列浮层、历史列表与详情、模型管理与详情、设置五组、深色模式，以及 11 个异常/确认状态（无模型、等待模型、导入失败、失败与部分结果、取消、关闭窗口确认、恢复队列、搜索无结果、窄窗折叠、目录不可用、查看最新）。
- 未改动应用代码与任何产品行为；当前 `apps/desktop` 仍是旧临时界面。下一步是把该基线实现到 Tauri + Svelte（V1.0 的 P3），届时需同时验证窄窗折叠（780px 滞回）、主题跟随、键盘焦点与阅读位置恢复。

## P3 第一步：窗口外壳与转写增量显示（2026-10-22）

- 前端按 `design/ui-v2` 基线重写：`apps/desktop/src/app.css`（浅深色 token）、`lib/components/`（Sidebar、Toolbar、TranscribeView、Reader、HistoryView、ModelView、SettingsView、Icon）、`routes/+page.svelte` 编排。`npm run check` 0 errors/0 warnings，`npm run build` 通过。
- 窗口外壳：侧栏 196px；折叠滞回（< 780 折叠，> 820 恢复，中间保持原状态），用户手动折叠独立记忆（`lt.sidebar`），窄窗下用浮层侧栏恢复导航；⌘\ 切换、Esc 关闭浮层；`:focus-visible` 统一焦点环，拖放区可用回车/空格触发。默认窗口改为 900×620、最小 640×460，产品名与窗口标题改为 Local Transcribe（identifier 未变）。
- 主题：跟随系统 / 浅色 / 深色，持久化于 `lt.theme`，`prefers-color-scheme` 变化时实时生效；设置页提供切换控件。已用浏览器预览在 900×620、760×560、深色三种情形截图核对（侧栏折叠、深色 token、工具条 inset 均正确）。
- 增量转写：核心新增 `transcribe_file_streaming`（每个稳定片段立即回调，原有 API 保持不变），Tauri 新增 `voice-to-text:transcript-segment` 事件与 `cancel_transcription` 命令。真实模型验证：`live_asr.rs` 新增的 ignored 测试用 paraformer-zh 与 74 秒样本跑出 3 段，顺序与最终结果完全一致（cargo 测试通过）。
- 阅读位置：Reader 按 jobId 记住滚动位置与自动跟随状态（`lt.reader.<jobId>`），滚离底部后显示「查看最新」，减少动态效果时改为瞬时滚动。
- 未验证 / 未完成：未在真实 Tauri 窗口里跑过一次完整转写（浏览器预览无后端）；标题栏融合（titleBarStyle Overlay）尚未启用，仍为原生标题栏；队列浮层、历史搜索与筛选、模型导入复制、部分结果持久化与导出仍未接入。
- 开发期辅助：`?theme=dark|light` 与 `?view=settings` 仅在 dev 服务器生效，用于设计走查，不影响构建产物。

## P0 补齐 + P2 模型库（2026-10-22）

- Whisper 词表识别修正：csukuangfj 发布的 `*-tokens.txt` 是 base64 词表（如 `IQ== 0`），旧逻辑按明文标记判断会把 Whisper 全部拒绝。现按「Whisper 标记或 ≥ 40000 条词表」识别，并新增 3 个布局单元测试（共 8 项通过）。
- 真实模型实测（8 秒样本，debug 构建，CPU）：Whisper Turbo 英文精确、RTF 0.95；同一模型中文较差（「今天天气,我们下午三点在口见面,请记得上。」）；Whisper Large V3 英文精确、RTF 2.56（CPU 明显偏慢）。至此 P0 兼容矩阵实测为：Paraformer ✓、Qwen3-ASR 0.6B ✓、Whisper Turbo ✓、Whisper Large V3 ✓、SenseVoice（该导出）✗。
- 模型库 `crates/transcription-core/src/library.rs`：导入=校验布局→空间预检→复制到临时目录→复制后再校验→原子改名；源目录只读；重复导入拒绝；删除移入废纸篓（`delete_to` 可注入回收站目录以便测试）。新增 5 项测试通过。
- Tauri 新命令：`library_directory`、`library_models`、`import_model`、`select_library_model`、`delete_library_model`；模型页改为「导入模型 / 设为默认 / 删除（确认对话框）」，工具条模型选择器改为跳转模型页。
- 已重新打包并安装到 `~/Applications/Local Transcribe.app`（旧版本移入废纸篓，未删除用户下载与数据）。
- 仍未完成：P1 SQLite/队列/120 秒卸载/恢复批准；队列浮层、历史搜索筛选、设置其余四组、部分结果导出；P4 CLI 与 MCP 服务化；P5 双端打包与 Windows 验收。

## P4：CLI 与 MCP 改为统一服务客户端（2026-10-22）

- 新增 `crates/transcription-cli`（`local-transcribe`）：status、models list/import/info、jobs add/list/show/cancel/retry/reorder/remove、export；统一 JSON 输出，退出码 0/1/2/4；服务按需拉起，单实例锁防双开。
- 服务协议扩展：`models`、`import_model`、`select_model`、`jobs`、`retry`、`remove`、`reorder`、`export`、`job_result`；`Submit` 支持 `model_id`（优先）与 `model_dir`，并记录 `owner`（gui/cli/mcp）。响应字段统一 camelCase。
- MCP 重写为服务客户端（stdio）：`list_models`、`import_model`、`transcribe_file`、`get_job_status`、`list_jobs`、`get_job_result`、`cancel_job`、`retry_job`、`export_result`；不再在 MCP 进程内加载模型；路径仍按 `--allow-root` fail-closed。
- 实测（真实 Qwen3-ASR 0.6B，74 秒样本）：CLI 提交 → 3 段完成 → `export` 写出正确中文；MCP `initialize` 协商 2025-03-26，`tools/list` 返回 9 个工具，`list_models`/`list_jobs`/`get_job_result`/`export_result` 均返回真实数据。
- 测试：工作区 34 项通过、4 项真实模型测试 ignored；`cargo clippy -D warnings` 通过。
- 仍未完成：队列与任务未落 SQLite（服务重启丢失等待队列与重试记忆）、GUI 仍在本进程推理、120 秒空闲卸载、恢复批准、P5 打包与 Windows 验收。

## FireRedASR2-AED 与 Paraformer-zh 真实测试 + FireRedASR 族支持（2026-10-22）

- 需求：Timmy 指定下载 `FireRedTeam/FireRedASR2-AED` 与 `funasr/paraformer-zh` 并做真实测试。两者 HF 仓库是 PyTorch 原始格式，sherpa-onnx 无法直接执行；采用 sherpa-onnx 官方从这两个仓库本体转换的 int8 预转换包（`sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26`、`sherpa-onnx-paraformer-zh-2023-09-14`），即同一批模型。
- 兼容性：锁定的 sherpa-onnx Rust 绑定 1.13.8 已含 `OfflineFireRedAsrModelConfig`，引擎无需升级。App 的 `detect_layout` 原会把 FireRed 布局误当残缺 Whisper 拒绝（字符级词表无 Whisper marker），已修。
- 代码（TDD）：`ModelFamily::FireRedAsr` + `ModelFiles::FireRedAsr { encoder, decoder, tokens }`；`detect_layout` 在 encoder+decoder 分支先按目录名（含 `fire-red`/`firered`）识别 FireRedAsr，再做 Whisper 词表检查；新增推理配置分支；language 与 SenseVoice/Whisper/Qwen3 同列 `None`（不编造）；三处 family_label 加 `FireRedASR`；`SUPPORTED_LAYOUTS` 文案更新。目标测试 9/9、工作区全绿。
- 真实转写（共享服务 + 真实人声）：FireRedASR2 中英混读与 sherpa-onnx 官方基准输出逐字一致，四川话、河南话均正确；Paraformer-zh 中文与中英混合均正确。JSON 导出含 `engine: sherpa-onnx/1.13.8`、`durationMs`、`segments[]`。GUI 端到端：模型页正确显示 FireRedASR · 1.15 GB / Paraformer · 234 MB，历史页完整显示全部真实结果。
- 状态：模型库现有 3 个模型共 2.31 GB（应用内副本），默认模型已恢复 Qwen3-ASR。P0 兼容矩阵新增：FireRedASR2-AED ✓、Paraformer-zh（2023-09-14 包）✓。
- 已知限制：GUI 文件对话框（NSOpenPanel sheet）CUA 后台模式无法操作（AX 面向解析失败），GUI 提交路径未用真实鼠标走通；`jobs show` 不含正文，需 `export`（既有行为）。

## 窗口外壳改版：原生红绿灯 + 平铺侧栏 + compact 模式（2026-10-22）

- 动因：浮动侧栏卡片设计需要 objc 把红绿灯搬进卡片，引发一串问题（缩放抖动、off-main-thread 崩溃）；且 Overlay 标题栏下 `startDragging()` 因 currentEvent 时序永远失效，窗口拖不动（已用 HID 真实注入复现）。
- 决策（Timmy 确认）：红绿灯交还 macOS 原生管理；侧栏从浮动卡片改为平铺通高列；折叠改为 compact（52px 图标列 + 悬停提示 + 记住状态），不再完全隐藏；窄窗口不再浮层盖内容。
- 配置：`titleBarStyle: Overlay → Transparent`，`hiddenTitle` 保留；删除 `transparent` 与 `windowEffects`。
- Rust 侧净删：`place_traffic_lights`、`schedule_place_traffic_lights`（代际防抖）、`enable_window_background_drag`、`debug_drag_probe`、Focused/Resized 挂钩、setup 整块；Cargo 移除 `objc2-foundation`。新增 `set_native_window_background`（sync_window_theme 内，主线程执行）：透明标题栏条底色随主题对齐页面 `--bg`（浅 #fbfbfd / 深 #141416），消除接缝色差。
- 前端：`+page.svelte` 删 `.side-shell`/`.scrim`/`.overlay`/窄窗滞回折叠；侧栏 in-flow（196px ⇄ 52px，`lt.sidebar.collapsed.v1` 持久化，⌘\\ 快捷键保留）；工具栏删拖动区域与 inset 让位逻辑，侧栏开关移到侧栏底部。Icon 新增 `chevronRight`。
- 验证：svelte-check 0/0；release 构建零警告；CUA HID 真实拖动原生标题栏条窗口移动 ✓；折叠/展开/重启后保持 compact ✓；深色主题标题栏条无接缝 ✓；23:08 构建后无新崩溃报告。
- 待 Timmy 真实手感确认：拖动、悬停 tooltip、⌘\\ 折叠。

## 界面多语言适配首轮（中/英/西/法/德）（2026-10-22）

- 方案：手写零依赖 i18n（未引入 svelte-i18n，避免 Svelte 5/Vite 8 兼容风险与运行时体积）。`src/lib/i18n/messages/` 五个目录，zh 为源（99 个 key），其余语言类型 `Record<MessageKey, Message>`，**key 对齐由 tsc 编译期强制**；复数走 `Intl.PluralRules`，日期时间走 `Intl.DateTimeFormat`（替换历史页硬编码 zh-CN）。状态层 `i18n.svelte.ts`（runes + `lt.locale.v1` 持久化，默认跟随系统，系统语言按启动时 `navigator.languages` 解析，范围外语回退英语）。
- 范围：前端 GUI 全部 9 个视图/组件；品牌名、技术标识（MCP/CLI/pid/--allow-root 等）、GB/MB 单位、转写正文有意不翻译。设置页新增「界面语言」原生下拉（语言名按母语显示）。
- 验证：svelte-check 0/0；残留中文扫描仅剩注释；release 零警告；CUA 实测 zh（默认跟随系统）/en/de/es 切换渲染、ARIA 标签同步、复数（3 models/3 Modelle/3 modelos）正确、德语重启后持久化 ✓、外壳拖动复验 ✓、无新崩溃报告；验收后已恢复「跟随系统」。
- 未覆盖（第二轮范围）：Rust 后端错误文案、CLI/MCP 输出、`job.phase` 后端值。法语运行时视觉抽查未做（类型对齐已保证）。

## 侧栏折叠改为左上圆形按钮 + 完全隐藏（2026-10-23）

- Timmy 对照 TranscribeX 截图要求照抄折叠设计。新形态：28px 圆形图标按钮（新 `sidebar` 图标）固定窗口左上（原生 strip 正下方），展开时浮于侧栏列顶部（品牌名下移一行），折叠时侧栏完全隐藏（0 宽 + inert），内容全宽，工具栏标题自动让位；删 52px compact 列与底部「折叠」文字按钮。i18n 文案改为「显示/隐藏侧栏」五语言同步。
- 与 TranscribeX 唯一差异：其按钮嵌于更高的原生标题栏行；我们的 28px strip 为原生层 HTML 不可渲染，按钮位于其下一行。
- 实测：svelte-check 0/0，展开/折叠/恢复两跳 CUA 截图验证 ✓。因 Timmy 有进行中转写任务未重启，折叠状态跨重启持久化与深色主题外观待空闲补验（持久化逻辑与样式 tokens 未变，风险低）。

## 设计照抄 TranscribeX + 首页合并 + 速度/ETA + 存储策略（2026-10-23）

- 视觉语言按 Timmy 提供的 TranscribeX 参考图重做：白底内容 + 浅灰侧栏、衬线大标题、12px 圆角卡片、深色药丸主按钮、分组小标题、发丝分隔；历史改为日期分组卡片网格；模型/设置/转写引导同步改版；浅色/深色两套实测。
- 导航重构（Timmy 方案）：`View` 改为 `home | model | settings`；**首页合并转写与历史**（标题 + 导入磁贴 + 最近记录卡片，运行中/打开记录时显示任务卡片与正文）；模型与设置收进侧栏底部「管理」组。
- 任务状态新增 `startedAtMs` / `totalDurationMs`：界面实时显示倍速与剩余时间（实测 13.8× / 剩余约 1 分钟），进度条 indeterminate 改为滑动动画。
- 修复共享队列可见性：运行中每秒轮询、空闲每 2 秒轮询活动任务，CLI/MCP 任务自动出现在 GUI 并实时更新；限制：外部任务运行中无文件名与逐段正文。
- 存储：设置页展示模型库/历史结果/任务记录/应用数据四个目录；新增自动清理（关闭/按时间 1-3-5-7-30 天/按容量 1-5-10 GB），删除走废纸篓，默认关闭；已实测策略落盘并恢复关闭。当前版本无切片临时文件（内存分段）。
- 版本 0.2.0（关于页运行时读取）；GitHub 仓库已设为 private。

## 模型驻留缓存 + 顶栏下拉绿点 + 字号放大（2026-10-23）

- 按任务书落地模型驻留：进程内识别器缓存（同模型复用、切换模型在任务边界替换、空闲 120 秒卸载），导出 `loaded_model_id()`；Tauri 命令 `model_runtime`；新增 ignored 集成测试用真实 paraformer 验证任务后仍驻留（通过）。
- 顶栏「默认模型」改为下拉菜单：模型列表（名称/家族/体积）、当前默认打勾、**绿点=真正驻留内存（灰点=未加载）**、底部管理入口；前端 2 秒轮询运行时状态。限制：绿点仅反映 GUI 进程，CLI/MCP 任务在服务进程加载不显示。
- 字号整体放大：基准 13→14.5px，标题 30→34px（引导 36px），侧栏 14.5px，卡片/设置/正文同步上调，贴近参考图比例。

## 浅色主题改为 Anthropic 牛皮纸风（2026-10-23）

- 依据 Claude 品牌色（Clay #C15F3C / Ivory #FAF9F5 / Pampas #F4F3EE / Slate #141413）重做浅色 token：奶白内容底、牛皮纸色侧栏、陶土强调与选中、暖灰次级文字与阴影；原生标题栏条同步 #FAF9F5 消除接缝。深色主题不变。截图验证首页与设置页。

## 首页全文搜索 + 阅读器内搜索（2026-10-23）

- 新命令 `search_history`：文件名与转写正文大小写不敏感匹配，正文命中返回上下文片段（字符级截取，UTF-8 安全），附 2 个单元测试。
- 顶栏搜索框（220ms 防抖）+ 首页结果卡片（命中片段高亮底色、空/加载状态）；阅读器本稿搜索（过滤、高亮、计数）；历史详情改用 Reader 组件（`initialFollow=false`）以获得同样的稿内搜索。
- i18n 五语言补齐；README/README.zh-CN 增加 GUI 0.2.0 章节；实测：文件名 6 条、正文 7 条、稿内 9 处匹配。

## 关闭侧栏折叠（实现保留可恢复）（2026-10-23）

- 产品决定不再折叠侧栏。以双开关关闭入口：Rust `SIDEBAR_COLLAPSE_ENABLED=false`（不安装原生标题栏按钮、提示语命令 no-op）、前端同名常量（⌘\ 与持久化读取不生效）。`titlebar_toggle` 模块、折叠样式与 i18n 文案全部保留，两处常量改回 true 即可完整恢复。

## 本地录音（麦克风 + 会议应用双音轨）（1.0.0，2026-10-23）

- 产品范围更新：允许在应用内录音后再转写（仍为本地、不上传、无实时听写）；版本进入 1.0.0。
- 实现：cpal 麦克风采集 + Core Audio Process Tap 应用音频捕获，两条音轨各自 WAV、分别转写、各自成记录；异步 stop 避免阻塞 UI；Info.plist 增加麦克风与系统音频权限说明。
- 验证：麦克风录音实测落盘；**应用音频捕获与停止后自动入队转写尚未端到端验证**（需首次授予系统音频权限后复验）。

- 修订：录音改为**自由音轨列表**（可添加任意条输入设备/应用轨），不再固定「麦克风 + 会议应用」两条；每条独立录制、独立转写、独立成记录。
