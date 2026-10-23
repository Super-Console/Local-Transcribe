# 无头节点与菜单栏常驻（P1–P5、P7）

日期：2026-10-23（晚）
对象：VoiceToTextApp 桌面端 / lan-compute / transcription-core / transcription-service
依据：主任务书 V1.0 + 三份增补（Tailscale 网卡范围、mDNS 与发现、jobs list）。
> 按附加任务书 V1.0 调整了网卡范围；按 mDNS 补遗调整了宣告策略；按 jobs list 补遗把任务列表改为持久化单一数据源。

## 0. 基线核对（只读）

| 事实 | 结果 |
| --- | --- |
| 版本 `1.1.0-beta.5` | 符合（tauri.conf.json / Cargo.toml） |
| 无托盘能力 | 符合（`tauri features = []`） |
| 关窗即退出且节点被停 | 符合（`RunEvent::Exit` 调 `HostState::stop`） |
| 无单实例约束 | 符合（Cargo.toml 无插件） |
| 节点跑在 GUI 进程内 | 符合（`lan_host.rs` 直接 `NodeServer::start_on_lan_with_models`） |
| 无头节点只认单模型 | 符合（只有 `--model-dir`） |
| Windows 不能当节点 | 符合（`supported = cfg!(macos)`，非 macOS `interfaces()` 空表） |

差异：任务书说 `start_models` 里用 `find(address == ...)`；真实代码同名函数因此保留，但已按补遗 1.4(a) 在查表前特判 `0.0.0.0`。其余与描述一致。

## 1. P1 · 单实例

- 新增 `tauri-plugin-single-instance = "2"`（实际 2.5.4），注册为该 builder 的第一个插件；回调只做 `unminimize/show/set_focus`，并打印 `[single-instance] second launch forwarded to the running app`。
- 验收（debug 与安装版各一次）：
  - `open -n ~/Applications/Local\ Transcribe.app` 后 `pgrep -f .../voice-to-text-desktop` 仍为 1；
  - 第二次启动退出码 0；
  - 第一实例日志出现上述回调行（屏幕锁定，无法用 System Events 断言前台，回调执行有日志证据）。

## 2. P2 · 无头节点

`crates/lan-compute/src/bin/local-transcribe-node.rs` 重写：

- `--listen` 必填，不设默认值；拒绝回环以外的非法写法；`0.0.0.0` 明确表示所有接口。
- `--netmask` 语义（按 Tailscale 附加任务书 3.2）：
  - `0.0.0.0`、回环、Tailscale `100.64/10` 可省略；
  - `/1`–`/30` 保留同子网过滤；
  - `/31`、`/32` 不做对端过滤；
  - 普通地址缺掩码退出码 2，并说明「掩码就是这里的鉴权替代」。
- `--models-dir`：`ModelLibrary::new(dir).list()`；
- `--selected-model <文件>`：读 LocalModel JSON，在库中则排首位，读不到按库顺序；
- `--model-dir`：优先且去重；
- 状态文件 `<data-dir>/lan-node.json`：pid/name/address/mask/subnet_filter/port/version/started_at_unix_ms/served_models，0600，原子写；pid 存活时拒绝再次启动；正常退出删除；
- `--parent-pid`：每 2 秒探测，父进程消失即停；
- `--idle-unload`：见 P4.1；非法值退出 2。

测试（bin 19 项 + lib 22 项）：listen 必填、0.0.0.0/公网地址允许、Tailscale 可省掩码、`/31`/`/32` 报告 none、`/24` 报告 same_subnet、非连续掩码拒绝、模型库扫描/选中优先/双参数去重、空库、状态文件 0600 与回读、存活 pid 拒绝、死 pid 放行、pid_alive 边界、idle 档位。

实测（`127.0.0.1:7878`，真实 Paraformer，27.4 s WAV）：

- `GET /api/v1/health` → `{"proto_version":1,"status":"ok"}`
- `GET /api/v1/capabilities` → compute_enabled=true，model_id=paraformer
- `GET /api/v1/models` → 3 个库内模型，选中的 Paraformer 在首位
- 创建 → 上传 → `queued → transcribing → completed`，结果文本与音频一致
- 状态文件 0600；`kill -9` 后重启识别陈旧 pid 并恢复；存活时重复启动退出码 1；`kill -INT` 删除状态文件；父进程 `kill -9` 后子进程 5 秒内退出

## 3. 增补：网卡范围与 mDNS

