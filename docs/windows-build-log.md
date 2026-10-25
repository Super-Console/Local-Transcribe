# Windows Build & Dependency Log

## 2026-10-22T04:36:40+00:00 — Windows toolchain bootstrap started

- Project: `VoiceToTextApp`, provisional English workspace name; no Git commit or remote was created.
- Target: `REDACTED-HOST` / `REDACTED-HOST\REDACTED-USER`, Windows 11 x64 build 26300, checked over authenticated SSH before any install; current system drive free space was about 375.8 GB.
- User explicitly authorized installation of the build dependencies in the active conversation.
- Existing state: WebView2 system runtime is present. No Rust/Cargo, Node/npm, Git, Visual Studio C++ Build Tools/MSVC, or project copy was found in standard locations. No prior project directory exists at `C:\Users\REDACTED-USER\VoiceToTextApp`.
- Official sources: Tauri 2 Windows prerequisites (`https://v2.tauri.app/start/prerequisites/`) specify Microsoft C++ Build Tools, WebView2 and Rust; the source project uses Tauri 2 + Svelte/Node and sherpa-onnx 1.13.8.
- Planned official package-manager installs: Rustup (`Rustlang.Rustup` 1.29.1, publisher Rust Programming Language), Node.js LTS (`OpenJS.NodeJS.LTS` 24.20.0), and Git for Windows (`Git.Git` 2.55.0.5). Current winget metadata exposed publisher and SHA-256 for each installer; see the command transcript/session evidence. These toolchain installs may use their vendor-default user/system locations; no global PATH/config edits are planned.
- Planned Microsoft Build Tools 2022: official winget package `Microsoft.VisualStudio.2022.BuildTools` 17.14.42 and Microsoft command-line bootstrapper; install only x64/x86 MSVC toolchain, Windows SDK and required core build components, suppress recommended/optional workloads. Exact download size is not exposed in the read-only package metadata; Windows currently reports 375.8 GB free. This is a system-wide installation and can download substantial files. Tauri build target is desktop, not mobile. WebView2 will not be reinstalled.
- Project source will be copied only after toolchain bootstrap into the previously absent `C:\Users\REDACTED-USER\VoiceToTextApp` folder. Media, model weights, ignored archives, and user data will be excluded; no real transcription/model weights will be added.
- Installer artifacts / caches / exact versions / verification / build commands / exit codes / cleanup / shutdown evidence: **pending; append below as verified.** Do not infer successful installation or build from installer launch or a client timeout.



## 2026-10-22T05:19:14Z -- Final Windows build status and shutdown verification

- Windows installation completed: Build Tools 17.14.42 / MSVC 14.44.35207 / SDK 10.0.26100.0; Rust 1.99.1 MSVC target; Node.js 24.20.0/npm 11.19.0.
- Windows frontend checks passed; Rust formatting passed. The real workspace tests were blocked before execution by Code Integrity events 3033/3077, which rejected the generated `serde` build script under Enterprise signing-level requirements. No security policy was disabled, modified, or bypassed. No Windows Rust release build, Tauri installer, or runtime/UI verification is claimed.
- WinINet proxy was restored to `ProxyEnable=0` and `ProxyServer=127.0.0.1:7897`; `ProxyOverride` was untouched. Temporary SSH reverse tunnel stopped and port 17890 confirmed without a listener. No matching one-time installer package was found in the scoped temp/WinGet paths; shared caches were retained.
- Shutdown requested at `2026-10-22T05:17:02Z`; post-shutdown SSH timed out and Tailscale reported the peer offline at `2026-10-22T05:20:00.1Z`.
- Action needed before retry: use an administrator-approved, policy-compliant signing/build route or another approved Windows builder. Do not disable/bypass Code Integrity.


## 2026-10-22T05:27:49Z -- SAC policy root cause confirmed

- Read-only `CiTool` and registry confirm enforced system policy `VerifiedAndReputableDesktop` / `0283ac0f-fff1-49ae-ada1-8a933130cad6`, state 1.
- Code Integrity 3033/3077 identify `cargo.exe` launching the generated `serde` build script, blocked by the same policy at 2026-10-22T05:12:25Z. Microsoft Rust setup documentation describes this exact class of block and says Developer Mode/local-trust certificates are not exemptions. Microsoft Support says there is no per-app bypass.
- The latest installer batch exited at its safety guard because the project directory already exists; it did not reach package installation. Git is now confirmed at version 2.55.0.windows.5; MSVC remains installed.
- No policy change/bypass and no Windows build rerun. Another approved Windows builder is the safe path; external source upload requires Timmy approval.


