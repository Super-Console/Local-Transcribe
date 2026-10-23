# Local Transcribe — CLI 与 MCP 能力对等任务书 V1.0

> **性质**：能力补齐（第十五份）。目标是让 **CLI 与 MCP 成为与 GUI 对等的客户端**，而不是只能提交任务的瘦客户端。
> **传输无关**：不绑定 Tailscale、不绑定 IPv4、不绑定 mDNS。节点地址是任意主机名或 IP。
> **执行前提**：排在本轮发布任务之后。不得修改其它任务书。

---

## 1. 对等的边界（先说清，避免膨胀）

**必须对等**（与转写和节点直接相关）：

- 节点的增 / 改 / 删 / 启停
- 发现并加入局域网节点
- 本机作为节点的启动与停止
- 计算方式（仅本机 / 优先局域网）
- 提交、查询、取消、重试、导出（已有）

**允许只存在于 GUI**（并写进 README 说明）：

- 外观：主题、语言、按钮样式
- 模型下载与删除
- 保留策略与清理开关
- 首次知情同意的弹窗交互

理由：这些是「一次性设置」或「纯视觉」，做成命令行没有收益，只会让 CLI 变成设置面板。

---

## 2. 节点模型：地址必须是字符串，不许假设 IPv4

**现状**：节点记录里 `host` 已经是字符串，但发现逻辑（mDNS + 扫描本机 /24）只认 IPv4。

**要求**

- `host` 接受任意形式：`192.168.1.20`、`mbp.local`、`macbook-pro.tail1f3ba1.ts.net`、任意 DNS 名
- 连接时直接拼 `http://{host}:{port}`，**不做 IP 解析、不做前缀判断**
- 保存时**不校验是否为 IP**；只在连接失败时报告真实错误
- 允许用户用主机名代替 IP —— 这样 Tailscale/局域网/VPN 换地址时，只要名字不变就不受影响
- 发现仍只有两条路（mDNS 与同网段扫描），**明确写进文档**：经 tailnet / VPN 的节点必须手动添加，不会被自动发现

---

## 3. "配对"的真实含义（本轮不引入鉴权）

**配对 = 添加 + 验证可达 + 保存**，不是认证。

- 添加时立刻调用目标节点的 `/api/v1/capabilities`，把 `name`、`protocol_version`、`compute_enabled`、模型列表一起存下来并显示
- 连不上就明确报错（超时 / 拒绝 / 版本不匹配），但**允许用户选择"仍然保存"**——因为节点可能只是暂时没开
- **不引入令牌、配对码、证书**。这是既定边界，本轮不改

---

## 4. 单一数据源：`lan-settings.json`

CLI、MCP、GUI 三方都会读写它，必须避免各写各的。

- 抽一个共享函数（放 `transcription-core` 或新的小模块）：`read_lan_settings` / `validate` / `atomic_write`
- 写入一律：临时文件 → `fsync` → `rename`，并用 `fs2` 文件锁包住整段读-改-写
- GUI 现有的直接读写改为调用同一函数（行为不变，只是收敛）
- 文件不存在时返回默认值，不要报错
- 读到损坏内容时：**不要静默覆盖**，先备份成 `.corrupt-<时间戳>` 再回落默认值

---

## 5. CLI 命令面

在现有基础上新增（其余不变）：

```
local-transcribe lan list                        # 现有 lan nodes 的别名，保留两者
local-transcribe lan add <name> <host>[:port]    # 添加并验证
local-transcribe lan remove <id|name>
local-transcribe lan enable|disable <id|name>
local-transcribe lan set <id|name> [--host H] [--port P] [--model ID] [--name N]
local-transcribe lan scan [--timeout 8]          # 局域网发现（mDNS + /24），只打印结果，不自动加入
local-transcribe lan start <id|name>             # 本机作为节点开始监听（host 侧）
local-transcribe lan stop                        # 停止本机节点
local-transcribe lan status                      # 本机节点状态 + 各节点可达性
local-transcribe mode local|prefer-lan           # 计算方式
```

规则：

- 所有命令输出 JSON；退出码沿用 0 / 1 / 2 / 4
- `add` 与 `set` 的 `host` 可以是 IP、`.local`、tailnet 名、任意 DNS 名
- `scan` 只负责"看见"，**不自动写入设置**（避免一次误扫把陌生设备加进来）
- `lan start` 在 macOS 上等价于"让本机节点开始监听"，与 GUI 的共享开关共用同一份设置

---

## 6. MCP 工具面

现有 9 个工具不变，新增 4 个（总数 13）：

| 工具 | 参数 | 说明 |
| --- | --- | --- |
| `list_nodes` | — | 已有 |
| `add_node` | `name`、`host`、`port?` | 添加并验证；返回 capabilities 摘要 |
| `update_node` | `id`、`enabled?`、`host?`、`port?`、`modelId?` | 修改 |
| `remove_node` | `id` | 仅从列表移除，不影响对方机器 |
| `node_status` | `id` | 可达性、协议版本、模型列表 |

**硬性约束**（防止变成后门）：

- 所有工具只能操作 **`lan-settings.json` 里已存在的节点**；`add_node` 是唯一可以引入新地址的入口
- **不接受**任意命令、路径、URL scheme
- MCP 仍为 stdio，不新增监听端口
- `remove_node` 只删本地记录，不向对方发送任何请求

---

## 7. 本机节点启停（host 侧）

