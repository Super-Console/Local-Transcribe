# Project Status

## Current milestone

**Current build — 1.3.0-beta.1 (GitHub Pre-release, tag `1.3.0-beta.1` = commit `dea1c4f`, CI run `40066611735` all green). Product name: Local Transcribe (decided 2026-10-24); repository: `TimmySheep/Local-Transcribe`.** GUI, CLI and MCP are service clients over the shared core; the LAN node runs as a supervised `local-transcribe-node` child process. Verified facts: model downloads, deletion and local transcription on Apple Silicon; Windows x64 MSI built by CI, installed on real hardware (Tims-Gaming-PC, SAC off), where runtime window enumeration confirmed no native menu bar, the app-spawned sidecar owned port 7878 with sharing enabled, and `cargo fmt/test/clippy` passed without Code Integrity interception; both Macs replaced with the release asset (identical SHA-256, each `mdfind` shows only the install path); two-Mac LAN end-to-end (Air → MBP node) passed with the release build. Remaining for Timmy: visual checks on Windows (tray white icon, glass, fonts, first-screen clipping); Windows inbound firewall for port 7878 not opened, so other machines cannot reach the Windows node yet. Signing unchanged: macOS ad-hoc/notarization none; Windows unsigned — machines with Smart App Control forced ON can install but not run it.

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
- MCP uses the official Rust SDK `rmcp = 3.6.0`, stdio only, with exactly nine tools (`import_model` removed; `list_nodes` added).
- MCP file access requires explicit repeated `--allow-root` paths; no root means deny. There is no arbitrary output path or shell tool.
- Task/result persistence currently uses JSON files and cross-process `fs2` locks; SQLite is deferred, not a user-approved permanent architecture decision.
- The original `GPT聊天记录/` material remains unchanged and Git-ignored. No Git commit or remote was created.

## Blockers / not verified

- No model weights were downloaded. Real audio/video inference, compatible model export, quality, chunk timestamps, cancellation during a real run, performance, offline run and model licensing are **not verified**. The ignored test `transcribes_a_local_recording_without_changing_it` needs an approved model and a short local recording.
- The CLI is implemented as a service client (`local-transcribe status | models | jobs | export`); no MCP client has been configured. A stdio command runs on the client's host; the process must run on the same Mac as the desktop app/media to share the default app-data path.
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

## Windows Early Beta + 局域网 Compute V1.1（2026-10-23，无人值守任务书）

- Phase 1 Windows 构建链打通：service 跨平台 IPC（macOS/Linux Unix socket 不变，Windows 仅回环 TCP + `service.port`，fs2 单实例锁与协议不变）；desktop macOS-only 依赖门控；测试/断言按平台门控；clippy `-D warnings` 双目标清零。**CI（windows-2022 真实 runner）完整管线首次全绿**：fmt/test/clippy/release/npm/MSI/产物暂存（25 分钟）。此前 8 连红的首败点是 recording.rs 未过 rustfmt，Windows 编译从未被真正验证过。
- Phase 2/3 局域网 Compute V1.1 核心：新 crate `crates/lan-compute`（`voice-to-text-lan`）——九态任务状态机、任务持久化/崩溃恢复、axum HTTP API（分块上传+断点续传、令牌认证恒定时间比较、收集导出防路径穿越、ZIP+sha256 清单）、mDNS 注册/发现（`_localtranscribe._tcp.local.`）、`local-transcribe-node` 程序（令牌自动生成 0600）。23 测试全绿，双平台 clippy 零警告。
- Windows Early Beta 冻结灰度设计稿：`design/windows-early-beta.html`（§0-§9：色板/转写运行状态/文字稿与 Result 控件/模型导入/局域网 Compute 与 Node 设置/节点配对与任务状态/输入输出与观察文件夹/全局拖拽/构建与测试版本信息/帮助文档入口；文案、尺寸、状态、交互、转场逐项标注）。
- 许可证：MIT（LICENSE）。README 双语更新（架构/局域网/构建/验证状态/许可证）。
- 真机档案：目标 Windows 11 Pro（build 26300，x64，AMD RX 6600、无 NVIDIA）、WebView2 155 已装、SSH 管理员权限可用；HA `pc_wake` 实体交叉验证后以同 MAC/同广播魔术包唤醒（HA API 无可用 token 的等效做法，已记录）。
- 状态：GUI Client 端集成、真机安装/跨设备实测、测试报告与 Early Beta 发布见后续追加条目（本条记录到 2d034df/34799d3 为止的已验证事实）。

## Sidebar / 窗口顶部融合迭代（2026-10-23）

- Timmy 参照 TranscribeX 截图授权调整侧栏，并要求直接更新 MBP、打开验收。本轮保留现有功能与浅色暖色 token，仍为待验收的视觉迭代，不代表最终产品名/视觉定稿。
- 侧栏宽 204px、导航 13px / 最小行高 32px、品牌 16px；管理入口放到主导航下方；恢复可见键盘焦点。共享 52px 顶栏延续侧栏底色，macOS 使用 Overlay、保留原生红绿灯并调整位置；没有新增材质依赖。
- 本轮恢复前端折叠入口、⌘\\ / Ctrl+\\ 与持久化；使用共享前端按钮，旧 Rust 原生 accessory 开关仍关闭，避免重复按钮。按钮不在折叠区域内，折叠时导航 inert。
- 验证：结构回归测试先出现 3 项预期失败，再实现后 `npm test` 4/4；`npm run check` 0 errors / 0 warnings；`npm run build` 和离线 Tauri release `.app` 打包成功（未签名，25.41 MiB）。独立 headless 浏览器验证 900/640px、顶栏/侧栏对齐、展开/收起、inert、刷新后记忆与快捷键，检查浅深色截图。
- 在 `tims-mbp` 更新 `~/Applications/Local Transcribe.app`，源/安装可执行文件 SHA-256 一致：`a21cfcb748dc377fbeef7d23f8adef116c3062af1e3a01298e4e56f06fcaee9e`。旧版与第一次对齐前的中间版保存在 `~/Applications/Local Transcribe Backups/`，未删除应用或用户数据。精确路径启动后核实进程来自安装位置；真实 Tauri 窗口截图确认红绿灯同排、侧栏通顶、历史显示；AX 操作验证折叠/展开，最后留在展开首页供 Timmy 验收。
- 限制：Windows 本轮未验证；没有重跑真实 ASR。`cargo test --workspace --offline` 在未改动的 `voice-to-text-lan` 测试 `client::tests::resolved_service_txt_records_map_to_node_info` 失败（`ParseIpAddr("invalid IP address syntax")`），不能宣称全工作区全绿。本轮未修改该模块。详情见 `logs/2026-10/2026-10-23-sidebar-window-integration.md`。

