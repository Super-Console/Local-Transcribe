# Local Transcribe — 服务任务列表补遗（jobs list / list_jobs）V1.0

> **文件性质**：第三份增补，接在前面两份之后生效。
> **不得修改正在执行的任何任务书。**
> **范围**：只修一个已确认的缺陷，外加它修复后必然带出的一个新风险。不扩大范围。

---

## 1. 缺陷：`jobs list` / `list_jobs` 看不到正常完成的任务

### 现状（已核实，2026-10-23）

`crates/transcription-service/src/lib.rs`：

- `queue.finished_in_memory` 只有两处 `push`：
  - `cancel()` 里取消**尚未开始**的排队任务（约 786 行）
  - worker 在 `begin_with_retry` 永久失败时（约 1168 行）
- `execute_job` 正常跑完（完成或失败）后，**只往 `state.runs` 推时间区间，从不往 `finished_in_memory` 推结果**（约 1293 行）。

### 后果

- CLI `local-transcribe jobs list` 的 `finished` 基本恒为空
- MCP `list_jobs` 工具的描述写着「active, waiting, **recently finished**」，但 `finished` 拿不到正常完成的任务
- `jobs show <id>` 仍然正常，因为它走的是 `store.job_status()`，**不是同一条数据源**——所以「列表里没有、单查却有」这种迷惑现象会同时出现

这在 `2026-10-22-MCP-CLI能力实测.md` 里已经记录过，当时标注「修复方向待 Timmy 决定」，至今未修。本轮 P3.5 会让远端任务也进这套列表，**如果不修，局域网任务同样会从列表里消失**。

### 要改成什么

`execute_job` 结束时（包括成功、失败、以及被取消而中断的三条路径），把最终 `JobStatus` 转成 `JobReply` 推入 `finished_in_memory`，字段与 `status()` 的取值保持一致（`state` 用同一套小写命名，`phase` 用同一段文案，`error` 原样带上）。

**数据源只能有一个**：要么统一从 store 读，要么统一从内存读，不得继续「列表读内存、单查读 store」。

- **推荐从 store 读**，但注意：`JobStore` 目前**没有枚举 API**（只有 `job_status(job_id)` 与 `active_job_status()`），所以需要**新增**一个（例如 `recent_jobs(limit)`：扫描 `jobs/` 下的状态 JSON，按开始时间排序，取最近 N 条）。这一步不能省，不能假装现成函数存在。
- 次选是内存方案（每次完成 push + 固定上限）。它的代价是**重启后列表里看不到重启前完成的任务**，而 `jobs show` 仍然查得到——「列表没有、单查却有」这个现象会在重启后重新出现。若选这条，必须在文档里写明这个限制，并且第 3 节的测试 5 要按「同一进程生命周期内一致」来写，不许写成永久一致。

二选一，但要写清选了哪条以及它的边界。

---

## 2. 修复必然带出的新风险：这个列表会无限增长

`finished_in_memory` 是一个 `Vec`，service 是常驻进程。一旦每次完成都往里推，**跑一整天就会累积成千上万条**，把内存吃掉，而且 `jobs` 每次都要整个序列化一遍。

必须同时做：

- 上限固定（例如 200 条），超过时丢最旧的
- 或者按第 1 节的推荐改为从 store 读最近 N 条（天然有界）
- 无论哪种，**`jobs list` 的输出必须是"最近 N 条"**，且在文档与工具描述里说明这一点，不要让调用方以为它能列出全部历史

这一条不许省略。否则修好一个「列表为空」的 bug，会换来一个「内存缓慢上涨」的 bug，后者更难发现。

---

## 3. 测试（TDD，先红后绿）

1. 提交一个任务并跑到完成 → `Request::Jobs` 的 `finished` 里能查到它，`state` 为 `completed`。
2. 提交一个任务并让它失败 → `finished` 里能查到，`state` 为 `failed` 且 `error` 非空。
3. 任务在运行中被取消 → `finished` 里能查到，`state` 为 `cancelled`（不得永远停在 `running`）。
4. 连续完成超过上限数量的任务（用较小的上限做测试）→ 列表长度不超过上限，且保留的是最近的。
5. `jobs list` 与 `jobs show` 对同一个任务的状态一致（这条是防止两个数据源再次分叉）。
6. 删除任务（`Request::Remove`）后，该任务**不得残留在 finished 列表里**（防止内存列表或缓存变成幽灵条目）。
7. 重试（`Request::Retry`）后，**同一个 job id 不得在列表里出现两次**；以最新状态为准，重试前的失败记录不得与新记录并存造成歧义。

---

## 4. 验收

- CLI：`jobs add` → 等待完成 → `jobs list` 里出现该任务；`jobs show <id>` 状态一致。
- MCP：`list_jobs` 返回的 `finished` 含该任务，与 `get_job_status` 一致。
- 连续跑 5 个任务，`jobs list` 能看到 5 条，顺序为最近优先或最近在后（写明即可，但必须稳定）。
- 长跑检查：把上限调小做一次压力测试，确认不会无限增长。

---

## 5. 文档

- `docs/mcp.md` 的 `list_jobs` 描述补一句「仅最近 N 条」。
- 若采用内存方案，`docs/mcp.md` 中「queue persistence across a service restart」的未完成项要**分开表述两件事**：① 排队中的任务在重启后仍会丢失；② 完成列表是否会跨重启保留，取决于本节选的是哪条方案。不得把两者混成一句话。
- 若采用 store 方案，同一处应说明：完成/失败的任务列表在重启后仍完整（因为它来自磁盘），但**排队中的任务仍会丢失**。两者不是一回事。

---

## 6. 执行位置

- 主体：`crates/transcription-service/src/lib.rs`（`jobs()`、`execute_job()`、`finished_in_memory` 的定义与所有使用点）
- 与 P3.5 **同一批改动**，因为它们改的是同一个文件的同一个区域，分开做会冲突。

**第一条命令**

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  grep -n "finished_in_memory" crates/transcription-service/src/lib.rs && \
  grep -n "fn execute_job" -A 5 crates/transcription-service/src/lib.rs && \
  grep -n "fn jobs" -A 40 crates/transcription-service/src/lib.rs
```
