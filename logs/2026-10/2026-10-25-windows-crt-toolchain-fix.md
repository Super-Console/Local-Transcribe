# Windows CI 静态 CRT 不匹配修复（LNK2005/LNK2038）

日期：2026-10-25
对象：本项目的 GitHub Actions Windows 验证流水线（`.github/workflows/windows-validation.yml`）
执行者：OpenCode（MiMo）会话，经 Timmy 授权的修复任务

## 事实（已核实）

- 症状：sidecar 链接步骤失败，`LNK2038` 报 `MT_StaticRelease`（`libllama_cpp_sys_2 rlib(llama.obj)`）与 `MD_DynamicRelease` 不匹配；随后 `LNK2005` 重复符号全部是 `std` 符号（`msvcprt`/`libcpmt` 混链）。
- 根因：llama.cpp 的 `cmake_minimum_required(3.14...3.28)` 使 `CMP0091=NEW`，CMake 由 `CMAKE_MSVC_RUNTIME_LIBRARY`（默认 `/MD`）决定运行库，覆盖 cmake-rs 传入的 `/MT`；因此仅设置 `LLAMA_STATIC_CRT=1` 无效（`llama-cpp-sys-2-0.1.160/build.rs` 第 501、917 行读取该变量）。
- CI run 序列：
  - `40178996567`（无修复）→ 失败，LNK2005/LNK2038。
  - `40180926235`（`237f54f`，仅 `LLAMA_STATIC_CRT=1`）→ 仍 LNK2038，证明该变量不足以对齐运行库。
  - `40185381468`（`ff175fa`）→ 链接通过（方案生效），但 `cargo fmt` 检查失败（`llm.rs:293/300`）。
  - `40186220524`（`2113292`，本机 `cargo fmt --all` 后仅含 `llm.rs` 的格式化提交）→ **全绿，产出 1.4.0-beta.1 MSI**。CRT 改动以来首个完全通过的 Windows 验证。

## 修复内容

- 新增 `scripts/msvc-static-crt.cmake`：设 `CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded$<$<CONFIG:Debug>:Debug>`。
- `.github/workflows/windows-validation.yml`：注入 `CMAKE_TOOLCHAIN_FILE` 环境变量 + `CXXFLAGS: /MT`（对齐 common feature 的 `wrapper_common.cpp`）。
- `cargo fmt --all` 修正 `llm.rs`（提交 `2113292`）。

## 提交链

`237f54f`（无效尝试）→ `ff175fa`（toolchain 修复，链接已通）→ `2113292`（fmt）→ `d160a99`（工具栏/模型页 UI，另见同日 UI 日志）。

## 详细记录

完整技术记录见 `docs/windows-build-log.md` 的「Windows sidecar CRT mismatch」小节。

## 未验证 / 阻塞

- ~~Windows MSI 安装与运行时验收未做~~ → 已于同日完成安装与运行时验收，见下节。

## 验收补记（2026-10-25 晚，run 40190713084 / commit 362959c）

- MSI（sha256 `65acfdc5…`）经 SSH 上传至 win（100.94.223.63，用户 TGP），`msiexec /i /qn` 退出码 0，注册表 DisplayVersion 由 1.3.0.1 升至 1.4.0.1，两个 exe 就位。**SAC 未拦截 CI 构建的 MSI**（此前拦截的是本机未签名构建）。
- SSH 会话 0 直接启动会因无交互桌面在 WebView2 处 panic（`0x80070578`，预期内）；改用 `LogonType Interactive` 计划任务在 tgp 控制台会话启动成功：进程 `Responding=True`、真实窗口（class `Tauri Window`）存在、4 个 WebView2 子进程运行。
- 截屏曾显示纯色 `#012456`，经采样比对确认为截屏任务自带 PowerShell 窗口背景（应用色板无此色），应用窗口当时处于最小化；恢复窗口、唤醒屏幕后机器正在被 Timmy 全屏游戏中，遂停止继续截屏以免打扰。**Windows 视觉 UI 验收仍开放**，进程/窗口/WebView2 健康已验证。
- 临时产物（Windows 端）：`C:\Users\TGP\LocalTranscribe-1.4.0-beta.1.msi`、`lt-*.ps1/txt/png`、计划任务 `LT-*` 若干，待 Timmy 确认后清理。
