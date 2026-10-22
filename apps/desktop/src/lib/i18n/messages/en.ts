import type { Message } from "./types";
import type { MessageKey } from "./zh";

export const en: Record<MessageKey, Message> = {
  // Navigation
  "nav.transcribe": "Transcribe",
  "nav.history": "History",
  "nav.model": "Models",
  "nav.settings": "Settings",
  "nav.home": "Home",
  "nav.manage": "Manage",
  "nav.workspace": "Workspace",
  "nav.mainLabel": "Main navigation",
  "nav.expandTitle": "Show sidebar (⌘\\)",
  "nav.collapseTitle": "Hide sidebar (⌘\\)",

  // Toolbar
  "toolbar.ariaLabel": "Transcription toolbar",
  "toolbar.toLight": "Switch to light appearance",
  "toolbar.toDark": "Switch to dark appearance",
  "toolbar.defaultModel": "Default model",

  // Common
  "common.system": "System",
  "common.cancel": "Cancel",
  "common.delete": "Delete",
  "common.copied": "Copied",
  "common.open": "Open",
  "common.noModel": "No model selected",
  "common.notSelected": "None selected",

  // Job states
  "state.running": "Transcribing",
  "state.completed": "Completed",
  "state.cancelled": "Cancelled",
  "state.failed": "Failed",

  // Transcribe view
  "home.subtitle": "Drop audio or video files anywhere, or open one of the transcriptions below.",
  "home.import": "Import files",
  "transcribe.untitled": "Untitled media",
  "transcribe.introTitle": "Start a transcription",
  "transcribe.introBody":
    "Drop an audio or video file, or pick one with the button below; imports are queued automatically.",
  "transcribe.dropAria": "Drop an audio or video file, or press Enter to choose one",
  "transcribe.dropTitle": "Drop an audio or video file",
  "transcribe.dropSub":
    "Common audio and video formats are supported; you can add several files at once",
  "transcribe.chooseFile": "Choose file",
  "transcribe.copyAll": "Copy full text",
  "transcribe.model": "Model",
  "transcribe.chunksBefore": "Transcribed",
  "transcribe.chunksAfter": { one: "segment", other: "segments" },
  "transcribe.progressIdle": "Recognizing",
  "transcribe.progressPercent": "Processed {percent}%",
  "transcribe.partialNote":
    "This is a partial, incomplete result; you will be asked again before exporting.",
  "transcribe.preparingExport": "Preparing export…",
  "transcribe.speed": "{value}× realtime",
  "transcribe.etaSeconds": "about {seconds}s left",
  "transcribe.etaMinutes": "about {minutes} min left",
  "transcribe.etaHours": "about {hours} h {minutes} min left",

  // History view
  "search.placeholder": "Search names or transcript text",
  "search.clear": "Clear",
  "search.results": "{count} results",
  "search.empty": "No matching records",
  "history.loading": "Loading history…",
  "history.emptyTitle": "No history yet",
  "history.emptyBody": "Once a transcription finishes, its result appears here.",
  "time.today": "Today",
  "time.yesterday": "Yesterday",

  // Reader
  "reader.recognizing": "Recognizing — text will appear segment by segment shortly.",
  "reader.noText": "No text to show yet.",
  "reader.searchPlaceholder": "Search this transcript",
  "reader.matchCount": "{count} matches",
  "reader.noMatch": "No matches",
  "reader.viewLatest": "View latest",

  // Models view
  "model.loadedHint": "Loaded in memory",
  "model.unknownSize": "Size unknown",
  "model.import": "Import model",
  "model.emptyTitle": "No models imported yet",
  "model.emptyBody":
    "Download a Whisper, Qwen3-ASR, Paraformer or SenseVoice export from sherpa-onnx yourself, then choose its folder to import. Imports are copied into the app's directory; your original downloads stay untouched.",
  "model.chooseFolder": "Choose model folder",
  "model.defaultTag": "Default",
  "model.installedNotLoaded": "Installed, not loaded",
  "model.setDefault": "Set as default",
  "model.currentDefault": "Current default",
  "model.footer": {
    one: "{count} model in total · {size}. Deleting only removes the in-app copy and never touches your original downloads.",
    other: "{count} models in total · {size}. Deleting only removes the in-app copy and never touches your original downloads.",
  },

  // Settings view
  "settings.general": "General",
  "settings.transcription": "Transcription",
  "settings.storage": "Storage",
  "settings.integration": "Integrations",
  "settings.about": "About",
  "settings.themeName": "Appearance",
  "settings.themeDesc": "When following the system, light and dark switch with macOS",
  "settings.themeLight": "Light",
  "settings.themeDark": "Dark",
  "settings.serviceName": "Local service",
  "settings.mcpName": "MCP (stdio)",
  "settings.languageName": "Interface language",
  "settings.languageDesc":
    "Language of the app interface; “System” uses the macOS language at launch",
  "settings.audioLangName": "Language",
  "settings.audioLangDesc":
    "Auto-detected by the model; results keep their original language",
  "settings.audioLangValue": "Auto-detect",
  "settings.defaultModelDesc": "Only affects newly created jobs",
  "settings.manageModels": "Manage models",
  "settings.libraryDir": "Model folder",
  "settings.historyDir": "History results",
  "settings.storageNote":
    "Moving the model folder requires a space pre-check and a rollback plan first, so it is not open yet; deleting a model only removes its in-app copy.",
  "settings.jobsDir": "Job records",
  "settings.dataDir": "App data",
  "settings.retentionName": "Auto-cleanup",
  "settings.retentionDesc":
    "Moves the oldest history results and job records to the Trash once a threshold is exceeded; the model library and your original files are never cleaned (off by default)",
  "settings.retentionOff": "Off",
  "settings.retentionAge": "By age",
  "settings.retentionSize": "By size",
  "settings.retentionDays": "{days} days",
  "settings.chunkNote":
    "Audio decoding and chunking happen in memory; this version writes no temporary chunk files.",
  "settings.countModels": { one: "{count} model · {size}", other: "{count} models · {size}" },
  "settings.countHistory": {
    one: "{count} entry · {size}",
    other: "{count} entries · {size}",
  },
  "settings.serviceRunning": "Running · pid {pid}",
  "settings.serviceStopped": "Not running: started automatically when the CLI or MCP calls it",
  "settings.refresh": "Refresh",
  "settings.mcpDesc": "9 tools: models, submit, status, result, queue, cancel, retry, export",
  "settings.copyConfig": "Copy config",
  "settings.copyExample": "Copy example",
  "settings.cliDesc": "Shares the same service and queue as MCP",
  "settings.troubleIntro":
    "Troubleshooting: the service listens only on a local Unix socket (mode 0600); file access requires authorization at startup via",
  "settings.troubleOutro":
    "; logs are written to the terminal and never include audio content.",
  "settings.aboutDesc": "Local audio & video transcription · Preview {version}",
  "settings.aboutBuild": "Unsigned development build",
  "settings.mcpCommandPlaceholder": "<path to the voice-to-text-mcp executable>",
  "settings.mcpPathPlaceholder": "/path/to/your/media/dir",
  "settings.cliMediaPlaceholder": "<media file>",
  "settings.cliModelPlaceholder": "<model id>",

  // Dialogs & errors
  "dialog.mediaFilter": "Audio or video",
  "dialog.importModelTitle": "Choose model folder",
  "dialog.deleteModelTitle": "Delete model",
  "dialog.deleteModelMessage":
    "Delete the in-app copy of “{name}”? Your original download is not removed; the copy is moved to the Trash.",
  "error.copyFailed": "Copy failed: {error}",
};
