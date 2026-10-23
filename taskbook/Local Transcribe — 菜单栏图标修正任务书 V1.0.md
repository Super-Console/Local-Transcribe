# Local Transcribe — 菜单栏图标修正任务书 V1.0

> **性质**：补丁任务书（第十一份）。只改托盘图标资源与加载方式。
> **不得修改其它任务书。**

---

## 1. 问题与根因

**现象**：菜单栏里显示为一个**纯色圆角方块**，没有任何图形。

**根因**（`apps/desktop/src-tauri/src/tray.rs:93`）：

```rust
let icon = app.default_window_icon()...   // 应用图标：一整块不透明的圆角方块
    .icon(icon)
    .icon_as_template(true)               // macOS 只使用 alpha 通道
```

template 模式下，macOS 丢弃颜色、只按 alpha 上色。应用图标是满幅不透明 → 结果是整块纯色。**不是尺寸问题，是资源用错了。**

---

## 2. 目标图标

**字形**：三条竖条（波形），代表转写/音频。要求 16px 下仍能辨认，不要细节，不要文字。

**几何（1x，22×22 画布，内容居中 16×16）**

- 3 条竖条，宽 3px，间距 3px，总宽 15px，水平居中（x 起点 3）
- 高度分别 7 / 13 / 9 px，垂直居中（中线 y = 11）
- 圆角 1.5px（两端圆头）
- 颜色：纯黑 `#000000`，alpha 255；**背景完全透明**，不得有任何白底或边框

**两个平台两份资源**

| 平台 | 文件 | 规格 | 说明 |
| --- | --- | --- | --- |
| macOS | `icons/tray-template.png` | 22×22，黑+透明 | template，系统自动反色适配深浅菜单栏 |
| macOS | `icons/tray-template@2x.png` | 44×44，同上 | 同上，2 倍 |
| Windows | `icons/tray-windows.png` | 32×32，**白色**+透明 | Windows 不用 template，纯黑在深色任务栏上看不见 |

> Windows 这份必须是白色或高对比浅色。不要复用 macOS 那份。

---

## 3. 资源生成方式（不许装依赖）

本机没有 ImageMagick、没有 PIL、没有 rsvg。**用 Python 标准库生成 PNG**（`zlib` + `struct` 手写编码即可）。

要求：

- 4 倍超采样后降采样（4x 下绘制再取平均），保证圆头与斜边平滑
- 生成后回读校验：尺寸、像素格式（RGBA）、四角 alpha 为 0、中心列有 alpha 255 的像素
- 生成脚本放 `scripts/gen-tray-icon.py`，可重复运行（幂等），并在文件头注释写清用途

---

## 4. 代码改动

1. `apps/desktop/src-tauri/Cargo.toml`：给 `tauri` 加 `image-png` feature（解码 PNG 需要）
2. `tray.rs`：
   - macOS：从 `icons/tray-template.png` 加载（`Image::from_path` 或 `include_image!`），**不再用 `default_window_icon()`**
   - Windows：从 `icons/tray-windows.png` 加载，`icon_as_template` 不要开
   - 加载失败要有可读错误，不要 panic
3. 保留现有的四态状态逻辑与菜单，不要顺手改

---

## 5. 可自动断言的检查

1. `tray.rs` 中不再出现 `default_window_icon`
2. 三个资源文件存在，尺寸分别为 22×22 / 44×44 / 32×32，格式 RGBA
3. macOS 资源四角 alpha = 0（透明底），且存在 alpha = 255 的像素（有图形）
4. Windows 资源的主色为白色（RGB 均 ≥ 240）
5. `image-png` feature 已在 Cargo.toml 中
6. 构建通过：`cargo build -p voice-to-text-desktop`

## 6. 需人工（Timmy 看）

- 菜单栏图标在浅色与深色菜单栏下都能辨认
- 视觉大小是否与系统图标相称（过大/过小要调 canvas，不是调字形）
- Windows 托盘是否可见

---

## 7. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  sed -n '85,110p' apps/desktop/src-tauri/src/tray.rs && \
  ls -la apps/desktop/src-tauri/icons/ | head -6 && \
  grep -n "image-png\|features" apps/desktop/src-tauri/Cargo.toml | head -5
```