## 2026-10-22T05:29:19Z -- Final shutdown after policy review

- Confirmed WinINet proxy disabled (`ProxyEnable=0`), original server value retained, temporary reverse tunnel absent, and no Cargo/winget/Visual Studio installer processes active.
- Triggered HA `script.pc_shutdown`; verified its `last_triggered` advanced. After 35 seconds SSH timed out and Tailscale reported the Windows peer `Online=False` at `2026-10-22T05:30:00.1Z`.
- No policy changes, no local bypass, and no new Windows build/test.


## 2026-10-22T05:30:37Z -- Late wrapper output invalidated

The delayed old validation process transcript shows that each of four supposed Cargo steps (fmt, workspace tests, strict Clippy, release MCP build) emitted Cargo's generic help text while the wrapper recorded exit 0. None is a valid pass; there is no Windows Rust test, Clippy, or release-build result from that process. The earlier genuine `cargo test --workspace` remains blocked by Smart App Control event 3033/3077. The Windows-side `windows-native-validation.log` could not be inspected or corrected because the machine is now offline; annotate those help-text sections as invalid on the next authorized connection. Current power state remains corroborated by SSH timeout and Tailscale `Online=False`.


## 2026-10-22T05:31:49Z -- Exact Cargo workspace-test failure captured from delayed process output

`cargo test --workspace` exited 101 after compiling initial dependencies and stopped at `serde v1.0.229`; no tests ran. Cargo could not execute `target\debug\build\serde-e329f3b96886e2fc\build-script-build.exe`: Windows returned OS error 4551 (`应用程序控制策略已阻止此文件`). This directly confirms the Smart App Control / Code Integrity block, not an index download failure. The same process shows only `cargo fmt --all -- --check` as a valid passing Cargo command; no passing Windows workspace tests, Clippy, or release build. Windows was offline at time of receipt, so its log was not remotely changed.


## 2026-10-22T05:34:22Z -- Separate delayed Cargo retry stopped at crates.io index

A later process transcript records `cargo fmt --all -- --check` exit 0, then `cargo test --workspace` stopped at `Updating crates.io index`; outer SSH process exited 1. The supplied output contains no detailed Cargo network error. No compile or tests were reached, and no Clippy or release build was run. Treat this as a distinct incomplete retry, not a replacement for the earlier `cargo test --workspace` exit 101 with the explicit Smart App Control/Code Integrity block on the generated `serde` build script (OS error 4551). Windows is now offline, so its `windows-native-validation-effective.log` was not read back.


## Follow-up: proxy-enabled Windows retry and verified shutdown (2026-10-22T07:10:16Z)

