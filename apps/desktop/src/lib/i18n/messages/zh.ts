/**
 * 中文（简体）文案：源语言与 key 真源。
 * 其余语言的目录类型为 Record<keyof typeof zh, Message>，
 * 缺 key / 多 key 都会在 npm run check（tsc）阶段报错。
 *
 * 约定：品牌名（Local Transcribe）、技术标识（MCP、CLI、pid、Unix socket、
 * sherpa-onnx、模型 id、--allow-root）与单位（GB/MB）不翻译。
 */
export const zh = {
  // 导航（侧栏与顶栏标题共用）
  "nav.transcribe": "转写",
  "nav.history": "历史",
  "nav.model": "模型",
  "nav.settings": "设置",
  "nav.home": "首页",
  "nav.manage": "管理",
  "nav.workspace": "工作区",
  "nav.mainLabel": "主导航",
  "nav.expandTitle": "显示侧栏 (⌘\\)",
  "nav.collapseTitle": "隐藏侧栏 (⌘\\)",

  // 顶栏
  "toolbar.ariaLabel": "转写工具栏",
  "toolbar.toLight": "切换到浅色外观",
  "toolbar.toDark": "切换到深色外观",
  "toolbar.defaultModel": "默认模型",

  // 通用
  "common.system": "跟随系统",
  "common.cancel": "取消",
  "common.delete": "删除",
  "common.copied": "已复制",
  "common.open": "打开",
  "common.noModel": "未选择模型",
  "common.notSelected": "未选择",

  // 任务状态
  "state.running": "正在转写",
  "state.completed": "已完成",
  "state.cancelled": "已取消",
  "state.failed": "失败",

  // 转写页
  "home.subtitle": "随时拖入音频或视频文件，或打开下面任意一条转写记录。",
  "home.import": "导入文件",
  "transcribe.untitled": "未命名媒体",
  "transcribe.introTitle": "开始一段转写",
  "transcribe.introBody": "拖入音频或视频文件，或使用下方按钮选择；导入后会自动排队。",
  "transcribe.dropAria": "拖入音频或视频文件，或按回车选择文件",
  "transcribe.dropTitle": "拖入音频或视频文件",
  "transcribe.dropSub": "支持常见的音频与视频格式，可一次放入多个文件",
  "transcribe.chooseFile": "选择文件",
  "transcribe.copyAll": "复制全文",
  "transcribe.model": "模型",
  "transcribe.chunksBefore": "已转写",
  "transcribe.chunksAfter": { one: "段", other: "段" },
  "transcribe.progressIdle": "正在识别",
  "transcribe.progressPercent": "已处理 {percent}%",
  "transcribe.partialNote": "该结果为部分内容，不完整；导出时会再次提示。",
  "transcribe.preparingExport": "正在准备导出…",
  "transcribe.speed": "速度 {value}×",
  "transcribe.etaSeconds": "约 {seconds} 秒后完成",
  "transcribe.etaMinutes": "剩余约 {minutes} 分钟",
  "transcribe.etaHours": "剩余约 {hours} 小时 {minutes} 分",

  // 历史页
  "search.placeholder": "搜索文件名或转写内容",
  "search.clear": "清除",
  "search.results": "找到 {count} 条",
  "search.empty": "没有匹配的记录",
  "history.loading": "正在读取历史…",
  "history.emptyTitle": "还没有历史记录",
  "history.emptyBody": "完成一次转写后，结果会出现在这里。",
  "time.today": "今天",
  "time.yesterday": "昨天",

  // 阅读器
  "reader.recognizing": "正在识别，稍候会逐段出现文字。",
  "reader.noText": "还没有可显示的文字。",
  "reader.searchPlaceholder": "在这份转写里搜索",
  "reader.matchCount": "{count} 处匹配",
  "reader.noMatch": "没有匹配",
  "reader.viewLatest": "查看最新",

  // 模型页
  "model.loadedHint": "已加载到内存",
  "model.unknownSize": "大小未知",
  "model.import": "导入模型",
  "model.emptyTitle": "还没有导入模型",
  "model.emptyBody":
    "自己下载 sherpa-onnx 的 Whisper、Qwen3-ASR、Paraformer 或 SenseVoice 导出包，然后选择文件夹导入。导入会复制到应用目录，原始下载保持不动。",
  "model.chooseFolder": "选择模型文件夹",
  "model.defaultTag": "默认",
  "model.installedNotLoaded": "已安装，未加载",
  "model.setDefault": "设为默认",
  "model.currentDefault": "当前默认",
  "model.footer":
    "共 {count} 个模型 · {size}。删除只会移除应用内的副本，不会影响你下载的原始文件。",

  // 设置页
  "settings.general": "通用",
  "settings.transcription": "转写",
  "settings.storage": "存储",
  "settings.integration": "集成",
  "settings.about": "关于",
  "settings.themeName": "主题",
  "settings.themeDesc": "跟随系统时，浅色与深色随外观自动切换",
  "settings.themeLight": "浅色",
  "settings.themeDark": "深色",
  "settings.serviceName": "本地服务",
  "settings.mcpName": "MCP（stdio）",
  "settings.languageName": "界面语言",
  "settings.languageDesc": "应用界面的显示语言；「跟随系统」按启动时的 macOS 语言决定",
  "settings.audioLangName": "语言",
  "settings.audioLangDesc": "由模型自动检测，结果保留原语言",
  "settings.audioLangValue": "自动检测",
  "settings.defaultModelDesc": "只影响之后新建的任务",
  "settings.manageModels": "管理模型",
  "settings.libraryDir": "模型目录",
  "settings.historyDir": "历史结果",
  "settings.storageNote":
    "迁移模型目录需要先做空间预检与回滚方案，尚未开放；删除模型只会移除应用内副本。",
  "settings.jobsDir": "任务记录",
  "settings.dataDir": "应用数据",
  "settings.retentionName": "自动清理",
  "settings.retentionDesc":
    "超过阈值后把最旧的历史结果与任务记录移入废纸篓；模型库与原始文件永不自动清理（默认关闭）",
  "settings.retentionOff": "关闭",
  "settings.retentionAge": "按时间",
  "settings.retentionSize": "按容量",
  "settings.retentionDays": "{days} 天",
  "settings.chunkNote": "音频解码与分段在内存中进行，本版本不产生切片临时文件。",
  "settings.countModels": "{count} 个 · {size}",
  "settings.countHistory": "{count} 条 · {size}",
  "settings.serviceRunning": "运行中 · pid {pid}",
  "settings.serviceStopped": "未运行：CLI 或 MCP 调用时会自动启动",
  "settings.refresh": "刷新",
  "settings.mcpDesc": "9 个工具：模型、提交、状态、结果、队列、取消、重试、导出",
  "settings.copyConfig": "复制配置",
  "settings.copyExample": "复制示例",
  "settings.cliDesc": "与 MCP 共用同一服务与队列",
  "settings.troubleIntro":
    "故障排查：服务只监听本机 Unix socket（权限 0600）；文件访问需在启动时用",
  "settings.troubleOutro": " 授权；日志在终端输出，不记录音频内容。",
  "settings.aboutDesc": "本地音视频转写 · 预览版本 {version}",
  "settings.aboutBuild": "未签名开发构建",
  "settings.mcpCommandPlaceholder": "<voice-to-text-mcp 可执行文件路径>",
  "settings.mcpPathPlaceholder": "/路径/到/你的/媒体目录",
  "settings.cliMediaPlaceholder": "<媒体文件>",
  "settings.cliModelPlaceholder": "<模型 id>",

  // 对话框与错误
  "dialog.mediaFilter": "音频或视频",
  "dialog.importModelTitle": "选择模型文件夹",
  "dialog.deleteModelTitle": "删除模型",
  "dialog.deleteModelMessage":
    "删除「{name}」在应用内的副本？原始下载文件不会被删除，副本会移入废纸篓。",
  "error.copyFailed": "复制失败：{error}",
};

/** 所有语言目录共用的 key 集合（由中文源导出）。 */
export type MessageKey = keyof typeof zh;