## macOS 局域网共享与两模式收敛（2026-10-23）

- 按 Timmy 明确授权，只保留「仅本机」「局域网优先」；旧 prefer_local 按 local_only 读取。节点失败自动在原任务 ID / lease 内本地重跑；需已选本地模型，主动取消不重跑。节点能力需认证且启用计算；模型不匹配上传前拒绝。
- 局域网页增加默认关闭的「允许其他设备使用本机算力」（仅 macOS）：私有 IPv4 Wi-Fi/以太网接口、名称、端口、当前模型、状态、显式显示配对令牌。只绑定选定地址并限制同私有子网，不配置公网/防火墙；正常退出关闭，不自动重启。HTTP 未加密的限制已明示；接收媒体留在 lan-host，不自动清理。
- 验证：默认 workspace tests、严格 Clippy、前端 check/build/tests 均通过；显式真实 Mac 接口的节点启停/令牌/端口冲突测试，以及已安装 Paraformer + scratch WAV 经真实本机 LAN 地址上传/转写/取回结果并校验源文件未改均通过。子网/伪造 forwarded headers/模型不匹配/卡住上传停止回归通过。
- UI 用 mock Tauri 的独立 headless 浏览器验证两模式、开关、令牌按需显示、浅深色和 900/640px 无溢出。未在真实 Tauri 窗口点击完整链路；未做两台电脑间断线回退端到端验收；Windows 本轮未验证。
- macOS 最终 `.app` 打包通过（26.02 MiB，未签名/未公证），bundle 中局域网用途与 Bonjour 声明已回读。随后按 Timmy 授权更新 `~/Applications/Local Transcribe.app`：旧版备份至 `Local Transcribe Backups/before-lan-sharing-20261023-133520/`，安装前后 SHA-256 一致（`4953c749…592d3`，adhoc linker-signed），从安装路径启动后真实窗口 AX 验证两模式、共享开关默认关闭、真实私有接口列出。设置文件的旧 `prefer_local` 按读取时 alias 生效为仅本机，下次保存设置才改写。不提交 Git、不发布、不安装新工具或下载模型。完整记录见 `logs/2026-10/2026-10-23-macos-lan-sharing.md`。
- 同日 14:25 将同一构建（`4953c749…592d3`）部署到 MBA（SSH `airm1`）：暂存与安装后哈希均一致，上午 beta.2 版本备份至 Air 的 `Local Transcribe Backups/before-lan-sharing-20261023-142456/`，`open` 启动后进程级验证通过（PID 存活、可执行路径正确、无崩溃记录）；GUI 交互验证待 Timmy 在 Air 上查看，两台设备间配对/上传/回退仍未端到端实测。详情见 `logs/2026-10/2026-10-23-MBA-beta2-installation.md`。
- 同日傍晚修复长期存在的 macOS 窗口拖不动问题（上游 `tauri#9503` Overlay 拖动 bug + 本地 `data-tauri-drag-region` 命中区被子元素遮挡的双层根因）：TDD 先红后绿（7/7 测试、0/0 警告），标题栏改为显式 `startDragging()`；本机安装并三点位实测拖动全部恢复、按钮点击不受影响（备份 `before-dragfix-20261023-152953/`，新包 `f67b9430…1d16`）。修复未提交、MBA 未更新。另发现发布链路问题：`1.1.0-beta.3` 升版未进入提交 `48a327e` 但 tag 已指向它，发布继续前须先修正。详情见 `logs/2026-10/2026-10-23-window-drag-fix.md`。

## Windows 本机验证 1.1.0-beta.2 与跨平台测试修复（2026-10-23）