- `LanScope::new`：任何非回环 IPv4 + 合法掩码；`100.64.0.1/32`、`8.8.8.8/24` 均接受；回环、`0.0.0.0` 地址、非法掩码拒绝。
- `LanScope::allows`：`/31`、`/32` 全部放行；`/24` 仍同子网放行、跨子网拒绝（测试保留）。
- `0.0.0.0`：走 `start_with_models`（无 scope），状态 `subnet_filter: none`；`resolve_mask` 在查表前特判，不再出现「请选择一个当前连接的私有局域网接口」。
- mDNS：`mdns_plan` 纯函数——回环/IPv6/非 RFC1918（含 100.x）跳过；`0.0.0.0` 只宣告一个 RFC1918 地址（优先路由表地址，其次 `en*` 私有地址），无私有地址则跳过；不再使用 `enable_addr_auto()`。
- `interfaces()`：收集所有非回环 IPv4（`utun*`/`tun*`/`ppp*` 等），跳过 169.254/16，带 `kind`（lan/vpn/tailscale/other）；UI 下拉增加「所有接口（0.0.0.0）」与三种范围文案；扫描空结果增加「Tailscale/VPN 节点需手动添加」。

对照实验（Air → MBP，真实请求）：

| 起点 | 目标 | 结果 | 判读 |
| --- | --- | --- | --- |
| Air | 192.168.1.243:7878 | `{"status":"ok"}` | 局域网链路当前可用 |
| Air | 100.104.46.54:7878 | `{"status":"ok"}` | tailnet 可用；日志 `mDNS announcement skipped (non-rfc1918)` |

结论：既有「TCP 通、HTTP 超时」本轮**未能复现**；两条链路都成功。可能是今天早些时候授予本地网络权限/重装后的变化，未继续归因。没有声称修复了该历史问题，只记录当前两次成功。

## 4. M1 · 拆进程前抖动基线（P3 之前完成）

方式：同进程集成（`crates/lan-compute/examples/overlap_baseline.rs`），模型 A = FireRedASR2 AED，模型 B = Zipformer CTC small，音频 27.4 s。

| 步骤 | 耗时 | RSS |
| --- | ---: | ---: |
| 启动 | — | 9.3 MB |
| local A #1（加载） | 17,909 ms | 2,259,248 KB |
| local A #2（复用） | 16,110 ms | 1,182,816 KB |
| node B #1（切换） | 1,757 ms | 736,112 KB |
| node B #2（复用） | 519 ms | 927,808 KB |
| local A #3（切回） | 17,861 ms | 1,882,848 KB |
| local A #4（复用） | 15,566 ms | 1,890,800 KB |

结论：单槽缓存下同时只驻留一个模型；切换成本 A≈1.75 s、B≈1.24 s；内存不翻倍。

## 5. P3 · GUI 接管子进程

- 新增 `node_supervisor.rs`：定位 sidecar（`current_exe` 同目录，可被 `VOICE_TO_TEXT_NODE_BINARY` 覆盖）、拼参数、`try_wait`、SIGINT→等待→kill；stdout/stderr 直接写 `app_log_dir()/lan-node.log`（无管道），5 MB 轮转保留 1 份，旧文件走废纸篓；启动失败返回子进程日志末 12 行。
- `lan_host.rs`：删除 `NodeServer` 调用与模型拼装，只做校验 + 监管；`HostStatus` 增加 `mask`、`subnet_filter`；`HostState` 形状和四个命令名不变。
- 测试（真实 sidecar）：正常启动/停止（状态文件删除、端口关闭、进程消失）、端口占用错误含 `Address already in use`、`kill -9` 后状态变 stopped。
- 安装版 `Contents/MacOS/` 同时含 `voice-to-text-desktop` 与 `local-transcribe-node`，均为 ad-hoc 签名。
- 未完成：GUI 内点击开启共享的完整验收（屏幕锁定）——见 §10。

## 6. P3.5 · CLI/MCP 局域网派发

**未实施**（按主任务书 §6.6 允许整段放弃）。原因：与 jobs list 同文件、同区域；本次时间用于完成 P4/P5 与真实测量。`crates/transcription-service` 的 `Submit` 语义、CLI `jobs add`、MCP `transcribe_file` 均未加入节点参数，也没有半成品分支。记录为未完成，不声称可用。

## 7. jobs list 补遗

- 单一数据源：`jobs()` 改为 `JobStore::recent_jobs(200)`（新增 API，扫描 `jobs/*.json`，按 `started_at_ms` 倒序，先恢复中断任务），只列终态，排除当前 active。
- 排队中被取消的任务：新增 `JobStore::record_status` 持久化 `cancelled`；永久启动失败同样落盘 `failed`。
- 删除后的残留：任务文件被删后不再出现在列表（同一目录扫描）。
- 重试：同一 id 不会出现两行；`record_status` 拒绝覆盖，lease 覆盖同文件。
- 边界：完成/失败/取消列表跨重启保留（来自磁盘）；**排队中的任务重启仍会丢失**（未改队列持久化）。
- 测试：core 3 项（顺序/上限/单 id/删除）、service 3 项（列表与单查一致、上限 200、删除消失）。

