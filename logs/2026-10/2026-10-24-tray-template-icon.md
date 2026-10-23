# 菜单栏图标资源与平台加载修正

- 范围：菜单栏图标修正任务书 V1.0；未修改任何任务书，未改四态状态逻辑、菜单或事件处理。
- 先运行 `python3 scripts/test-tray-icon.py`：2项失败，分别为仍使用应用窗口图标、生成脚本不存在。随后实现并重跑通过。
- `scripts/gen-tray-icon.py` 只用标准库，以4倍分辨率绘制三条圆头竖条，再对每个输出像素的16个alpha采样取平均；用struct/zlib写8-bit RGBA PNG，写后重新读文件并核对CRC、尺寸、像素格式、透明四角和中心列实心像素。使用任务指定x起点3/9/15、高7/13/9、宽3、y中心11；高分辨率/Windows等比缩放。
- 输出均在 `apps/desktop/src-tauri/icons/`：tray-template.png（22×22，黑）、tray-template@2x.png（44×44，黑）、tray-windows.png（32×32，白）。背景alpha=0；圆头边缘为4x降采样的中间alpha，主体alpha=255。
- `tray.rs` 通过include_bytes嵌入平台资源，再调用Image::from_bytes解码：macOS使用22px模板；其它桌面平台使用白色Windows资源；template只在macOS开启。无default_window_icon调用，解码失败返回含资源名和原错误的可读错误，不panic。44px文件已生成并测试，当前加载按任务书第4节指定使用22px文件。
- `Cargo.toml` 增加用户明确要求的image-png feature；Cargo.lock因此新增image 0.25.10、byteorder-lite 0.1.0、moxcms 0.8.1、pxfm 0.1.30。这是批准feature的传递依赖解析，无手动安装图形工具或全局依赖。
- 工具复用：python3=/usr/bin/python3，cargo=~/.cargo/bin/cargo；未使用/安装ImageMagick、PIL、rsvg。
- 最终验证：`python3 -B scripts/test-tray-icon.py` 2/2通过，包括三文件RGBA/尺寸/透明角/中心列alpha255、可见像素纯黑或纯白、边缘中间alpha、再次生成字节一致、源码平台资源/template与feature断言。
- `cargo build -p voice-to-text-desktop` 通过（dev profile）；`cargo test -p voice-to-text-desktop tray::tests --lib` 1/1通过；`cargo fmt --all -- --check`通过。
- 未做图形界面或观感判断，未启动/换装/重启应用；未做Windows构建或运行。本轮不声称菜单栏已验收，当前已安装应用仍为此前包。未提交/发布，未清理任何文件。
