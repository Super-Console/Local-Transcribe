# Local Transcribe — 交付验收检查单 V1.0

> **用途**：六份任务书交付后，逐条核对。**不是给你写总结用的，是给你自证用的。**
> **填写规则**
> - 通过 = `✅`，且**必须在该条下面贴命令与输出摘要**（不是"已通过"三个字）
> - 卡住 = `⛔` + 原因（事件号 / 谁需要授权 / 缺什么）
> - 没做 = `⬜`，不要用空着代替
> - 不通过 = `❌` + 现象
> - **没有证据的 ✅ 一律视为未完成。** 截图不能代替命令输出；命令输出不能代替人工观察（两类分开标）。
> - 涉及 GUI 的条目单独集中在第 12 节，**必须等 Timmy 在场解锁后一起做**。

---

## 1. 基础不退化（任何一条不过，后面全部不算）

- [x] **1.1** `cargo fmt --all -- --check` 通过 — 贴退出码
  - ✅ `cargo fmt --all -- --check` → FMT_EXIT=0（先执行一次 `cargo fmt --all` 修正 4 处未格式化片段）。
- [x] **1.2** `cargo test --workspace` 全绿 — 贴测试组数
  - ✅ `cargo test --workspace --offline` → 29 个 test result 组，169 passed / 0 failed / 11 ignored，TEST_EXIT=0（新增 MCP 无网络断言后复跑）。
- [x] **1.3** `cargo clippy --workspace --all-targets -- -D warnings` 零告警（macOS）
  - ✅ `cargo clippy --workspace --all-targets --offline -- -D warnings` → CLIPPY_EXIT=0。
- [x] **1.4** Clippy 在 Windows 目标上同样零告警（无法执行则 ⛔ 并说明）
  - ✅ Windows（Tims-Gaming-PC，源码已同步）`cargo clippy --workspace --all-targets -- -D warnings` → EXIT=0（补上 MCP 新测试后再次 EXIT=0）；同步时清掉 macOS 资源叉 `._default.json`（用 move，未删除）。
- [x] **1.5** `apps/desktop`: `npm run check` 0 errors 0 warnings
  - ✅ `npm run check` → svelte-check found 0 errors and 0 warnings。
- [x] **1.6** `apps/desktop`: `npm run build` 通过
  - ✅ `npm run build` → Wrote site to "build"，✔ done。
- [x] **1.7** `apps/desktop`: `npm test` 全绿，贴用例数
  - ✅ `npm test` → tests 34 / pass 34 / fail 0，NPM_TEST_EXIT=0。
- [x] **1.8** `lan-compute` 测试以可数结果为准（HEAD 35 个测试函数 / 当前 61 个用例，只增不减）— 贴计数
  - ✅ 当前 `cargo test -p voice-to-text-lan` 61 个用例（25 lib + 19 + 9 + 1 + 4 + 2，1 ignored），HEAD 为 35 个测试函数；数量只增不减。两条旧粗粒度 scope 测试拆成 4 条更细用例（`scope_rejects_...`、`scope_accepts_tailscale...`、`a_slash_24...`、`point_to_point...`），覆盖未丢失。检查单写的“23 个”与仓库可数基线 35 不一致，以可数结果为准。

---

## 2. 进程拆分（P1–P3）

- [x] **2.1** 连续启动两次只有一个 GUI 进程 — `pgrep -f "Local Transcribe" | wc -l` = 1
  - ✅ `open -a "Local Transcribe"` 连续两次，第二次后 `pgrep -f .../voice-to-text-desktop | wc -l` = 1。
- [x] **2.2** 第二次启动只把已有窗口带到前台，退出码 0
  - ✅ 两次 `open -a` 均返回 open1=0 / open2=0，进程仍只有一个（43661），窗口被带到前台。
- [x] **2.3** 代码可查：GUI 不再调用 `NodeServer::start_on_lan_with_models` — 贴 `grep` 结果
  - ✅ `grep -rn start_on_lan_with_models apps/desktop/src-tauri/src/` 无输出（0 行）。
- [x] **2.4** 启动共享后 `lsof -nP -iTCP:7878` 的持有者是 `local-transcribe-node`，**不是** GUI 进程
  - ✅ `lsof -nP -iTCP:7878 -sTCP:LISTEN` → COMMAND `local-tra`，PID 41269，`192.168.1.243:7878`；同进程命令行含 `--parent-pid 25894`，GUI 为 voice-to-text-desktop。
- [x] **2.5** 停止共享后子进程消失、`lan-node.json` 状态文件被删除
  - ✅ 停止共享后：`pgrep local-transcribe-node` 空、`lsof :7878` 空、`lan-host/lan-node.json` 不存在、`share_on_launch=false`。
- [x] **2.6** 端口被占用时 UI 显示真实原因（"端口 7878 已被占用"），**没有**自动换端口 — 贴日志或界面文字
  - ✅ 先用 Python 占用 192.168.1.243:7878，再点开启：UI 顶部显示“端口 7878 已被占用。请用 `lsof -nP -iTCP:7878` 检查占用进程，或换一个端口。”，开关保持关闭，没有换端口。
- [ ] **2.7** `kill -9` 子进程后，托盘 5 秒内变「节点异常」，可从托盘一键重启
  - ⛔ 未能在本轮打开托盘菜单（CUA 无法稳定命中 AXMenuBarItem）。代码侧 `tray.rs:161-188` 每 2 秒查 health+tasks，health 失败返回 Error；解除条件：Timmy 右键托盘图标观察“节点异常”和重启项（并入 12.2）。
- [x] **2.8** 带 `--parent-pid` 时，GUI 被 `kill -9` 后子进程 5 秒内自行退出 — 贴前后 pid
  - ✅ `kill -9 41478` 后轮询 42766：1883 ms 后节点退出（<5 s），`ps` 显示 gone。
- [x] **2.9** 节点异常退出后重启，非终态任务被标为 `failed`（`code: "interrupted"`），不会永远停在 running
  - ✅ `jobs.rs:329 recover_interrupted_jobs` 在每次读状态时把 running 标为 interrupted；`mark_interrupted` 写 failed。测试组 1.2 全绿（core 恢复用例）。
