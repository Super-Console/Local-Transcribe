<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { confirm, open, save } from "@tauri-apps/plugin-dialog";

  import AppSidebar from "$lib/components/AppSidebar.svelte";
  import AppToolbar from "$lib/components/AppToolbar.svelte";
  import TranscribeView from "$lib/components/TranscribeView.svelte";
  import HistoryView from "$lib/components/HistoryView.svelte";
  import ModelView from "$lib/components/ModelView.svelte";
  import SettingsView from "$lib/components/SettingsView.svelte";
  import RecordView from "$lib/components/RecordView.svelte";
  import LanView from "$lib/components/LanView.svelte";
  import { t } from "$lib/i18n.svelte";
  import { ensureModelNames, prettyName } from "$lib/model-names.svelte";
  import type {
    CatalogModel,
    HistoryEntry,
    JobStatus,
    LanSettings,
    LanNodeStatus,
    LibraryModelSummary,
    ModelDownloadEvent,
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
  const MODEL_DOWNLOAD_EVENT = "voice-to-text:model-download";
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
  // 本次任务实际使用的模型：由启动那一刻确定（远程任务用节点上的模型），
  // 不是渲染时的"当前选中"，避免结果页显示一条与本次转写无关的模型。
  let startedModelId = $state<string | null>(null);
  let error = $state("");
  let busy = $state(false);
  let dropping = $state(false);
  let libraryModels = $state<LibraryModelSummary[]>([]);
  let libraryBusy = $state(false);
  let catalog = $state<CatalogModel[]>([]);
  let modelDownloads = $state<Record<string, ModelDownloadEvent>>({});
  let storage = $state<StorageSummary | null>(null);
  let service = $state<ServiceStatus | null>(null);

  let history = $state<HistoryEntry[]>([]);
  let detail = $state<Transcript | null>(null);
  let detailJobId = $state<string | null>(null);
  let historyLoading = $state(false);
  /** 删除成功后的短暂提示；null 表示不显示。 */
  let historyNotice = $state<string | null>(null);
  let historyNoticeTimer: number | undefined;

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
  /** Models each enabled node offers, for the toolbar's grouped menu. */
  let lanGroups = $state<
    Array<{ id: string; name: string; models: Array<{ id: string; displayName: string }> }>
  >([]);

  const remoteSelection = $derived(
    lanSettings?.mode === "prefer_lan"
      ? (() => {
          const node = lanSettings.nodes.find((entry) => entry.enabled && entry.model_id);
          return node?.model_id ? { nodeId: node.id, modelId: node.model_id } : null;
        })()
      : null,
  );

  const remoteLabel = $derived(
    remoteSelection
      ? (() => {
          const node = lanSettings?.nodes.find((entry) => entry.id === remoteSelection.nodeId);
          const model = lanGroups
            .find((group) => group.id === remoteSelection.nodeId)
            ?.models.find((entry) => entry.id === remoteSelection.modelId);
          return node
            ? `${node.name} · ${prettyName(model?.displayName ?? remoteSelection.modelId)}`
            : null;
        })()
      : null,
  );

  /** 描述"这条结果"的模型：以记录里的 model_id 为准，取不到才是未知。 */
  const taskModelId = $derived(transcript?.modelId?.trim() || startedModelId);

  /** 正在运行的任务 id（首页卡片的删除入口据此置灰）。 */
  const runningJobId = $derived(job?.state === "running" ? job.jobId : null);

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
          // 外部（CLI / MCP）启动的任务：本界面不知道它用的模型。
          startedModelId = null;
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

  // macOS Overlay 标题栏下 data-tauri-drag-region 不生效（tauri#9503 上游 bug），
  // 且该属性只认直接命中，工具栏等子元素会挡住拖动；改为在标题栏统一显式
  // startDragging()，可交互控件（按钮/输入等）保持原有点击行为。
  function headerPointerDown(event: MouseEvent) {
    if (!tauriAvailable || event.button !== 0 || event.detail > 1) return;
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (
      target.closest(
        'a, button, input, select, textarea, label, summary, [contenteditable]:not([contenteditable="false"]), [tabindex]:not([tabindex="-1"]), [role="button"], [role="link"], [role="tab"], [role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"], [role="listbox"], [role="menu"]',
      )
    ) {
      return;
    }
    event.preventDefault();
    try {
      void getCurrentWindow().startDragging().catch(() => {
        /* 权限缺失或非窗口环境时静默降级 */
      });
    } catch {
      /* 非 Tauri 环境没有窗口对象 */
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
      await refreshLanGroups(lanSettings);
    } catch {
      /* 浏览器预览没有 Tauri 后端 */
    }
  }

  /** Ask each enabled node which models it offers; failures just hide that node. */
  async function refreshLanGroups(settings: LanSettings) {
    const enabled = settings.nodes.filter((node) => node.enabled);
    const groups = await Promise.all(
      enabled.map(async (node) => {
        try {
          const status = await invoke<LanNodeStatus>("lan_node_status", { nodeId: node.id });
          return {
            id: node.id,
            name: node.name,
            models: status.models.map((model) => ({
              id: model.id,
              displayName: model.display_name,
            })),
          };
        } catch {
          return { id: node.id, name: node.name, models: [] };
        }
      }),
    );
    lanGroups = groups.filter((group) => group.models.length > 0);
  }

  /** Pick a model on a remote node: the next job goes there (LAN first, local fallback). */
  async function selectRemoteModel(nodeId: string, modelId: string) {
    if (!lanSettings) return;
    libraryBusy = true;
    try {
      const next: LanSettings = {
        ...lanSettings,
        mode: "prefer_lan",
        nodes: lanSettings.nodes.map((node) =>
          node.id === nodeId ? { ...node, model_id: modelId, enabled: true } : node,
        ),
      };
      lanSettings = await invoke<LanSettings>("set_lan_settings", { settings: next });
      await refreshLanGroups(lanSettings);
    } catch (issue) {
      error = String(issue);
    } finally {
      libraryBusy = false;
    }
  }

  async function changeLanSettings(next: LanSettings) {
    try {
      lanSettings = await invoke<LanSettings>("set_lan_settings", { settings: next });
    } catch (issue) {
      error = String(issue);
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
        startedModelId = null;
      }
      void loadHistory();
    }
    if (next === "settings") {
      void refreshService();
      void refreshLibrary();
      void refreshLanSettings();
    }
    if (next === "lan") void refreshLanSettings();
    if (next === "home") void refreshLanSettings();
    if (next === "model") {
      void refreshLibrary();
      void refreshCatalog();
    }
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
      startedModelId = remoteSelection?.modelId ?? model?.id ?? null;
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

  async function exportResult(format: "txt" | "json") {
    // 导出按 jobId 读回已保存的转写记录：结果页、运行中任务与历史详情同一来源，
    // 因此 JSON 里的模型字段永远是这条记录的 model_id。
    const jobId = transcript?.jobId ?? job?.jobId ?? detailJobId;
    if (!jobId) return;
    busy = true;
    try {
      const base = (fileName ?? detail?.sourceFileName ?? "transcript").replace(/\.[^.]+$/, "");
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

  async function refreshCatalog() {
    if (!tauriAvailable) return;
    try {
      catalog = await invoke<CatalogModel[]>("model_catalog");
    } catch (issue) {
      error = String(issue);
    }
  }

  async function downloadModel(id: string) {
    try {
      error = "";
      modelDownloads = {
        ...modelDownloads,
        [id]: { id, state: "downloading", received: 0, total: 0 },
      };
      await invoke("download_model", { id });
    } catch (issue) {
      error = String(issue);
      const { [id]: _pending, ...rest } = modelDownloads;
      modelDownloads = rest;
    }
  }

  async function cancelModelDownload() {
    try {
      await invoke("cancel_model_download");
    } catch (issue) {
      error = String(issue);
    }
  }

  function applyDownloadEvent(event: ModelDownloadEvent) {
    if (event.state === "done" || event.state === "error" || event.state === "cancelled") {
      const { [event.id]: _finished, ...rest } = modelDownloads;
      modelDownloads = rest;
      if (event.state === "done") {
        void refreshLibrary();
        void refreshCatalog();
      }
      return;
    }
    modelDownloads = { ...modelDownloads, [event.id]: event };
  }

  async function refreshService() {
    if (!tauriAvailable) return;
    try {
      service = await invoke<ServiceStatus>("service_status");
    } catch (issue) {
      error = String(issue);
    }
  }

  async function openThirdPartyNotices() {
    try {
      const path = await invoke<string>("third_party_notices_path");
      await invoke("open_path", { path });
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

  async function selectModel(id: string) {
    error = "";
    libraryBusy = true;
    try {
      await invoke("select_library_model", { id });
      // Choosing a local model means "run here": drop back to local-only if a
      // remote model was driving the home screen.
      if (lanSettings?.mode === "prefer_lan") {
        lanSettings = await invoke<LanSettings>("set_lan_assignment", { mode: "local_only" });
      }
      await Promise.all([refreshLibrary(), refreshModel()]);
    } catch (issue) {
      error = String(issue);
    } finally {
      libraryBusy = false;
    }
  }

  async function deleteCatalogModel(entry: CatalogModel) {
    const approved = await confirm(t("dialog.deleteModelMessage", { name: entry.name }), {
      title: t("dialog.deleteModelTitle"),
      kind: "warning",
      okLabel: t("common.delete"),
      cancelLabel: t("common.cancel"),
    });
    if (!approved) return;
    error = "";
    libraryBusy = true;
    try {
      await invoke("delete_catalog_model", { id: entry.id });
      await Promise.all([refreshLibrary(), refreshCatalog(), refreshModel()]);
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

  /** 首页卡片删除：只删本应用的记录与任务状态，原始媒体不动（后端负责移废纸篓）。 */
  async function deleteHistoryEntry(entry: { jobId: string; sourceFileName: string }) {
    const approved = await confirm(
      t("dialog.deleteHistoryMessage", { name: entry.sourceFileName }),
      {
        title: t("dialog.deleteHistoryTitle"),
        kind: "warning",
        okLabel: t("common.delete"),
        cancelLabel: t("common.cancel"),
      },
    );
    if (!approved) return;
    error = "";
    try {
      const outcome = await invoke<{ removed: string[]; skipped: string[] }>(
        "delete_history_entry",
        { jobId: entry.jobId },
      );
      if (detailJobId === entry.jobId) {
        detail = null;
        detailJobId = null;
      }
      if (transcript?.jobId === entry.jobId) {
        transcript = null;
        job = null;
        segments = [];
        fileName = null;
        startedModelId = null;
      }
      await loadHistory();
      historyNotice = t("history.deletedNotice", { count: outcome.removed.length });
      if (historyNoticeTimer) window.clearTimeout(historyNoticeTimer);
      historyNoticeTimer = window.setTimeout(() => (historyNotice = null), 4000);
    } catch (issue) {
      error = String(issue);
    }
  }

  async function selectHistory(jobId: string) {    detailJobId = jobId;
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
    ensureModelNames();
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
    void refreshLibrary();
    void refreshCatalog();

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
        listen<ModelDownloadEvent>(MODEL_DOWNLOAD_EVENT, (event) =>
          applyDownloadEvent(event.payload),
        ),
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
          // 启动时已被外部任务占用的界面不知道模型；完成态改由记录补齐。
          startedModelId = null;
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
  <header class="window-header" role="presentation" onmousedown={headerPointerDown}>
    <div class="sidebar-header"></div>
    <div class="header-tools">
    <AppToolbar
      {model}
      models={libraryModels}
      {loadedModelId}
      remoteGroups={lanGroups}
      {remoteLabel}
      {remoteSelection}
      search={searchQuery}
      onSearchChange={(value) => (searchQuery = value)}
      onSelectModel={selectModel}
      onSelectRemote={selectRemoteModel}
      onManageModels={() => (view = "model")}
      {sidebarCollapsed}
      onToggleSidebar={toggleSidebar}
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
          onDelete={deleteHistoryEntry}
          {runningJobId}
          notice={historyNotice}
          onBack={() => {
            detail = null;
            detailJobId = null;
          }}
        />
      {:else if job || transcript}
        <TranscribeView
          modelId={taskModelId}
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
          onBack={() => {
            job = null;
            transcript = null;
            segments = [];
            fileName = null;
            error = "";
            startedModelId = null;
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
          onDelete={deleteHistoryEntry}
          {runningJobId}
          notice={historyNotice}
          onPickFile={pickFile}
          {dropping}
        />
      {/if}
    {:else if view === "record"}
      <RecordView onRecorded={handleRecorded} />
    {:else if view === "model"}
      <ModelView
        models={libraryModels}
        {catalog}
        downloads={modelDownloads}
        busy={libraryBusy}
        onSelect={selectModel}
        onDelete={deleteModel}
        onDeleteCatalog={deleteCatalogModel}
        onDownload={downloadModel}
        onCancelDownload={cancelModelDownload}
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
        {lanSettings}
        onLanSettingsChange={changeLanSettings}
        onRefreshService={refreshService}
        onOpenLibrary={() => openStorage(storage?.libraryPath)}
        onOpenHistory={() => openStorage(storage?.historyPath)}
        onOpenPath={openStorage}
        onOpenNotices={openThirdPartyNotices}
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

  .header-tools { flex: 1; min-width: 0; }

  .workspace {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  /* 工作区内角用统一 token；原生窗口外角仍由系统管理。 */
  .main {
    border-radius: var(--radius);
    overflow: hidden;
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
