# Local Transcribe — 应用图标改版任务书 V1.0

> **性质**：资源改版（第十二份）。只换**应用图标**，不动托盘图标、不动界面。
> **参考图**：`design/app-icon-reference.png`（1024×1024，RGBA）——已经确认过的效果，先打开看，再动手。
> **不得修改其它任务书。**

---

## 1. 目标图形（照抄，不要自己重新构图）

画布 1024×1024。

### 底

- 圆角方块，圆角半径 **229**（= 22.37%），填充 **#FAF9F5**
- 边缘：**内缩 0.75 的 1.5px 细环**，颜色 `rgba(20,20,19,0.07)`
  → 作用是在浅色 Dock 上不至于融进背景，**不要省**

### 字形（全部圆头，颜色 #C15F3C）

| 元素 | 参数 |
| --- | --- |
| 三条竖波 | 宽 62、圆角 31；x 起点 232 / 338 / 444（间距 44）；高度 150 / 300 / 210；垂直居中于 y = 512 |
| 一条横线 | x 566 → 792（宽 226）、高 62、圆角 31；与竖波同一条水平中线 y = 512 |

- 竖波块右缘 506 与横线左端 566 之间留 **60**
- 安全区检查：内容范围 x 232–792、y 362–662，全部落在 100–924 之内

> 关键：横线是**一条**，不是两三条。这版是 Timmy 在 96 / 48 / 32 三档实际渲染对比后选定的。

---

## 2. 生成方式（不许安装依赖）

本机没有 ImageMagick、没有 PIL、没有 rsvg。

1. 新建 `scripts/gen-app-icon.py`
   - 复用 `scripts/gen-tray-icon.py` 的纯标准库 PNG 写法（`zlib` + `struct`）
   - **4 倍超采样后降采样**，保证圆头平滑
   - 一次产出 1024 / 512 / 256 / 128 / 64 / 32 / 16
   - 幂等：重复运行产出字节一致
2. 用 `sips` 组 `.iconset` → `iconutil -c icns` 生成 `icon.icns`
3. 用 Python 标准库直接写 `icon.ico`（内含 16/32/48/64/128/256 的 PNG 条目，不需要外部工具）

**覆盖以下文件**（其余不动）：

```
icons/icon.png          1024
icons/128x128.png
icons/128x128@2x.png
icons/32x32.png
icons/64x64.png
icons/icon.icns
icons/icon.ico
```

**不要动**：`icons/tray-template.png`、`icons/tray-template@2x.png`、`icons/tray-windows.png`（菜单栏图标是另一套资源，三条细波、22px、黑色 template）。

---

## 3. 断言（只做能自动验的）

1. `icons/icon.png` 为 1024×1024、RGBA；四角 alpha = 0
2. 采样点判定：
   - (263, 512) 属陶土色（第一根竖波内）
   - (679, 512) 属陶土色（横线内）
   - (512, 512) 属米白（竖波与横线之间的空隙）
3. 32px 版本中陶土像素数 > 60（防止缩没了）
4. `icon.icns` 与 `icon.ico` 存在且非空
5. `cargo build -p voice-to-text-desktop` 通过

---

## 4. 打包与安装（同一轮做完，不要分两轮）

1. `cd apps/desktop && npm run bundle:mac`（脚本已含 sidecar 的 ad-hoc 重签，不要手写 tauri build）
2. 备份 `~/Applications/Local Transcribe.app` 到 `~/Applications/Local Transcribe Backups/before-<YYYYMMDD-HHMMSS>/`，记录旧可执行文件的 SHA-256
3. **正常退出**正在运行的实例（不要 `kill -9`），确认进程消失、7878 已释放；若有 `local-transcribe-node` 残留一并停掉
4. 安装新包，安装前后 SHA-256 对比一致
5. 从安装路径启动，确认 pid 是新的、可执行路径指向 `~/Applications/`
6. 回报：新包路径、可执行文件 SHA-256、备份目录、新 pid、启动时间

---

## 5. 不做

- 不评价观感、不截图（Timmy 自己看）
- 不改托盘图标、不改界面配色
- 不升版本号（这轮只是资源替换）
- 临时文件先别清理，需要清时先列清单

---

## 6. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sips -g pixelWidth -g pixelHeight design/app-icon-reference.png && \
  ls -la apps/desktop/src-tauri/icons/ && \
  sed -n '1,30p' scripts/gen-tray-icon.py 2>/dev/null
```
