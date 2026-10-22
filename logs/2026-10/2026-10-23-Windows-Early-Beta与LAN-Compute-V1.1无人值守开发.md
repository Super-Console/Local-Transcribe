# Windows Early Beta + LAN Compute V1.1 无人值守开发（任务书一）

日期：2026-10-23（夜间无人值守）。执行：OpenCode 会话（Claude Opus 4.7 路由）。
任务书：Local_Transcribe_Windows_LAN_Early_Beta 无人值守开发任务书。
本文按「事件」组织，后续同事件追加到末尾。只记录已验证事实与实际执行，不写未验证结论。

## Phase 0 · 现状调查（输出：docs/architecture/windows-lan-review.md）

- 仓库 `github.com/TimmySheep/VoiceToTextApp`（当时 PRIVATE、无 LICENSE）；CI `windows-validation.yml` 管线完整但最近 8 连红，首败于 `cargo fmt --check`（recording.rs 未格式化），**Windows 编译从未被 CI 真正验证过**。
- 关键雷区（代码核实）：`transcription-service` 无条件 `use std::os::unix`（Windows 无法编译）；desktop Cargo.toml 通用 [dependencies] 含 4 个 macOS-only crate；测试 service_ipc.rs 含 Unix 专属断言。
- Windows PC 唤醒：HA `pc_wake`（`button.wake_on_lan_*`，MAC 经 pc_shutdown/pc_sleep 交叉验证同指一台机）；因宿主无 HA API token（secrets.yaml 仅占位符），改为 HA 同宿主发送同 MAC/同广播 WoL 魔术包（等效该按钮动作，已在评审文档记录偏离）。PC 唤醒成功，hostname `REDACTED-HOST` 交叉验证通过。

## Phase 1 · Windows 构建链打通（fa449c7）

- service 跨平台 IPC：macOS/Linux 保持 Unix socket 0600；Windows 用仅回环 TCP + `service.port` 文件（fs2 单实例锁、换行 JSON 协议不变）——任务书允许的本地 IPC 之一。客户端统一 `ServiceClient::connect`，改造收敛在 service 一处。
- desktop 依赖门控（core-foundation / objc2-core-audio* 移入 macOS target）；recording.rs 平台 import 门控（麦克风路径 cpal 跨平台保留，应用音频非 macOS stub）；测试按平台门控。
- clippy `-D warnings` 双目标清零（修 3 处旧代码 + unused import）；rustfmt 修复。
- 本地验证：`cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` 零告警；macOS 42 测试通过。
- **CI（windows-2022 真实 runner）完整管线首次全绿**（25 分钟：fmt/test/clippy/release/npm/MSI/产物暂存）。
- 环境安装记录：`rustup target add x86_64-pc-windows-msvc`（本地交叉核对用）、`brew install llvm`（llvm-rc，交叉编译 Windows 资源用）。均已先查后装。

## Phase 2/3 · LAN Compute V1.1（2d034df + 78cb9d2）

- 新 crate `crates/lan-compute`（`voice-to-text-lan`）+ `local-transcribe-node` 程序：九态任务状态机、任务持久化/崩溃恢复、axum `/api/v1`（分块上传+断点续传、Bearer 恒定时间比较、collect 防路径穿越、ZIP+sha256 清单）、mDNS `_localtranscribe._tcp.local.`、token 自动生成（0600）。23 测试。
- GUI Client 集成：`LanView.svelte` + `lan.rs` 命令层（设置持久化/发现/4 MiB 分块提交/进度事件/结果回写历史）、`start_transcription` 优先局域网自动派发（不可达回落本机）、i18n 104 键×5 语言、6 个 TDD 单测。workspace 45 测试全绿、双平台 clippy 零告警、svelte-check 零告警。
- **跨设备真机实测（Mac → Windows Node）8 步全绿**：401 认证、capabilities（platform:windows/paraformer/cpu）、创建任务、分片上传+断点续传逐值核对、**Windows CPU 真实转写 queued→completed**（8.67 s 音频出稿）、结果取回、collect 导出。mDNS 真机广播日志确认。
- Windows 真机安装：MSI 静默安装返回码 0；交互会话 GUI 启动（PID 4836、窗口标题 `Local Transcribe`）；真实截图 `docs/testing/screenshots/windows-11-app-window.png`。headless（SSH session 0）启动 GUI 退出属 WebView2 会话限制，已记录为测试方法学备忘。

