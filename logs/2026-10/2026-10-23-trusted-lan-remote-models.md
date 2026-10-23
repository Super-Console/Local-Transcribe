# 可信局域网与远端模型选择（2026-10-23，待两机验收）

## 授权与边界

Timmy 明确要求删除整个配对/认证功能、精简共享文案、客户端获取并自由选择主机模型，并在 MBP / MBA 间实测。交互确认：暂缓 beta.3，修通再发布；beta.1 第四个裸 exe 也删除（已回读只剩 dmg/msi/SHA256SUMS）。本轮不发布、不移动 tag、不提交代码。

实现前读取项目 AGENTS.md、taskbook/任务书.md、docs/architecture.md、docs/technical-validation.md、PROJECT_STATUS.md。新增模型下拉沿用现有 Svelte / CSS 自定义属性及深浅色体系，无新增依赖。

## 代码变更

- 删除 NodeConfig / LanClient / 桌面共享主机的认证字段、HTTP 校验中间件、认证头、令牌生成/读取及 CLI 参数；移除 Tauri 令牌命令及 GUI 输入/显示控件。旧配置字段反序列化时忽略，下次保存不再序列化；未主动删除用户旧令牌文件。
- 新增 GET /api/v1/models，只返回 id/display_name，不暴露模型路径。主机共享已安装模型库与已选外部模型；任务可选任意已共享模型，worker 按任务 ID 取模型。未知 ID 在上传前拒绝。模型目录在开启共享时快照，变更模型库后需重启共享。
- 节点配置持久化 model_id；节点卡片增加模型下拉，手动发送及 prefer_lan 使用远端选择，不再挟带客户端本地模型 ID。无本地模型仍可提交远端任务；本地回退仍需本地模型。
- 五语言删除配对措辞与令牌控件文案。中文共享文案改为：开启后，同一局域网子网内的设备可上传媒体并使用本机转写。
- mDNS 发现优先使用已解析 IPv4，而非重新解析自造 .local 主机名；广播主机名增加随机后缀，避免两台默认同名 Mac 争用地址。仍保留私有接口/同子网限制，没有扩大为公网共享。

## 自动化验证

- TDD：新 trusted_lan 测试先因“required pairing token”预期失败；界面两项测试先失败；发现地址回归先因返回 hostname 而失败，再实现最小修复。
- cargo test --workspace 全套通过（真实权重/接口测试仍标 ignored）；cargo clippy --workspace --all-targets -- -D warnings 通过；npm test 9/9；svelte-check 0 errors / 0 warnings；前端和未签名 release app 构建成功。
- 模型列表/两模型选择测试用 layout-only fixtures，未宣称是真实推理。真实两机推理另行验证。

## 两机安装

- MBP：旧 app 正常退出，无活动任务。旧包备份 before-trusted-lan-20261023-162118，新包暂存校验后换装。MBA：安装前未运行 app，旧包备份 before-trusted-lan-20261023-162301，暂存/安装哈希一致。
- 两机新可执行文件 SHA-256：402df46e93eb4c78a4360ee9c506108fbaf00647acc2bd0368b8cb28f44a05b8。用户模型、历史、录音未改动。
- LaunchServices 按 bundle ID 误开 MBP 备份，已正常退出；改由 Finder 打开精确安装路径，确认 PID 45875 来自 ~/Applications/Local Transcribe.app。未把备份版本当当前验证对象。
- MBP 真实 GUI 点击开启共享，确认精简文案、无令牌控件、列出三个真实模型（Paraformer / FireRedASR2 / Qwen3-ASR）。MBA GUI 尚未启动验证：现有 CuaDriver 的 Screen Recording 权限缺失，未绕过；尝试启动的权限门禁进程随后 SSH 超时，未改 TCC。

## 两机实测发现的阻塞（尚未修通）

- Air 通过直连 HTTP 请求 MBP 实际安装版健康检查：TCP connect 成功但无响应超时；MBP 以局域网 IP 自访也超时。
- 同地址 Python 对照服务在 MBP 与 Air 均 HTTP 200，排除两机一般网络不通。独立 Rust 节点也超时；显式真实接口 ignored 测试重跑失败，未把默认全绿宣称为端到端通过。
- 读取 ALF 系统日志定位：Local Transcribe.app 的入站请求处于 doask，后续请求显示 Not prompting user until existing response received。真实 AX 确认 UserNotificationCenter 的系统弹窗“你要应用程序 Local Transcribe.app 接受传入网络连接吗？”，含“拒绝/允许”。未代用户点击或更改防火墙规则；只读 getappblocked 的 permitted 文案不能代表该弹窗已获准。
- 已通知 Timmy 需亲自允许该应用入站连接；无需关闭防火墙。Air GUI 另需 Screen Recording 授权。完整转写、结果回读、GUI 远端模型选择与断线回退验收均未完成，beta.3 保持暂停。
- 对照用独立 Rust 节点与 Python 服务已 SIGINT 正常退出；测试脚本/合成语音/安装暂存空目录留存待列清单征询。未永久删除文件或清空废纸篓。

## 16:57 清理与交接（Timmy 自行验收）

- Timmy 表示“清理，我自己来验收”；列出 MBP / Air 的临时文件清单、说明保留应用/备份/模型/历史/源码/正式日志后，取得“确认清理”。停止继续验收与发布。
- MBP：12 项批准对象（安装 tgz、测试脚本、合成 AIFF/WAV、测试输出、线程采样、lan-control 诊断目录、空暂存目录）已逐项 mv 到 `~/.Trash/opencode-trusted-lan-cleanup-20261023-165746/`，保留原路径层次；源消失与目标文件大小/SHA-256 manifest 均已核验一致。
- Air：原 SSH 地址与此前核实的局域网地址均在 8 秒连接期限内超时；后者使用原主机密钥别名及 StrictHostKeyChecking=yes，未绕过身份校验。连接未建立，4 项批准对象均未执行搬移，待 Air 在线后补做。
- 两台应用、旧版备份、模型、历史、项目文件及正式日志均不在清理目标中；未清空废纸篓，未永久删除任何文件。
