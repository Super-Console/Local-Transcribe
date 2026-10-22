# 浅色主题改为 Anthropic 牛皮纸奶白（2026-10-23）

执行 AI：OpenCode（DeepSeek V4.1 Flash）。

## 参考与取值

网络核对 Anthropic / Claude 品牌配色：Clay 陶土 `#C15F3C`、Ivory 奶白 `#FAF9F5`、Pampas 纸色 `#F4F3EE`、Cloudy 灰 `#B1ADA1`、Slate 墨色 `#141413`。

## 改动

- `app.css` 浅色 token 整体替换：
  - 内容底 `#FAF9F5`，侧栏 `#F1EFE7`（牛皮纸深一档），卡片 `#FFFEFB`，填充 `#F2EFE6`，胶囊 `#ECE7DB`
  - 正文墨色 `#141413`，次级暖灰 `#6B6660` / `#97928A`
  - 强调色由系统蓝改为陶土：`--accent #C15F3C`、`--accent-ink #A14A2F`，选中底色 `rgba(193,95,60,.13)`，选中项文字为陶土色
  - 语义色暖化：绿 `#3D7F52`、红 `#B3402E`；阴影改暖色底
- 原生标题栏条同步：浅 `#FAF9F5` / 深 `#1E1E20`（`set_native_window_background`），避免接缝色差。
- 下拉菜单阴影、模型圆点灰、卡片 hover 阴影改暖灰。
- 深色主题保持不变。

## 验证

- `npm run check` 0/0；release 构建、安装、启动正常。
- 截图：首页（奶白底 + 陶土选中 + 纸色卡片 + 大字号衬线标题）、设置页（陶土链接、暖灰控件、发丝分隔），标题栏条与页面底色无接缝。
