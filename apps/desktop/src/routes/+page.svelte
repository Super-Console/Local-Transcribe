<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { confirm, open, save } from "@tauri-apps/plugin-dialog";

  import AppSidebar from "$lib/components/AppSidebar.svelte";
  import AppToolbar from "$lib/components/AppToolbar.svelte";
  import TranscribeView from "$lib/components/TranscribeView.svelte";
  import HistoryView from "$lib/components/HistoryView.svelte";
  import ModelView from "$lib/components/ModelView.svelte";
  import SettingsView from "$lib/components/SettingsView.svelte";
  import type {
    HistoryEntry,
    JobStatus,
    LibraryModelSummary,
    ModelSummary,
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

  const TITLES: Record<View, string> = {
    transcribe: "转写",
    history: "历史",
    model: "模型",
    settings: "设置",
  };

  // 侧栏折叠阈值带滞回：< 780 折叠，> 820 才恢复，避免宽度抖动时反复闪烁。
  const NARROW = 780;
  const WIDE = 820;
  const SIDEBAR_KEY = "lt.sidebar.v2";

  function initialView(): View {
    if (import.meta.env.DEV) {
      const requested = new URLSearchParams(window.location.search).get("view");
      if (requested === "history" || requested === "model" || requested === "settings") {
        return requested;
      }
    }
    return "transcribe";
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

  let viewportWidth = $state(900);
  let autoHidden = $state(false);
  let userPreference = $state<"shown" | "hidden">("shown");
  let overlayOpen = $state(false);

  const sidebarMode = $derived(
    userPreference === "hidden" || autoHidden
      ? overlayOpen
        ? "overlay"
        : "hidden"
      : "inline",
  );
  const sidebarVisible = $derived(sidebarMode !== "hidden");
  const fullText = $derived(
    transcript?.text ?? segments.map((segment) => segment.text).join("\n"),
  );

  async function beginDragWindow(event: MouseEvent) {
    if (event.button !== 0) return;
    if (!tauriAvailable) return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().startDragging();
    } catch {
      /* 忽略 */
    }
  }

  async function toggleWindowMaximize() {
    if (!tauriAvailable) return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().toggleMaximize();
    } catch {
      /* 忽略 */
    }
  }

  let resizeSettle: ReturnType<typeof setTimeout> | undefined;

  function syncWidth(width: number) {
    viewportWidth = width;
    // 拖动窗口时先只记录宽度；停手 120ms 后再决定折叠，
    // 否则跨越阈值的一瞬间侧栏消失，看起来像窗口尺寸突变。
    if (resizeSettle) clearTimeout(resizeSettle);
    resizeSettle = setTimeout(() => {
      if (viewportWidth < NARROW) autoHidden = true;
      else if (viewportWidth > WIDE) autoHidden = false;
    }, 120);
  }

  function toggleSidebar() {
    if (sidebarMode === "inline") {
      userPreference = "hidden";
      persistSidebarPreference();
      overlayOpen = false;
      return;
    }
    // 宽窗下再次点击 = 恢复内联侧栏（否则手动折叠后没有回头路）；
    // 窄窗下才用临时浮层导航。
    if (viewportWidth > WIDE) {
      userPreference = "shown";
      persistSidebarPreference();
      overlayOpen = false;
      return;
    }
    overlayOpen = !overlayOpen;
  }

  function persistSidebarPreference() {
    try {
      localStorage.setItem(SIDEBAR_KEY, userPreference);
    } catch {
      /* ignore storage errors */
    }
  }

  function navigate(next: View) {
    view = next;
    overlayOpen = false;
    if (next === "history") void loadHistory();
    if (next === "settings") {
      void refreshService();
      void refreshLibrary();
    }
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
        filters: [{ name: "音频或视频", extensions: MEDIA_EXTENSIONS }],
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
      error = `复制失败：${String(issue)}`;
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
      const picked = await open({ multiple: false, directory: true, title: "选择模型文件夹" });
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
      `删除「${target.displayName}」在应用内的副本？原始下载文件不会被删除，副本会移入废纸篓。`,
      { title: "删除模型", kind: "warning", okLabel: "删除", cancelLabel: "取消" },
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
    try {
      const stored = localStorage.getItem(SIDEBAR_KEY);
      if (stored === "hidden" || stored === "shown") userPreference = stored;
    } catch {
      /* ignore storage errors */
    }
    syncWidth(document.documentElement.clientWidth || window.innerWidth);

    const onResize = () => syncWidth(document.documentElement.clientWidth || window.innerWidth);
    window.addEventListener("resize", onResize);

    const observer = new ResizeObserver(onResize);
    observer.observe(document.documentElement);

    const onKeydown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "\\") {
        event.preventDefault();
        toggleSidebar();
        return;
      }
      if (event.key === "Escape") {
        if (overlayOpen) overlayOpen = false;
        else if (error) error = "";
      }
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
        }),
        listen<SegmentEvent>(SEGMENT_EVENT, (event) => upsertSegment(event.payload)),
        listen(OPEN_EVENT, () => void pickFile()),
      );

      if (view === "history") void loadHistory();
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
      window.removeEventListener("resize", onResize);
      window.removeEventListener("keydown", onKeydown);
      observer.disconnect();
      dragDropUnlisten?.();
      for (const pending of unlisteners) {
        void pending.then((stop) => stop()).catch(() => {});
      }
    };
  });