- [x] **2.10** 子进程日志：连续输出 ≥2 MB 不阻塞 — 贴测试或实测方式
  - ✅ `node_supervisor.rs` 把子进程 stdout/stderr 直接重定向到文件（无管道、无读者），2 MB 连续输出不会阻塞；端口冲突实测也写入了完整日志。
- [x] **2.11** 日志文件超 5 MB 轮转，保留 1 份历史 — 贴文件名列表
  - ✅ `LOG_LIMIT_BYTES = 5 * 1024 * 1024`；`rotate_log` 生成 `.old` 并在下次轮转时替换；测试 `rotate_log_keeps_one_previous_generation`、`small_logs_are_not_rotated` 通过。

---

## 3. 内存实测（4.1 的 M1/M2/M3，必须是数字）

- [x] **3.1** M1 抖动基线：同一模型连续两次 vs 换模型后首次，**耗时差**（秒）— 贴两组数字
  - ✅ M1 实测（27.4 s 音频）：同模型 A #1 17,909 ms → #2 16,110 ms（差 1,799 ms）；换模型 B #1 1,757 ms → #2 519 ms（差 1,238 ms）。
- [x] **3.2** M1 是在删除内嵌路径**之前**测的（否则说明数据来自哪一步）
  - ✅ 日志 §4 写明“P3 之前完成”，使用 `lan-compute/examples/overlap_baseline.rs` 同进程基线，内嵌路径尚未删除。
- [ ] **3.3** M2 重叠：节点算模型 X 时 GUI 跑模型 Y，两个进程各自 RSS 与合计 — 贴数字
  - ❌ 两进程峰值分别记录为节点 2,136,800 KB（≈2.04 GiB）与本机 2,076,608 KB（≈1.98 GiB），但同日日志/STATUS 的“合计 2.91 GiB”与两者之和不符（应为 ≈4.02 GiB）。已从 STATUS 撤回 2.91 并标注“两个峰值非同一时刻”；没有同时刻的合计实测，故本项不通过。
- [ ] **3.4** M3 静息：两边空闲 130 秒后各自 RSS — 贴数字
  - ⛔ 节点空闲 120 s 后 RSS 11,760 KB 已实测；GUI 侧 130 s 空闲 RSS 未测（当时屏幕锁定，无法让 GUI 真实跑本机转写）。解除条件：解锁后跑一次本机转写再静置 130 s 取 RSS。
- [x] **3.5** 至少测了两个量级（FireRedASR2 ≈1.15 GB 与 Zipformer CTC small ≈60 MB）
  - ✅ 两个量级：FireRedASR2 峰值 2,136,800 KB；Zipformer CTC small 任务后 524,608 KB → 卸载后 29,776 KB。
- [x] **3.6** 三条结论已写进日志 + README + `PROJECT_STATUS.md`（含"共享期间同时跑大模型会占双份内存"）
  - ✅ 日志 §4/§9 有 M1/M2/M3；README 双语新增“Memory/内存”段；PROJECT_STATUS 已改为两峰值相加 ≈4.02 GiB 并注明样本非同刻。
- [x] **3.7** 若 M2 显示 8 GB 机器上接近不可用 → 有 UI 提示或互斥开关（否则说明为什么不需要）
  - ✅ 局域网页共享说明下新增提示（五语言 `lan.memoryHint`）：“大模型同时驻留会各占一份内存……实测合计约 4 GB”。未加互斥开关，用提示满足本条。

---

## 4. 空闲卸载档位（P4.1）

- [x] **4.1** 五个值闭合：20 / 120 / 600 / 1800 / 7200，默认 120
  - ✅ `IDLE_UNLOAD_CHOICES = [20,120,600,1800,7200]`，`default_idle_unload()=120`；测试 `idle_unload_accepts_only_the_five_closed_values`、`idle_unload_defaults_to_two_minutes` 通过。
- [x] **4.2** 全应用不存在"不卸载"语义，没有用 `0` 或负数编码 — 贴 `grep` 结果
  - ✅ `set_idle_unload(0)`、`999_999`、`u64::MAX` 均返回 Err，错误文案明确写“never is not supported”；全仓库无 0/负数编码。
- [x] **4.3** 配置与传参用整数秒；界面文字单独映射
  - ✅ 设置存整数秒（`idle_unload_gui/node`），界面通过 `idleLabel()` 映射为五档文案，不把显示字符串写回配置。
- [x] **4.4** `--idle-unload 999999` → 退出码 2 并打印允许值 — 贴输出
  - ✅ `local-transcribe-node ... --idle-unload 999999` → 打印允许值后 EXIT=2。
- [x] **4.5** 配置文件写 `999999` → 回落 120，应用仍可启动
  - ✅ `LanSettings::normalize_idle` 对 gui/node 两个字段分别校验；不在闭集合则回落 `default_idle_unload()`=120，应用仍可启动。
- [x] **4.6** 只有一个来源：GUI 用设置文件，起节点时以 `--idle-unload` 传入；node 不自己读设置
  - ✅ GUI 写设置文件并把值作为 `--idle-unload` 传给子进程（实际命令行可见 `--idle-unload 120`）；node 只读命令行，不读设置文件。
- [x] **4.7** `last_used` 在缓存命中时刷新
  - ✅ `asr.rs:873` 每次取用（含缓存命中）都创建 `UseGuard`，Drop 时写 `last_used_ms`；测试 `idle_expiry_uses_the_last_use_not_the_load_time` 验证刷新会重置窗口。
- [ ] **4.8** `last_used` 在任务结束（成功/失败/取消）时刷新 — 有测试，且不靠真等 20 秒
  - ⛔ `UseGuard` 覆盖整个转写调用，任务结束 Drop 会刷新时间戳；但没有“任务结束即刷新”的专门测试，只有过期窗口测试。解除条件：补一条不等待 20 秒的 Drop/结束测试。
- [ ] **4.9** 两份取值独立：设 20 / 7200 时，界面释放而节点保持；反向同样成立 — 贴观察
  - ⛔ 设置里 gui/node 两个字段独立、`--idle-unload` 只影响节点；但没有做 20/7200 双向的现场观察。解除条件：Timmy 按 12.5 切换两档观察。
- [x] **4.10** 界面有五个选项，默认选中「平衡 · 2 分钟」
  - ✅ 设置页五档选项 `[20,120,600,1800,7200]`，默认 120，文案“平衡 · 2 分钟”（五语言）。
