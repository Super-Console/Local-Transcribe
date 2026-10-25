# 工具栏按钮顺序、搜索按钮颜色与模型页建议标签调整

日期：2026-10-25
对象：桌面端 UI（`apps/desktop`）
执行者：OpenCode（MiMo）会话，按 Timmy 当日口头需求执行
提交：`d160a99`（10 文件，+133/−42）

## 需求（Timmy 原话归纳）

1. 工具栏按钮顺序：搜索 → 深浅色切换（月亮）→ 侧栏折叠，模型选择器保持最右。
2. 搜索按钮图标偏浅，加深到与其他按钮一致。
3. 模型页副标题去掉「建议：」前缀，直接显示 `预计内存 672 MB · 速度优先`。
4. 准确优先 3 个、速度优先 3 个 ASR 模型各配独立建议标签，参照语言模型写法。
5. Qwen3-4B 标签「中文默认」→「中文均衡」。

## 实现（已验证）

- `AppToolbar.svelte`：DOM 顺序调整为 search → theme → panel-toggle → model-pick；`.search`/`.search-icon` 颜色 `--text-3` → `--text-2`。
- `ModelView.svelte`：`recommendationFor()` 按 `entry.id` switch 六个 ASR 模型；副标题两处改为 `t(recommendation)` 直显；分组兜底键（recommendQuality/Speed）保留给清单外未来条目。
- 五个语言文件（zh/en/es/fr/de）：删除 `model.recommendation` 前缀模板，新增 6 个 profile key，改写 profileQwen4b。

新标签（zh）：

| 分组 | 模型 | 标签 | 依据 |
|---|---|---|---|
| 准确 | X-ASR Zipformer | 中英双语最准 | 实测 CER 0.84% 全清单最低 |
| 准确 | FireRedASR2 | 中英大模型 | 1.23 GB 最大 |
| 准确 | Qwen3-ASR 0.6B | 千问同款内核 | Qwen 系 LLM 内核架构 |
| 速度 | SenseVoice small | 五语速转 | zh/en/ja/ko/yue |
| 速度 | Paraformer | 中文速转 | 中文专用 |
| 速度 | Zipformer CTC small | 轻量极速 | 63 MB 最小、60.7× 最快 |

- 辅助模型「可选标点辅助」保持不变。

## 验证

- TDD：先写失败测试（按钮顺序、搜索颜色、六模型独立标签、无前缀副标题），确认红灯后实现。
- `npm test` 105/105 通过（含 reader-spec 浏览器实测：副标题无前缀且建议词可见）。
- `npm run check`（svelte-check）0 错误 0 警告。
- `cargo test --workspace` 全绿（一处 LAN mDNS 测试首跑因网络偶发失败，单跑与重跑均通过，与本改动无关）。
- 已推送 origin（Super-Console/Local-Transcribe），触发新一轮 Windows CI。

## 未完成

- 两台 Mac 换装新构建（本机 bundle 构建中，airm1 待传）。
- Windows 验收待 CI 产出新 MSI。
