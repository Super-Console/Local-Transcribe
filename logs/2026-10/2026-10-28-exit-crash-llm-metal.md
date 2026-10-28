# 退出应用时 SIGABRT（ggml Metal 资源未释放）修复

日期：2026-10-28；执行者：OpenCode；对象：Local Transcribe 桌面端（macOS 与 Windows 共用退出路径）。

## 现象与证据

- Timmy 反馈退出应用时崩溃两次：`voice-to-text-desktop-2026-10-28-160644.ips` 与 `…-164749.ips`，崩溃 UUID 均为 `4C64878C-D3DC-3E5D-9A33-F22FE8B3F8F4`。
- 主线程链路：`exit()` → `__cxa_finalize_ranges`（C++ 静态析构）→ ggml Metal device 析构 → `ggml_metal_rsets_free` 的 `GGML_ASSERT([rsets->data count] == 0)`（`ggml-metal-device.m:1040`）→ `abort`。
- 16:47 报告里线程 `llm-5e1564c6` 仍在 Metal `waitUntilCompleted` 上生成；16:06 报告无生成线程，但模型仍在驻留。两者签名相同。

## 根因

- Tauri 的 macOS 事件循环结束时，tao `run()`（tao-0.37.2 `platform_impl/macos/event_loop.rs:202`）在 `run_return` 返回后直接调用 `std::process::exit(exit_code)`。
- `process::exit` 不运行 Rust 析构函数，所以 `RunEvent::Exit` 之后托管的 `Arc<LlmRuntime>`、`LoadedModel`（`LlamaModel` + `LlamaBackend`）从未 drop，Metal buffer 依然登记在 device 的 residency sets 中。
- 进程真正退出时 libc 会运行 `__cxa_finalize_ranges`，llama.cpp/ggml 的静态析构释放 Metal device 时断言 rsets 已空，于是 SIGABRT。ggml 源码注释原文即说明该断言含义为「退出前没有释放全部 Metal 资源」。
- Windows 侧 tao `run()` 同样以 `std::process::exit` 结束（tao `platform_impl/windows/event_loop.rs`），路径同构；但 `llama-cpp-2` 只在 macOS 启用 `metal` feature，Windows 为 CPU 后端，不会触发该断言。两端残留模型未释放的问题一致。

## 修复

- `apps/desktop/src-tauri/src/llm.rs`：新增 `shutdown_for_exit(app)`，先 `cancel_generation()` 再 `unload()`；`unload()` 会阻塞等待生成线程释放 operation 锁，随后同步 drop 模型与 backend，Metal residency sets 归零。
- `apps/desktop/src-tauri/src/lib.rs`：`RunEvent::Exit` 在停止 HostState 之前调用 `llm::shutdown_for_exit(app)`。该事件由 tao 在 `process::exit` 之前触发，是唯一可靠的清理点。
- 新增开发依赖 `tauri`（`test` feature，无额外依赖）用于 MockRuntime 单元测试。

## 测试与验证

- TDD：先写 `exit_shutdown_cancels_the_generation_and_unloads_the_model`，以空实现运行并确认失败（`exit must cancel the active generation`），实现后通过。
- 机制级复现（临时探针 `crates/lan-compute/examples/exit_probe.rs`，验证后已移入 `~/.Trash/local-transcribe-exit-probe-20261028/`），使用本机真实 `Qwen3-1.7B-Q8_0.gguf`：
  - `legacy`（加载后直接 `process::exit`）：`ggml-metal-device.m:1040` 断言失败，exit 134，与两份崩溃报告一致。
  - `fixed`（先 cancel + unload 再 exit）：exit 0，无断言。
  - `generating-fixed`（生成线程仍在解码 token 时退出）：worker 被取消（`Ok(false)`），exit 0，无断言。
