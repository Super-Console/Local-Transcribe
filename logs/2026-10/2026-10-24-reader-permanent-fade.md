# 阅读器常驻渐隐、层叠与底色统一

- 本轮依据用户当轮要求，取代之前scrollTop<=2隐藏、56px顶部渐隐及阅读区独立底色要求；不修改历史任务书。
- 修改前真实组件computed：shell relative/auto/透明/opacity1；scroll absolute/auto/rgb(255,254,251)/1；fade absolute/2/旧单段线性渐变/0；bar absolute/3/透明/1。条内空白(500,36)命中DIV.reader-bar，不是正文，但不符合新shell/fade断言。
- 先修改测试再运行，因高度、常驻opacity、滚动背景及空白命中要求失败。随后最小实现：shell isolation:isolate/pointer-events:auto；scroll不设置z-index且背景透明；fade z2、bar z3。bar pointer-events:none，仅button/input恢复auto；fade装饰层仍pointer-events:none，空白交给shell。
- 去除at-top opacity0规则；顶部高72px，stop为页面色0/40px、页面色alpha.8在56px、透明72px；底部24px/到底或短文8px规则保留。两端渐隐和bar实心回退色均引用--bg（通过--reader-fade/--reader-solid-fill别名），浅#FAF9F5、深#1e1e20。未加描边掩盖接缝。
- 最终computed：shell relative/auto/透明/1；scroll absolute/auto/透明/1；fade absolute/2/多stop渐变/1；bar absolute/3/透明/1。空白(500,36)命中DIV.reader-shell；scrollTop0/2/3/160均opacity1。浅深背景及回退同色、isolation、控件点击和查找聚焦断言均通过。
- npm run check为0 errors/0 warnings，npm test为39 passed/0 failed/0 skipped；npm run bundle:mac（含前端build与ad-hoc重签）通过；未新跑Rust workspace测试，未声称视觉遮蔽效果已验收。
- 本轮覆盖安装仅本机Pro，Air保持上一轮A图标构建，未远程换装。
- 正常退出旧PID23684，7878无监听，无残留node需停止；无kill -9。备份~/Applications/Local Transcribe Backups/before-20261024-022303/，旧desktop SHA256=682df4b8bd7005bc97bc48ac5d557a832a3b13dfb8915db60d74aa7ebe552bfd。
- 新安装~/Applications/Local Transcribe.app，desktop安装前后SHA256=7430fe2fae9b4b138d41a1dbcec86b0c351dd83e2eacffc32e0b2be6aeea1780；sidecar安装前后哈希也一致；identifier正确，bundle和安装包strict/deep签名检查通过。
- 北京时间2026-10-24T02:23:04.106922+08:00从安装路径启动，新PID32381；15秒后仍存在，无新增相关DiagnosticReports。不评价观感、不采图、不清理、不发布。版本仍1.1.0-beta.5。
- 实测原始输出保留于会话临时目录reader-permanent-fade-before.log / reader-permanent-fade-red.log / reader-permanent-fade-tests.log / reader-permanent-fade-install.json。

## 用户追加：滚动内容顶部留出渐隐空间

- 不改常驻逻辑。新增.reader-content包裹标题、查找行和正文，只在此内容层padding-top，不加在scrollport上。
- 在.reader-shell集中定义fade-height72px、bar-height40px、clearance8px，以及content-padding-top=calc(fade-height - bar-height + clearance)，注释写明公式；scrollport top/bar高度/fade高度及渐变stop统一引用变量。
- 修改前真实History组件：scrollTop0、titleTop66、fadeBottom90，首次新增标题留白断言失败；标题自身及祖先opacity1、mask:none。原到底最后段落边界距scrollport底39.96875px、bottom fade8px。
- 修改后：paddingTop40px、titleTop106、fadeBottom90，标题间距16≥8；标题及祖先opacity1、mask:none。测试临时把fade-height改88px，padding自动变56px、titleTop122、fadeBottom106，仍间距16px；随后恢复默认72px。
- 滚动到底top fade仍高72px/opacity1；bottom fade仍8px、最后段落底间距仍39.96875px（采用0.1px子像素容差而非错误整数相等断言），中段底fade24px规则未动。没有滚顶关闭fade逻辑。
- 最终npm run check0 errors/0 warnings、npm test39 passed/0 failed/0 skipped。未打包或换装，两机已安装包均不含本次内容padding改动；未评价观感、不采图、不清理。原始输出reader-content-padding-before.log / reader-content-padding-after.log留在会话临时目录。