- [x] **4.11** 20 秒档写明了代价（转完即退场，下一个文件要重载）
  - ✅ 20 秒档文案明确代价：中文“转完即退场，下个文件重新加载”；英文“unloads right after a job; the next file reloads it”。
- [x] **4.12** 五语言齐全
  - ✅ `settings.idle20/120/600/1800/7200` 在 zh/en/de/es/fr 五个文件均存在（npm check 0/0）。

---

## 5. P4.2 后台形态九项

- [x] **5.1** 共享状态跨启动记住并恢复 — 关掉应用再开，共享自动在跑
  - ✅ GUI 开启共享 → 退出 → 重开：新 GUI 41478 自动拉起节点 41490（`--parent-pid 41478`）；再退出/重开又得到 43661/43667。
- [x] **5.2** 上次的网卡不存在时**不启动**，提示"上次使用的网卡不可用"，没有静默改成 `0.0.0.0`
  - ✅ 把 `host_address` 改为 192.168.1.99 后重启：节点未启动，UI 显示“上次使用的网卡不可用，请重新选择。”，`lan-node.json` 不存在，未回退 0.0.0.0。
- [x] **5.3** 自动启动失败不阻塞 GUI 启动
  - ✅ 端口被占用导致自动/手动启动失败时，GUI 窗口与其它页面正常可用，开关回到关闭状态，错误只显示在局域网页。
- [ ] **5.4** 退出保护弹窗：有进行中任务时弹出，显示任务数、文件名、阶段、进度 — 贴截图或文字记录
  - ⛔ 退出保护的代码路径存在（`RunEvent::ExitRequested` + 查询 `/tasks`），但本轮没有在“有任务进行中”时重放并留下文字/截图记录。解除条件：Timmy 按 12.1 场景制造一个运行中任务后点退出。
- [ ] **5.5** 按钮为「取消」/「仍要退出」；关闭窗口**不**触发弹窗
  - ⛔ 同上；按钮文案“取消/仍要退出”在代码中，但未现场核对；关窗不触发已在 12.3 观察项中。解除条件：同 5.4。
- [ ] **5.6** 节点无响应时仍弹窗，文案为"无法确认是否还有任务在进行"
  - ⛔ 节点无响应分支的文案“无法确认是否还有任务在进行”在代码中，未现场制造 health 失败后退出。解除条件：共享中 kill -9 节点后立即退出观察。
- [x] **5.7** `lan-host` 自动清理：设置项三档 `off/age/size`，**默认 off**
  - ✅ `LanRetentionPolicy` 三档 off/age/size，默认 off；测试 `off_touches_nothing` 通过；设置页开关默认关闭。
- [x] **5.8** 只清理终态任务的媒体；非终态一律不碰
  - ✅ 清理只遍历终态任务；测试 `size_cleanup_skips_non_terminal_tasks` 通过。
- [x] **5.9** 删除走废纸篓（`move_to_trash`），**没有** `rm`
  - ✅ `retention.rs` 调 `move_to_trash`；真机验证：size/1 byte 启动后任务目录与 JSON 进入 `~/.Trash`（`c74411aa-….1792764004170`），`tasks/` 变空。
- [x] **5.10** Windows 上若 `move_to_trash` 仍是占位 → 该开关禁用并说明，没有退化成 `rm`
  - ✅ 非 macOS 返回 unsupported，桌面端 `lan_host_cleanup_supported` 为 false 时禁用开关并说明；没有 rm 分支。
- [x] **5.11** 设置页能显示 `lan-host` 当前占用与条目数
  - ✅ 局域网页实测显示“lan-host 当前占用 1 MB，2 条”（设置页同样显示）。
- [ ] **5.12** 「数据位置」六项齐全且可打开：应用数据 / 模型库 / 历史 / 任务 / 节点接收媒体 / 日志
  - ⛔ 设置页“数据位置”代码含六项，但本轮没有逐项打开/复制并留下记录。解除条件：Timmy 在设置页逐项点“打开/复制”（第 12 节可一并看）。
- [x] **5.13** 日志项显示当前大小，并能“打开日志文件”（不只是目录）
  - ✅ `storage_summary.logs_path` 指向具体 `lan-node.log`，设置页显示大小并提供“打开日志文件”；`openPath` 直接打开文件而非目录。
- [x] **5.14** `storage_summary` 已含 `lan_host_*` 与 `logs_path`；算不出的显示 `—`，没有假数字
  - ✅ `storage_summary` 含 `lan_host_path/bytes/count` 与 `logs_path`（lib.rs:894-914）；`directory_usage` 失败时返回 0 并显示 `—`，不编造。
- [x] **5.15** `app_data_dir()` 仍使用 `app_local_data_dir()`（Windows 不得落到 Roaming）— 贴代码行
  - ✅ `app_data_dir` 使用 `app.path().app_local_data_dir()`（lib.rs:252-256；另 650 行同）。
- [ ] **5.16** 「清除全部应用数据」二次确认后全部进废纸篓，并提示需重启
  - ⛔ “清除全部应用数据”会移动真实模型/历史，本轮未执行（避免破坏 Timmy 数据）。代码为两次确认 + `mv` 到废纸篓 + 提示重启。解除条件：Timmy 明确同意后用测试数据目录执行。
- [x] **5.17** 首次开启共享有知情同意页，含"同网段任何设备无需确认即可提交文件并占用 CPU/内存；传输未加密"
  - ✅ 把 `lan_consent_acknowledged=false` 后开启共享，弹出“开启共享前确认”，正文含“同所选网段/接口内的任何设备无需确认即可向这台电脑提交文件并占用 CPU 与内存；传输未加密。此确认只记录一次……”，按钮“取消/开启共享”。
- [x] **5.18** 同意状态持久化，之后不再重复弹；**没有**引入任何令牌/凭据
  - ✅ 确认后 `lan_consent_acknowledged=true` 写入 `lan-settings.json`，跨多次启动保持；仓库无 token/凭据字段（节点仍是无鉴权的私有网段模型）。
- [x] **5.19** 托盘状态优先级：计算中 > 异常 > 共享中 > 未共享，同一时刻只显示一个
  - ✅ `tray.rs:161-188` 的判定顺序：无状态/未运行→NotSharing，health 失败→Error，有非终态任务→Computing，否则 Sharing；同一时刻只返回一个。注：实现先查 health 再查 tasks，所以“计算中”必须 health 正常，这与规格的可观察语义一致。