- 修复 `npm test` 在 Windows 的唯一失败（`core.autocrlf=true` 使 `Cargo.lock` 工作区为 CRLF，版本同步断言以 `\n` 字面匹配）。按 TDD 先加 LF/CRLF 双夹具回归测试并确认预期失败，再最小修复为 `\r?\n`；仅改 `apps/desktop/tests/sidebar-shell.test.mjs`，未改产品代码与 Git 换行策略。macOS 与 Windows 双端定向/全量 `npm test` 6/6、`npm run check` 0/0、`npm run build` 均通过（未提交，工作区另有并行改动未触碰）。
- Windows 真机（Windows 11 build 26300，SAC 强制）：依赖核查后仅做项目本地 `npm ci`（61 包，锁文件哈希不变），未装全局工具；`cargo fmt --check` 通过。`cargo test --workspace` 受 Smart App Control 阻断（未签名 build-script，os error 4551；CI 3033/3077），未绕过；真实 Windows Rust 验证由 Actions run `39899731019`（全绿）承担。
- beta.2 MSI 本机安装成功（1.0.0 → 1.1.0.2，`C:\Program Files\Local Transcribe\`）；安装 exe 与 CI 暂存 exe 仅 3 字节差异（Tauri `_TAURI_BUNDLE_TYPE_VAR_UNK`→`_TAURI_BUNDLE_TYPE_VAR_MSI` 打包标记，良性）。**启动被 SAC 阻止**（CI 3033/3077，13:04:15）：未签名应用在 SAC 强制 Windows 上「可安装、不可运行」。GUI/ASR 本机验收在该机受阻，不以安装成功冒充验收；SAC 保护机器分发需受信任代码签名（待 Timmy 决策）。
- 详情见 `logs/2026-10/2026-10-23-windows-beta2-local-validation.md` 与 CM 机器日志 `Windows电脑/2026-10/2026-10-23-Win-VoiceToTextApp-beta2本机验证.md`。

## 可信局域网 / 远端模型改造（2026-10-23，发布暂停）

- Timmy 已确认删除整个认证功能、精简共享文案，客户端获取并选择主机模型。已移除认证中间件、令牌生成/读取/GUI/CLI 字段与认证头，加入模型列表接口与逐任务模型选择；mDNS 连接使用解析后的 IP 并避免默认同名主机名碰撞。
- TDD 先红后绿；workspace tests、严格 Clippy、npm 9/9、check 0/0、release app 构建通过。两机均已安装新包（402df46e…a05b8），备份完整；当前改动未提交。
- **真实两机仍受阻，不能宣布修通**：Air→MBP 以及 MBP 自访真实 LAN 地址均 TCP 成功但 HTTP 超时；Python 对照双端 HTTP 200。系统 ALF 日志与 AX 弹窗证实 MBP 正等待 Local Transcribe.app 入站连接授权，未代点允许或关闭防火墙；Air CuaDriver 另缺 Screen Recording。MBP 共享 GUI 已确认展示三个真实模型且无令牌控件；完整转写与 Air GUI 选择尚未验收。
- beta.3 仍暂停；先前提交 2fc8ed2 的 CI 不代表本轮改造已过 Windows。beta.1 四个获批裸 exe 已删，剩 dmg/msi/校验和。详情见 `logs/2026-10/2026-10-23-trusted-lan-remote-models.md`。

## 两机重构建安装与旧备份清理（2026-10-23 17:50）

- Timmy 要求在两台 Mac 安装最新版并删除旧版。经确认按「当前工作区源码重构建」执行（含 16:31 未提交的 lan.rs / node.rs / types.ts 改动），旧版指两台 `~/Applications/Local Transcribe Backups/` 现有备份。
- 验证：`cargo test --workspace --offline` 0 failed（真实权重/接口用例仍 ignored）、严格 Clippy 与 fmt 通过、`npm test` 9/9、`check` 0/0、`tauri build --bundles app --no-sign` 成功（26.02 MiB，未签名/未公证）。
- 新包 SHA-256 `854e59c7…6dd18`，已装到 MBP 与 Air 的 `~/Applications/Local Transcribe.app`，安装后哈希与构建产物一致；两机均由安装路径启动（进程 73951 / 19629），MBP GUI 截图确认正常渲染，Air GUI 未验证（CuaDriver 缺 Screen Recording）。
- 旧备份仅 `mv` 入废纸篓：MBP 6 个（约 154 MB）、Air 3 个（约 74 MB）→ `~/.Trash/local-transcribe-old-backups-20261023-1750/`；两机各留一个回滚点 `before-rebuild-20261023-1752`（原 `402df46e…a05b8`）。未用 `rm`、未清空废纸篓、未动用户数据与模型。
- 仍未做：真实 ASR 重跑、两机 LAN 端到端、Windows 验证；改动未提交，beta.3 发布仍暂停。详情见 `logs/2026-10/2026-10-23-macos-rebuild-and-old-backup-trash.md`。

## 局域网发现：TCP 回退（2026-10-23 傍晚）

- 两机互不可见的根因分成三层：MBP 上普通进程收发局域网多播失败（`EHOSTUNREACH`，`nc` 可连、`python3` 不可，system mDNSResponder 正常）；Air 当时共享关闭无服务可发现；Air 节点列表另有一条 name/host 写反的旧记录。详见 `logs/2026-10/2026-10-23-lan-tcp-fallback-discovery.md`。
- 按 Timmy「不用 Apple Bonjour、保持跨平台」的要求，`client.rs` 增加纯 Rust 的 TCP 回退发现：`local_scan_hosts`（UDP connect 取本机 /24）+ `scan_blocking`（并发探测端口并校验 `GET /api/v1/capabilities`）+ `merge_discovered`（mDNS 优先、按 host:port 去重）+ `discover_with_scan`。`Capabilities` 新增 `name`（serde default，兼容旧节点）。
- 界面扫描窗口 3 s → 8 s，空结果增加「本地网络权限 / 对方未共享」提示（五语言 `lan.scanEmptyHint`）。`bundle:mac` 脚本在打包后用 `codesign --identifier com.timmy.voicetotextapp` 重签，代码标识从此稳定。
- 验证：新增 3 个 Rust 测试（TCP 扫描发现节点并读到名字、裸端口不算节点、合并去重）与 3 个前端测试，先红后绿；workspace 测试 0 failed、严格 Clippy、fmt、`npm test` 12/12、`check` 0/0 通过；`bundle:mac` 产出 26.06 MiB，`codesign -dv` 为 `com.timmy.voicetotextapp`，`--verify --deep --strict` 通过（仍 ad-hoc 未公证）。
- 两台均已换装 `8a5bacec…22e73`（旧包备份至 `before-scan-discovery-20261023-1835`）。Air 侧用同代码探针在真实 /24 上成功发现临时节点，TCP 路径单测用 127.0.0.1 夹具验证。
- 未验证：MBP 应用内点击扫描（当时屏幕锁定）、授权后 mDNS 是否直接可用、两台间的完整转写链路、Windows。Air 需开启共享才会被 MBP 发现。

## 局域网权限提醒与首页远程模型菜单（2026-10-23 晚）

- 局域网页新增权限横幅：应用内探测（一次丢弃式多播发包，`PermissionDenied`/`HostUnreachable` 判为未授权），页面加载与每次扫描后各检测一次；未授权时给出说明、`打开系统设置`（本地网络）与`重新检测`。同一台 MBP 上应用进程报 granted、普通 shell 进程报 denied，说明探测反映的是应用自身授权，不会误报。
- 删除共享区块的「可用模型」与「关闭共享/退出应用」两段文案（五语 key 一并删除）；修复节点模型下拉下方的大块空白（`.list` 纵向 flex 中 `.field` 被 flex-basis 撑高，改为 `flex: 0 0 auto`）。
- 首页模型菜单按设备分组：`本机` + 每个已启用节点一段；选远程模型会启用该节点并切到 `prefer_lan`，选本地模型切回 `local_only`；按钮显示`节点 · 模型`。
- 验证：新增前端测试先红后绿，`npm test` 15/15、`check` 0/0、`build` 通过；新包 `bf78401d…471385` 已装 MBP 与 Air（备份 `before-ui-tweaks-20261023-1915`），签名标识保持 `com.timmy.voicetotextapp`。视觉与交互验收由 Timmy 自行完成。

## 局域网页收窄与首页模型指示灯修复（2026-10-23 晚二）

- 局域网页不再承担添加任务与选择模型：移除计算方式分段控件、节点「转写模型」下拉、「发送文件…」按钮及相关逻辑/事件监听；只保留共享开关、节点管理、选中节点的任务列表与结果保存/回收。模式由首页模型菜单决定（远端模型 → prefer_lan，本机模型 → local_only），设置页该项改为只读说明。
- 修复首页模型按钮圆点在选中远端模型后变灰：新增 `remoteDotOn`，选中远端模型即保持绿点；同步修正 tooltip。
- 文案改动（五语）：`lan.subtitle` 说明新流程；删除 `lan.sendFile`/`lan.sending`/`lan.remoteModel`/`lan.remoteDefault`。
- 验证：`npm test` 17/17、`check` 0/0、`build` 通过；workspace 测试 28 组全 ok、严格 Clippy、fmt 通过。新包 `41cd00f8…975c0` 已装两台（备份 `before-lan-scope-20261023-1945`），未提交。

## 模型下载功能并回当前构建（2026-10-23 晚三）

- 现象：模型页的「可下载的模型」不见了。原因不是回退——该功能一直在 `model-download-quality` 分支（另一 worktree `VoiceToTextApp-mq`），而安装的构建来自 `main`。分支与 main 无分叉（合并基即 main tip）。
- 处理：未提交 Git，把分支内容并入当前工作区。直接取分支文件（`model_download.rs`、`ModelView.svelte`、transcription-core 的标点/质量修复、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock`、`package-lock.json`、测试与文档），手工合并 `lib.rs`、`+page.svelte`、`types.ts`、五语言文案与 `package.json` 版本。
- 修掉分支自身的一个缺口：catalog 只在下载完成后刷新，页面初次打开是空的；补上挂载与进入模型页时的 `refreshCatalog()`。
- 版本统一为 `1.1.0-beta.4`；`cargo test --workspace` 29 组全 ok、严格 Clippy、fmt、`npm test` 17/17、`check` 0/0、`build` 通过。新包 `ecd77e14…2f966`（26.17 MiB，签名标识不变）已装 MBP 与 Air，备份 `before-model-download-20261023-2020`。
- 未验证：真实下载与下载后推理质量（由 Timmy 验收）。