- Timmy reauthorized waking Windows and continuing this VoiceToTextApp work, with proxy use allowed. Target identity was rechecked as `REDACTED-HOST` / `tims-gaming-pc\tgp`.
- A temporary SSH reverse tunnel exposed the NUC Clash proxy at Windows loopback `127.0.0.1:17890` only for the build attempt. A Windows request to `https://index.crates.io/config.json` returned HTTP 200. Windows WinINet stayed disabled (`ProxyEnable=0`); its stored `ProxyServer=127.0.0.1:7897` remained unchanged.
- Fresh Windows frontend verification: `npm run check` exited 0, with 0 Svelte errors and 0 warnings; `npm run build` exited 0 and generated the static site under `apps\desktop\build`.
- Fresh direct Rust checks: `cargo fmt --all -- --check` exited 0. `cargo metadata --no-deps --locked --format-version 1` exited 0 and parsed exactly three workspace packages: `voice-to-text-core`, `voice-to-text-mcp`, `voice-to-text-desktop`.
- With the proxy working, the real `cargo test --workspace --locked` downloaded/compiled dependencies, then exited 101 before tests started: Smart App Control blocked generated build scripts for `parking_lot_core 0.9.12` and `crc32fast 1.5.3`, Windows OS error 4551 (“Application Control policy has blocked this file”). SAC remains enforced and was not changed, bypassed, or exempted. This is separate from—and resolves—the earlier crates.io index connectivity issue.
- Not run: Windows Clippy, release Rust build, Tauri bundle/installer, runtime/UI acceptance, and real ASR. No model weights were downloaded. No successful Windows native application/installer is claimed.
- The Windows-side `C:\Users\REDACTED-USER\VoiceToTextApp\windows-build.log` now has the exact command outcomes and was read back; the full failure is `windows-cargo-test-proxy-verified.log`. Corrections were appended to `windows-native-validation.log` and `windows-native-validation-effective.log` to invalidate old wrapper output that printed Cargo help but reported false zero exits.
- The temporary reverse tunnel was stopped; Windows reported zero listeners on port 17890, no Cargo/Rust build processes, and WinINet proxy disabled. HA `script.pc_shutdown` returned HTTP 200 and `last_triggered` advanced to `2026-10-22T07:07:49.988096+00:00`. After 35 seconds SSH to `REDACTED-TAILSCALE-IP:22` timed out and Tailscale reported `REDACTED-HOST` `Online=False` at `2026-10-22T07:08:10.1Z`. Shutdown is verified.
- Remaining blocker: complete Rust/Windows packaging on a policy-compliant, approved Windows builder or obtain a separately authorized change in build approach. No source was uploaded to external CI; no security-policy change is authorized by the proxy permission.


## Windows build verified via GitHub Actions (2026-10-22T09:24:38Z)

- A cross-platform lock bug found only on Windows was fixed in `crates/transcription-core/src/jobs.rs`: `fs2 0.4.3` reports `LockFileEx` contention as `ERROR_LOCK_VIOLATION` (33) with `ErrorKind::Other`, not `WouldBlock`, so `job_is_live` wrongly treated a live worker as dead. `is_lock_contention` now accepts `WouldBlock` or Windows OS error 33. Added `jobs::lock_error_tests` (the Windows case runs only on Windows).
- Mac regression before push: `cargo fmt --all -- --check` 0; `cargo test -p voice-to-text-core --test job_store` 6/6; `cargo test --workspace` 27 passed, 0 failed, 1 ignored (real ASR, needs a model); strict `cargo clippy --workspace --all-targets -- -D warnings` 0. Clippy's `items_after_test_module` was corrected by moving the test module to the end of the file.
- Committed narrowly and pushed to the private repo `TimmySheep/VoiceToTextApp`:
  - `6459bea` initial 54-file audited source snapshot + Windows CI workflow (no chat archives, media, models, or credentials)
  - `7dea042` Windows lock-contention fix
  - `56cbc0f` correct the MSI verification path (the MSI was already being built at the workspace `target/` directory)
  - `2ff8864` publish the installer as a 7-day artifact
- **Windows CI run 39749144534 (`56cbc0f`) passed end to end on `windows-2022`**: Rust formatting, `cargo test --workspace` (all suites, including the 6 cross-process job-store tests and the Windows lock test), strict Clippy, release workspace build, `npm ci`, `npm run check` (svelte-check 0 errors / 0 warnings), `npm run build`, Tauri `--bundles msi`, and artifact verification.
- **Windows CI run 39752222060 (`2ff8864`) passed and published artifact `voice-to-text-windows`** (id 12415685312, expires `2026-10-29T09:18:35Z`).
- Verified outputs (SHA-256 matches between the CI log and the copy now on the Mac):
  - `VoiceToTextApp_0.1.0_x64_en-US.msi` 16,384,000 bytes, `b2b3716ae360f2dd15dbc027ae0060fcc5d31af9804fa7030630fa95ff2eb693`
  - `voice-to-text-desktop.exe` 25,267,200 bytes, `f5be4f6f38c978209389c8a2b5dae5fcc173e1940e524db3ab15430cce072bc8`
  - Local copy: `/Users/REDACTED/Downloads/VoiceToTextApp-Windows/`
- Still not validated: real speech-to-text inference. No model weights were downloaded and no audio was transcribed, so transcription quality/accuracy is unverified. The installer is unsigned; Smart App Control on a protected Windows device may still block it at run time, which is separate from the CI build result.
- The Windows gaming PC stayed shut down throughout; this build used GitHub-hosted `windows-2022`. No security policy was changed.


