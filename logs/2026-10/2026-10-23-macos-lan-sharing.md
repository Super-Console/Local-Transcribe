# macOS 局域网计算收敛与本机算力共享

## 授权与边界

2026-10-23，Timmy 明确要求先实现苹果桌面端：计算模式只保留「仅本机」「局域网优先」，删除未实现的「本机优先」；局域网失败时自动回退本机；在局域网页添加默认关闭的本机计算节点开关。Timmy 已确认「暴露」仅指同一局域网，不是公网。

实施前已阅读项目 AGENTS.md、任务书、architecture、technical-validation 和 PROJECT_STATUS。`logs/` 可读，`logs/README.md` 不存在。保留开始时已存在的 AGENTS/状态文件改动、未跟踪任务书/日志；执行过程中出现的 `tests/sidebar-shell.test.mjs` 他方改动也未覆盖。不提交 Git、不发布、不修改系统防火墙/代理/路由器、不替换正在运行的已安装应用。

## 实现

- Rust 的 `LanMode` 只剩两项；serde alias 将旧 `prefer_local` 读为 `local_only`，再次保存只写新值。五语言界面移除第三个选项，明确「仅本机」只约束自动派发，保留现有手动发送。
- 自动派发要求节点能力接口认证通过且启用计算，不再只凭无认证的 health 判断可计算。节点不支持所请求模型时在上传前拒绝，不静默换模型；未指定模型的节点任务记录实际配置模型。
- 节点上传/执行/轮询/结果读取失败后，在同一个 JobStore lease / job ID 内重新执行本地核心；本机未选模型则明确失败。用户取消不触发回退。推理放在 blocking worker，继续推送进度与稳定片段；历史只保存该 job ID 的结果。失联时尽力取消远端，但不保证远端立即停止，也不会把远端晚到结果并入本机重试。
- 新增 `lan_host.rs`：macOS 通过 getifaddrs 列出 `en*` 私有 IPv4 接口，启动前重新核对所选接口；只绑定这个地址，不绑定 0.0.0.0/IPv6/公网/VPN 地址。HTTP 按真实 socket peer 限制同一私有子网，不信任 forwarded headers。Bearer 配对沿用现有协议。
- 共享默认关闭，不记忆自动启动；名称/地址/端口只用于当前运行会话，模型固定为开启时的默认模型。正常退出停止监听、撤销 mDNS、停止节点 worker。HTTP 优雅停止限制 1 秒，避免客户端卡在请求体中让关闭无限等待；实际推理退出仍依赖核心的取消检查点。
- 令牌随机生成，持久化到应用数据目录 `lan-host/pairing-token`（0600），重开仍可沿用。状态响应不包含令牌，仅点击「显示配对令牌」读取；默认隐藏，不写日志。接收媒体与任务留在 `lan-host`，不自动清理。
- 补充 NSLocalNetworkUsageDescription / NSBonjourServices；UI 明示 HTTP 未加密、只适合可信局域网，不承诺令牌认证等于加密，也不承诺能阻止用户自行设置的路由器转发/反向代理。
- README 双语同步；MCP 仍为 stdio，不增加 MCP 网络监听。Windows 本机共享入口隐藏，本轮不宣称 Windows 验收。

## 依赖核查

`which cargo rustc node npm python3` 均找到已有工具。新增直接依赖复用 Cargo.lock 已存在的 `libc = 0.2.191`，仅 macOS 目标；没有安装任何全局工具、npm 新包或模型权重。

## 测试与证据

先写并确认预期失败：旧模式迁移（原为 PreferLocal）、回退策略测试（尚无实现）、空令牌仍可启动、未完成请求体阻塞停止、模型不匹配仍被接受。然后实现并回归。

- `cargo test --workspace`：全部默认测试通过；依赖真实模型/真实网卡的测试仍默认 ignored，不能以此冒充已执行。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `npm run check`：0 errors / 0 warnings；`npm test` 最后一轮 6/6；前端生产构建通过。
- 显式运行 ignored `mac_host_start_auth_stop_restart_and_port_conflict`：真实本机私有接口监听、TCP/HTTP 可达、能力接口令牌验证、停机后端口关闭、重复启动拒绝、占用端口拒绝、重开令牌稳定，全部通过。第一次 HTTP 请求曾超时；只读检查接口/本地路由/防火墙及 Python TCP 基线，没有改任何系统设置；增加 TCP checkpoint 后重跑成功，首次超时原因未确认。
- 显式运行 ignored `mac_host_transcribes_an_upload_with_a_real_model`：复用已安装 Paraformer 模型与批准 scratch 中的 `e2e-test.wav`，经本机真实局域网地址上传、计算、取回非空文本及正时长，回读源文件字节完全一致；结束关闭节点。没有打印原始媒体或转写正文。这是同机真实 HTTP + 模型测试，**不是跨设备验收**。
- 子网范围测试、真实 HTTP forwarded-header 伪装拒绝、空令牌拒绝、错误令牌、计算关闭节点不自动派发、模型不匹配拒绝、取消不回退、卡住上传有界停止等回归通过。
- 独立 Chrome headless / CDP：**mock Tauri**，两选项、默认不调用启动/令牌、模式保存、共享开启/关闭、令牌显示/隐藏、900/640px 无水平溢出、浅深色截图通过。此项只验证前端，不是原生 Tauri invoke 或跨设备测试。临时脚本为批准 scratch 的 `check-lan-sharing.mjs`，最后截图目录 `lan-sharing-preview-5SOUmx/`。