## 模型下载说明文案删除与两机换装（2026-10-23 20:45）

- 删除模型页「以上模型来自官方 Hugging Face 仓库…」说明（`model.downloadHint` 五语言 key 一并移除）；`npm test` 17/17、`check` 0/0 通过。
- 重新打包 `1.1.0-beta.4`，可执行文件 `231fb48c…bc9dbc`，签名标识 `com.timmy.voicetotextapp`。
- 两台机器先备份被替换的 `ecd77e14…2f966` 到 `before-hint-removal-20261023-2045`，安装后哈希核对一致；其余 6 个旧备份目录整体移入 `~/.Trash/local-transcribe-old-backups-<时间戳>/`，仅 mv、未清空废纸篓，各机保留本次一份回滚点。

## 已下载模型的删除入口（2026-10-23 21:15）

- 下载列表的已安装条目新增「删除」按钮（确认后移入回收站）；模型库列表的删除按钮改为始终显示，默认模型也能删。
- 后端新增 `delete_catalog_model`、`installed_path`、`delete_installed*`；删除默认模型时先清空选择（写空文件，不删文件），`read_selected_model` 把空文件视为「无默认模型」。ASR 目录走 `ModelLibrary::delete_to`，标点模型走文件级 move 到回收站。
- 新增 3 个 Rust 用例与 1 个前端用例，先红后绿；`cargo test --workspace` 29 组全 ok、严格 Clippy、fmt、`npm test` 18/18、`check` 0/0 通过。新包 `72d39998…377af` 已装两台（备份 `before-model-delete-20261023-2115`）。

## 模型清单重选、导入移除与 beta.5（2026-10-23 深夜）