## 8. P4 / P4.1 · 托盘与空闲卸载

- `tauri` 增加 `tray-icon`（`image-png` 尝试后移除，当前不需要）。
- 托盘菜单：打开主窗口 / 状态（只读）/ 停止共享 / 退出；状态优先级 computing > error > sharing > not sharing；每 2 秒轮询 `health` 与 `/tasks`；pid 存活但 health 失败显示「节点异常」。
- 关窗：`keep_running_in_background` 且共享运行中 → `prevent_close` + `hide`；否则保持退出。
- 语言切换：前端调用 `set_tray_locale`，原生菜单立即重建（五语言）。
- 空闲卸载：五个闭集合 20/120/600/1800/7200；`OnceLock` 进程内一次性设定；GUI 与节点两份字段互不影响；非法读入回落 120；`--idle-unload` 越界退出 2；任务结束（成功/失败/取消）由 `UseGuard::drop` 刷新 `last_used`；`loaded_model_id` 改读独立 `CURRENT_MODEL`，转写期间不再阻塞。
- 实测：
  - Zipformer：任务后 RSS 524,608 KB → 150 s 后 29,776 KB，日志 `[model] idle unload after 120s`。
  - FireRed：任务后峰值 2,136,800 KB → 130 s 后 11,760 KB（同上日志）。
  - 设置文件手写非法值、20 秒/7200 的界面切换未点验（屏幕锁定）——blocked。

## 9. M2 / M3 · 拆进程后的内存

M2（节点 FireRed 与本地 Qwen3 同时处理 116.2 s 音频）：

| 进程 | 峰值 RSS | 说明 |
| --- | ---: | --- |
| 节点 | 2,136,800 KB（≈2.04 GiB） | 加载 FireRed 后 |
| 本地 | 2,076,608 KB（≈1.98 GiB） | 加载 Qwen3 后 |
| 合计 | 3,052,416 KB（≈2.91 GiB） | 两模型同时驻留，确为双份 |

本地 Qwen3 用时 45,766 ms；节点 FireRed 完成并写回结果。

M3（两边都空闲）：节点在 120 s 后回落到 11,760 KB，证明卸载在子进程内生效；GUI 侧未做真实本机转写（锁定屏幕无法点击），因此「GUI 进程 130 s 后 RSS」未测，记录 blocked。

## 10. 未完成 / blocked（逐条）

1. **GUI 验收（P3/P4/P4.1/P5 的共同项）**：验收时 macOS 处于锁屏（`loginwindow` 前台），无法点击开启共享、切换空闲档位、观察托盘菜单与关窗行为。已用真实 sidecar 测试与安装版单实例验证补足能自动化的部分；界面项标记 blocked，不写 verified。
2. **P3.5**：未实施（见 §6）。
3. **P4.2**：仅实现「关闭窗口后继续在后台共享」与其设置；以下未实现：共享状态跨启动自动恢复（字段已存在但未接线）、退出保护弹窗、`lan-host` 保留清理、设置页「数据位置」六项与清除全部数据、首次开启知情同意弹窗、重启共享横幅、端口占用 UI 文案（后端已透出真实原因）。均未声称完成。
4. **Windows**：本轮未构建 MSI；`--listen`/托盘/`interfaces()` 的 Windows 编译未验证，标记 blocked。
5. **两机真实提交**：Air 可 SSH，但本轮只做了 `curl /health` 对照，未做上传→转写→取回的完整两机流程。
6. **20 秒与 7200 档位的真实验收**：因无法点击设置而未做。

## 11. 文档

- README 双语：LAN 段落改为掩码语义；新增无头节点/托盘章节、Tailscale 与 mDNS 说明、睡眠失联与卸载残留说明。
- `docs/mcp.md`：`list_jobs` 标注「最近 200 条」；重启语义拆成「排队任务会丢」与「完成列表来自磁盘」。
- `PROJECT_STATUS.md`：修正过时的 milestone/decisions，并新增本轮 verified/blocked 两节。

## 规则修正：P3.5 放弃条款作废（2026-10-23 深夜）

- 主任务书 §6 第 6 条的「时间不足可整段放弃 P3.5」**作废**，以第 3 节阶段索引为准（P3.5 不得跳过）。
- 替代规则：不许整段放弃；也不许留半坏。做不完就停下，把 service 恢复到可编译、测试全绿，并如实报告完成到哪一步。
- 本修正只针对 P3.5；§6 其余条款继续有效。

