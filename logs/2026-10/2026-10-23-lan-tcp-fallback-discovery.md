# 局域网发现：mDNS 之外增加 TCP 回退（2026-10-23）

## 问题

Timmy 反馈两台 Mac 互相搜不到对端。实测结论分三层：

1. 代码与服务类型正常：在 Air 上用同一份 `local-transcribe-node` 跑临时节点（`AIR-probe`），MBP 的系统 Bonjour 能浏览到 `_localtranscribe._tcp`。
2. MBP 上普通进程无法收发局域网多播：Python、Rust（`mdns-sd`）、`launchctl` 拉起的进程发送 `224.0.0.251:5353` 均返回 `EHOSTUNREACH`；同一时刻 `/usr/bin/nc` 却能连上 `192.168.1.184:22`，`/usr/bin/python3` 连同一地址失败。Air 上同类测试全部正常。系统自带的 mDNSResponder 不受影响（Air 能解析 MBP 的临时注册）。
3. Air 的 `lan-settings.json` 当时是 `local_only`，7878 未监听（共享关闭），因此即使发现逻辑正常，Air 也没有可被发现的服务；另外 Air 节点列表里有一条 `name=192.168.1.243 / host=Mac` 的字段写反记录。

Timmy 明确：**不使用 Apple Bonjour API**（软件跨平台，Windows 也要用同一套发现逻辑）。因此采用纯 Rust、跨平台的 TCP 回退方案。

## 变更

- `crates/lan-compute/src/client.rs`
  - 新增 `local_scan_hosts()`：用 UDP `connect` 取本机出口地址（不读网卡表、无新依赖、跨平台），生成同网段 /24 主机列表（排除自身；非私有地址返回空）。
  - 新增 `scan_blocking()`：按主机 × 端口并发（最多 64 线程）TCP 探测，连接超时 250 ms、响应 800 ms；对每个成功连接读取 `GET /api/v1/capabilities`，仅当返回 200 且能解析出 `proto_version` 才认定为节点。
  - 新增 `merge_discovered()`：mDNS 结果优先，TCP 结果按 `host:port` 去重补位。
  - 新增 `discover_with_scan()`：先补端口表里的自定义端口，再跑 TCP 扫描，剩余时间用于 mDNS 浏览；总耗时仍受调用方给的超时约束。
- `crates/lan-compute/src/protocol.rs`：`Capabilities` 增加 `name`（`#[serde(default)]`，旧节点不返回时退回显示 IP）。
- `crates/lan-compute/src/node.rs`：`AppState` 记录节点名，`/api/v1/capabilities` 返回 `name`。
- 桌面端：`lan_discover` 接受已保存节点的端口列表并改走 `discover_with_scan`；界面扫描窗口 3 s → 8 s，空结果时新增提示（本地网络权限 / 对方未共享）。
- 五语言新增 `lan.scanEmptyHint`。
- `apps/desktop/package.json` 新增 `bundle:mac`：`tauri build --bundles app --no-sign` 之后用 `codesign --force --sign - --identifier com.timmy.voicetotextapp` 重签，使代码标识与 `CFBundleIdentifier` 一致（此前每次构建是 `voice_to_text_desktop-<hash>`，不稳定）。

## 验证

- TDD：先写 `crates/lan-compute/tests/scan_discovery.rs`（3 项）与 `apps/desktop/tests/lan-discovery.test.mjs`（3 项），确认按预期失败（编译缺失符号 / 断言失败），再实现至全绿。
- `cargo test --workspace --offline`：全部 test result ok，0 failed（真实权重/真实接口用例仍按原样 ignored）。
- `cargo clippy --workspace --all-targets --offline -- -D warnings`、`cargo fmt --all -- --check`：通过。
- `npm test` 12/12；`npm run check` 0 errors / 0 warnings；`npm run build` 通过。
- `npm run bundle:mac`：26.06 MiB，`codesign -dv` 显示 `Identifier=com.timmy.voicetotextapp`，`codesign --verify --deep --strict` 通过（仍是 ad-hoc，未公证）。
- 真机：两台均装 `8a5bacec…22e73`。在 Air 上跑临时节点 `MBP-node-7878`（MBP:7878）后，用同一份 `discover_with_scan` 探针在 Air 侧成功发现该节点；TCP 单测用 127.0.0.1 夹具验证「只认 capabilities，不认裸端口」。
- **未完成**：MBP 侧应用内的实际点击验证被屏幕锁定挡住（CuaDriver 报 screen_locked）；MBP 上重新授权后的 mDNS 是否直接可用也未复测。

