# Windows 本机验证 1.1.0-beta.2（含跨平台测试修复）

时间：2026-10-23（北京时间 12:20–13:10 前后）
执行：OpenCode（MiMo V2.6 Pro）；目标机 Timmy 的 Windows 电脑（SSH 别名 `win`，用户 `TGP`）。
范围：beta.2 发布资产的 Windows 本机验证、`npm test` CRLF 失败修复（TDD）、依赖核查与项目本地依赖安装。
不含：真实模型转写（模型权重未下载，需另行批准）、最终产品名/视觉定稿。

## 环境（已核实）

- Windows 11 专业版 build 26300，x64，C: 剩余约 360 GB。
- 已有工具链（未新装全局依赖）：Node v24.20.0 / npm 11.19.0、rustc/cargo 1.99.1（stable-x86_64-pc-windows-msvc）、Git 2.55.0.windows.5、VS Build Tools 2022 17.14.42（MSVC 14.44.35207）、Windows SDK 10.0.26100.0、WebView2 运行时。
- Smart App Control 处于强制状态（Code Integrity 策略 `VerifiedAndReputableDesktop`，`0283ac0f-fff1-49ae-ada1-8a933130cad6`）。本次未做任何策略修改或绕过。
- 测试源：从公开 GitHub tag `1.1.0-beta.2` 克隆到 `C:\Users\TGP\VoiceToTextApp-beta2-validation`，HEAD `b233bc823eff9213474a6163a044240a5b889a5a`，无禁止跟踪文件（录音/模型/聊天归档/`.DS_Store` 均为 0）。

## 依赖安装记录（按规则先查后装）

- 检查项：`node/npm/rustc/cargo/git --version`、vswhere、Windows SDK Lib 目录、WebView2 目录、Cargo registry 缓存、`node_modules` 是否存在。结论：系统级构建工具链全部已存在，**没有安装任何全局工具**。
- 唯一安装：测试克隆 `apps\desktop` 内按 `package-lock.json` 执行 `npm ci`（项目本地，61 个包）。安装前后 `package-lock.json` SHA-256 一致（`578cad0cdd4622745e1ccb5662571a838f351d38ae5d184d23fecf9c23dd3132`），Node v24.20.0 / npm 11.19.0。npm 报告 3 个 low severity vulnerabilities（仅审计提示，未执行 `npm audit fix`）。
- sherpa-onnx 原生静态库：按 `docs/mcp.md` 机制由 `sherpa-onnx-sys` 构建脚本自动下载官方 release 资产（原生运行库，非模型权重）；本次因 SAC 拦截未走到下载完成。

## 测试修复：`npm test` 在 Windows 的 4/5 失败（TDD）

- 根因：Windows Git 默认 `core.autocrlf=true`（`git ls-files --eol` 显示 `i/lf w/crlf`），`Cargo.lock` 工作区副本为 CRLF；`desktop release versions stay in sync` 用 `\n` 字面匹配版本行导致唯一失败（macOS LF 检出 5/5 通过）。
- TDD 过程：先抽取 `desktopLockPattern(version)` 并新增 LF/CRLF 双夹具回归测试 `release version check accepts LF and CRLF Cargo.lock checkouts`，确认预期失败（仅 CRLF 夹具失败，报错与 Windows 实机一致）；随后最小修复为 `\r?\n`，不改产品代码、不改 Git 全局/仓库换行策略。
- 修复后验证：macOS 定向 + 全量 `npm test` 6/6；`npm run check` 0 errors / 0 warnings；`npm run build` 通过。Windows（同一测试文件 SHA-256 `e9bab5379733937f4735a6eb7810a1c6b434f66342f4579f28bf970fc984b1f5` 双端一致）定向 + 全量 `npm test` 6/6、`npm run check` 0/0、`npm run build` 通过。
- 改动文件：`apps/desktop/tests/sidebar-shell.test.mjs`（测试专用）。按项目规则未创建 git commit；工作区另有并行改动（LAN Compute 相关）非本任务产生，未触碰。

## Rust 本机验证：受 Smart App Control 阻断（未绕过）

- `cargo fmt --all -- --check`：通过。
- `cargo test --workspace --locked`：退出码 101，编译早期即失败；`proc-macro2`/`quote`/`serde`/`icu_*` 等 build-script 可执行文件被应用控制策略阻止（os error 4551「应用程序控制策略已阻止此文件」）。CI 事件 3033/3077（12:58 前后）与 2026-10-22 记录的根因一致。
- 结论：Windows 本机 Rust 测试/Clippy/release 构建记为**受阻**；真实 Windows 的 Rust 验证由 GitHub Actions `windows-2022` 承担（run `39899731019` 全绿：workspace tests、严格 Clippy、release build、MSI 打包）。Microsoft 文档确认 SAC 无逐应用豁免；不关闭、不绕过。