## Phase 4/5 · 文档与发布线

- 冻结灰度设计稿 `design/windows-early-beta.html`（§0-§9 全板块：文案/尺寸/状态/交互/转场）；MIT LICENSE；README 双语更新；测试报告 `docs/testing/windows-early-beta-test-report.md`（严格区分已验证/待验证/不支持）；版本定格 `1.1.0-beta.1`。
- Gate G4 敏感信息扫描：git 历史无凭据类泄漏；内网 IP/主机名/SSH 用户/MAC/真名（1 处）定脱敏规则，公开前 git-filter-repo 全历史脱敏（`brew install git-filter-repo`，先查后装）。
- 真机档案：Win11 Pro build 26300 x64、AMD RX 6600（无 NVIDIA，compute 角色 cpu）、WebView2 155、SSH 管理员权限、C: 剩余 ~362 GB。

## 排障备忘（实证）

- sherpa-onnx-sys 构建脚本不支持 SOCKS 代理：构建需 `env -u ALL_PROXY`（保留 HTTP(S)_PROXY）。
- Mac curl 访问 Tailscale 地址须 `--noproxy '*'`，否则被 Clash 127.0.0.1:7890 吞掉（表现为 52 空回包/超时，极具误导性）。
- Windows 里 SSH 会话内 `Start-Process` 起的进程随会话回收；需长期驻留用 WMI `Win32_Process.Create`（或交互计划任务）。GUI 必须在交互会话跑（WebView2）。
- 模型目录族识别依赖原始文件夹名（如 `sherpa-onnx-paraformer-zh-2023-09-14`），改名会拒绝并给出可读错误（校验行为正确）。
- 测试用防火墙限端口入站规则 `LT-EarlyBeta-Test node 7878` 为临时规则，收尾删除。

（后续追加：GUI 层跨设备实测、历史脱敏、公开、GitHub Release。）

## 追加：GUI 层跨设备实测发现并修复「代理吞局域网」真 bug（2026-10-23）

- GUI 实测「测试连接」显示节点**离线**，但同机 `curl --noproxy '*'` 到 `REDACTED-TAILSCALE-IP:7878` 健康检查恒通。定位：`voice-to-text-lan` 的 `LanClient::new` 用默认 `reqwest::Client`，会读取环境 `HTTP(S)_PROXY`/`ALL_PROXY`（本机 Clash 127.0.0.1:7890），把 100.x/192.168.x 私网地址送进代理后被吞。GUI 状态探测/自动派发/上传共用此 client（全仓唯一构造点）。
- TDD 修复（`crates/lan-compute/tests/proxy_policy.rs`，独立集成测试进程，环境变量无竞态）：设置死代理 + NO_PROXY 不覆盖目标 → 要求 `health()` 仍直连成功。先复现失败（`Err(Network(...))`，坐实走代理），再在 `LanClient::new` 加 `.no_proxy()`（LAN 流量一律直连，本就无需代理），测试转绿，全 workspace 回归通过，双平台 clippy `-D warnings` 清零。
- 排障备忘补充：`open -a` 启动的 GUI 进程会继承调用 shell 的代理环境变量（`ps eww <pid>` 可查）；诊断网络问题先看进程 env，别只看系统设置。
- 附带修复：CI `cargo test --workspace --locked` 因版本号升至 1.1.0-beta.1 后 Cargo.lock 未随提交而失败（620d44f 补提交锁文件）。

## 追加：状态覆写误报「离线」bug 与 MSI 版本打包修复（2026-10-23）

