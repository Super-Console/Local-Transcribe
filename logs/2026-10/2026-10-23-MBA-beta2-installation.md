# MacBook Air 安装 Local Transcribe 1.1.0-beta.2 供验收

## 授权与范围

2026-10-23，Timmy 要求将当前 macOS Apple Silicon 测试包安装到 MacBook Air 供验收。仅安装应用，不改用户数据、不绕过 Gatekeeper、不提交 Git。

## 目标与安装

- 通过已配置 SSH 别名 `airm1` 只读核验目标：MacBook Air M1、arm64、macOS 27.0.1；安装前旧 `~/Applications/VoiceToTextApp.app`（0.1.0）未运行，目标 `~/Applications/Local Transcribe.app` 不存在。
- 源文件：`target/release/bundle/dmg/Local.Transcribe_1.1.0-beta.2_aarch64.dmg`，SHA-256：`9ada742f80031c52293a39f9cbfef47a86338cb5c03f228233d2d59cf9724e29`。以只读方式挂载；包内应用版本/build 均为 `1.1.0-beta.2`，bundle ID `com.timmy.voicetotextapp`，最低系统版本声明 14.2。
- 将应用传至 MBA 暂存目录并核对；可执行文件、Info.plist、图标哈希与源包一致，arm64 主可执行文件 SHA-256：`0682bd3c57ee771ebc095081dbb1eccdad6de9bd4ae4068e7ea9377bd36f520f`。执行文件签名为 ad-hoc，未设置 Team ID。
- 保留旧应用至 `~/Applications/Local Transcribe Backups/before-beta2-20261023-122557/VoiceToTextApp.app`，再将 beta.2 安装到 `~/Applications/Local Transcribe.app`。未删除旧应用，也未触碰其数据。

## 验证与限制

- MBA 回读确认安装版本/build、bundle ID、最低系统版本和 arm64 架构正确；安装后精确 `pgrep -x voice-to-text-desktop` 返回无运行进程，旧版备份仍为 0.1.0。
- 应用未从远程 SSH 会话启动；本轮没有完成 GUI 交互或真实模型转写验收。包没有 Developer ID 签名或公证，首次打开可能受 Gatekeeper 限制；未移除隔离属性或尝试绕过安全策略。
- 项目 `logs/README.md` 不存在；已阅读项目 `AGENTS.md` 并按现有 `logs/2026-10/` 分类记录。

## LAN 共享版更新（2026-10-23 14:25，Timmy 授权）

- Timmy 要求“帮我在我的 air 也安装一下”，即把本轮 macOS 局域网共享版部署到 MBA。预检（只读）：SSH 别名 `airm1` 可达；macOS 27.0.1、arm64；当前安装为当天上午的 beta.2（exe SHA-256 `0682bd3c…f520f`），未运行，应用数据目录无任务文件（无进行中任务）；备份目录已存在。
- 传输：tar-over-SSH 传到远端暂存 `~/.lt-install-20261023-142456/`，回读 exe SHA-256 为 `4953c7495be880339101d4cd063627c5937fc373c583402634aee5e9a18592d3`，与本机源包一致；版本 1.1.0-beta.2；xattr 仅 `com.apple.provenance`，无 quarantine 属性。
- 换入：旧版 `mv` 到 `~/Applications/Local Transcribe Backups/before-lan-sharing-20261023-142456/Local Transcribe.app`（回读哈希仍为 `0682bd3c…f520f`，确认上午版本完整保留）；新版 `mv` 入正式位置后再回读哈希一致；空暂存目录 `mv` 到远端废纸篓 `~/.Trash/lt-staging-emptied-20261023-142456`。全程无 `rm`、无永久删除，未触碰 Air 上的用户数据或任何设置。
- 启动验证：`open` 精确路径启动，8 秒后 `pgrep` 确认进程存活（PID 885），`lsof` 确认可执行文件来自安装位置，近 3 分钟 DiagnosticReports 无崩溃记录。至此本机与 Air 均运行同一构建（`4953c749…592d3`）。
- 限制：无法从本机对 Air 做 AX/GUI 交互验证，窗口级确认待 Timmy 在 Air 上查看；应用仍为 ad-hoc 签名、无公证；共享本机算力开关在两台机器上均保持默认关闭；两台设备之间的实际配对/上传/回退仍未做端到端实测。
