# 按钮体系规范 V1.0

## 范围与实现

- 按用户指定任务书统一 app.css 的 btn-primary、btn-secondary、btn-danger、link-action/link-danger；迁移对应组件并删除重复按钮样式。
- 录制/保存节点使用主操作；扫描、下载、阅读器动作使用次操作；行内动作保留文字形态；设置页仅清除使用危险确认胶囊。表单控件、文案、回调及确认流程未调整。
- reader-bar button.act 保留自身 backdrop-filter，复用次操作参数；首页保留已有底色变体。
- 组件既有警告色和深色主题分段底色移入 app.css token，未改变其颜色值。

## 自动验证

- 新增角色断言和真实 Svelte 组件 computed-style 探针，先运行确认失败，再实现。
- 最终 npm run check：0 errors / 0 warnings。
- 最终 npm test：41 passed / 0 failed / 0 skipped。
- 最终 npm run build：成功，adapter-static 输出 build。
- 实测录制、局域网、转录空闲页各一个主操作；模型行胶囊数量 1/0/0；设置页18个行内动作背景透明、圆角6px，仅一个危险确认胶囊。主操作 disabled 使用 fill/text-3 且无 outline。
- 原始测试输出：系统批准临时目录 opencode/button-system-red.log、button-system-final.log、button-system-build.log。

## 边界

- 上述为自动化浏览器组件验证，不是原生 WKWebView/Windows 验收或用户视觉验收。未截图、未评价观感。
- 本轮未构建原生安装包、未安装、未提交、未发布。npm run build 仅为前端构建。
- 测试临时目录、日志及构建产物保留；未经确认不清理。
