<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { confirm, open, save } from "@tauri-apps/plugin-dialog";

  import AppSidebar from "$lib/components/AppSidebar.svelte";
  import AppToolbar from "$lib/components/AppToolbar.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import TranscribeView from "$lib/components/TranscribeView.svelte";
  import HistoryView from "$lib/components/HistoryView.svelte";
  import ModelView from "$lib/components/ModelView.svelte";
  import SettingsView from "$lib/components/SettingsView.svelte";
  import RecordView from "$lib/components/RecordView.svelte";
  import LanView from "$lib/components/LanView.svelte";
  import { t } from "$lib/i18n.svelte";
  import type {
    HistoryEntry,
    JobStatus,
    LanSettings,
    LibraryModelSummary,
    ModelRuntime,
    ModelSummary,
    RecordedTrack,
    RetentionPolicy,
    SearchHit,
    ServiceStatus,
    StorageSummary,
    Segment,
    SegmentEvent,
    Transcript,
    View,
  } from "$lib/types";

  const JOB_EVENT = "voice-to-text:job-status";
  const SEGMENT_EVENT = "voice-to-text:transcript-segment";
  const OPEN_EVENT = "voice-to-text:open-media";

  const tauriAvailable = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  const MEDIA_EXTENSIONS = [
    "wav", "wave", "aiff", "aif", "caf", "mp3", "mp2", "mp1", "m4a",
    "mp4", "mov", "mkv", "ogg", "oga", "flac", "aac", "opus", "3gp", "webm",
  ];

  // 本轮视觉迭代恢复折叠；使用共享标题栏按钮，不再安装原生 accessory。
  const SIDEBAR_COLLAPSE_ENABLED = true;

  // 折叠状态的持久化键（保留，便于恢复）。
  const SIDEBAR_KEY = "lt.sidebar.collapsed.v1";

  function initialView(): View {
    if (import.meta.env.DEV) {
      const requested = new URLSearchParams(window.location.search).get("view");
      if (requested === "model" || requested === "settings") return requested;
    }
    return "home";
  }

  let view = $state<View>(initialView());
  let model = $state<ModelSummary | null>(null);
  let fileName = $state<string | null>(null);
  let job = $state<JobStatus | null>(null);
  let segments = $state<Segment[]>([]);
  let transcript = $state<Transcript | null>(null);
  let error = $state("");
  let busy = $state(false);
  let dropping = $state(false);
  let libraryModels = $state<LibraryModelSummary[]>([]);
  let libraryBusy = $state(false);
  let storage = $state<StorageSummary | null>(null);
  let service = $state<ServiceStatus | null>(null);

  let history = $state<HistoryEntry[]>([]);
  let detail = $state<Transcript | null>(null);
  let detailJobId = $state<string | null>(null);
  let historyLoading = $state(false);

  let sidebarCollapsed = $state(false);
  let appVersion = $state("0.2.0");
  let loadedModelId = $state<string | null>(null);
  // 录音结束后按轨道依次转写（两条音轨分别成记录）。
  let pendingTracks = $state<string[]>([]);
  let searchQuery = $state("");
  let searchHits = $state<SearchHit[] | null>(null);
  let searchBusy = $state(false);
  let retention = $state<RetentionPolicy | null>(null);
  let lanSettings = $state<LanSettings | null>(null);

  const fullText = $derived(
    transcript?.text ?? segments.map((segment) => segment.text).join("\n"),
  );

  // 局域网派发提示：prefer_lan 模式下在转写页说明任务会去哪里。
  const lanHint = $derived(
    lanSettings?.mode === "prefer_lan" ? t("transcribe.lanHint") : null,
  );

  // 运行中每秒拉一次状态：CLI/MCP 启动的任务没有本进程事件，也能看到实时进度、速度与剩余时间。
  $effect(() => {
    if (!tauriAvailable) return;
    const runningId = job?.state === "running" ? job.jobId : null;
    if (!runningId) return;
    const timer = window.setInterval(async () => {
      try {
        const status = await invoke<JobStatus>("get_job_status", { jobId: runningId });
        job = status;
        if (status.state === "completed") void loadResult(status.jobId);
      } catch {
        /* 任务刚好结束或后端短暂不可用 */
      }
    }, 1000);
    return () => window.clearInterval(timer);
  });

  // 首页搜索：文件名或转写正文全文匹配（后端读取本地历史 JSON）。
  $effect(() => {
    if (!tauriAvailable) return;
    const query = searchQuery.trim();
    if (view !== "home" || query.length === 0) {
      searchHits = null;
      searchBusy = false;
      return;
    }
    searchBusy = true;
    const timer = window.setTimeout(async () => {
      try {
        searchHits = await invoke<SearchHit[]>("search_history", { query });
      } catch {
        searchHits = [];
      } finally {
        searchBusy = false;
      }
    }, 220);
    return () => window.clearTimeout(timer);
  });

  // 模型运行时状态：只用于顶栏绿点（是否真的驻留内存）。
  $effect(() => {
    if (!tauriAvailable) return;
    const tick = async () => {
      try {
        const runtime = await invoke<ModelRuntime>("model_runtime");
        loadedModelId = runtime.loadedModelId ?? null;
      } catch {
        /* 后端未就绪 */
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), 2000);
    return () => window.clearInterval(timer);
  });

  // 空闲时也每 2 秒看一眼共享队列：CLI / MCP 启动的任务会自动出现在界面里
  //（此前只在启动瞬间读一次，外部任务看不到进度）。
  $effect(() => {
    if (!tauriAvailable) return;
    if (job || transcript) return;
    const timer = window.setInterval(async () => {
      try {
        const status = await invoke<JobStatus | null>("get_active_job_status");
        if (status && status.state === "running") {
          job = status;
          fileName = null;
        }
      } catch {
        /* 后端未就绪 */
      }
    }, 2000);
    return () => window.clearInterval(timer);
  });

  function toggleSidebar() {
    if (!SIDEBAR_COLLAPSE_ENABLED) return;
    sidebarCollapsed = !sidebarCollapsed;
    try {
      localStorage.setItem(SIDEBAR_KEY, sidebarCollapsed ? "collapsed" : "expanded");
    } catch {
      /* ignore storage errors */
    }
  }

  async function refreshRetention() {
    try {
      retention = await invoke<RetentionPolicy>("get_retention_policy");
    } catch {
      /* 浏览器预览没有 Tauri 后端 */
    }
  }

  async function refreshLanSettings() {
    if (!tauriAvailable) return;
    try {
      lanSettings = await invoke<LanSettings>("get_lan_settings");
    } catch {
      /* 浏览器预览没有 Tauri 后端 */
    }
  }

  async function changeRetention(policy: RetentionPolicy) {
    try {
      retention = await invoke<RetentionPolicy>("set_retention_policy", { policy });
    } catch (issue) {
      error = String(issue);
    }
  }

  function navigate(next: View) {
    view = next;
    if (next === "home") {
      // 回到首页 = 回到列表：清掉当前的转写详情（正在跑的任务保持可见）。
      if (!job || job.state !== "running") {
        job = null;
        transcript = null;
        detail = null;
      }
      void loadHistory();
    }
    if (next === "settings") {
      void refreshService();
      void refreshLibrary();
      void refreshLanSettings();
    }
    if (next === "lan") void refreshLanSettings();
    if (next === "model") void refreshLibrary();
  }

  async function loadHistory() {
    historyLoading = true;
    try {
      history = await invoke<HistoryEntry[]>("list_history");
    } catch (issue) {
      error = String(issue);
    } finally {
      historyLoading = false;
    }
  }

  async function loadResult(jobId: string) {
    try {
      const result = await invoke<Transcript>("get_job_result", { jobId });
      transcript = result;
      fileName = result.sourceFileName;
      if (result.segments && result.segments.length > 0) {
        segments = result.segments;
      } else if (result.text) {
        segments = [{ startMs: 0, endMs: 0, text: result.text }];
      }
    } catch (issue) {
      error = String(issue);
    }
  }

  async function pickFile() {
    error = "";
    try {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: t("dialog.mediaFilter"), extensions: MEDIA_EXTENSIONS }],
      });
      if (typeof picked === "string") await startTranscription(picked);
    } catch (issue) {
      error = String(issue);
    }
  }

  async function startTranscription(path: string) {
    error = "";
    busy = true;
    try {
      const input = await invoke<{ fileName: string }>("inspect_input", { path });
      fileName = input.fileName;
      segments = [];
      transcript = null;
      const status = await invoke<JobStatus>("start_transcription", { path });
      job = status;
    } catch (issue) {
      error = String(issue);
    } finally {
      busy = false;
    }
  }

  function handleRecorded(tracks: RecordedTrack[]) {
    pendingTracks = tracks.map((track) => track.path);
    view = "home";
    void startNextTrack();
  }

  async function startNextTrack() {
    if (job?.state === "running") return;
    const next = pendingTracks.shift();
    if (!next) return;
    pendingTracks = [...pendingTracks];
    await startTranscription(next);
  }

  async function cancelJob() {
    if (!job) return;
    try {
      await invoke("cancel_transcription", { jobId: job.jobId });
    } catch (issue) {
      error = String(issue);
    }
  }

  async function copyAll() {
    if (!fullText) return;
    try {
      await navigator.clipboard.writeText(fullText);
    } catch (issue) {
      error = t("error.copyFailed", { error: String(issue) });
    }
  }

  async function exportResult(format: "txt" | "json") {
    if (!job && !transcript) return;
    const jobId = transcript?.jobId ?? job?.jobId;
    if (!jobId) return;
    busy = true;
    try {
      const base = (fileName ?? "transcript").replace(/\.[^.]+$/, "");
      const target = await save({
        defaultPath: `${base}.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (typeof target === "string") {
        await invoke("export_transcript", { jobId, path: target, format });
      }
    } catch (issue) {
      error = String(issue);
    } finally {
      busy = false;
    }
  }

  function chooseModel() {
    view = "model";
  }

  async function refreshLibrary() {
    if (!tauriAvailable) return;
    try {
      const [models, summary] = await Promise.all([
        invoke<LibraryModelSummary[]>("library_models"),
        invoke<StorageSummary>("storage_summary"),
      ]);
      libraryModels = models;
      storage = summary;
    } catch (issue) {
      error = String(issue);
    }
  }

  async function refreshService() {
    if (!tauriAvailable) return;
    try {
      service = await invoke<ServiceStatus>("service_status");
    } catch (issue) {
      error = String(issue);
    }
  }

  async function openStorage(path: string | undefined) {
    if (!path) return;
    try {
      await invoke("open_directory", { path });
    } catch (issue) {
      error = String(issue);
    }
  }

  async function refreshModel() {
    if (!tauriAvailable) return;
    try {
      model = await invoke<ModelSummary | null>("get_selected_model");
    } catch (issue) {
      error = String(issue);
    }
  }

  async function importModel() {
    error = "";
    libraryBusy = true;
    try {
      const picked = await open({ multiple: false, directory: true, title: t("dialog.importModelTitle") });
      if (typeof picked !== "string") return;
      await invoke("import_model", { path: picked });
      await Promise.all([refreshLibrary(), refreshModel()]);
    } catch (issue) {
      error = String(issue);
    } finally {
      libraryBusy = false;
    }
  }

  async function selectModel(id: string) {
    error = "";
    libraryBusy = true;
    try {
      await invoke("select_library_model", { id });
      await Promise.all([refreshLibrary(), refreshModel()]);
    } catch (issue) {
      error = String(issue);
    } finally {
      libraryBusy = false;
    }
  }

  async function deleteModel(target: LibraryModelSummary) {
    const approved = await confirm(
      t("dialog.deleteModelMessage", { name: target.displayName }),
      {
        title: t("dialog.deleteModelTitle"),
        kind: "warning",
        okLabel: t("common.delete"),
        cancelLabel: t("common.cancel"),
      },
    );
    if (!approved) return;
    error = "";
    libraryBusy = true;
    try {
      await invoke("delete_library_model", { id: target.id });
      await Promise.all([refreshLibrary(), refreshModel()]);
    } catch (issue) {
      error = String(issue);
    } finally {
      libraryBusy = false;
    }
  }

  async function selectHistory(jobId: string) {
    detailJobId = jobId;
    detail = null;
    try {
      detail = await invoke<Transcript>("get_job_result", { jobId });
    } catch (issue) {
      error = String(issue);
    }
  }

  function upsertSegment(event: SegmentEvent) {
    if (job && event.jobId !== job.jobId) return;
    const segment = { startMs: event.startMs, endMs: event.endMs, text: event.text };
    if (event.index === segments.length) {
      segments = [...segments, segment];
    } else if (event.index < segments.length) {
      const next = [...segments];
      next[event.index] = segment;
      segments = next;
    } else {
      segments = [...segments, segment];
    }
  }

  onMount(() => {
    if (SIDEBAR_COLLAPSE_ENABLED) {
      try {
        sidebarCollapsed = localStorage.getItem(SIDEBAR_KEY) === "collapsed";
      } catch {
        /* ignore storage errors */
      }
    }

    void getVersion()
      .then((value) => (appVersion = value))
      .catch(() => {
        /* 浏览器预览没有 Tauri 后端 */
      });
    void refreshRetention();
    void refreshLanSettings();

    const onKeydown = (event: KeyboardEvent) => {
      if (SIDEBAR_COLLAPSE_ENABLED && (event.metaKey || event.ctrlKey) && event.key === "\\") {
        event.preventDefault();
        toggleSidebar();
        return;
      }
      if (event.key === "Escape" && error) error = "";
    };
    window.addEventListener("keydown", onKeydown);

    const unlisteners: Array<Promise<() => void>> = [];

    let dragDropUnlisten: (() => void) | undefined;

    // 浏览器预览没有 Tauri 后端：只保留纯前端外壳，避免调用注入对象。
    if (tauriAvailable) {
      unlisteners.push(
        listen<JobStatus>(JOB_EVENT, (event) => {
          if (job && event.payload.jobId !== job.jobId) return;
          job = event.payload;
          if (event.payload.state === "completed") {
            void loadResult(event.payload.jobId);
          }
          if (event.payload.state !== "running" && pendingTracks.length > 0) {
            window.setTimeout(() => void startNextTrack(), 400);
          }
        }),
        listen<SegmentEvent>(SEGMENT_EVENT, (event) => upsertSegment(event.payload)),
        listen(OPEN_EVENT, () => void pickFile()),
        // 原生标题栏按钮（红绿灯同一行）发出的折叠开关事件。
        listen("voice-to-text:toggle-sidebar", () => toggleSidebar()),
      );

      if (view === "home") void loadHistory();
      void refreshLibrary();
      void refreshService();

    void invoke<ModelSummary | null>("get_selected_model")
        .then((value) => {
          model = value;
          void refreshLibrary();
        })
        .catch(() => {
          /* 后端未就绪时保持空模型状态 */
        });

      void invoke<JobStatus | null>("get_active_job_status")
        .then(async (status) => {
          if (!status) return;
          job = status;
          if (status.state === "completed") await loadResult(status.jobId);
        })
        .catch(() => {
          /* no active job */
        });

      try {
        void getCurrentWebview()
          .onDragDropEvent((event) => {
            if (event.payload.type === "over") {
              dropping = true;
            } else if (event.payload.type === "drop") {
              dropping = false;
              const first = event.payload.paths[0];
              if (first) void startTranscription(first);
            } else {
              dropping = false;
            }
          })
          .then((unlisten) => (dragDropUnlisten = unlisten))
          .catch(() => {
            /* 拖放不可用时不影响其他功能 */
          });
      } catch {
        /* 非 Tauri 环境没有 webview 注入对象 */
      }
    }

    return () => {
      window.removeEventListener("keydown", onKeydown);
      dragDropUnlisten?.();
      for (const pending of unlisteners) {
        void pending.then((stop) => stop()).catch(() => {});
      }
    };
  });
</script>

<div class="app" class:collapsed={sidebarCollapsed}>
  <header class="window-header">
    <div class="sidebar-header" data-tauri-drag-region>
      <button
        type="button"
        class="sidebar-toggle"
        aria-label={sidebarCollapsed ? t("nav.expandTitle") : t("nav.collapseTitle")}
        title={sidebarCollapsed ? t("nav.expandTitle") : t("nav.collapseTitle")}
        aria-expanded={!sidebarCollapsed}
        aria-controls="app-sidebar"
        onclick={toggleSidebar}
      ><Icon name="panel" size={18} /></button>
    </div>
    <div class="header-tools" data-tauri-drag-region>
    <AppToolbar
      {model}
      models={libraryModels}
      {loadedModelId}
      search={searchQuery}
      onSearchChange={(value) => (searchQuery = value)}
      onSelectModel={selectModel}
      onManageModels={() => (view = "model")}
    />
    </div>
  </header>
  <div class="workspace">
  <AppSidebar {view} collapsed={sidebarCollapsed} onNavigate={navigate} />
  <main class="main">

    {#if view === "home"}
      {#if detail && detailJobId}
        <HistoryView
          entries={history}
          {detail}
          {detailJobId}
          loading={historyLoading}
          onSelect={selectHistory}
          onExport={exportResult}
          onBack={() => {
            detail = null;
            detailJobId = null;
          }}
        />
      {:else if job || transcript}
        <TranscribeView
          {model}
          {fileName}
          {job}
          {segments}
          {transcript}
          {error}
          {busy}
          {dropping}
          {lanHint}
          onPickFile={pickFile}
          onChooseModel={chooseModel}
          onCancel={cancelJob}
          onExport={exportResult}
          onCopy={copyAll}
          onBack={() => {
            job = null;
            transcript = null;
            segments = [];
            fileName = null;
            error = "";
            void loadHistory();
          }}
        />
      {:else}
        <HistoryView
          entries={history}
          loading={historyLoading}
          hits={searchHits}
          {searchBusy}
          onSelect={selectHistory}
          onExport={exportResult}
          onPickFile={pickFile}
          {dropping}
        />
      {/if}
    {:else if view === "record"}
      <RecordView onRecorded={handleRecorded} />
    {:else if view === "model"}
      <ModelView
        models={libraryModels}
        busy={libraryBusy}
        onImport={importModel}
        onSelect={selectModel}
        onDelete={deleteModel}
      />
    {:else if view === "lan"}
      <LanView
        settings={lanSettings}
        onSettingsSaved={(value) => (lanSettings = value)}
      />
    {:else}
      <SettingsView
        {model}
        {storage}
        {service}
        {appVersion}
        {retention}
        lanMode={lanSettings?.mode ?? null}
        onRefreshService={refreshService}
        onOpenLibrary={() => openStorage(storage?.libraryPath)}
        onOpenHistory={() => openStorage(storage?.historyPath)}
        onOpenPath={openStorage}
        onRetentionChange={changeRetention}
        onGoModels={() => (view = "model")}
        onGoLan={() => (view = "lan")}
      />
    {/if}
  </main>
  </div>
</div>

<style>
  .app {
    --sidebar-width: 204px;
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    background: var(--bg);
    color: var(--text);
  }

  .window-header {
    display: flex;
    height: 52px;
    flex: none;
    user-select: none;
  }

  .sidebar-header {
    width: var(--sidebar-width);
    flex: none;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 10px 0 80px;
    background: var(--sidebar);
    border-right: 1px solid var(--hairline);
    transition: width var(--speed) var(--ease-out);
  }

  .collapsed .sidebar-header {
    width: 126px;
    background: var(--bg);
    border-right-color: transparent;
  }

  .sidebar-toggle {
    width: 30px;
    height: 30px;
    flex: none;
    display: grid;
    place-items: center;
    border: 1px solid var(--hairline-2);
    border-radius: 50%;
    background: var(--hover);
    color: var(--text-2);
    cursor: default;
  }

  .sidebar-toggle:hover { background: var(--selected); }

  .header-tools { flex: 1; min-width: 0; }

  .workspace {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  /* 内容区：侧栏右侧的通高区域；圆角与窗口边框交给原生窗口 */
  .main {
    flex: 1;
    /* 必须可收缩：否则 min-content（MCP 代码块 pre 长行/长路径）会把整条布局顶得比窗口宽 */
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 8px 12px 12px 12px;
    background: var(--bg);
  }
</style>