## MSI 本机安装与启动冒烟

- 安装包：发布资产 `Local Transcribe_1.1.0-beta.2_x64_en-US.msi`（17,903,616 字节）原样传到测试目录，SHA-256 `258bdfd674c782d3f6db4da168d5642c30dae0b68392884ad450957e214b36f9` 与 CI staging、GitHub release 一致。
- 安装：`msiexec /i ... /qn /norestart /l*v`，退出码 0；升级 `Local Transcribe 1.0.0` → `1.1.0.2`，位置 `C:\Program Files\Local Transcribe\`，MsiInstaller 事件 11707/1033 记录安装成功。
- 安装后回读：`voice-to-text-desktop.exe` 27,587,072 字节，SHA-256 `1129ae545e224f5de89a36da2bfed6464499d18e31a9d56a644b68410b9c70bf`，`Get-AuthenticodeSignature` = NotSigned。
- 哈希差异已查清：与 CI 独立暂存的 exe（SHA-256 `422ff848b5b6d85ae17b34a7c12ffe6694141a35003e9ab993a3b22d8c15e4aa`）逐字节比对**仅 3 字节不同**（偏移 0x1497F1F–0x1497F21）：`_TAURI_BUNDLE_TYPE_VAR_UNK` → `_TAURI_BUNDLE_TYPE_VAR_MSI`，是 Tauri 打包器在 MSI 构建时写入的打包类型标记，属良性差异；MSI 内即 beta.2 原始二进制。
- 启动冒烟：进程未启动。CI 事件 3033/3077（13:04:15）明确记录 `powershell.exe` 尝试加载 `C:\Program Files\Local Transcribe\voice-to-text-desktop.exe` 未满足企业签名级别要求，即被 Smart App Control 阻止。
- 结论：**未签名 beta.2 在 SAC 强制的 Windows 11 上「可安装、不可运行」**；GUI/ASR 本机验收在该机受阻。若要在 SAC 保护的 Windows 分发，需受信任的代码签名路径（涉及签名证书/付费服务，须 Timmy 另行决策）。

## 未完成 / 待决策

- Windows GUI 与真实 ASR 验收：受 SAC 阻断，未完成；不以安装成功冒充验收。
- 真实模型转写：模型权重未下载（任务书要求 Timmy 批准具体模型/来源/许可/体积后再下载）。
- MacBook Air 侧 GUI/真实 ASR 验收仍待 Timmy 打开应用反馈。
- 测试修复是否提交/推送、是否在 GitHub release notes 增补 SAC 实测限制，待 Timmy 决策。

## 清理结果（2026-10-23，经 Timmy 批准「全部清理」）

- Mac：`beta2-windows-artifact-39899731019/`、`SHA256SUMS-beta2.txt`、`release-1.1.0-beta.2.md` 已 `mv` 至 `~/.Trash/opencode-beta2-cleanup-20261023-1324*/`，未永久删除。
- Windows：`.git`（改名为 `.git-to-recycle`）与空的测试克隆目录已通过 Shell「回收站」命名空间投递并验证落箱（条目 `$I/$R6AH0KN`、`$I/$RZ8ZSYT`）。
- **偏离规则（如实记录）**：第一批内容（克隆内 apps/、node_modules、target、传入的 MSI/exe 副本等）与 MSI 安装日志调用 `Microsoft.VisualBasic.FileIO.FileSystem::…SendToRecycleBin` 后原位置消失，但回收站两个 SID 目录及近 2 小时条目均无对应记录——该 API 在非交互 SSH 会话中静默变成了永久删除，不符合「删除必须走回收站」。内容全部可再生（公开 tag、GitHub release、日志要点已存档），无用户数据损失。后续删除已改用 Shell `Namespace(10).MoveHere` 并逐项验证落箱。
- 保留：`C:\Program Files\Local Transcribe\`（1.1.0.2 安装本体）。

## 相关路径

- 测试克隆：`C:\Users\TGP\VoiceToTextApp-beta2-validation`（含下载的 MSI 与 artifact exe 副本）。
- MSI 安装日志：`C:\Users\TGP\AppData\Local\Temp\local-transcribe-beta2-msi-install.log`。
- 机器侧维护日志：`/Users/TimmySheep/AWS/CM/logs/Personal Computers/Windows电脑/2026-10/2026-10-23-Win-VoiceToTextApp-beta2本机验证.md`。
