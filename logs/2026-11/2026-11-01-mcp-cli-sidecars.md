# 1.6.0：MCP 与 CLI 随 App 一起安装

日期：2026-11-01
项目：VoiceToTextApp（Local Transcribe）
执行：OpenCode（deepseek-flash）

## 起因

用户要求：装完 GUI 之后 MCP 与 CLI 开箱即用，不要让用户自己 `cargo build`；同时把 beta 去掉，发布 1.6.0 正式版，并在 MacBook Pro、MacBook Air 安装，Windows 侧同样改进。

## 先核实（未假设）

- `crates/transcription-cli` 有 501 行实现（`local-transcribe`），`crates/transcription-mcp` 1074 行、13 个工具，`crates/transcription-service` 为共享服务（Unix socket 0600 + fs2 单实例锁）。
- `docs/architecture.md:11` 的 “The CLI is not implemented.” 是 2026-10-22 的过期快照；README 的三入口描述与代码一致。
- MCP/CLI 自身不推理，会 spawn 同目录的 `voice-to-text-service`；因此 sidecar 必须三个一起打包，只打 MCP 会连不上执行者。
- 本机实测（改动前）：`~/Applications/Local Transcribe.app` 1.5.0-beta.5，`Contents/MacOS` 只有 `local-transcribe-node` 与 `voice-to-text-desktop`；`target/` 无 release；PATH 无 `voice-to-text-*`。
- 现有设置页的“复制 MCP 配置”用占位符并默认带 `--allow-root`，与需求相反，改为后端生成真实配置。

## 体积核实（纠正先前估算）

先前凭印象估“每个二进制 30 MB、总共 +90~120 MB”，实测后纠正：

| 二进制 | 大小 |
|---|---|
| `voice-to-text-service` | 29 MB（唯一链接 sherpa-onnx） |
| `voice-to-text-mcp` | 2.1 MB |
| `local-transcribe` | 598 KB |

增量约 32 MB。实际 app 包 114.62 MiB（本机 release 必须 `CARGO_PROFILE_RELEASE_STRIP=none`，符号未剥离），DMG 40 MB。

## 代码改动

- 新增 `crates/transcription-service/src/sidecar.rs`：`sidecar_dir()`（canonicalize 当前可执行文件，保证 `~/.local/bin` 软链也能找到 App 包内兄弟程序）、`bundled_binary()`、`expand_home()`、`render_mcp_config()`（默认 `args: []`，root 必须绝对路径）、`mcp_config_note()`；5 个单元测试。
- CLI 新增 `mcp-config` 子命令与 `--allow-root DIR`（支持 `~`），stdout 纯 JSON、说明走 stderr；`LOCAL_TRANSCRIBE_MCP` 供开发/测试覆盖路径。测试 `crates/transcription-cli/tests/mcp_config.rs`。
- 桌面端新增 `apps/desktop/src-tauri/src/mcp_cli.rs`：`mcp_config`、`cli_tool_status`、`install_cli_tool`、`uninstall_cli_tool`。macOS 建软链 `~/.local/bin/local-transcribe`→App 包内 CLI；Windows 写 `%USERPROFILE%\.local\bin\local-transcribe.cmd` 垫片。已存在且不属于本应用的条目不覆盖；卸载只移除自己的条目并移入回收站（Windows 为真实回收站）。5 个单元测试。
- `SettingsView.svelte`：替换占位配置为 `invoke("mcp_config")` 的真实文本 + 复制；新增“命令行工具”安装/卸载开关与 PATH 提示。i18n 五种语言同步，删除四个占位键；`settings.mcpDesc` 由 9 工具改为 13，`aboutDesc` 去掉“预览版本”。
- `tauri.conf.json` `externalBin` 增加三个 sidecar；`bundle:mac` 先构建再 stage 四个带 triple 的文件，`tauri build` 后逐个签嵌套 Mach-O，最后签 app。
- Windows CI：`Build and stage sidecars` 构建并 stage 四个 `*.exe`，产物清单加入 `voice-to-text-mcp.exe`。
- 版本 1.6.0：`apps/desktop/package.json`、`package-lock.json`、`tauri.conf.json`（含 WiX `1.6.0.0`）、桌面与 LAN crate `Cargo.toml`、`Cargo.lock`、`sidebar-shell.test.mjs` 夹具。
- 文档：README/README.zh-CN 新增“MCP 与 CLI 随 App 一起安装”，`docs/architecture.md` 更正 CLI 状态与两条 invariant，`docs/mcp.md` 增加安装路径、`mcp-config`、1.5.x→1.6.0 兼容说明。

## 构建绕行（本机）

- `sherpa-onnx-sys` 不认 `ALL_PROXY=socks5://`，且本仓库 `target/sherpa-onnx-prebuilt/` 为空；改用同机另一个工作区的 v1.13.8 归档作为 `SHERPA_ONNX_ARCHIVE_DIR`，构建后缓存已补回本仓库。
- 按 `docs/local-build.md` 使用 `CARGO_PROFILE_RELEASE_STRIP=none` 与 `-lclang_rt.osx` 链接参数。

## 验收（实测）

1. `cargo build --release -p voice-to-text-mcp` 出产物（`target/release/voice-to-text-mcp`，2.1 MB）。
2. 打包后运行包内 MCP：`initialize` 成功，13 个工具；`list_models` 返回 GUI 选中的 FireRedASR2、paraformer、zipformer（`libraryPath` 指向共享数据目录）。
3. 对 `recordings/1792690744582/track-1.wav` 用 paraformer 跑 `transcribe_file`→`get_job_status`→`get_job_result(includeText=true)`，得到 `一二三一二三的这个测试音频啊测试一下你的这个能力`，与 GUI 同文件同模型记录完全一致。
4. 不带 `--allow-root` 调用 `transcribe_file`：`MCP file access is disabled. Start the server with --allow-root directories.`（isError）。
5. `local-transcribe mcp-config` 输出可解析 JSON，`command` 指向存在的二进制，`args: []`；GUI“复制配置”写入剪贴板的文本与其一致。
6. `codesign --verify --deep --strict` 通过（含四个嵌套 Mach-O）；`spctl -a` 显示 accepted，但带 `override=security disabled`——本机安全评估被关闭，且构建为 ad-hoc 签名、无公证，不能算公证通过。

GUI 内还实测：设置页显示真实路径与 13 工具描述；点“安装”生成软链，`which local-transcribe` 与 `local-transcribe status` 正常工作；`pbpaste` 得到的配置与 CLI 输出一致。

## 已知行为

- 共享服务由第一个启动它的客户端确定 `--allow-root` 集合；后到的 MCP 客户端若请求别的目录，会复用已在运行的服务并被拒绝，需先结束该服务。
- `punctuation-ct-transformer` 在数据目录里是 0 字节空目录，`list_models` 按结构校验不列出，属预期。
- MacBook Air 上 `models list` 返回 `selection_error`：该机 `selected-model.json` 不存在、模型目录为空，是“尚未选模型”的真实状态，不是本次改动引入。
