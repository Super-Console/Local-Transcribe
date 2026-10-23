# 当前源码本机打包安装

## 授权与边界

- 用户要求先依次运行workspace测试、严格Clippy、前端check/test/build；任一失败即停，不继续打包。
- 只用现有 `npm run bundle:mac` 生成.app及ad-hoc重签，不生成DMG、不发布；备份现安装后正常退出，再校验安装/启动。
- 不使用图形验收、不评价观感、不清理临时文件。

## 开始前基线（北京时间2026-10-24 02:00）

- 原安装：`~/Applications/Local Transcribe.app`；GUI PID 97943，路径为该包的 `Contents/MacOS/voice-to-text-desktop`。
- 原desktop SHA-256：`092dca3664b38f1031aa6429d2405620e7628e946be98e076ada85c1e8d8089a`。
- `pgrep -fl local-transcribe-node` 返回1（无节点进程）；`lsof -nP -iTCP:7878 -sTCP:LISTEN` 返回1（无监听）。
- 用户及系统DiagnosticReports只找到两份较早CPU资源诊断，时间2026-10-22/23；未把这些旧记录归因于本轮启动。
- 检查/构建结果及安装事实后续追加；本节不代表已完成安装。

## 测试门与打包结果

- 严格按顺序执行用户指定命令，整个命令链成功：`cargo test --workspace` 29组、169 passed/0 failed/11 ignored；忽略项没有当作通过。
- `cargo clippy --workspace --all-targets -- -D warnings`成功；前端check 0 errors/0 warnings，npm test 39 passed/0 failed/0 skipped；npm build成功。
- 使用 `npm run bundle:mac` 成功生成唯一.app（约46.24 MiB），未生成DMG。版本仍1.1.0-beta.5，脚本完成sidecar与.app的ad-hoc签名。
- `Contents/MacOS/local-transcribe-node`存在；对desktop Mach-O逐字节查找，包含当前 `icons/tray-template.png` 的完整PNG数据（编译嵌入，不是Resources下独立文件）。macOS当前加载22px资源；44px/Windows资源不声称在Mac包中加载。
- `codesign -dv`显示.app与sidecar的Identifier均为`com.timmy.voicetotextapp`、Signature=adhoc；新bundle与安装后均通过`codesign --verify --deep --strict`。不是Developer ID签名/公证。

## 备份、退出与安装

- 备份目录：`/Users/TimmySheep/Applications/Local Transcribe Backups/before-20261024-020458/`，其中保留完整`Local Transcribe.app`；备份desktop SHA-256与原包一致：`092dca3664b38f1031aa6429d2405620e7628e946be98e076ada85c1e8d8089a`。
- 通过正常Apple Event quit退出PID97943，确认GUI进程消失；无残留node需要停止，没有使用任何kill -9。安装前lsof返回1，确认7878无监听。
- ditto安装到`/Users/TimmySheep/Applications/Local Transcribe.app`；安装前bundle与安装后desktop SHA-256均为`bf14d5b783ad23449228e7b7134769cf2ace03f54d18447d003593cfc50dee07`；sidecar也逐个哈希对比一致。
- 从安装路径open启动，启动时间北京时间`2026-10-24T02:04:58.655315+08:00`；新PID14026，区别于旧PID97943。
- ps显示路径为`/Users/TimmySheep/Applications/Local Transcribe.app/Contents/MacOS/voice-to-text-desktop`；lsof的进程text映射独立确认相同可执行路径。
- 启动后观察20秒进程仍存在；用户与系统DiagnosticReports未出现新的voice-to-text/local-transcribe相关报告。这是有限启动窗口检查，不推断长期稳定。
- 机器可读回执保留于本会话临时目录`current-source-install-result.json`；全量测试输出保留于shell调用sh_169ebe8e8001zbVufqKgg1av1l的输出文件。

## 本次未验证

- 真实模型转写、两机局域网、Windows、长期运行，以及所有图标/胶囊/深色观感均未检查；未执行CUA或图像采集。
- 不清理临时文件、备份或任何缓存；未提交、打tag或发布。
