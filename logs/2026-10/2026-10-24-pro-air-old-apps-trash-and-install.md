# Pro / Air 旧版本移入废纸篓与新版同步

- 用户要求清除两机旧版本并安装新版；先只读列出清单，通过question获得“确认，按清单执行”。仅移动获准目录到各机废纸篓，未永久删除或清空废纸篓。
- Pro清单：~/Applications/Local Transcribe Backups/（14份.app备份，du563M）、~/Applications/LocalTranscribe-backups/（1份，du46M）。当前安装与构建的A版相同，不重复替换或重启。
- Air清单：~/Applications/Local Transcribe Backups/（2份，du52M）、~/Applications/Local Transcribe.app（旧版，du26M）。其它数据/模型/设置/源码/B设计与Downloads的Windows安装包明确排除。
- 源为当前已通过测试与打包的A版1.1.0-beta.5，不新打包、不发布。Pro安装及bundle strict签名验证通过；Air为arm64/macOS27.0.1，空闲空间26GiB。
- 先tar/SSH传到Air的~/Applications/.local-transcribe-incoming-20261024-a/Local Transcribe.app，严格校验签名与desktop/node哈希通过后才移动旧版本。
- Pro移动目标：~/.Trash/local-transcribe-old-20261024-021718/Applications/，保留两个备份目录原名及内部路径。
- Air移动目标：~/.Trash/local-transcribe-old-20261024-021742/Applications/，保留备份目录和旧Local Transcribe.app。移动前复查2份备份数量和旧desktop哈希72d399986021dba92dbbc240db33742b36c7b6995360aa5a6466ddb6880377af未变化。
- Air旧GUI/node原本未运行；安装前确认7878无监听，无残留node需终止；未kill -9。将验证好的暂存.app原子rename到~/Applications/Local Transcribe.app；安装后再次strict验证通过。
- 两机安装desktop SHA256同为682df4b8bd7005bc97bc48ac5d557a832a3b13dfb8915db60d74aa7ebe552bfd；sidecar同为e536446b71f3f764d15f19451c6731d668394679f6b19051d809de0b18a97e8a。
- Pro保留当前运行PID23684，路径为安装包Contents/MacOS/voice-to-text-desktop；Air北京时间2026-10-24T02:17:42.403586+08:00从安装路径启动，新PID14850。15秒后仍运行，用户/系统DiagnosticReports未出现新相关报告；不推断长期稳定。
- 两机Applications获准旧备份源目录现已不存在；当前.app仍存在且哈希一致。废纸篓中的旧版本可由用户手动恢复/清空，AI不清理废纸篓。
- Air空暂存目录保留，未顺带清理；未改模型、转写记录、设置，未做真实转写、两机推理、Windows或观感验证。