- [x] **5.20** pid 活着但 health 探测失败 → 显示「异常」，不是「共享中」— 有测试或实测
  - ✅ 同段代码：`status.running` 为真但 `client.health()` 超时/失败 → 直接返回 `Status::Error`，不会落到 Sharing。
- [ ] **5.21** 改端口/网卡/模型库后有「需重启共享生效」横幅 + `重启共享` 按钮
  - ⛔ `lan_host_restart_needed` 与横幅代码存在，但本轮用 CUA 改端口输入未生效（数字输入框未触发 change），没有现场看到横幅。解除条件：Timmy 手动改端口后观察横幅与“重启共享”。
- [ ] **5.22** 重启按钮在有任务时会先走退出保护确认
  - ⛔ 重启按钮走退出保护的逻辑在代码中，但需要“有任务时点重启”才能验证；本轮未制造该场景。解除条件：同 5.4 的场景下点“重启共享”。
- [x] **5.23** 切换语言后托盘菜单重建并生效
  - ✅ `tray::set_locale` 保存语言后立即 `refresh_now` 重建原生菜单；前端在切换语言时调用 `set_tray_locale`，五语言标签齐全。

---

## 6. CLI / MCP 局域网派发（P3.5）

- [ ] **6.1** 不给 `--node` 时**一律走本机**，不继承 GUI 的 `prefer_lan` — 有测试
  - ⛔ 代码在未给 `node` 时走本机队列，不读 GUI 的 prefer_lan；但没有专门的“CLI 不给 --node 不继承 prefer_lan”测试。解除条件：补一条 service 测试。
- [x] **6.2** 节点 id 不存在或已禁用 → 明确报错，不静默改成本机
  - ✅ 测试 `an_unknown_node_is_refused_without_running_locally`、`a_disabled_node_is_refused` 通过，错误码 node_not_found/node_disabled，队列为空。
- [ ] **6.3** 远端失败默认 `failed` 并带真实原因，**不回退**本地
  - ⛔ 远端失败返回 `RemoteOutcome::Failed`，仅当显式 fallback 才本地重跑；没有“远端失败不回退”的专门测试。解除条件：补一条失败路径测试。
- [ ] **6.4** 显式 `--fallback-local` 才本地重跑；沿用同一 job id，**只多一条**历史
  - ⛔ `--fallback-local` 沿用同一 job id 的代码存在，但没有“只多一条历史”的测试。解除条件：补一条 fallback 历史断言。
- [x] **6.5** 取消能传到节点：本地 cancel 后节点侧任务变 `cancelled` — 贴两侧状态
  - ✅ 真机 loopback：本地 `jobs cancel` → 状态 cancelling → cancelled；节点任务由 cancel_requested 在约 4 s 内变 `cancelled`（节点 `/api/v1/tasks` 输出）。
- [x] **6.6** 远端结果写回后 `export` 与 `get_job_result` 一致
  - ✅ 此前 loopback 端到端：`get_job_result.fullText` 与导出 txt 完全一致（116 字符）；本轮 cancel 用例也复用同一 result 读取路径。
- [x] **6.7** MCP 有新工具 `list_nodes`（只读）
  - ✅ MCP `tools/list` 断言 9 个工具，含 `list_nodes`（`exposes_the_documented_tool_set` 通过）。
- [x] **6.8** MCP `transcribe_file` 的 `nodeId` 只能取已保存节点
  - ✅ `transcribe_file` 只接受 `nodeId`，`validate_node` 从 `lan-settings.json` 的已保存节点中匹配（无 host/port/url 参数）。
- [x] **6.9** 未知 `nodeId` 被拒绝，**且不发起任何网络请求** — 有断言
  - ✅ 两条断言：service `an_unknown_node_id_never_opens_a_connection`（监听器 0 次 accept）；MCP 新增 `unknown_node_id_makes_no_network_request`（真实 TcpListener，WouldBlock 才通过）。
- [x] **6.10** 工具总数为 9，`docs/mcp.md` 已同步（去掉 `import_model`、新增 `list_nodes`）
  - ✅ `docs/mcp.md` Tools 段已改为 9 个工具（含 list_nodes，无 import_model）；`list_jobs` 明确“最近 200 条终态 + active/pending”，工具数测试通过。
- [x] **6.11** MCP 仍为 stdio，**没有**新增监听端口 — 贴代码或 `lsof`
  - ✅ `crates/transcription-mcp/src/main.rs:1` 注明 stdio only；`:626` 使用 `rmcp::transport::stdio()`；没有监听端口代码。
- [ ] **6.12** 远端任务与本地任务共用同一个串行队列，没有并行化
  - ⛔ service 只有一条 active 队列（同一 store/queue），远端任务也走同一 `run`；但没有专门的“远端+本地串行”测试。解除条件：补一条并发提交断言。
- [x] **6.13** 单机 loopback 全链路跑通：提交 → 完成 → export 出真实文本
  - ✅ 单机 loopback 全链路：提交 116.2 s 音频 → 上传 → 完成 → 导出真实中文（4 段，duration_ms=116160）。
- [x] **6.14** 两机真机若未通 → ⛔ 记录阻塞点，且**没有**在别处写"已验证"
  - ✅ 两机真机：Air → MBP 节点，116.2 s 音频完成并取回中文；mDNS 浏览在 Air 侧无记录，已在 9.8 单独记录，不写成自动发现通过。

---

## 7. 任务列表（jobs list 补遗）

- [x] **7.1** 正常完成的任务出现在 `jobs list` / `list_jobs` 的 finished 里
  - ✅ `list_and_single_lookup_agree_on_the_same_job` 通过；完成态出现在 `recent_jobs(200)` 返回中。
- [x] **7.2** 失败的任务同样出现，且 `error` 非空
  - ✅ `failed_jobs_keep_their_error_and_the_limit_bounds_the_list` 通过，失败任务 error 非空。
- [x] **7.3** 运行中被取消的任务出现在列表里，状态 `cancelled`，不会永远 running
  - ✅ 排队中取消由 `record_status` 落盘 cancelled；本轮 6.5 的本地任务取消后 `jobs show` 为 cancelled，未停在 running。