## 已知限制与交付边界

- 尚未在真实 Tauri 窗口点完整条链路，也未做两台电脑间的上传/断线/自动回退端到端实测；自动回退目前有决策测试与代码路径覆盖，不把本机共享节点的真实模型测试说成自动回退端到端验收。
- 初次 release `.app` 已构建（26.01 MiB，`--no-sign`）；最终代码的再次打包结果在本日志后续追加。均不声称签名/公证或公开发布。
- 未替换当前 `~/Applications/Local Transcribe.app`，未中断该应用或其任务。
- HTTP 没有加密；允许范围是选中接口的 IPv4 子网，不支持 IPv6-only LAN、VPN 或跨子网路由。
- 接收媒体/测试 profile/截图/测试数据保留，尚未进行清理。清理需先列精确清单、取得同意后移到废纸篓；不清理共享构建缓存或废纸篓。

## 最终打包回读

- 最终 `npm run tauri -- build --bundles app --no-sign` 成功：`target/release/bundle/macos/Local Transcribe.app`，26.02 MiB。读取 bundle 的 Info.plist 核实局域网用途说明与 `_localtranscribe._tcp` 服务声明存在。
- bundle 可执行文件 SHA-256：`4953c7495be880339101d4cd063627c5937fc373c583402634aee5e9a18592d3`。未签名、未公证、未发布、未安装；原有运行应用不受影响。
- 本轮启动的独立 Vite 预览进程已终止，真实节点测试均在结束时停止监听；临时文件未清理。

## 本机安装更新（Timmy 授权）

- 2026-10-23 13:35 前后，Timmy 明确要求“现在更新本机的 macOS 应用”。更新前只读核查任务存储：无 running/queued 任务（历史仅 completed/failed；扫描中出现的整数 `state` 属于模型词表 JSON，不是任务状态），随后旧版进程（安装路径运行中的 PID 83189）经正常 quit 退出，1 秒内终止，未有备份副本被连带启动。
- 流程：先把新 bundle 复制到批准的 scratch 暂存目录并校验 SHA-256，与源 bundle 一致（`4953c7495be880339101d4cd063627c5937fc373c583402634aee5e9a18592d3`）；再把旧应用整体 `mv` 进备份目录，暂存应用 `mv` 到正式位置。全程无 `rm`、无永久删除（尝试的 `rm` 被权限规则拦截后改用时间戳新目录）。
- 旧版备份：`~/Applications/Local Transcribe Backups/before-lan-sharing-20261023-133520/Local Transcribe.app`。此前两个备份未改动。
- 安装后回读：`~/Applications/Local Transcribe.app` 可执行文件 SHA-256 与源一致；`codesign -dv` 显示 `adhoc, linker-signed`（与既有安装一致，未声称签名/公证）。按精确路径启动，进程 PID 60966 来自安装位置，启动 6 秒后存活，近 2 分钟 DiagnosticReports 无该应用崩溃记录。
- 真实 Tauri 窗口 AX + 截图验证（只读导航，未改动任何设置）：局域网页计算方式仅「仅本机」「局域网优先」两项，当前选中「仅本机」；「允许其他设备使用本机算力」显示默认「已关闭 · 点击开启」；接口下拉列出真实私有接口 `en15 · 192.168.1.243` 并预选，端口 7878；既有节点与任务列表保持原样。这是安装版的原生窗口验证，补上了此前“未在真实 Tauri 窗口点验”的缺口（仍非两台设备间端到端验收）。
- 磁盘上的 `lan-settings.json` 仍写 `prefer_local`：迁移是读取时的 serde alias（测试覆盖），仅在下一次保存设置时改写为 `local_only`；当前生效模式为仅本机。文件内的节点令牌/令牌类值未抄入本日志、状态文件或任何通知。
- 未启用共享开关（默认关闭未变），未改防火墙/代理/路由器，未提交 Git、未发布、未安装新工具或下载模型。

## 临时文件清理（Timmy 确认）

- Timmy 在收尾询问后回复“直接清理”，授权清理此前列出的范围。2026-10-23 14:15，将 4 项共约 26 MB `mv` 到废纸篓子目录 `~/.Trash/opencode-lan-cleanup-20261023-141553/`：`check-lan-sharing.mjs`（UI 测试脚本）、`lan-sharing-preview-QdNy0j/`、`lan-sharing-preview-5SOUmx/`（浅深色截图）、`lt-install-20261023-133500/`（空的安装暂存目录）。回读确认 scratch 中已无这些项目，废纸篓内容完整、未清空。
- 保留未动：`target/`、`node_modules` 构建缓存与产物、测试素材 `e2e-test.wav`、其他会话的 scratch 项目、回收站原有内容。全程无 `rm`、无永久删除。