## Windows sidecar CRT mismatch (LNK2005/LNK2038) and the toolchain fix (2026-10-25)

- The current `windows-validation.yml` release build failed at the sidecar link step with two coupled errors: `LNK2038` reported `MT_StaticRelease` (in `libllama_cpp_sys_2 rlib(llama.obj)`) versus `MD_DynamicRelease`, and the follow-up `LNK2005` duplicates were all `std` symbols (`msvcprt`/`libcpmt` mixed linkage).
- Root cause: llama.cpp's `cmake_minimum_required(3.14...3.28)` makes policy `CMP0091=NEW`, so CMake selects the MSVC runtime from `CMAKE_MSVC_RUNTIME_LIBRARY` (default `/MD`), overriding the `/MT` flag cmake-rs passes on the command line. Setting only `LLAMA_STATIC_CRT=1` (which `llama-cpp-sys-2-0.1.160/build.rs` reads around lines 501 and 917) therefore did not align the runtime, and run `40180926235` still failed with `LNK2038`.
- Fix (`237f54f` attempt, then `ff175fa`): added `scripts/msvc-static-crt.cmake` setting `CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded$<$<CONFIG:Debug>:Debug>`, injected it via `CMAKE_TOOLCHAIN_FILE` in `.github/workflows/windows-validation.yml`, and added `CXXFLAGS: /MT` so llama.cpp's `common` feature `wrapper_common.cpp` matches. Run `40185381468` (`ff175fa`) linked the sidecar successfully, proving the approach; it then failed only on `cargo fmt` for `llm.rs:293/300`.
- `cargo fmt --all` was applied locally and pushed as `2113292` (only `llm.rs` changed). Run `40186220524` (`2113292`) passed end to end: **first fully green Windows validation since the CRT changes, MSI produced**.
- Not yet validated on Windows: MSI installation and runtime/UI acceptance (SAC blocks unsigned local builds with no interactive login, so acceptance is SSH/CI only).

## MSI install and runtime acceptance on the gaming PC (2026-10-25, run 40190713084)

- Artifact: `voice-to-text-windows` from run `40190713084` (commit `362959c`, contains the CRT fix and the toolbar/model-label UI commit `d160a99`). MSI sha256 `65acfdc59359a5f7e05665d184ef20760ba91d4cb1e6f24212a089fcae8af377`, 30,330,880 bytes.
- Upgrade path: previous install was 1.3.0.1 at `C:\Program Files\Local Transcribe\` (product code `{76DCDFD1-E361-4DA7-A527-CCE2D2E63B18}`). `msiexec /i ... /qn /l*v` from SSH (user `TGP`) exited 0; registry now reports DisplayVersion `1.4.0.1` (WiX four-part of 1.4.0-beta.1) and both `voice-to-text-desktop.exe` + `local-transcribe-node.exe` are in place. **SAC did not block the CI-built MSI** — the earlier SAC block applied to unsigned *locally built* executables.
- Direct launch from the SSH session 0 panicked in WebView2 (`HRESULT 0x80070578`, invalid window handle) — expected: session 0 has no interactive desktop. Launched instead via a scheduled task with `LogonType Interactive` under the console user `TGP` (console session 1, logged in since 20:46).
- Runtime evidence in the interactive session: `voice-to-text-desktop.exe` PID 21292, `Responding=True`, ~30 MB working set, real top-level window (class `Tauri Window`, title `Local Transcribe`) restored from minimized to 611x439 at (895,160); four `msedgewebview2.exe` child processes running. `local-transcribe-node.exe` was not running — it is spawned on demand by the supervisor, not at GUI startup.
- Screenshot caveat (verified, not assumed): full-screen captures showed a solid `#012456` rectangle — sampled and identified as the capture task's own PowerShell/Windows Terminal background, not the app (the app palette has no such color). The app window was minimized at that moment (its rect was the 237x39 minimized thumbnail rectangle). After restoring the window and waking the display, the screen showed the user actively playing a fullscreen game, so no further screenshot was taken to avoid interfering. Visual UI acceptance on Windows therefore remains open; process/window/WebView2 health is verified.
- No security policy was changed; the machine's interactive session was in active use (a fullscreen game) — window restore and display wake were the only interactions.