- CLI 的 `lan start` / `lan stop` 与 GUI 的共享开关**读同一份设置**（`host_name`、`host_address`、`host_port`、`share_on_launch`）
- 启动失败要给出真实原因（端口占用、地址不可用、无模型）

### 7.1 远程启停：走 SSH，**不安装任何常驻进程**

**原则**：本项目不安装守护进程、不做开机自启、不做常驻监听。远程拉起节点只用**系统自带的 sshd**，与用户已有的免密登录共用。

```
local-transcribe lan start <node>     # 在远端把节点拉起来
local-transcribe lan stop  <node>     # 在远端停掉
```

**前置条件（不满足就报错，不许回退到交互式密码）**

- 目标机已开启「系统设置 → 通用 → 共享 → 远程登录」
- 本机对该主机已配置免密 SSH（调用时用 `BatchMode=yes`，失败立即返回）

**命令必须来自固定模板，不接受任意命令**

- 节点记录新增两个可选字段：`ssh`（形如 `user@host` 或 ssh 别名，缺省用 `host`）、`node_binary`（缺省按 sidecar 路径推断）
- 远端命令固定为：`<node_binary> --from-settings`，不接受用户提供的任何命令片段
- 用户可控的只有「节点 id」——它映射到已保存记录里的字段，**不进入命令字符串的拼接**
- 需要转义的只有这些字段本身；测试要断言：传入形如 `x; rm -rf /` 的节点名或地址不会被当成命令执行

**别名约定（以「发起端」为准，这条最容易被搞错）**

- 发起端是 Air 时：`ssh pro` → MacBook Pro；发起端是 Pro 时：`ssh air` → MacBook Air
- 已在 Pro 上核实：`ssh air` 可用（解析到 `100.91.84.122`，hostname `tims-air`）
- 已在 Pro 上核实：`ssh pro` **不可解析** —— 这是正常的，因为本机就是 Pro；`pro` 这个别名应当配置在 **Air** 上
- 因此 `ssh` 字段按「谁发起」填写，**不要假设同一组别名在两端都存在**；README 与 `lan add` 的报错文案里都要点出这一点

**`--from-settings`：新增给节点的开关**

节点启动时从同一份 `lan-settings.json` 读 `host_name` / `host_address` / `host_port` 与模型目录，省掉一长串参数。这样远端命令只有一句，不会因为参数拼接出错。

**启动后的确认**

- 轮询 `/api/v1/capabilities`，最多 60 秒
- 成功：打印 pid、绑定地址、协议版本、模型列表
- 失败：打印远端 stderr 前 20 行（不要只说"启动失败"）

**停止**

- 读远端同一数据目录下的 `lan-node.json` 拿 pid → `ssh ... kill <pid>`
- **不要用 `kill -9`**；10 秒后仍在则报告，让用户手动处理

**明确不做**

- 不写 LaunchAgent / launchd / systemd / 计划任务
- 不安装、不修改任何远程守护配置
- 不做 WoL（机器已开机的场景不需要；关机场景另议）

---

## 8. 测试

1. `lan add` 一个不可达地址：报错可读，且 `--force`（或交互确认）允许保存
2. `lan add` 一个可达地址：返回 capabilities，字段与 `/api/v1/capabilities` 一致
3. `host` 为 `mbp.local` 形式时可正常连接（用本地 hosts 或 mock）
4. `host` 为非法字符串时不崩溃、不尝试猜测
5. 并发写 `lan-settings.json`：两个进程同时改不同节点，结果两条都在（文件锁生效）
6. 读取损坏 JSON：原文件被备份为 `.corrupt-*`，程序用默认值继续
7. MCP `add_node` 传未知 scheme（如 `ssh://x`）被拒绝
8. MCP 任何工具都不接受任意命令参数（用 schema 断言）
9. `scan` 不写入设置文件（对比调用前后的文件哈希）
10. `lan start` 在未配置免密 SSH 时立即失败并给出可读原因（不得挂在密码提示上）
11. `lan start` 时节点名或地址含 `; rm -rf /`、`$(...)` 之类的片段：断言它只作为普通字符串被拒绝或转义，绝不执行
12. `lan stop` 读到不存在的 pid 时报告"节点未在运行"，不报错退出码 1

---

## 9. 断言与验收

- `local-transcribe lan list` 与 GUI 节点列表内容一致（同一个 `lan-settings.json`）
- 从 CLI 添加的节点，打开 GUI 能看到；反过来也成立
- `host` 用主机名保存后，把该机器的 IP 改掉（模拟换网络），只要名字仍可解析，连接照常
- 文档写明：**经 tailnet / VPN 的节点不会被自动发现，需手动添加**

---

## 10. 不做

- 不引入鉴权、令牌、配对码、证书
- **不安装任何常驻进程**：不写 LaunchAgent / launchd / systemd / 计划任务，不做开机自启
- 不做 WoL 唤醒（本轮场景是"机器开着、软件没跑"）
- 不做公网暴露、端口映射、Tailscale 专属逻辑
- 不改动 `/api/v1` 协议
- 不做 GUI 与 CLI 的界面性功能对等（主题、语言、模型下载）

---

## 11. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sed -n '1,40p' crates/transcription-cli/src/main.rs && \
  grep -n "lan-settings\|LanSettings" apps/desktop/src-tauri/src/lan.rs | head -6 && \
  grep -n "list_nodes" -A 6 crates/transcription-mcp/src/main.rs | head -12
```