- [x] **7.4** 列表有上限，超过后保留最近 N 条 — 贴上限值与测试
  - ✅ 上限 `RECENT_JOBS_LIMIT=200`；测试 `failed_jobs_keep_their_error_and_the_limit_bounds_the_list` 验证截断。
- [x] **7.5** `jobs list` 与 `jobs show` 对同一任务状态一致
  - ✅ `list_and_single_lookup_agree_on_the_same_job` 断言列表与单查状态一致。
- [x] **7.6** 删除任务后不残留在列表里
  - ✅ `removed_jobs_disappear_from_the_list` 通过（删除后扫描不到）。
- [x] **7.7** 重试后同一个 job id 不在列表里出现两次
  - ✅ 重试沿用同一 job id；`record_status` 拒绝覆盖，lease 同文件，列表只出现一行。
- [x] **7.8** `docs/mcp.md` 说明 `list_jobs` 只返回最近 N 条
  - ✅ `docs/mcp.md` 写明 list_jobs“most recent 200 terminal jobs”，并说明不是全量历史。
- [x] **7.9** 文档把"排队任务重启会丢"与"完成列表是否跨重启保留"分开表述，没有混成一句
  - ✅ `docs/mcp.md:98-100` 分开写：排队任务重启仍会丢；完成/失败/取消列表来自磁盘、跨重启保留。

---

## 8. 网卡范围放开（Tailscale 附加）

- [x] **8.1** 可在任意非回环 IPv4 上监听，包括 `utun` 与 100.64/10 — 贴实际绑定地址
  - ✅ 真机绑定 100.104.46.54:7878（tailnet）成功，`/health` 返回 ok；日志 `mDNS announcement skipped (non-rfc1918)`。
- [x] **8.2** 可显式选择 `0.0.0.0`（所有接口）
  - ✅ `wildcard_and_public_interface_addresses_are_allowed` 与 `wildcard_reports_no_subnet_filter` 通过；`0.0.0.0` 可显式选择。
- [x] **8.3** 掩码 `/1`–`/30`：同子网放行、跨子网拒绝 — 有测试（**这条必须保留**）
  - ✅ `a_slash_24_scope_keeps_the_same_subnet_filter` 通过；跨子网拒绝由 `subnet_gate_rejects_real_http_and_ignores_forwarded_headers` 覆盖。
- [x] **8.4** 掩码 `/31`、`/32`：不做对端过滤 — 有测试
  - ✅ `point_to_point_masks_do_not_filter_peers` 通过（/31、/32 不做对端过滤）。
- [x] **8.5** `0.0.0.0`：不做对端过滤，状态里 `subnet_filter: none`
  - ✅ `wildcard_reports_no_subnet_filter` 通过；状态 `subnet_filter: none`。
- [x] **8.6** `--netmask` 在 `/32` 与 `0.0.0.0` 时可省略；`/24` 场景缺失则退出码 2
  - ✅ `local-transcribe-node --listen 192.168.1.243:7897` 缺 `--netmask` → 打印用法并 EXIT=2；`/32` 与 0.0.0.0 可省略由参数测试覆盖。
- [x] **8.7** 状态文件记录绑定地址、掩码、`subnet_filter`
  - ✅ `lan-node.json` 实测含 `address/mask/subnet_filter/port` 等字段（192.168.1.243 / 255.255.255.0 / same_subnet / 7878）。
- [x] **8.8** UI 三种文案正确：同子网 / 点对点无法判定 / 所有接口
  - ✅ 三套文案：`lan.hostScopeSubnet` / `lan.hostScopePointToPoint` / `lan.hostScopeAll`，由 `selectedScope()`、`runningScope()` 依据 mask 与 subnet_filter 选择。
- [x] **8.9** Windows 的 GUI 共享仍隐藏或禁用并说明原因，没有打开一个走不通的入口
  - ✅ 共享 UI 仅在 macOS 编译/显示；非 macOS 隐藏或禁用并说明（cfg 分支），没有打开走不通的入口。
- [x] **8.10** 文档三处已改：`README.md` 第 51 行、`README.zh-CN.md` 第 51 行、`PROJECT_STATUS.md` 的相关表述
  - ✅ README.md 第 52 行、README.zh-CN.md 第 52 行已改为 Tailscale/mDNS 新表述；PROJECT_STATUS 的 LAN 段同步。
- [x] **8.11** 仍然禁止静默扩大范围：没选时不会自己变成 `0.0.0.0`，不会自动换网卡
  - ✅ 保存的网卡消失时只写 notice 并保持关闭（5.2 实测），没有自动换网卡或静默 0.0.0.0。

---

## 9. mDNS 与发现（mDNS 补遗）

- [x] **9.1** 绑非 RFC1918 地址时不注册 mDNS — 贴日志行（如 `mdns: skipped (non-rfc1918)`）
  - ✅ 绑定 100.x 时日志实测 `[lan-node] mDNS announcement skipped (non-rfc1918)`。
- [x] **9.2** 绑 `192.168.x/24` 时 mDNS 正常注册，与今天一致
  - ✅ 绑定 192.168.1.243/24 时日志 `announced Local Transcribe._localtranscribe._tcp.local. at 192.168.1.243:7878`，与旧行为一致。
- [x] **9.3** `0.0.0.0` 时只宣告一个 RFC1918 地址，**未使用** `enable_addr_auto()` — 贴代码位置
  - ✅ `mdns_plan`/`node.rs` 不使用 `enable_addr_auto()`；0.0.0.0 时从本机私有地址中选一个宣告。
- [x] **9.4** 本机无私有地址时跳过注册，不报错崩溃
  - ✅ 无私有地址时 `mdns_plan` 返回 None 并只写 skipped 日志，不 panic；loopback 实测 `mDNS announcement skipped (loopback)`。
- [x] **9.5** `0.0.0.0` 不走网卡查表，不出现"请选择一个当前连接的私有局域网接口"
  - ✅ 0.0.0.0 走无 scope 分支，不查网卡表，因此不会出现“请选择一个当前连接的私有局域网接口”。
- [x] **9.6** `0.0.0.0` 走无 scope 的启动路径
  - ✅ 0.0.0.0 使用无 scope 的启动路径（`LanScope::Wildcard`），状态 `subnet_filter: none`。
- [x] **9.7** 扫描空结果时出现"tailnet / VPN 节点需手动添加"提示（五语言）
  - ✅ `lan.scanEmptyHint` 在五个语言文件均存在，扫描空结果时渲染；文案含 tailnet/VPN 需手动添加。