## P3.5 收尾：CLI / MCP 局域网派发（2026-10-23 深夜二）

### 做了什么

- `transcription-service`：`Request::Submit` 增加 `node` / `lan_model` / `fallback_local`；新增 `Request::LanNodes`（读 `lan-settings.json`，最多 3 秒 `capabilities` 探测）。
- 节点解析：只认已保存节点，按 id 或名称精确匹配；不存在 → `node_not_found`，已禁用 → `node_disabled`，都不改走本机。
- 远端执行在 service 的 worker 内：`LanClient` 创建任务 → 4 MiB 分块上传 → 1 秒轮询 → 结果写入同一份 `transcripts/<job_id>.json` 与 `JobStatus`（`engine = lan/<name>`，phase `Completed on LAN node`）。
- 失败默认 `failed`（带节点原因）；只有 `--fallback-local` 才用同一 job id 本地重跑，历史仍只有一条。
- 取消：观察到本地 cancel 标记后 POST `/cancel`，**立即停止轮询**并落 `cancelled`；节点自己完成取消。
- CLI：`lan nodes`；`jobs add --node <id|name> [--lan-model ID] [--fallback-local]`。
- MCP：新增 `list_nodes`；`transcribe_file` 增加 `nodeId`，只接受节点 id，且在 `connect()` 之前读 `lan-settings.json` 校验，未知/禁用直接拒绝（无 host/port/url 参数）。
- 修正一个自己引入的缺陷：远端任务在 pending 阶段被 `status()` 与 `jobs()` 报成 `waiting_for_model`（只判断了 `model`，没判断 `remote`），已改为 `model.is_some() || remote.is_some()`。
- `docs/mcp.md`：工具 9 个（去掉 `import_model`、新增 `list_nodes`）；写明 `nodeId` 约束与 `list_jobs` 只返回最近 200 条。

### 验到什么

- 单元测试（service）：未知节点拒绝、禁用节点拒绝、只有远端目标时仍可排队、未知 id 不向配置端口发起任何 TCP 连接（监听器收到 0 次 accept）。
- MCP 测试：工具集 9 个；未知/禁用 nodeId 在启动 service 之前返回错误（数据目录里没有 `service.sock` / `service.port`）。
- loopback 全链路（真实 WAV、真模型）：`queued → completed`，`export --format txt` 写出中文文本；停掉节点后提交 → `failed`（`The LAN node rejected the task: network error…`）；加 `--fallback-local` → 同一 job id 本地 `completed`，`jobs list` 无重复。
- 取消：本地 `cancelled`；节点任务 `cancel_requested → cancelled`（约 20 秒，按 30 秒分块的协作式取消），节点日志 `task … cancelled`。
- MCP stdio：`list_nodes` 返回 `n1 / loop / reachable=true / paraformer`；`transcribe_file` 带 `nodeId` 得到 `Completed on LAN node`，不带得到本地 `Completed`；`nodeId: missing` 返回 `isError=true`；`get_job_result` 的 `fullText` 与导出的 txt 完全一致（116 字符）。
- `cargo fmt --check`、`cargo test --workspace`（29 组全 ok）、严格 Clippy 全绿。

### 还差什么

- 两机真机（Air → MBP）的 CLI/MCP 派发未跑；本轮只验证了 loopback。
- Windows 上的 service/CLI 派发未构建验证。
- GUI 的局域网页仍走自己的客户端路径，未改为 service 派发（不在本轮范围）。

## P4.2 主体实现（2026-10-23 深夜三）

### 做了什么

