# Local Transcribe — 共享开关改造任务书 V1.0

> **性质**：小改造（第十八份）。只改「允许其他设备使用本机算力」这一个控件、其文案与一条提示。
> **不得修改其它任务书。**
> **执行前提**：排在其它界面任务之后（同样改 `LanView.svelte` 与五语言文件）。

---

## 1. 问题

现在它是一个**文字胶囊在冒充开关**：

```svelte
<button class="btn-secondary" role="switch" aria-checked={host.running}>
  {t("lan.hostOn")}   // 「已开启 · 点击关闭」
</button>
```

```
lan.hostOn:  "已开启 · 点击关闭"
lan.hostOff: "已关闭 · 点击开启"
```

- **文案重复**：状态（已开启）和行为（点击关闭）挤在同一个按钮里
- **角色错位**：`role="switch"` 的语义是开关，外形却是文字按钮
- **翻译走样**：`On · Turn off` 这类拼接在别的语言里更难读

---

## 2. 目标形态：状态点 + 动作按钮（Timmy 已定，不要再给方案）

```
● 允许其他设备使用本机算力                        [ 关闭共享 ]
开启后，同一子网内的设备可上传媒体并使用本机转写。
```

- **左侧一个状态点**（7×7），与模型页那个点一致：绿 = 运行中，灰 = 已关闭
- 标题文字跟在其后，间距 8px
- 右侧一个胶囊按钮，**只写动作**：`开启共享` / `关闭共享`
- 说明文字（`lan.hostDesc`）保持在下面不动

---

## 3. 规格

**状态点**（复用 `AppToolbar.svelte` 里 `.dot` 的既有规格，不要另造）

| 项 | 值 |
| --- | --- |
| 尺寸 / 圆角 | 7×7 / 50% |
| 关 | `#cbc5b7`（深色主题 `#5a5a60`） |
| 开 | `var(--green)` |
| 位置 | 标题左侧，与文字间距 8px，垂直居中 |
| 可访问性 | `aria-hidden="true"`（状态由按钮的 `aria-label` 传达，见第 5 节） |

建议把 `.dot` / `.dot.on` 抽成 `app.css` 里的共享类，`AppToolbar` 与 `LanView` 同时使用——现在只有 `AppToolbar` 里有这份样式。

**按钮**

- 沿用 `.btn-secondary`：高 30px、圆角 999、底色 `var(--capsule-find)`、1px 边
- 文案只写动作，不带状态
- 切换中：`disabled` + `aria-busy="true"`，文案显示 `正在开启…` / `正在关闭…`
- 切换失败：文案回到实际状态对应的动作，错误走既有的 `host.notice` 通道
- 点击区 ≥ 高 30px（现状即可），不要为了对齐把高度压小

---

## 4. 文案（五语言）

**新增 / 改名**（旧的两个 key 删除，避免"key 叫 hostOn 却存着关闭共享"这种混乱）：

| key | zh | en |
| --- | --- | --- |
| `lan.hostActionOn` | 开启共享 | Start sharing |
| `lan.hostActionOff` | 关闭共享 | Stop sharing |
| `lan.hostStateOn` | 已开启 | On |
| `lan.hostStateOff` | 已关闭 | Off |
| `lan.hostChangingOn` | 正在开启… | Starting… |
| `lan.hostChangingOff` | 正在关闭… | Stopping… |

- 删除 `lan.hostOn` / `lan.hostOff` / `lan.hostChanging`（全仓库只有 `LanView.svelte` 用它们）
- de / es / fr 同步，不得保留 `Turn on` / `点击` 这类描述
- 状态词（`已开启` / `已关闭`）用于 `aria-label` 与 tooltip，**不显示在按钮上**

---

## 5. 语义与可访问性

- 按钮改为**普通动作按钮**：去掉 `role="switch"` 与 `aria-checked`
- `aria-label` 必须同时包含主体与状态，例如：`允许其他设备使用本机算力，当前已开启`
  —— 这样不用看到一个 7px 的点也能知道状态
- 状态点 `aria-hidden="true"`，不要让读屏去念一个点
- 颜色不是唯一线索：按钮文案在开/关时不同（开启共享 ↔ 关闭共享），满足
- 键盘：Tab 可达，`Enter` / `Space` 都能触发

---

## 6. 顺带：删掉那条内存提示

**删除** `LanView.svelte:530`：

```svelte
<p class="note subtle">{t("lan.memoryHint")}</p>
```

并删除五语言里的 `lan.memoryHint`。

**理由**：那条里的"合计约 4 GB"是把两个进程**峰值相加**得来的，不是同一时刻采样，数字本身不可靠；这类解释性长句放在设置页只会增加噪音。信息保留在 README 与 docs 里。

---

## 7. 断言（只做能自动验的）

1. 五语言里不再有 `lan.hostOn` / `lan.hostOff` / `lan.hostChanging` / `lan.memoryHint`（grep 为空）
2. 文案值里不含 `点击` / `Turn on` / `Turn off`
3. 状态点：7×7、`border-radius: 50%`；`on` 时背景为 `var(--green)`，关时为 `#cbc5b7`
4. 状态点有 `aria-hidden="true"`
5. 按钮无 `role="switch"`、无 `aria-checked`；`aria-label` 同时包含"允许其他设备使用本机算力"与状态词
6. `running = true` 时点为绿、按钮文字为「关闭共享」；`false` 时相反
7. 切换中按钮 `disabled` 且 `aria-busy="true"`，文案为「正在开启…」/「正在关闭…」
8. 局域网页该区块内不再出现"内存"或"GB"字样

---

## 8. 不做

- 不改启停逻辑、不改知情同意流程、不改端口/网卡表单
- 不引入 switch 控件
- 不改「刷新网络接口」「扫描」等其它控件
- 不评价观感、不截图

---

## 9. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sed -n '515,535p' apps/desktop/src/lib/components/LanView.svelte && \
  sed -n '428,460p' apps/desktop/src/lib/components/AppToolbar.svelte && \
  grep -n "lan.hostOn\|lan.hostOff\|lan.hostChanging\|lan.memoryHint" apps/desktop/src/lib/i18n/messages/*.ts
```