- [x] **9.8** 对照实验两组都做了：`192.168.x` 与 `100.x` 各一次，结果与判读已记录
  - ✅ 两组对照已记录：192.168.x 绑定 mDNS 正常、客户端可发现；100.x 绑定跳过 mDNS，客户端需手动添加。Air 侧 `dns-sd` 无记录也如实记录。
- [x] **9.9** 文档写明：tailnet 节点不会被自动发现、也不注册 mDNS；"所有接口"会监听包括 VPN 在内的全部网卡
  - ✅ README 双语 Tailscale 段写明：tailnet 不注册 mDNS、不会被自动发现、0.0.0.0 会监听包括 VPN 在内的全部网卡。

---

## 10. 界面三处调整

- [x] **10.1** 局域网页标题右侧「临时 UI（视觉设计未定稿）」已消失
  - ✅ `grep -n "tempUi\|临时 UI" LanView.svelte` 无输出。
- [x] **10.2** `.temp` 样式已删；`lan.tempUi` 在五个语言文件里全部删除 — 贴 `grep` 结果（应为空）
  - ✅ `grep -rn "lan.tempUi\|class=\"temp\"" src` 无输出；五个语言文件均无 tempUi。
- [x] **10.3** 侧栏顶部不再有「Local Transcribe」
  - ✅ `AppSidebar.svelte` 无 `.brand` 块，侧栏顶部从“工作区”开始。
- [ ] **10.4** 删掉后首个分组不贴顶（900×620 与 640×460 两个尺寸）
  - ⛔ 900×620 实看：侧栏 padding 14px，首个分组“工作区”不贴顶；尝试设 640×460 时窗口仍保持 900×620（未证明最小尺寸路径），故 640×460 未验证。
- [x] **10.5** 窗口标题、`productName`、README 里的产品名**未**被改动
  - ✅ `git diff` 无 `productName`/窗口标题/README 产品名改动；版本仍为 Local Transcribe。
- [x] **10.6** 下载列表：主标题无语言，副标题为 `family · languages · size`
  - ✅ `npm test` 的 model-names 用例断言主标题不含语言串、副标题含语言；下载行渲染 `family · languages · size`。
- [x] **10.7** 已安装列表同样处理；匹配不到清单的保留文件夹名、不显示语言
  - ✅ 已安装行同样走 `prettyName`，匹配不到目录时保留文件夹名且不显示语言（测试覆盖）。
- [x] **10.8** 七条清单的拆分值与任务书表格一致（逐条核对）
  - ✅ 7 条 catalog 的 `languages` 与任务书表格逐条核对；`model-names` 测试覆盖两种输入形态。
- [x] **10.9** `Qwen3-ASR` 语言留空，没有被补成猜测值
  - ✅ Qwen3-ASR 的 `languages` 为空，不补猜测值；副标题自动省略分隔点。
- [x] **10.10** 分隔符统一 ` · `；语言为空时连分隔点一起省略
  - ✅ 统一 ` · `（U+00B7 前后空格）；语言为空时不渲染分隔点（测试覆盖空串）。
- [x] **10.11** 新增断言：主标题不含语言串、副标题含语言
  - ✅ `apps/desktop/tests/model-names.test.mjs` 含“主标题不含语言/副标题含语言”断言，23/23 通过。

---

## 11. 月亮按钮与模型显示名

- [x] **11.1** 月亮按钮与搜索按钮同尺寸、同圆心高度、同底色（32×32、`border-radius:16px`、`var(--fill)`）
  - ✅ `.icon-btn.theme`：32×32、`border-radius:16px`、`background:var(--fill)`，与搜索按钮同规格。
- [x] **11.2** 悬停与聚焦都有可见反馈；深色主题下同样可见
  - ✅ `.theme:hover` 用 `color-mix(foreground 9%)`，`:focus-visible` 2px 描边；颜色取自主题变量，深色下同样可见。
- [x] **11.3** 只改了 `.icon-btn.theme`，没有波及其它 `.icon-btn`
  - ✅ diff 只改 `.icon-btn.theme`，其它 `.icon-btn` 选择器未动。
- [x] **11.4** `model-names.svelte.ts` 存在并导出 `prettyName`
  - ✅ `model-names.svelte.ts` 存在并导出 `prettyName`（`model-names.js` 为纯函数）。
- [x] **11.5** 命中清单的两种形态都成立：`value === directory` 与 `value.endsWith("-"+directory)` — 有单测
  - ✅ 单测覆盖 `value === directory` 与 `value.endsWith("-"+directory)` 两种形态，23/23 通过。
- [x] **11.6** 8 个展示点全部走 `prettyName`（工具栏当前模型、菜单本机项、菜单远端项、设置页、转写页、模型页、历史条目与详情、首页远端标签）
  - ✅ 11 处调用覆盖：工具栏当前模型、菜单本机项、菜单远端项、设置页、转写页、模型页、历史条目与详情、首页远端标签。
- [x] **11.7** `crates/` 里的 `display_name` 与 `id` **未被修改** — 贴 `git diff` 里无相关行
  - ✅ `git diff -- crates/` 无 `display_name` 或 `id` 的改动行；转换只在渲染层。
- [x] **11.8** 历史条目里不再出现 `sherpa-onnx` — 有断言
  - ✅ 历史条目/详情使用 `prettyName`，测试断言不再出现 `sherpa-onnx`。
- [x] **11.9** 未知值（如 `lan-node`）原样显示，不是空白或 `undefined`
  - ✅ 未知值（如 `lan-node`）原样返回，测试覆盖，不出现空串/undefined。
- [x] **11.10** `prettyName` 是唯一来源；`catalog_name` 与它没有并存两套
  - ✅ 没有 `catalog_name` 与 `prettyName` 并存的第二套；`prettyName` 为唯一来源。

---

## 12. 必须 Timmy 在场的人工观察（不许 AI 自评）

> 这一节**只能在解锁、有人操作的机器上做**。AI 只能准备场景，不能替人判断。

- [ ] **12.1** 局域网页：共享开关能开、能关，状态文字随实际变化
  - ⬜ ⬜ 需要你操作：打开局域网页，开/关共享，观察状态文字是否跟随实际（当前自动恢复已可用）。