</script>

<div class="app">
  <div class="body">
  <div class="side-shell" class:collapsed={sidebarMode !== "inline"} inert={sidebarMode !== "inline"}>
    <AppSidebar {view} onNavigate={navigate} />
  </div>

  <main class="main" class:collapsed={sidebarMode !== "inline"}>
    <AppToolbar
      title={TITLES[view]}
      {model}
      {sidebarVisible}
      onToggleSidebar={toggleSidebar}
      onChooseModel={chooseModel}
    />

    {#if view === "transcribe"}
      <TranscribeView
        {model}
        {fileName}
        {job}
        {segments}
        {transcript}
        {error}
        {busy}
        {dropping}
        onPickFile={pickFile}
        onChooseModel={chooseModel}
        onCancel={cancelJob}
        onExport={exportResult}
        onCopy={copyAll}
      />
    {:else if view === "history"}
      <HistoryView
        entries={history}
        detail={detail}
        detailJobId={detailJobId}
        loading={historyLoading}
        onSelect={selectHistory}
        onBack={() => {
          detail = null;
          detailJobId = null;
        }}
        onExport={exportResult}
      />
    {:else if view === "model"}
      <ModelView
        models={libraryModels}
        busy={libraryBusy}
        onImport={importModel}
        onSelect={selectModel}
        onDelete={deleteModel}
      />
    {:else}
      <SettingsView
        {model}
        {storage}
        {service}
        onRefreshService={refreshService}
        onOpenLibrary={() => openStorage(storage?.libraryPath)}
        onOpenHistory={() => openStorage(storage?.historyPath)}
        onGoModels={() => (view = "model")}
      />
    {/if}
  </main>
  </div>

  {#if sidebarMode === "overlay"}
    <button class="scrim" type="button" aria-label="关闭导航" onclick={() => (overlayOpen = false)}></button>
    <div class="overlay">
      <AppSidebar {view} onNavigate={navigate} />
    </div>
  {/if}
</div>

<style>
  .app {
    display: flex;
    height: 100vh;
    overflow: hidden;
    border-radius: 12px;
    background: transparent;
    color: var(--text);
  }

  .body {
    flex: 1;
    /* 必须可收缩：否则 min-content（MCP 代码块 pre 长行/长路径）会把整条布局顶得比窗口宽 */
    min-width: 0;
    min-height: 0;
    display: flex;
    position: relative;
    gap: 0;
    padding: 0;
  }

  /* 侧栏是唯一浮起的圆角卡片，压在铺满窗口的内容区之上 */
  .side-shell {
    position: absolute;
    left: 12px;
    top: 8px;
    bottom: 8px;
    width: 196px;
    z-index: 2;
    overflow: hidden;
    /* 与 .sidebar 同圆角，避免 overflow 裁剪把投影切成直角 */
    border-radius: 14px;
    transition: width 160ms ease, opacity 140ms ease;
  }

  .side-shell.collapsed {
    width: 0;
    opacity: 0;
    pointer-events: none;
  }



  /* 内容区铺满窗口，圆角交给窗口本身裁切，不再是一块独立面板 */
  .main {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 8px 12px 12px 230px;
    background: var(--bg);
    border: 0;
    border-radius: 0;
    transition: padding-left 160ms ease;
  }

  /* 侧栏收起后内容回到窗口左侧 */
  .main.collapsed {
    padding-left: 12px;
  }

  .scrim {
    position: fixed;
    inset: 0;
    border: 0;
    padding: 0;
    background: rgba(0, 0, 0, 0.16);
    z-index: 9;
  }

  .overlay {
    position: fixed;
    left: 12px;
    top: 12px;
    bottom: 12px;
    width: 196px;
    z-index: 10;
    box-shadow: 0 12px 34px rgba(0, 0, 0, 0.24);
    border-radius: 14px;
  }
</style>