## 待 Timmy 处理

- 解锁后在 MBP 应用「局域网」页点一次“扫描”；若 macOS 因重新签名再次询问本地网络权限，请在弹窗允许。
- 要在 MBP 上发现 Air，需要 Air 端把「允许其他设备使用本机算力」打开（目前是关的）。

## 追加：权限提醒、文案清理与首页远程模型（同日）

- 局域网页新增权限横幅：`local_network_access()` 用一次丢弃式多播发包判断本进程能否使用局域网（`PermissionDenied` / `HostUnreachable` → Denied），页面加载和每次扫描后各检测一次；被拒时显示标题、说明、`打开系统设置`（跳 `x-apple.systempreferences:...Privacy_LocalNetwork`）与`重新检测`。探测在应用进程内执行，因此反映的是该应用自己的授权状态：MBP 应用内实测为 granted（不误报），同一台机器的普通 shell 进程为 denied。
- 删除共享区块里的「可用模型：…」与「关闭共享或退出应用会…」两段文案（五语 key 一并移除）。
- 修复节点列表中「转写模型」下方的大块空白：`.list` 是纵向 flex 容器，`.field { flex: 1 1 180px }` 在纵向轴上被撑成 180px 高；改为 `.list > .field { flex: 0 0 auto }`。
- 首页右上角模型菜单改为分组：`本机` 段列本地模型，每个已启用节点单独一段列该节点模型（`toolbar.localSection` / `toolbar.remoteTag`，五语）；选中远程模型会 `set_lan_settings` 把该节点设为启用并选定模型，同时把模式切到 `prefer_lan`；改选本地模型则把模式切回 `local_only`。菜单顶部按钮显示 `节点名 · 模型名`。
- 测试：新增 `apps/desktop/tests/home-model-groups.test.mjs`（先红后绿）；`npm test` 15/15、`check` 0/0、`build` 通过。新包 `bf78401d…471385` 已装两台（备份 `before-ui-tweaks-20261023-1915`），`Identifier=com.timmy.voicetotextapp`。
- 验收交由 Timmy 自行完成，本轮不代替验收。

## 追加：局域网页收窄为任务视图；首页模型点状态修复（同日）

- 按 Timmy 要求收窄局域网页职责：移除「计算方式」分段控件、节点卡片里的「转写模型」下拉、任务区的「发送文件…」按钮，以及配套的 `sendFile`/`changeRemoteModel`/`changeMode`、上传进度监听与 `lan_submit_file` 调用。现在该页只保留：共享开关、节点管理（添加/测试/编辑/启停/删除）、选中节点的任务列表与结果保存/回收。
- 模式改由首页模型菜单决定：选远端模型 → `prefer_lan`，选本机模型 → `local_only`；设置页的「局域网计算」条目改为只读说明并保留跳转。
- 文案：`lan.subtitle` 改为说明「在首页选远端模型后导入的文件在该设备转写」；删除 `lan.sendFile`/`lan.sending`/`lan.remoteModel`/`lan.remoteDefault` 与分段控件样式（五语）。
- Bug 修复：首页右上角模型按钮的圆点在选中远端模型后变灰。原因是指示灯只判断本地模型是否已加载；新增 `remoteDotOn`（选中远端模型即为真），按钮圆点与 tooltip 一并修正。
- 测试：`trusted-lan.test.mjs` 更新为「LAN 页不再提交任务/选择模型」；新增 LAN 范围与圆点回归用例。`npm test` 17/17、`check` 0/0、`build` 通过；workspace 测试 28 组全 ok、严格 Clippy、fmt 通过。
- 新包 `41cd00f8…975c0` 已装 MBP 与 Air（备份 `before-lan-scope-20261023-1945`），签名标识仍为 `com.timmy.voicetotextapp`。验收由 Timmy 自行完成。