- 门禁：`cargo fmt --all -- --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`npm run check`（0/0）、`npm test`（119/119）。
- 一次并行执行 `npm test` 与 `npm run check` 时 `reader-spec` 因 Chrome 启动竞态报 `Inspected target navigated or closed`；串行重跑 4 次均通过，与本次改动无关。

## 版本与构建

- 版本升至 `1.5.0-beta.5`（WiX `1.5.0.5`）：`apps/desktop/package.json`、`package-lock.json`、`tauri.conf.json`、桌面与 LAN crate `Cargo.toml`、`Cargo.lock`、`apps/desktop/tests/sidebar-shell.test.mjs`。
- macOS 27 / Xcode 27 / rustc 1.97.1 下 release 构建复现已知缺陷：proc-macro dylib 被 strip 后报 `mis-aligned LINKEDIT string pool`，以及链接缺 `___isPlatformVersionAtLeast`。沿用上次发布验证过的环境级参数（不改全局设置、不清缓存）：`CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false` 与 `RUSTFLAGS="-C link-arg=-L<Xcode clang 21 darwin 目录> -C link-arg=-lclang_rt.osx"`。

## Windows 真机验证（Tims-Gaming-PC）

- 该机已有 rustc 1.99.1、Node 24、VS 2022 BuildTools（含 CMake）、sherpa-onnx 离线包 `C:\Users\TGP\lt-sherpa`；缺少 bindgen 需要的 `libclang`，经 Timmy 同意后用 `winget install LLVM.LLVM` 安装 LLVM 23.1.4（未改动其他全局设置）。
- GitHub 在该机直连被重置，改用 `git archive` 源码包 scp 同步至 `C:\Users\TGP\lt-beta5-check`。
- 首次运行在 `cargo test -p voice-to-text-desktop --lib` 前构建 sidecar 时失败：sherpa 下载被阻断、libclang 缺失；改用 `SHERPA_ONNX_ARCHIVE_DIR` 指向本地离线包并设置 `LIBCLANG_PATH` 后成功。
- 结果：`cargo build -p voice-to-text-lan --release --bin local-transcribe-node` 成功（仅有既存的 LNK4098 CRT 混用警告）；`cargo test -p voice-to-text-desktop --lib` **68 passed / 0 failed，exit 0**，CI 上曾出现的 `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` 未再出现。

## 发布与部署

- 发布提交 `e639bc7`（`main`），annotated tag `1.5.0-beta.5` 指向该提交，已推送。
- Windows CI run `40677166209` 在该提交全绿（node sidecar、fmt、workspace tests、strict Clippy、release build、npm check、onboarding 测试、前端 build、MSI 构建与上传）。
- GitHub Pre-release：<https://github.com/Super-Console/Local-Transcribe/releases/tag/1.5.0-beta.5>；资产为 macOS DMG、Windows MSI 与 `SHA256SUMS`。
- 资产回下载校验：`shasum -a 256 -c SHA256SUMS` 两项 OK，`hdiutil verify` 通过。DMG `dc9bbaeee93f5edaf5d57ee02fe1246a76f4803be987cf3156620e8c70ed7850`；MSI `1a89bc1d767774e244e171289feef092b856c39af8ff877e15b3911692f2cbfe`。
- MacBook Pro：从发布 DMG 安装 `~/Applications/Local Transcribe.app`，版本 `1.5.0-beta.5`；主程序 `b66c043190d73c4d068fcb4e77d8072600b602ea562698eeddb2efb7f2b4ff47`，node `f3260b66a4d0b389a0cc481bdd17e6b090179e730f88a5e8cdd43eae2d5dc466`；启动与退出正常，无新崩溃报告；`mdfind` 只返回安装路径。
- MacBook Air：从同一 DMG 安装同一路径，版本与两个哈希均与 MBP 一致；旧 beta.4 安装移入 `~/.Trash/local-transcribe-1.5.0-beta.5-upgrade-20261028-203716/`；通过 AppleScript 在 GUI 会话启动成功（SSH 会话直接 `open` 无法进入 GUI 会话），退出干净、无崩溃报告；`mdfind` 返回本机安装路径与 SMB 挂载的 Pro 安装路径（同一新版本，非旧版）。
- 清理：MBP 旧安装与构建副本移入 `~/.Trash/` 下 `local-transcribe-1.5.0-beta.5-upgrade-20261028-185931`、`local-transcribe-1.5.0-beta.5-rebuild-20261028-191022`、`local-transcribe-build-copy-20261028`、`local-transcribe-build-copy-20261028-2`；临时探针源码移入 `local-transcribe-exit-probe-20261028`。回收站未清空。
- Windows 主机只做了源码构建与测试验证，未安装本次 MSI（Timmy 本次只要求两台 Mac 安装）；Windows 的 LLM 退出路径为 CPU 后端，未做 GUI 退出实测。