- **共享跨启动恢复**：`lan-settings.json` 新增 `share_on_launch`、`host_name`、`host_address`、`host_port`；开启共享成功时写入，停止时写 false。启动时后台线程按保存的网卡/端口重开；网卡不存在则**不启动**并在状态里给出 notice「上次使用的网卡不可用，请重新选择」，不静默换网卡、不回退 0.0.0.0。
- **退出保护**：拦截 `RunEvent::ExitRequested`；共享运行时查询节点 `/api/v1/tasks`（2 秒超时），有非终态任务则弹原生确认框，列出文件名与阶段；无响应时弹「无法确认是否还有任务在进行」；`关闭窗口`不触发。确认后置 `QUIT_CONFIRMED` 再退出，避免二次弹窗。
- **节点媒体清理**：`lan-compute/src/retention.rs` 新增 `LanRetentionPolicy`（off/age/size）；节点启动时与每个任务结束后执行；只处理终态任务，目录与 JSON 一起移入 `~/.Trash`；非 macOS 返回「不支持」，桌面开关禁用（绝不退化成 `rm`）。策略文件 `lan-host-retention.json`，由 GUI 写入并通过 `--retention-file` 传给子进程。
- **数据位置**：`storage_summary` 增加 `lan_host_*` 与节点日志文件路径/大小；设置页补「节点接收媒体」与「节点日志」两行，六个路径都有打开与复制，日志可「打开日志文件」；新增「清除全部应用数据」（两次确认，全部 `mv` 到废纸篓，提示重启）。
- **知情同意**：首次开启共享前弹确认（同网段可提交、占用 CPU/内存、未加密），确认后写 `lan_consent_acknowledged`，不再重复。
- **托盘状态优先级**：沿用既有实现（计算中 > 异常 > 共享中 > 未共享，health 失败判异常），本轮未改动。
- **改设置重启提示**：新增 `lan_host_restart_needed`（比较保存的网卡/端口与模型库列表）；局域网页显示横幅与「重启共享」按钮。
- **端口占用**：子进程启动失败且日志含 `Address already in use` 时，返回「端口 N 已被占用」并提示 `lsof -nP -iTCP:N`；不自动换端口。

### 验到什么

- 前端：`npm run check` 0/0、`npm test` 18/18、`npm run build` 通过。
- Rust：`cargo fmt --check`、`cargo test --workspace` 29 组全 ok、严格 Clippy 全绿。
- 新增单测：退出确认的三条路径（有任务/无任务/节点无响应）；节点清理的关闭、仅终态、按容量删除最旧；端口占用测试改为断言中文提示与 `lsof`。
- 未做 GUI 实测：自动恢复、同意弹窗、退出弹窗、重启横幅、清理开关、六个路径打开/复制——均需解锁屏幕（第 4 步统一验收）。

### 还差什么

- 以上 GUI 项全部未点验（第 4 步）。
- 节点清理未做真机端到端（单元测试覆盖策略本身，未在真实节点上等到按天/按容量触发）。
- 托盘四态未在真机逐态切换验证。
- Windows 上「清理开关禁用」的界面路径未运行。

## 界面三处调整 + 主题按钮与模型显示名（2026-10-23 深夜四）

### 做了什么

**界面三处**

- 删除局域网页标题右侧的「临时 UI（视觉设计未定稿）」`span`、`.temp` 样式与五个语言文件里的 `lan.tempUi`。
- 删除侧栏顶部 `.brand` 块与 `.brand` 样式；侧栏 `padding-top` 原本就是 14px（属于 12–16px 要求），删除后首个分组标题不再贴顶，未额外叠加间距。
- 模型清单：`CatalogEntry` 新增 `languages`，7 条 name 按表格去掉语言后缀；`CatalogState` 增加 `languages` 与 `directory`；`LibraryModelSummary` 增加 `languages`（文件夹名与 catalog `directory` 完全相等才命中）；`ModelView` 下载行副标题 `family · languages · size`、已安装行 `family · languages · size · 已安装，未加载`，语言为空时连分隔点一起省略。

**主题按钮与模型显示名**

- `.icon-btn.theme`：32×32、`border-radius: 16px`、常态 `var(--fill)`；hover 用 `color-mix(foreground 9%)`；`focus-visible` 2px 描边。只改 `.theme`，其它 `.icon-btn` 不动。
- 新增 `model-names.js`（纯函数）+ `model-names.svelte.ts`（缓存 `model_catalog`、`$state` 响应式）；`prettyName` 命中 `value === directory` 或 `value.endsWith("-"+directory)`，否则去掉 `sherpa-onnx-` 前缀与结尾 `-YYYY-MM-DD`，未知值原样返回。
- 8 个展示点全部走 `prettyName`：工具栏当前模型、菜单本机项、菜单远端项、设置页当前模型、转写页底部、模型页已安装标题、历史条目/详情 3 处、首页远端标签（共 11 处调用）。
- 未改 `crates/` 的 `display_name` 与 `id`，历史 JSON 与 `selected-model.json` 保持原值。

### 验到什么

- `npm test` 22/22（新增 4 条：7 个清单名的两种输入形态、回退清理/未知值/空串、历史不再渲染原始 modelId、模型标题不含语言串）。
- `npm run check` 0/0、`npm run build` 通过。
- `cargo fmt --check`、`cargo test --workspace` 29 组全 ok、严格 Clippy 全绿。
- 源码核对：`tempUi`/`.brand` 无残留；侧栏第一个元素是「工作区」；6 个组件共 11 处 `prettyName`。

### 还差什么