- 以中文为主重新实测 11 个候选模型（6 段 MiMo TTS 音频，27–116 s，含 30/60 s 切块边界与难点稿，字符级 CER）：
  X-ASR Zipformer 0.84%、FireRedASR2 AED 1.29%、SenseVoice 2025 1.67%、Paraformer 2023 1.74%、
  Qwen3-ASR 1.80%、Zipformer CTC small 2.57%；Whisper small 26.06%、Dolphin 6.82%、
  TeleSpeech 10.49%、FireRed CTC 3.22%、FunASR Nano 59.65%（KV 512 截断）被淘汰。
- 模型页改为「准确优先 3 + 速度优先 3 + 标点」分组；清单新增 X-ASR、Zipformer CTC small，
  SenseVoice 换成 2025-09-09 导出，Whisper small 移除。
- 移除任意模型导入：GUI 按钮、`import_model` 命令、CLI `models import`、MCP `import_model`
  全部删除（MCP 工具 9 → 8）；模型库只保留清单下载的模型。
- transcription-core 新增 6 个模型族布局与配置：FireRed CTC、Zipformer CTC、Dolphin、
  TeleSpeech、FunASR Nano、transducer（X-ASR），并新增布局检测用例与清单一致性用例。
- 版本 `1.1.0-beta.5`；文档 `docs/model-download-and-quality.md` 记录全部 CER/速度数据。
- Windows：在 Timmy 的 Windows 11（build 26300）本机 `npm ci` + `tauri build --bundles msi` 成功，
  产物 11,476,992 字节；未在 Windows 跑测试套件。
- 发布：tag `1.1.0-beta.5`（pre-release），资产 DMG + MSI + SHA256SUMS
  （https://github.com/TimmySheep/VoiceToTextApp/releases/tag/1.1.0-beta.5）。
- MBP 已换装，旧版备份 `before-model-catalog-20261023-2007`；Air 不可达，未更新。
- 待办：GUI 下载流程、Windows 真机 GUI/录音、跨机局域网转写仍待 Timmy 验收。

## Headless node, tray and jobs list (2026-10-23)

### Verified

- Single instance: second launch exits 0, one GUI process, wake-up callback logged (debug and installed app).
- Headless node: `--listen` required; netmask rules (`/1`–`/30` same-subnet, `/31`/`/32` unfiltered, `0.0.0.0` unscoped); Tailscale `100.64/10` may omit the mask; state file 0600 with address/mask/`subnet_filter`; stale pid recovery; duplicate start exits 1; `--parent-pid` child exits within 5 s after `kill -9`; real WAV transcribed over HTTP on `127.0.0.1:7878`.
- mDNS: skipped for non-RFC1918 binds (`100.x`) and loopback; wildcard announces one RFC1918 address without `enable_addr_auto()`; `/24` unchanged.
- Supervisor: GUI no longer embeds `NodeServer`; child stdout/stderr go to `~/Library/Logs/com.timmy.voicetotextapp/lan-node.log` (5 MB rotation, one generation); port conflict and abnormal exit surfaced from real child runs.
- Memory measurements: M1 single-process switching cost ≈1.75 s (FireRed) / ≈1.24 s (Zipformer), one model resident; M2 same-second sampling: node peak 1,808,320 KB (1.72 GiB), local peak 958,048 KB (0.91 GiB), combined peak 2,265,264 KB (2.16 GiB) — individual peaks were not simultaneous, so do not add them; M3 debug run: both processes logged the 120 s unload, but at 260 s RSS was 872 MB (node) and 484 MB (local), GUI 42 MB — cache release, not immediate RSS return.
- Idle unload: 20/120/600/1800/7200 closed set, invalid falls back to 120, task end refreshes the timer, `loaded_model_id` no longer blocks during inference.
- `jobs list` / `get_job_status` now share the persisted store; finished list capped at 200 and survives restarts; queued jobs are still lost on restart.
- Tests: `cargo test --workspace` 29 groups all ok; strict Clippy; `cargo fmt --check`; `npm test` 18/18; `svelte-check` 0/0; frontend build.
- Packaging: `.app` 45.99 MiB with `local-transcribe-node` sidecar (both ad-hoc signed), installed at `~/Applications/Local Transcribe.app`; version 1.1.0-beta.5.

### Blocked / not done

- GUI acceptance was blocked by the locked screen: starting sharing from the UI, tray menu interaction, close-to-background behaviour and the 20 s/7200 s setting switch are **not verified**. P4.2 is implemented in code (auto-resume with notice, quit confirmation, `lan-host` retention with Trash, data-location section with six paths and clear-all, consent, restart banner, explicit port-in-use message) but its GUI paths and screenshots are pending the same unlock.
- The two UI taskbooks (temporary-UI label removal, sidebar brand removal, model title/subtitle split, theme button, `prettyName` across 8 display points) are implemented; `npm test` 22/22, `svelte-check` 0/0, build passes. Screenshots and five-language visual checks are pending the unlock.
- P3.5 (CLI/MCP LAN dispatch) is implemented and verified on loopback end to end: `jobs add --node` completed on the node with real text (`export` and `get_job_result.fullText` identical), failure without fallback stayed `failed`, `--fallback-local` reused the same job id, cancel reached the node (`cancel_requested → cancelled`), MCP `list_nodes`/`nodeId` behaved and rejected unknown ids before any connection. Two-machine dispatch is still unverified.
- Windows: no MSI was built for this milestone; tray/sidecar compile and packaging there are unverified.
- The old "LAN TCP works but HTTP times out" symptom did not reproduce today: Air → 192.168.1.243:7878 and Air → 100.104.46.54:7878 both returned `ok`. This is a two-sample observation, not a confirmed fix.

## 阅读器交互修复与胶囊统一（2026-10-24）

