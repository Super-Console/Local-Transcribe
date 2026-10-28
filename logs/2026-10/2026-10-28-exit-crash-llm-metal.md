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

## 发布与部署

（本节在发布会话结束时追加。）