- 三张截图（局域网页标题行、侧栏顶部、模型页下载+已安装）与浅色/深色对照未截：屏幕锁定，纳入第 4 步 GUI 验收。
- 模型列表的目视核对（五种语言切换、无连续分隔点）同样待解锁。

## P7 文档 + Windows 预检（2026-10-23 深夜五）

### 做了什么

- README 双语 LAN 段落按新行为重写（不是追加）：共享状态会被记住并在网卡仍存在时恢复；`lan-host` 默认不清理但可启用自动清理（只清终态、最短 1 天、走废纸篓）；关窗不中断、显式退出且有任务时先确认；补上「本机未运行 GUI 时节点仍可工作」、数据与日志的确切位置。
- `PROJECT_STATUS.md`：把 P3.5 从 blocked 移到已实现（loopback 端到端证据），补 P4.2 与两份界面任务的实现状态，GUI 验收仍列为 pending。
- `lan-host` 清理真机验证（无 GUI）：用一个已终态任务的数据目录 + `{"mode":"size","maxBytes":1}` 启动节点，任务目录与 JSON 被移入 `~/.Trash`（`c74411aa-….1792764004170`），`tasks/` 变空。
- Windows 预检（SAC 已关闭）：同步源码 → `cargo check --workspace --all-targets` 通过；严格 Clippy `-D warnings` 通过；`cargo test --workspace` 29 组全 ok、0 failed。
- 修掉两处只在 Windows 暴露的 dead-code 警告：`lan_host::classify`/`is_tailscale_cgnat` 加 `cfg_attr(not(macos), allow(dead_code))`；sidecar 测试里仅 Unix 使用的 pid 变量改为 `_pid`/`_child_pid`。

### 验到什么

- Windows 命令输出：`CHECK_EXIT=0`、`CLIPPY_EXIT=0`、`TEST_EXIT=0`（29 组 ok，0 failed）。
- macOS 侧全量测试仍为 29 组 ok、`npm test` 22/22。

### 还差什么

- Windows 的 sidecar 是**占位文本文件**（tauri-build 会校验 `externalBin` 是否存在，`cargo check` 仅需存在）。正式 MSI 必须在 Windows 先构建 `local-transcribe-node.exe`（release），再放到 `apps/desktop/src-tauri/binaries/local-transcribe-node-x86_64-pc-windows-msvc.exe`，然后 `tauri build --bundles msi`。
- GUI 验收与三张/浅深色截图仍待解锁（第 4 步）。

## 两机局域网端到端（无 GUI，2026-10-23 深夜六）

- MBP 起节点：`--listen 192.168.1.243:7878 --netmask 255.255.255.0 --compute on --models-dir <app data> --selected-model <...>`，mDNS 日志：`announced mbp-two-mac._localtranscribe._tcp.local. at 192.168.1.243:7878`。
- Air 侧：`curl /health` → `ok`；创建任务（116.2 s / 5,575,724 B）→ 上传 → 轮询 → `completed`；`/result` 返回真实中文（4 段，`duration_ms=116160`）。
- mDNS 浏览：Air 上 `dns-sd -B _localtranscribe._tcp local.` 6 秒无记录（输出为空），`dns-sd -L` 也无结果；同机 HTTP 直连正常。判断为 Air 侧多播/本地网络受限，客户端实际依赖 mDNS + TCP 扫描回退；GUI 扫描结果留第 4 步确认，不写成已通过。
- 结论：**两机提交→转写→取回已通过**；自动发现未在 Air 侧观察到（待 GUI 验收）。

## 打包预演（beta.5，2026-10-23 深夜七）

- macOS：`npm run bundle:mac` 产出 `.app`，`Contents/MacOS` 同时含 `voice-to-text-desktop`（26,074,256 B）与 `local-transcribe-node`（21,037,232 B），两者 ad-hoc 签名标识均为 `com.timmy.voicetotextapp`；DMG `hdiutil verify` 通过。
  - SHA-256 `e8ec2f7e8b3798c7ea53033623cd907b031f385fecc64740e6617493bc2bc77b`
- Windows：在 `tims-gaming-pc` 先构建 `local-transcribe-node.exe`（release）再放进 `binaries/local-transcribe-node-x86_64-pc-windows-msvc.exe`，`tauri build --bundles msi` 成功，MSI 20,099,072 B（19.17 MiB）。
  - 用 `msiexec /a` 行政安装解包核对：`local-transcribe-node.exe` 22,519,296 B + `voice-to-text-desktop.exe` 28,206,592 B，sidecar 确实在包内。
  - SHA-256 `7072bc92a40a4126a60efe7cc0d19e6c7799bb68c8bd6dbe1993227b112837f7`