- 已实现：reader bar 明确启用 pointer-events，装饰层不接收点击；移除 TOP 调试标签，scrollport 从 40px 下方开始；模型菜单移除带 opacity 的过渡并使用不透明底及更高 stacking context；内容区圆角、导出/右上胶囊、原生细滚动条统一。
- 已验证：真实 Svelte 组件在 Chrome headless 内执行中心命中、两轮首页回调、两轮查找展开与聚焦、浅深色菜单不透明、胶囊计算样式/长模型名截断。`npm run check` 0/0，`npm test` 38/38（0 skipped），前端 build 通过；不是 WKWebView 或 Windows GUI 验收。
- 原截图「…ON / TOP / thumb」三处 elementFromPoint 输出未采集，不使用 fixture 坐标替代。fixture 的实际命中与测试先红后绿记录见 `logs/2026-10/2026-10-24-reader-interaction-and-capsules.md`。
- 按 Timmy 要求停止 CUA 验收；真实应用导航、菜单/圆角放大截图、浅深色、玻璃对照、macOS/Windows 滚动条与性能均待本人验收。macOS bundle/安装结果另行追加，未发布新版本。

### 阅读器补丁最终安装（2026-10-24 01:45，本机时间）

- 最终源码 check 0/0、38/38 前端测试、`bundle:mac` 均通过；已备份旧应用并正常退出/换装/重开 `~/Applications/Local Transcribe.app`。构建包与安装包通过 strict codesign 校验，desktop/node 哈希各自匹配。
- ad-hoc 签名、版本仍 1.1.0-beta.5；无 Git 提交/发布。未做 CUA/Windows 验收，人工清单不变。安装路径、备份及哈希见上述阅读器日志。

### 两份阅读器任务书源码自查补漏（2026-10-24 后续）

- 修复fade/正文底色不一致、off/reduce填充token、contrast覆盖优先级、渐隐子层圆角、首页实际滚动列表规则及长模型名挤压箭头；仅限这两份任务书对应的源码与测试，无观感判断。
- 真实组件/CDP计算样式与几何测试先红后绿；最终check 0/0、前端测试39/39、build通过。保留原异常三处历史命中缺失与两份TXT/JSON要求互斥的说明，不伪称已补齐证据。
- 本轮未重打/换装.app、未重启，当前安装版本不含本段补漏；未提交/发布。详细变更与数值追加于阅读器交互日志末尾。

### 菜单栏图标资源修正（2026-10-24）

- 使用标准库4x超采样生成22/44px黑色透明模板与32px白色透明Windows资源；托盘按平台嵌入并可读报错，只有macOS启用template；四态逻辑与菜单未变。增加image-png feature。
- 资源/幂等/加载源码断言2/2通过，指定桌面cargo build通过，原四态测试1/1及fmt检查通过。未做观感或Windows运行判断，未换装或重启应用。详见 `logs/2026-10/2026-10-24-tray-template-icon.md`。

### 当前源码本机安装（北京时间2026-10-24 02:04:58）

- 用户指定全量测试门通过：Rust169 passed/0 failed/11 ignored，严格workspace Clippy通过；前端check0/0、测试39/39、build通过。随后仅用bundle:mac生成.app，sidecar与嵌入的macOS托盘PNG已核对，identifier及strict签名校验通过。
- 已备份至`~/Applications/Local Transcribe Backups/before-20261024-020458/`，正常退出旧PID97943，安装前7878无监听且无node残留。新包安装到`~/Applications/Local Transcribe.app`，desktop安装前后SHA-256相同：`bf14d5b783ad23449228e7b7134769cf2ace03f54d18447d003593cfc50dee07`。
- 新PID14026，ps及lsof确认从安装路径执行；启动后20秒未发现新相关崩溃报告。包含此前阅读器补漏和托盘图标修正，版本仍1.1.0-beta.5/ad-hoc；未发布。
- 本次未验证真实转写/两机LAN/Windows/长期运行或观感，未清理文件。完整安装证据在`logs/2026-10/2026-10-24-current-source-local-install.md`。

### A版应用图标已应用（2026-10-24 02:14:51 北京时间）

- Timmy选择A；按任务几何生成七份应用图标资源，托盘和参考原图未变，保留B版。资源幂等/采样校验及cargo build、前端check/test、bundle:mac通过；只打.app，版本未变。
- 备份before-20261024-021451后正常退出并确认7878释放，安装~/Applications/Local Transcribe.app；新PID23684，desktop SHA256=682df4b8bd7005bc97bc48ac5d557a832a3b13dfb8915db60d74aa7ebe552bfd，安装前后一致；包内ICNS与生成A资源哈希相同，strict签名校验通过。
- 不作观感判断、不清理、不发布；详细参数、像素统计口径及安装记录见logs/2026-10/2026-10-24-app-icon-a-apply.md。

### Pro/Air旧版本回收与新版同步（2026-10-24 02:17 北京时间）

- 用户确认精确清单后，Pro15份旧.app备份、Air2份旧备份及旧安装.app分别移入各机带时间戳废纸篓目录；不永久删除、不清空废纸篓。模型/数据/设置/源码/B设计/Windows安装包未动。
- Pro原已安装A版，保留并复核；Air先暂存校验后换装相同A版，strict签名及desktop/node哈希两机一致。Pro PID23684；Air PID14850，启动02:17:42，15秒无新相关崩溃报告。
- 当前两机安装均为1.1.0-beta.5当前源码A版，desktop SHA256=682df4b8bd7005bc97bc48ac5d557a832a3b13dfb8915db60d74aa7ebe552bfd；无发布/新版本号。详细移动清单见logs/2026-10/2026-10-24-pro-air-old-apps-trash-and-install.md。

### 阅读器常驻渐隐与本机覆盖安装（2026-10-24 02:23北京时间）