- GUI 排障继续：修完代理 bug 后节点仍显示「离线」。判别实验（本机 loopback 测试节点 127.0.0.1:7879——loopback 不受代理/本地网络权限影响）同样「离线」，排除网络层，锁定应用层。给 `loadStatus` 加错误透出后拿到真错误：`node returned status 401: missing or invalid bearer token`。
- 根因有二：(1) 测试侧令牌打字失误（自动化工具回报「送达 22/28」不准确，实际全量送达后又补打后缀，令牌多 6 字符 → 401）；(2) **产品 bug**：`loadStatus` 里存储探测（需鉴权）失败走 catch，把已成功的 `reachable: true` 覆写为 `false`——令牌错误被误报成「节点离线」。
- TDD 修复（`apps/desktop/src-tauri/src/lan.rs`）：探测语义下沉纯函数 `assemble_status`，可达性只由 health 探测决定，能力/存储探测相互独立、失败进 `warning` 字段；`LanNodeStatus` 扩展 `storage`/`warning`；前端 `loadStatus` 改单次原子调用。3 个 TDD 单测先红（编译失败）后绿，全 workspace + svelte-check + 双平台 clippy 清零。
- CI 620d44f 在 MSI 打包步骤失败：Tauri MSI 打包器要求版本预发布标识纯数字（`beta.1` 不合法）。修复：`bundle.windows.wix.version = "1.1.0.1"`（MSI 专用数字版本覆盖），应用语义化版本保留 `1.1.0-beta.1`；CI 暂存用 `*.msi` 通配不受文件名影响。
- 排障方法学备忘：合成点击/打字回执不可信，须截图回读核对；`nohup` 启动的 GUI 进程对合成 HID 事件无响应，测试须用 `open -a` 启动；插桩要临时加、用完即撤（本次 eprintln 插桩已撤）。
- 又一真 bug（GUI 闭环测试发现）：Tauri v2 插件命令受能力白名单约束，`capabilities/default.json` 漏放 `dialog:allow-confirm`，「删除节点」「删除模型」的确认框被静默拒绝（无对话框、无报错、操作无反应）。修复：补权限条目。教训：插件 API 的 ACL 放行要逐一对照前端实际调用（本项目用 dialog open/save/confirm 三种）。
- GUI 实测确认修复全链生效：Gaming-PC 节点「测试连接」显示 在线 · 3 毫秒 · 可接收任务 · paraformer 模型 · 角色 cpu · 存储 361.24 GB / 464.36 GB（代理 env 下经 Tailscale 100.x 直连 Windows 真机成功）。

（后续追加：GUI 发送文件闭环、历史脱敏、公开、GitHub Release。）

## 追加：GUI 发送文件完整闭环实测通过（2026-10-23）

- 删除链路验证：修复构建重装后，GUI 点测试节点「删除」→ 原生确认框弹出（`dialog:allow-confirm` 修复生效）→ 确认后 LocalTest 节点卡片消失；随后停掉本机 7879 loopback 测试节点。
- 发送文件闭环（全链路每步截图回读，无一步靠推断）：选中真实 Windows 节点 Gaming-PC →「发送文件」→ macOS 原生选择器经 Cmd+Shift+G 定位本机 `say`+`afconvert` 合成的 16 kHz 中文测试音频 `e2e-test.wav`（171 KB）→ 任务入队（排队中 1 MB/1 MB）→ 节点转写完成（排队中 → 已完成）→「获取结果」显示文字稿「你好这是本地转写端到端测试很高兴认识你」（与合成语音一致）→「保存到历史」→ 首页历史出现 `e2e-test.wav`（engine `lan-node`，0:05）。
- GUI 自动化排障备忘（补充）：Tauri 原生文件选择器/确认框是独立窗口，AX 输入常被拒（"not among the process's current AXWindows"），需改用 `scope:"desktop"` 前台键鼠投送；Go-to 面板补全条合成点击不 accept，可靠路径是清空字段 + Esc 关面板后点 Open；窗口截图与屏幕坐标换算须按 `window_bounds`/截图宽高重新标定，偏差几像素就会点空。
- 测试报告 §4.2 补全（删除验证 + 发送文件闭环），§7 结论按「已验证/未验证」如实分列。