- 这是 **beta.5 的打包预演**，验证了「先构建 node.exe 再打 MSI」的链路；正式 1.2.0-beta.1 发布会按同一流程重打并重新计算哈希。
- 未安装到本机或 Windows：避免在使用者不在场时替换正在运行的版本。


## 本地网络排障入口与会话授权缓存排查（2026-10-23 深夜八）

### 做了什么

- 按 Timmy 要求调整入口位置：删除局域网页标题右侧常驻的「排障」按钮；「排障」只保留在权限提示条内（「打开系统设置」「重新检测」右侧），多播被阻断的提示条也各有一个。
- 新增 `apps/desktop/src/lib/components/LanDiagnosticsPanel.svelte`，局域网页与设置页共用：显示权限探测结果与 errno、TCP 网关探测结果、判定、处理建议、bundle id 与版本；提供「重启应用」「打开系统设置」「复制诊断信息」「关闭」。
- 设置页在「关于」上方新增「故障排查」分组，内含「本地网络排障」一行和「排障」入口；未改动其它设置项。
- 后端新增 `local_network_access_detail()`（在原有探测基础上带回 OS errno）、`lan_diagnostics`（camelCase 序列化）与 `restart_app` 命令；诊断同时做一次到默认网关的 TCP 探测（macOS 用 `route -n get default` 取网关），据此区分 `ok` / `multicast_blocked` / `permission` / `no_network`。
- 权限提示条出现前对 `permission` 判定追加一次 700ms 后的复测，避免 macOS 刚应用授权时的瞬时误报；判定为 `ok` 时不再显示「建议重启」一段。
- 五个语言文件补齐 `lan.troubleshoot`、`lan.diag*`、`lan.multicast*`、`lan.close`、`settings.troubleshoot*` 等键。
- 更新 `apps/desktop/tests/lan-discovery.test.mjs`：改为校验 `lan_diagnostics`，并新增「排障在提示条与设置页、不在标题行」「设置页故障排查必须位于关于之前」「面板调用 `restart_app`」三条断言。

### 排查结论（Local Network 权限为什么像「缓存坏了」）

- `tccutil reset LocalNetwork com.timmy.voicetotextapp` 在本机 macOS 27.0.1 (26A434) 返回 `Failed to reset ...`、退出码 70；`tccutil` 只支持 reset，没有 list。
- 系统 `TCC.db`（`/Library/Application Support/com.apple.TCC/`，本轮可只读）共 184 条记录，服务列表中没有 `LocalNetwork`；用户级 TCC.db 不存在。说明 Local Network 与 Location 一样不走 TCC。
- Apple TN3179 明确：macOS 27.2 之前在系统设置里没有移除按钮，也没有受支持的代码级重置方式（FB14944392）；建议用新用户或 VM 复测。授权状态按用户保存，文件位于 `com.apple.networkextension*.plist`，但 Apple 说明那是实现细节，运行中直接改/删不可靠且有破坏性。
- 因此「App 在本地网络列表里消失又出现」更可能是应用代码身份变化（ad-hoc 重签会换 cdhash）或系统重新登记，而不是公开的强制刷新接口；没有可安全调用的「清缓存」API。
- 对本项目真正有效的是：授权决定在进程启动时读取，运行中改开关不一定立即生效；重启应用即可让新决定生效。本轮实测同一构建先报 `denied · errno 65`、TCP 网关也 `unreachable`，而 shell 同一时刻 `nc 192.168.1.1 80` 成功；约 10 分钟后未重装即变为 `granted`、网关 `reachable`，说明系统应用授权存在延迟，单次探测不足以断言「没授权」。

### 验到什么

- 设置页浅色主题下目视确认：「故障排查」位于「关于」上方，行内「排障」可展开面板；面板显示 `权限探测: granted`、`TCP 网关: reachable · 192.168.1.1`、`判定: 本地网络正常`。
- 点「重启应用」后 GUI 进程 PID 由 25323 变为 25894，应用自动回到首页；重新进入局域网页后无权限提示条，标题右侧无「排障」按钮。
- 权限提示条内的「排障」在 23:18 的拒绝状态下已实际打开面板（截图），显示 `denied · errno 65`、`TCP 网关: unreachable · 192.168.1.1 · errno 65`、`判定: 权限被拒，或系统仍在使用旧缓存`。
- `npm test` 23/23、`npm run check` 0/0、`cargo clippy -p voice-to-text-desktop -- -D warnings` 通过；`npm run bundle:mac` 重新打包并替换 `~/Applications/Local Transcribe.app`。

### 还差什么