- 按最新要求改72px多stop常驻渐隐，shell独立局部层叠上下文、fade2/bar3/scroll-auto，条空白命中shell；滚动背景透明，两端渐隐与回退均用页面底色。底部24/8px行为保留。
- 真实组件computed/命中断言先红后绿，check0/0、测试39/39、bundle:mac通过。Pro已备份并覆盖安装，新PID32381，desktop SHA256=7430fe2fae9b4b138d41a1dbcec86b0c351dd83e2eacffc32e0b2be6aeea1780；15秒无新相关诊断报告。
- Air未做本轮更新，仍为前一构建；无观感判断/图像采集/清理/发布。证据见logs/2026-10/2026-10-24-reader-permanent-fade.md。

### 应用及macOS托盘波形加高（2026-10-24 02:28北京时间）

- 用户要求更长竖波/更大起伏差；应用高度168/384/240、macOS托盘8/17/11，宽度/配色/中线不变；Windows托盘、A参考及B方案保留。
- 几何测试2/2、托盘资源测试2/2、前端check/test和bundle:mac通过；Pro已备份正常quit覆盖安装，严格签名和哈希通过，新PID37818。Air未更新，未发布；详细数值与备份追加在app-icon-a-apply日志末尾。

### 阅读器滚动内容顶部留白（2026-10-24 后续）

- 新增内容包裹层padding-top，公式为fade高度-bar高度+8px，集中CSS变量，当前40px；不改变scrollport padding-top或常驻fade。
- 先红后绿实测：scrollTop0标题top106、fade底90、间距16px，标题及祖先opacity1/mask:none；改fade高88时padding自动56px。到底最后段落底间距保持39.96875px、bottom fade8，top fade72/opacity1。
- check0/0、npm test39/39通过；本轮未打包/换装，安装包尚不含此padding改动。细节追加于reader-permanent-fade日志。

### 按钮体系规范 V1.0（2026-10-24）

- 已集中四档样式及行内危险变体，迁移组件并移除重复定义；模型页仅下载使用胶囊，设置页仅清除使用危险胶囊，表单控件保持。
- check 0/0、测试41/41（无跳过）、前端build通过；自动断言覆盖每页主操作上限、模型每行胶囊上限、设置文字动作透明背景/非胶囊圆角和主操作disabled。
- 未截图或评价观感；未原生打包/安装/发布，已安装版本不含本轮修改。详见logs/2026-10/2026-10-24-button-system.md。

## 移动端 P1 · 假数据可点击界面（2026-10-24）

- 按移动端任务书 V1.0 顺序只完成 P1：`apps/ios/`（SwiftUI，xcodebuild 可构建）与 `apps/android/`（Kotlin/Compose，gradlew assembleDebug 可构建）各一套全量假数据、可点击界面，结构学「语音备忘录」，视觉用项目自己的 token；已停下等 Timmy 审阅，P2 未开始。
- 实测：iOS build SUCCEEDED、test 9/9（Format 7 + Font 2）；Android assembleDebug BUILD SUCCESSFUL、testDebugUnitTest 7/7；截图证据 `docs/screenshots/mobile-p1/`。
- 字体（已核实）：iOS 18.4 模拟器无 Songti SC，标题衬线落点 New York，中文标题在模拟器截图不衬线；真机中文衬线待验证。详见 logs/2026-10/2026-10-24-mobile-p1-fake-data-ui.md 与 docs/mobile.md 第 9 节。
- 本阶段全部为假数据，未做任何真实录音/上传/转写/推理；所有留接口入口显式标注「未完成」；未引入云服务/账号/遥测，未 git commit。

## CLI/MCP 对等、首页删除、共享开关、许可证合规（2026-10-24 下午）

- **CLI/MCP 对等**：共享 `lan-settings.json`（fs2 锁、损坏备份、原子写）；CLI 增加 `lan list/nodes/add/remove/enable/disable/set/scan/start/stop/status` 与 `mode`；MCP 增加 `add_node/update_node/remove_node/node_status`（共 13 个）；远程启停只走 SSH（BatchMode，固定 `<node_binary> --from-settings`，不装守护进程）。任务书 §8 的 12 条测试全部落地（服务 8 条、设置 2 条、MCP 2 条，其中第 12 条另有 CLI 端到端退出码测试）；第 11 条注入用 canary 文件验证未执行。
- **首页删除 + 模型页去重**：历史卡片右键 / Shift+F10 / ContextMenu 打开只有「删除」的菜单；确认文案含文件名与「只删本应用记录、不动原始音视频」；后端 `delete_history_entry` 只把 `transcripts/<id>.json`、`jobs/<id>.json`、`jobs/<id>.cancel` 移入废纸篓，拒绝运行中任务与不安全 id，`jobs/.dispatch.lock` 与源媒体不动；模型页移除重复的已安装列表，只有清单外模型才出现「其它已安装」。
- **共享开关**：状态点抽成 `app.css` 的共享 `.status-dot`（7×7，绿/灰）；按钮去掉 `role="switch"`/`aria-checked`，只写动作并带状态 `aria-label`，切换中 `disabled + aria-busy`；删除内存提示与五语言 `lan.memoryHint`。
- **许可证**：5 个 crate 与 workspace 改为 MIT；两只 README 补名称/图标排除；新增 `THIRD-PARTY-NOTICES.md`（605 个包，点名 sherpa-onnx Apache-2.0 与 symphonia MPL-2.0）与 `docs/model-licenses.md`（7 个模型：Paraformer Apache-2.0，其余 6 个模型卡未标注）。`bundle.resources` 把 LICENSE 与声明打进 `.app/Contents/Resources/`，设置页「关于」新增打开入口；本地 release 构建实测两份文件存在，ad-hoc 签名后 `codesign --verify --deep --strict` 通过。
- **验证**：`npm run check` 0 errors / 0 warnings；`npm test` 64/64；`cargo test --workspace` 通过。服务 mock 在并行负载下出现过一次连接抖动，已给测试 mock 增加启动就绪探测，随后连续 4 次全量通过。
- **未做/边界**：以上改动未提交、未推送，也不在已发布的 1.2.0-beta.1 安装包内；发布页仍缺 `THIRD-PARTY-NOTICES.md`。远程启停未在第二台真实 Mac 上跑通（需要对方开机 + 免密 SSH）；Air 当前不可达，未复核安装。

