# 两台 Mac 重构建安装与旧备份清理（2026-10-23）

## 请求与澄清

Timmy：「在我的 MacBook Pro 和我的 air 上都安装一份最新版，并且删除掉旧版」。

只读取证后发现两台机器当时已装同一构建 `1.1.0-beta.3`（`402df46e…a05b8`），但工作区在 16:31 又改过 `apps/desktop/src-tauri/src/lan.rs`、`crates/lan-compute/src/node.rs`、`apps/desktop/src/lib/types.ts`，未提交也未构建。经询问确认：

- 「最新版」= 用当前工作区源码重新构建后安装到两台，而不是沿用已装的 `402df46e…a05b8`。
- 「旧版」= 两台机器 `~/Applications/Local Transcribe Backups/` 下现有备份全部清理（MBP 6 个约 154 MB，Air 3 个约 74 MB）。

## 构建与自动化验证（MBP，本轮工作区）

- `cargo test --workspace --offline`：全部 test result 为 ok，0 failed；需真实权重/接口的用例仍标 ignored（未声称跑了真实推理）。
- `cargo clippy --workspace --all-targets --offline -- -D warnings`：通过；`cargo fmt --all -- --check`：通过。
- `npm test`（apps/desktop）：9/9 通过；`npm run check`：0 errors / 0 warnings；`npm run build` 通过。
- `CARGO_NET_OFFLINE=true npm run tauri -- build --bundles app --no-sign`：release 编译成功，bundle `26.02 MiB`，`CFBundleShortVersionString` = `1.1.0-beta.3`，adhoc linker-signed（未签名/未公证）。
- 新可执行文件 SHA-256：`854e59c7ae625042437e48542089c7fe8748b6323202cb5ca0723aaa6626dd18`。
- 未提交 Git、未移动 tag、未发布。

## 旧备份清理（按授权，仅移入废纸篓）

- MBP：`before-beta3-20261023-154506`、`before-dragfix-20261023-152953`、`before-lan-sharing-20261023-133520`、`before-sidebar-20261023-114123`、`before-traffic-alignment-20261023-114547`、`before-trusted-lan-20261023-162118` → `~/.Trash/local-transcribe-old-backups-20261023-1750/`，保留原名称。
- Air：`before-beta2-20261023-122557`、`before-lan-sharing-20261023-142456`、`before-trusted-lan-20261023-162301` → `~/.Trash/local-transcribe-old-backups-20261023-1750/`。
- 全部为 `mv`，未使用 `rm`，未清空废纸篓；清理后两机 `Local Transcribe Backups/` 已为空。

## 两机安装与验证

- MBP：旧包（`402df46e…a05b8`）移入 `Local Transcribe Backups/before-rebuild-20261023-1752/` 作为本次回滚点，`ditto` 安装新包；安装后哈希 `854e59c7…6dd18` 与构建产物一致。`open` 后进程 `73951` 来自 `~/Applications/Local Transcribe.app`，Cua 截图确认窗口正常渲染（侧栏、首页、历史记录、模型选择均在）。
- Air（SSH `airm1`）：暂存 `/tmp/lt-install-20261023`，暂存哈希与构建产物一致；安装前 app 未运行，旧包移入同机 `before-rebuild-20261023-1752/`，`ditto` 安装后哈希 `854e59c7…6dd18`，`open` 后进程 `19629` 可执行路径正确。Air GUI 交互未验证（现有 CuaDriver 缺 Screen Recording 权限，未绕过）。
- 用户设置、转写历史、录音与模型库均未改动。

## 未完成 / 注意

- 未重跑真实 ASR 推理，未做两台机器间局域网端到端转写，Windows 本轮未验证。
- 两台各保留一个回滚备份 `before-rebuild-20261023-1752`（内容为被替换的 `402df46e…a05b8`），可另行确认后清理。
- Air 暂存目录 `/tmp/lt-install-20261023`（约 26 MB）待 Timmy 批准后清理。