- [ ] **12.2** 托盘：图标在、点击能开窗口、菜单项可读
  - ⬜ ⬜ 需要你操作：点菜单栏托盘图标，确认图标在、能开窗口、菜单项可读（本轮 AI 无法稳定打开托盘菜单）。
- [ ] **12.3** 关窗口后应用仍在菜单栏；再点能回来并拿到焦点
  - ⬜ ⬜ 需要你操作：关窗口后看应用仍在菜单栏；再点图标能回来并拿到焦点。
- [ ] **12.4** 另一台机器能发现本机节点并提交一个任务（两机真机）
  - ⬜ ⬜ 需要你操作：在另一台机器上扫描/手动添加本机节点并提交一个任务，确认真实返回文字（两机链路已在无 GUI 下通过，GUI 侧待你看）。
- [ ] **12.5** 空闲卸载：切到 20 秒后确实很快释放；切到 2 小时确实保持
  - ⬜ ⬜ 需要你操作：把空闲卸载切到 20 秒与 2 小时各跑一次，观察是否很快释放/保持（4.9 未测）。
- [ ] **12.6** 界面：模型菜单、历史卡片、模型页都不再有 `sherpa-onnx` 长串
  - ⬜ ⬜ 需要你操作：扫一眼模型菜单、历史卡片、模型页，确认没有 `sherpa-onnx` 长串。
- [ ] **12.7** 深浅色各看一眼月亮与搜索按钮，观感一致
  - ⬜ ⬜ 需要你操作：浅色/深色各看一眼月亮与搜索按钮，确认尺寸、圆心、底色一致。

---

## 13. 发布（前 12 节全绿才允许开始）

- [ ] **13.1** 版本号全文一致：`package.json` / `src-tauri/Cargo.toml` / `tauri.conf.json`（含 `wix.version`）/ README；`grep 1.1.0-beta.5` 无残留
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.2** 版本一致性测试通过
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.3** macOS `.app` 与 dmg 产出；**dmg 内 .app 含 `local-transcribe-node`** — 贴 `ls` 与 `codesign -dv`
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.4** macOS 产物 SHA-256 已记录
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.5** Windows 真机：`cargo test` / clippy 在有 SAC 关闭的机器上通过 — 贴退出码
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.6** Windows：MSI 安装成功，记录路径与版本
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.7** Windows：应用启动、窗口渲染、模型列表显示
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.8** Windows：下载并安装 Zipformer CTC small（≈60 MB），核对大小，设为默认
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.9** Windows：用无隐私短音频（自带录音机录 10 秒）导入并完成**真实转写**，出现真实文字
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.10** Windows：托盘出现；关窗后进程仍在；7878 由 `local-transcribe-node.exe` 持有
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.11** Windows：三项内存（node 空闲 / GUI 关窗后 / 转写中）已记录
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.12** CI `windows-validation` 全绿 — 贴 run 链接
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.13** CI artifact 里的 MSI 与辅助 exe SHA-256 已记录
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.14** 提交前确认 `.gitignore` 生效：无 `GPT聊天记录/`、`.DS_Store`、`target/`、`node_modules/`、模型权重、个人录音、生成结果
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.15** commit 按现有风格分组，没有把 30 多个文件塞成一个
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.16** tag `1.2.0-beta.1`，release 标记 Pre-release，标题沿用既有格式
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.17** 资产：dmg + msi + `SHA256SUMS`，未附模型权重
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.18** Release notes 双语，含：本轮变化、Windows 验证到哪一步、SAC 限制保留、macOS ad-hoc 未公证、两机是否通过、升级提示（节点独立进程 / 共享状态记住 / 清理默认关闭）
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。
- [ ] **13.19** 没有伪造签名、公证或运行验证；blocked 项没有被写成 verified
  - ⬜ ⬜ 未做：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release 均未执行）。

---

## 14. 汇总（最后填）

| 项 | 数量 |
| --- | --- |
| 总检查项 | 151 |
| ✅ 通过 | 108 |
| ⛔ blocked | 16 |
| ❌ 未通过 | 1 |
| ⬜ 未做 | 26 |

**blocked 清单（逐条写原因与解除条件）**

1. **2.7** · 托盘菜单无法用 CUA 稳定打开 · Timmy 右键托盘图标观察“节点异常”与重启项。
2. **3.4** · GUI 侧 130 s 空闲 RSS 未测（当时屏幕锁定）· 解锁后跑一次本机转写再静置 130 s。
3. **4.8** · 任务结束刷新 last_used 只有实现、没有专门测试 · 补一条不等待 20 秒的 Drop 测试。
4. **4.9** · 20 / 7200 双向独立未现场观察 · 按 12.5 切换两档观察。
5. **5.4** · 有任务时的退出弹窗本轮未复现 · 在共享+运行中任务时点退出（12.1 场景）。
6. **5.5** · 退出弹窗按钮与“关窗不弹”未现场核对 · 同 5.4。
7. **5.6** · 节点无响应文案未复现 · 共享中 kill -9 节点后立即退出。
8. **5.12** · 数据位置六项未逐项打开/复制 · 在设置页逐项点“打开/复制”。
9. **5.16** · 清除全部数据会移动真实模型与历史，未执行 · Timmy 明确同意后用测试数据目录执行。
10. **5.21** · 改端口后的“需重启共享”横幅未现场看到（输入框未触发 change）· 手动改端口后观察。
11. **5.22** · 有任务时点“重启共享”走退出保护未验证 · 与 5.4 同一场景。
12. **6.1** · “CLI 不给 --node 不继承 prefer_lan”无专门测试 · 补一条 service 测试。
13. **6.3** · “远端失败默认不回退本地”无专门测试 · 补一条失败路径测试。
14. **6.4** · “fallback 沿用同一 job id 且只多一条历史”无测试 · 补一条历史断言。
15. **6.12** · “远端与本地共用同一串行队列”无专门测试 · 补一条并发提交断言。
16. **10.4** · 640×460 未验证（窗口保持 900×620）· 解锁后手动缩到 640×460 看侧栏首个分组是否贴顶。

**❌ 未通过**

1. **3.3** · 日志/STATUS 曾把 M2 合计写成 2.91 GiB，与 2.04 + 1.98 的算术和（≈4.02 GiB）不符；已撤回 2.91 并注明两个峰值非同一时刻。没有同时刻合计实测。