## 应用图标替换为奶油珊瑚引号（2026-10-24）

- 以 Timmy 提供的奶油方块 + 珊瑚引号图为源（`design/app-icon-quote-source.png`），新增 `scripts/gen-quote-icon.py` 生成：`.icns`、`.ico`（6 档）、32/64/128/128@2x、Windows Square/Store 标志、macOS 菜单栏模板（22/44）、Windows 托盘（32）、favicon。
- 菜单栏模板改为引号剪影并随系统着色；Windows 任务栏/快捷方式由 `icon.ico` 决定，需下一次 Windows 构建嵌入（本轮资产已更新，无法本机交叉构建）。旧波形生成器加了防误覆盖保护。
- Pro 已完成备份、覆盖安装与 ad-hoc 签名（`codesign --verify --deep --strict` 通过），新 PID 37167、版本 1.2.0-beta.1；bundle 内 icon.icns 哈希 `e66c5177…f46763`，二进制内已嵌入新菜单栏模板。Air 不可达未换装；未重新发布。
- 新测试：`scripts/test-quote-icon.py` 5/5、`scripts/test-tray-icon.py` 4/4。

## 1.3.0-beta.1 发布与三端同步（2026-10-24）

- 版本 1.3.0-beta.1（MSI 1.3.0.1）；发布 GitHub Pre-release，资产 DMG `2a18be12…914e`、MSI `c64b1dfb…6b34`（与 CI 一致）、SHA256SUMS。仓库在 GitHub 上已改名为 Local-Transcribe，原地址重定向。
- 内容：CLI/MCP 能力对等、首页记录删除、模型页去重、共享开关改造、结果模型修正、奶油珊瑚引号图标（含菜单栏/Windows 托盘）、MIT 许可与第三方声明。
- 三端：MBP（pid 87717）与 Air（pid 87494）均从同一 DMG 安装并运行，exe `d452ed4f…`、node `f77904a4…`；Windows MSI 安装成功，版本 1.3.0-beta.1，node `880f9528…`（= CI 产物），GUI 未在 SSH session 0 启动。
- CI：格式、Windows 编译（Arc/Mutex、stderr 绑定）、平台测试（dns-sd/ssh）三次修复后 run `40066611735` 全绿。
- 清理：Pro 的 `target/` 等 54 GB 与旧包/备份/临时文件、Air 的旧应用与 DMG、Windows 的安装器与日志均移入废纸篓（未用 rm、未清空废纸篓）；Windows 两个源码/构建目录保留（0.49 GB + 25.97 GB）。
- 边界：macOS ad-hoc 未公证、Windows 未签名；Windows 7878 入站防火墙仍未开放；模型许可证 6/7 未标注（见 docs/model-licenses.md）。

## 菜单栏图标重做（2026-10-24）

- 托盘模板从"两个粗逗号"改回三条竖波：22pt 画布、条宽 1.5pt、间距 3pt、高 6.5/11/8.5、圆头 0.75、整组 10.5pt 居中；三份 22/44/32 由同一参数缩放，Windows 为白色同形状。
- `gen-tray-icon.py` 重写并成为托盘资源的唯一生成器；`gen-quote-icon.py` 不再写托盘；测试 7/7（含 @2x y=22 三段扫描与 ≤12% 面积上限）。
- 本机 Pro 已重装含新模板的构建（pid 87150，1.3.0-beta.1）；因当前 Xcode/ld 的 strip 缺陷需以 `CARGO_PROFILE_RELEASE_STRIP=none` 构建，故该安装与 release 资产不同。未重新发布。

## 菜单栏引号图标（细体）重做（2026-10-24）

- 作废上一轮"三条竖波"方向：引号形状恢复（与应用图标同一套语言），只把笔画改细。22pt 整组 8.5×10.5、单枚 3.5、间距 1.5、竖笔 ≤2pt、尾部收细；22/44/32 同参数缩放，Windows 白色。
- `gen-tray-icon.py` 重写为引号几何单一参数表；测试 9/9（@2x：连通块 2、外接框 18×22、25% 处每段 3px、alpha=255 面积 3.51%）。
- 本机 Pro 已重装（pid 27825，1.3.0-beta.1）；未单独发版，GitHub release 资产待下一批。本机构建 strip 绕行写入 docs/local-build.md，未改 release 配置。

## 菜单栏引号图标（取中）重做（2026-10-24）

- 作废"三条竖波"（方向错）与"细体"（过细）两版，按可测量中间值定稿：@2x 竖笔 3.5px、整组 17×22、间距 3.5px、面积 3.2%。
- `gen-tray-icon.py` 增加 A/B/C 三档参数（3.0/3.5/4.0px），正式资源按 B 档生成（22/44/32，macOS 黑/Windows 白）；实测 16×22、竖笔 3px、间距 3px、面积 3.7%。
- 对照图 `design/tray-weight-compare.png`（深/浅两行 × A/B/C 真实尺寸 + 4 倍放大）；`gen-tray-compare.py` 生成，仅作证据。
- 测试 11/11。本机 Pro 已按 B 档重装（pid 45524，1.3.0-beta.1）；未单独发版，GitHub release 资产待下一批。本机构建 strip 绕行沿用 docs/local-build.md，未改 release 配置。