- 提示条在「macOS 显示已授权但应用仍被拒」时的最终文案是否要从「系统没有给 … 权限」改成更中性的「无法访问局域网」，待 Timmy 定；当前保留原文并靠复测降低误报。
- 1.2.0-beta.1 版本号、正式 DMG/MSI、Windows 真机转写与发布均未开始，仍按原计划待办。


## 收尾 A–C：按检查单逐项自证（2026-10-23 深夜九）

### 做了什么

- 补齐 MCP 的“未知 nodeId 不得发起网络请求”断言：新增 `unknown_node_id_makes_no_network_request`，在配置节点端口上起真实 `TcpListener`，调用 `transcribe_file(nodeId="missing")` 后要求 accept 只能是 `WouldBlock`。
- `docs/mcp.md` 的 Tools 段从旧的 5 个工具改为 9 个（新增 `list_nodes`，补 `list_jobs` 最近 200 条、`retry_job`、`export_result`）；`list_jobs` 的“不是全量历史”和“排队任务重启会丢、完成列表落盘保留”分开表述。
- README 双语补“内存”段（节点+本机两个大模型约 4.0 GiB、空闲 120 s 回落约 12 MB）；局域网页共享说明下新增五语言 `lan.memoryHint`，满足 3.7 的“有提示”。
- 纠正 `PROJECT_STATUS.md` 的 M2 合计：2.04 + 1.98 GiB 应为约 4.02 GiB，撤回与算术和不符的 2.91 GiB，并注明两个峰值不是同一时刻采样。
- 按检查单填完 151 项：108 ✅、16 ⛔、1 ❌、26 ⬜（12/13 节全部未做/未自评）。

### 验到什么（本轮新增，均可复跑）

- 取消传播真机：本地 `jobs cancel` → cancelling → cancelled；节点任务 `cancel_requested` → 约 4 s 后 `cancelled`（`/api/v1/tasks`）。
- 自动恢复共享：退出/重开两次，节点均以新 `--parent-pid` 自动拉起；网卡改成 192.168.1.99 后不启动节点并提示“上次使用的网卡不可用”，没有回退 0.0.0.0。
- 停止共享：子进程消失、7878 释放、`lan-node.json` 删除、`share_on_launch=false`。
- 端口冲突：占用 7878 后开启，UI 显示“端口 7878 已被占用…lsof…”，开关保持关闭，没有换端口。
- 父进程看护：`kill -9` GUI 后节点 1,883 ms 退出（<5 s）。
- 知情同意：把 `lan_consent_acknowledged` 置 false 后开启共享，弹出“开启共享前确认”，正文含“同所选网段…占用 CPU 与内存；传输未加密”；取消后恢复标记。
- 单实例：连续两次 `open -a` 均退出 0，GUI 进程数 1。
- 全量：`cargo test --workspace` 29 组 159 passed / 0 failed；严格 Clippy macOS 与 Windows 均 EXIT=0；`npm test` 23/23、`svelte-check` 0/0、build 通过。

### 还差什么

- 16 个 ⛔ 集中在：托盘菜单/退出弹窗/数据路径六项/清理全部数据/重启共享横幅（需 Timmy 在场或需授权），以及 4 条 P3.5 分支测试（6.1、6.3、6.4、6.12）和 4.8、4.9、10.4。
- 1 个 ❌：3.3 的 M2 合计数字历史不一致；没有同时刻合计实测。
- 第 12 节 7 项、第 13 节 19 项未做；发布未开始。


## 排障面板补充麦克风/系统音频权限（2026-10-24）

- 新增 `apps/desktop/src-tauri/src/permissions.rs`：macOS 用 `AVCaptureDevice authorizationStatusForMediaType` 读麦克风状态（not_determined/restricted/denied/authorized），用 `CGPreflightScreenCaptureAccess` 读屏幕/系统音频录制状态；非 macOS 返回 unknown。
- 新命令 `app_permissions`、`open_privacy_settings(pane)`；后者打开 `Privacy_Microphone` / `Privacy_ScreenCapture`，本地网络继续复用原命令。
- 排障面板新增“权限”区：本地网络、麦克风、系统音频录制三行，各带状态与“打开设置”，并可直接“重新检查”；文案说明“导入文件转写不需要这两项”。
- 五语言补齐 `permissions.*` 键；前端测试新增 `app_permissions`/两条权限断言。
- 验证：`cargo test --workspace` 29 组 159 passed / 0 failed；桌面包 40 passed；严格 Clippy EXIT=0；`npm test` 23/23、`svelte-check` 0/0、build 通过；重新打包并替换 `~/Applications/Local Transcribe.app`。
- 未做 GUI 验收：按 Timmy 要求由他本人进行。
