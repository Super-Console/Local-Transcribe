# A版应用图标应用与本机换装

- 用户选择A并授权apply，按应用图标任务书第4节同轮生成/打包/备份/安装；不改托盘、界面、版本或任务书，B版保留。
- 新增scripts/gen-app-icon.py；标准库4x超采样，复用已有标准库PNG codec与圆角scanline函数，按A几何生成。底#FAF9F5、圆角229、内环1.5px（中心内缩0.75）、陶土三波62px宽/150、300、210高及独立226×62横线。
- 复用已有/usr/bin/python3、sips、iconutil、cargo；未安装依赖。生成1024/512/256/128/64/32/16及ICO所需48中间PNG；sips组iconset后iconutil生成ICNS，标准库写含16/32/48/64/128/256 PNG的ICO。
- 只覆盖任务指定七个icons文件；重复生成七文件字节一致；非目标受保护文件（含三份tray与A参考）哈希未变。B资源未修改。
- 回读验证1024 RGBA、四角透明、指定三个采样点正确。32px纯陶土像素28，计入抗锯齿覆盖且排除细环后80像素；“>60”按覆盖像素而非纯色像素统计，未修改A几何来凑纯色数量。
- cargo build -p voice-to-text-desktop通过；前端check0错误/0警告、npm test通过；随后按npm run bundle:mac生成.app，未生成DMG。bundle ICNS哈希与新生成A资源一致；identifier=com.timmy.voicetotextapp，strict/deep签名校验通过（ad-hoc，不是公证）。
- 备份~/Applications/Local Transcribe Backups/before-20261024-021451/，旧desktop SHA256=bf14d5b783ad23449228e7b7134769cf2ace03f54d18447d003593cfc50dee07。
- 正常quit旧实例，确认进程消失、7878无监听，无残留node需终止；无kill -9。安装~/Applications/Local Transcribe.app，安装前后desktop SHA256=682df4b8bd7005bc97bc48ac5d557a832a3b13dfb8915db60d74aa7ebe552bfd相同，安装ICNS也与bundle匹配，strict校验通过。
- 北京时间2026-10-24T02:14:51.797967+08:00从安装路径启动，新PID23684，ps确认执行路径位于~/Applications/Local Transcribe.app/Contents/MacOS/；10秒后仍运行。
- 未评价观感、未图像采集、未验证真实转写/双机/Windows；未清理生成中间目录，未提交/发布，版本仍1.1.0-beta.5。

## 用户追加：竖波上下更长、起伏差加大（同日02:28）

- 按用户要求同时调整应用图标与macOS菜单栏图标；不改宽度、位置、横线、颜色或版本。应用三波高度150/300/210→168/384/240，仍以512为中线；macOS模板7/13/9→8/17/11，仍以11为中线，22/44px两份同步。Windows托盘保持原7/13/9，字节哈希与原基线一致；A参考原图与B方案不改。
- 新增scripts/test-wave-icon-geometry.py，先跑2项失败（旧高度），修改生成脚本后2/2通过；中心列alpha覆盖高度允许4x抗锯齿的0.35px容差。原托盘资源/幂等断言2/2通过；应用32px计入抗锯齿覆盖的陶土像素90，纯色32。
- npm check0/0、npm test通过、bundle:mac通过；未跑Windows或重新声明Rust全量测试通过。新bundle包含改后macOS模板完整PNG字节，ICNS哈希与改后生成资源一致；strict签名检查通过。
- 仅本机Pro覆盖安装，Air未更新。备份~/Applications/Local Transcribe Backups/before-20261024-022836/，旧desktop SHA256=7430fe2fae9b4b138d41a1dbcec86b0c351dd83e2eacffc32e0b2be6aeea1780。正常quit后确认GUI/node无残留、7878无监听，无kill -9。
- 安装~/Applications/Local Transcribe.app；desktop SHA256=ccbb43e90781bf4b1265725eb06210a7876bbe01bebe9be8002785294ee9a058，安装前后相同；strict校验通过。北京时间2026-10-24T02:28:36.924826+08:00从安装路径启动，新PID37818，10秒后仍存在。
- 不作观感评价，不采图，不清理临时文件或旧备份，不提交/发布。