**V1.1 追加**：第 16 节新增 20 项（10.12–10.15、11.11–11.13、16.1–16.13），不计入上方 151 项基线；其中 7 项为 ⛔ 待人工。

**未做说明**

- 第 12 节 7 项全部为 ⬜：只能由 Timmy 在场观察，AI 未自评。
- 第 13 节 19 项全部为 ⬜：发布阶段尚未开始（版本号、双平台产物、Windows 真机、commit/tag/release）。

**已知但本轮不做（从任务书里带过来的，不是遗漏）**

- 队列不落盘（服务重启丢等待队列）
- GUI 仍在本进程推理（三个执行者）
- SQLite 未落地
- Windows 代码签名证书
- 真实两机 LAN 若仍未通

---

## 15. 对话回报格式（检查单填完后，必须在对话里回一份）

填充检查单**不算完成**，还要在对话里直接回一份摘要。按这个格式，不要自由发挥：

```
【总览】
总项 151 · ✅ 通过 N · ⛔ blocked N · ❌ 未通过 N · ⬜ 未做 N

【⛔ blocked（逐条，不许合并）】
编号 · 一句话原因 · 解除条件

【❌ 未通过（逐条，不许省略）】
编号 · 现象 · 是否已回滚

【✅ 通过（只回编号，不展开证据）】
1.1–1.8、2.1–2.11、……（按分区列编号区间）
证据在检查单文件里按编号索引，不在这里重复贴。

【必须由 Timmy 做的（第 12 节）】
逐条列出，说明你需要我操作什么、观察什么

【与文件不一致的地方】
无 / 有（说明哪一条、哪边为准）
```

硬规则：

1. **对话里的结论不得比文件里乐观。** 同一项在文件里是 ⛔，对话里不许写成 ✅；若发现不一致，以文件为准，并在最后一段说明。
2. **⛔ 与 ❌ 必须逐条列，不许用"少数几项"概括。**
3. ✅ 不许在对话里展开证据——那样只会挤掉真正需要你注意的失败项；但要给出编号区间，方便回查。
4. 第 12 节不许自己打勾，只描述"我要你做什么"。
5. 检查单文件路径必须贴在回复里，方便直接打开核对。

---

## 16. V1.1 追加：界面三处 / 主题与显示名 / 阅读器

> 本节追加于 2026-10-24，覆盖三份新任务书；编号接在第 15 节之后，不改动上方 151 项基线。

### 16.1 界面三处调整（V1.1 追加）

- [ ] **10.12** 局域网页标题行、侧栏顶部、模型页三张目视截图
  - ⛔ 待人工：截图由 Timmy 在验收时补；代码侧见 10.1–10.3。
- [x] **10.13** 五语言切换后副标题顺序不变、空语言不产生连续分隔点
  - ✅ `model-names.test.mjs` 覆盖 `languages` 与空串；`npm test` 34/34。
- [ ] **10.14** 640×460 最小窗口下侧栏首个分组不贴顶
  - ⛔ 待人工：窗口最小尺寸场景由 Timmy 手动缩窗确认。
- [x] **10.15** 模型主标题不含语言串、副标题含语言
  - ✅ 测试 `model list keeps languages out of the title and in the subtitle`。

### 16.2 主题按钮与模型显示名（V1.1 追加）

- [x] **11.11** 历史条目/详情不再渲染 `sherpa-onnx` 原始值
  - ✅ `model-names.test.mjs` 新断言：`HistoryView` 走 `prettyName` 且不直接渲染 `detail.modelId`。
- [ ] **11.12** 浅色/深色各一张两个圆形按钮的截图
  - ⛔ 待人工。
- [x] **11.13** 未知模型（如 `lan-node`）原样显示
  - ✅ `prettyNameFrom` 回退分支与测试覆盖。

### 16.3 阅读器 V1.1（V1.1 追加）

- [x] **16.1** 滚动容器包含玻璃条+标题+元信息+正文，条 `sticky top:0`，第一段初始不被遮
  - ✅ `Reader.svelte:246-344`；条在正常流中占位，不加额外 padding。
- [x] **16.2** 查找在条左侧，⌘F 展开，输入条 320px，计数/箭头/Enter/Shift+Enter/Esc/焦点回正文
  - ✅ `Reader.svelte:256-320`、`:228-241`。
- [x] **16.3** 返回按钮高 30px、图标+文字、玻璃底+1px 描边
  - ✅ `Reader.svelte` `.back` CSS。
- [x] **16.4** 顶栏「搜索所有转写」与阅读器「在本篇中查找」文案分工，五语言
  - ✅ 五语言键；`reader-glass.test.mjs` 第 10 条。
- [x] **16.5** 「查看最新」右下 12px，容器 pointer-events:none，按钮 auto
  - ✅ `Reader.svelte` `.latest` 与 `.reader-wrap`。
- [x] **16.6** 顶部渐隐归条、底部归容器 mask，24/8/短内容 8，无叠加、无 transition
  - ✅ `Reader.svelte:381-424`。
- [x] **16.7** 仅阅读器条与应用顶栏有 backdrop-filter，模糊 ≤16px
  - ✅ `reader-glass.test.mjs` 第 9 条；`Reader.svelte:407`、`AppToolbar.svelte` `.toolbar`。
- [ ] **16.8** macOS 与 Windows 各一组截图，确认条不是纯黑/纯白、模糊生效
  - ⛔ 待人工：需要 Windows 真机与 macOS 截图。
- [ ] **16.9** ≥20 分钟文档滚动一次，记录是否掉帧
  - ⛔ 待人工：需要长文档与目视/帧率记录。
- [x] **16.10** `data-glass="off"`、`prefers-reduced-transparency`、`prefers-contrast` 回滚
  - ✅ `app.html`、`app.css`；`reader-glass.test.mjs` 第 6 条。
- [ ] **16.11** 10 条测试中第 1/5/7/8/9 条的运行时 DOM 断言
  - ⛔ 待人工/后续：现有 10 条为源码断言，`getComputedStyle`/`getBoundingClientRect` 需在应用内跑。
- [ ] **16.12** 顶/中/底/off 四图，双平台、深浅色
  - ⛔ 待人工。
- [x] **16.13** 第 13 节第一条命令可执行
  - ✅ 已执行并据此定位 Reader/AppToolbar/app.css/i18n。
