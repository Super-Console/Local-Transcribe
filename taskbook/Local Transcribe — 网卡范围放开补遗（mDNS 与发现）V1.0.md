# Local Transcribe — 网卡范围放开补遗（mDNS 与发现）V1.0

> **文件性质**：第二份增补。接在 `taskbook/Local Transcribe — 局域网范围放开（Tailscale）附加任务书 V1.0.md` 之后生效，覆盖关系不变（附加任务书 > 主任务书被点名的条款）。
> **不得修改正在被执行的主任务书与附加任务书。**
> **本文件只解决一件事**：把网卡范围放开之后，**mDNS 宣告**与**客户端发现**这两条链路没有被一起改的问题。

---

## 0. 为什么需要这份补遗

前一份只改了「能不能绑这个地址」和「要不要过滤对端」。但有两条链路是**按「私有局域网」这个前提写死的**，放开范围后它们会给出错误结果：

1. 节点会通过 mDNS 向本地网段宣告一个自己在本地网段根本不可达的地址（tailnet 100.x），制造「看得见、连不上」的幽灵节点。
2. 客户端发现只有 mDNS + 扫本机 `/24` 两条路，**都命中不了 tailnet 节点**，用户会得到「扫描为空」却不知道为什么。

这两条不在前一份的范围内，必须单独补。

---

## 1. 四条必须修

### 1.1 绑到非 RFC1918 地址时，不要注册 mDNS

`crates/lan-compute/src/node.rs` 现在只有「回环就跳过」一个条件：

```rust
let mdns = if config.listen.ip().is_loopback() { ... None } else { register_mdns(...) };
```

改为：**绑定地址不是 RFC1918（10/8、172.16/12、192.168/16）且不是 `0.0.0.0` 时，跳过 mDNS 注册**，并打一行日志说明原因。

理由：mDNS 是多播、只在本网段有效。把 `100.x` 通过 mDNS 告诉 LAN 邻居，只会让他们连一个不可达地址。

### 1.2 `0.0.0.0` 时不要用 `enable_addr_auto()`，只宣告局域网地址

现状：`config.listen.ip().is_unspecified()` 时用 `detect_primary_ipv4()` 且 `info.enable_addr_auto()`。

`enable_addr_auto()` 会把**本机所有地址**都塞进 A 记录，包括 tailnet 的 100.x。LAN 邻居于是又拿到一个不可达地址，等于 1.1 的问题换个入口重现。

改为：

- 只显式宣告**一个 RFC1918 地址**（优先 `detect_primary_ipv4()` 的结果；若它不是 RFC1918，就取第一块 `en*` 的私有地址）。
- **不使用** `enable_addr_auto()`。
- 若本机一个 RFC1918 地址都没有（例如只用 tailnet 上网），**跳过 mDNS 注册**并记日志。

### 1.3 客户端「扫描为空」时，要提示 tailnet 节点需手动添加

- `LanView` 的扫描空结果提示里增加一句：**经 Tailscale / VPN 连接的节点不会被扫描发现，请手动填写地址与端口。**
- CLI 侧：`lan nodes` 已经列出已保存节点，保持可用即可；`README` 里写明 tailnet 节点必须手动添加。
- 不需要实现「扫描 tailnet」——tailnet 不是广播域，做不到，也不要假装能做到。

### 1.4 「所有接口 0.0.0.0」的两处实现陷阱

**(a) 网卡查表会直接拒掉它。** `apps/desktop/src-tauri/src/lan_host.rs` 的 `HostState::start_models` 里有：

```rust
interfaces()?.into_iter().find(|item| item.address.to_string() == address)
    .ok_or("Choose a currently connected private LAN interface.")?
```

没有任何网卡的地址是 `0.0.0.0`，所以必须**在查表之前特判**「所有接口」这个选项，不要让它走进这块逻辑。

**(b) 必须换启动函数。** `LanScope::new` 拿到 `0.0.0.0` 一定非法。所以全开必须走**不带 scope** 的路径（`NodeServer::start_with_models`），不是给 `LanScope` 传 0.0.0.0。前一份的语义表写了「不做过滤」，但没写「因此要换函数」，这里点名。

---

## 2. 顺带做一次对照实验（很有价值）

既然 tailnet 能连了，那个卡了很久的「局域网 TCP 通、HTTP 超时」就多了一组对照。在验收时**额外做一次**：

| 起点 | 目标地址 | 结果 | 说明 |
| --- | --- | --- | --- |
| Air | MBP 的 192.168.x（局域网） | ？ | 已知失败 |
| Air | MBP 的 100.x（tailnet） | ？ | 新增对照 |

判读：

- **局域网失败、tailnet 成功** → 锁定为 macOS 本地网络权限 / 入站授权问题，与协议、端口、防火墙无关。
- **两者都失败** → 是应用层或节点本身的问题，回到协议与绑定检查。
- **两者都成功** → 说明局域网那条已经被别处的改动顺带修好了，去确认是哪一处并记录下来。

把结论写进日志。这一条不要求"修好"，只要求给出可据此行动的判读。

---

## 3. 测试

1. 绑定 `100.64.0.1/32` → **不注册 mDNS**，状态/日志可查（例如 `mdns: skipped (non-rfc1918)`）。
2. 绑定 `192.168.x.x/24` → 正常注册 mDNS，行为与今天一致。
3. `0.0.0.0` → 只宣告一个 RFC1918 地址，且**不使用** `enable_addr_auto()`；本机无私有地址时跳过注册。
4. `0.0.0.0` → 不走网卡查表，直接可用，不出现「请选择一个当前连接的私有局域网接口」。
5. `0.0.0.0` → 走无 scope 的启动路径，状态里 `subnet_filter: none`。
6. 扫描空结果时，界面出现「tailnet / VPN 节点需手动添加」的提示（前端测试）。

---

## 4. 验收

- 绑 tailnet 地址起节点：同机 `curl` 通；另一台 LAN 设备扫描**不应**出现这个节点（因为它没注册 mDNS）。
- 绑 `192.168.x.x`：LAN 内仍能发现与提交，与今天一致。
- 绑 `0.0.0.0`：可用，UI 明确写着「所有网络接口都可访问」，且 mDNS 对外宣告的是局域网地址、不是 100.x。
- 第 2 节的对照实验两组结果都记录在案。

---

## 5. 文档（跟随主任务书 P7 一起做）

- `README` 双语：写明「经 Tailscale / VPN 连接的节点不会被自动发现，需手动添加地址」。
- `README` 双语：写明 Tailscale 场景下**不注册 mDNS**（因此不会出现在局域网扫描结果里），这是有意行为。
- `README` 双语：写明「所有接口」模式会监听全部网卡（包括 VPN），并显示实际过滤状态。

---

## 6. 执行位置

- 1.1 / 1.2 / 1.4(b)：`crates/lan-compute/src/node.rs`，与附加任务书的 `LanScope` 改动**同一次提交范围**（不提交 Git，指同一批工作）。
- 1.3：`apps/desktop/src/lib/components/LanView.svelte` + 五语言文案。
- 1.4(a)：`apps/desktop/src-tauri/src/lan_host.rs`。
- 第 2 节：验收时执行，写进日志。

**第一条命令**

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  grep -n "fn register_mdns" -A 40 crates/lan-compute/src/node.rs && \
  grep -n "let mdns = if" -A 8 crates/lan-compute/src/node.rs && \
  grep -n "interfaces()?.into_iter()" -A 4 apps/desktop/src-tauri/src/lan_host.rs
```
