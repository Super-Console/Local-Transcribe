<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  type InputFile = {
    path: string;
    fileName: string;
    sizeBytes: number;
  };

  type ModelSummary = {
    id: string;
    displayName: string;
    family: string;
  };

  type Transcript = {
    schemaVersion: string;
    jobId: string;
    sourceFileName: string;
    modelId: string;
    engine: string;
    language?: string;
    durationMs?: number;
    text: string;
  };

  type JobStatus = {
    jobId: string;
    state: "running" | "completed" | "failed" | "cancelled";
    phase: string;
    completedChunks: number;
    totalChunks?: number;
    transcriptAvailable: boolean;
    resultSaved: boolean;
    error?: string;
  };

  type HistoryEntry = {
    jobId: string;
    sourceFileName: string;
    modelId: string;
    durationMs?: number;
    preview: string;
    updatedAtMs: number;
  };

  let selected = $state<InputFile | null>(null);
  let model = $state<ModelSummary | null>(null);
  let currentJob = $state<JobStatus | null>(null);
  let transcript = $state<Transcript | null>(null);
  let history = $state<HistoryEntry[]>([]);
  let busy = $state(false);
  let modelBusy = $state(false);
  let errorMessage = $state("");

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let unlistenJob: (() => void) | undefined;
    void listen("voice-to-text:open-media", () => {
      void chooseFile();
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {
        if (!disposed) {
          errorMessage = "菜单快捷键暂不可用，仍可使用页面内的“选择文件…”按钮。";
        }
      });
    void listen<JobStatus>("voice-to-text:job-status", (event) => {
      currentJob = event.payload;
      if (event.payload.state === "completed" && event.payload.transcriptAvailable) {
        void loadTranscript(event.payload.jobId);
      } else if (event.payload.state === "failed" || event.payload.state === "cancelled") {
        errorMessage = event.payload.state === "cancelled"
          ? "转写已取消。"
          : event.payload.error ?? "转写失败。";
      }
    })
      .then((stop) => {
        if (disposed) stop();
        else unlistenJob = stop;
      })
      .catch(() => {
        if (!disposed) errorMessage = "任务状态监听暂不可用。";
      });
    void refreshModel();
    void refreshHistory();
    void refreshActiveJob();
    const sharedTaskPoll = window.setInterval(() => {
      void refreshActiveJob();
      void refreshHistory();
    }, 3000);

    return () => {
      disposed = true;
      window.clearInterval(sharedTaskPoll);
      unlisten?.();
      unlistenJob?.();
    };
  });

  async function chooseFile() {
    if (currentJob?.state === "running") return;
    errorMessage = "";
    try {
      const path = await open({ multiple: false, directory: false });
      if (typeof path !== "string") return;
      busy = true;
      selected = await invoke<InputFile>("inspect_input", { path });
      currentJob = null;
      transcript = null;
    } catch {
      selected = null;
      errorMessage = "无法读取所选文件。请确认它仍存在且当前用户有读取权限。";
    } finally {
      busy = false;
    }
  }

  async function refreshModel() {
    try {
      model = await invoke<ModelSummary | null>("get_selected_model");
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function refreshHistory() {
    try {
      history = await invoke<HistoryEntry[]>("list_history");
    } catch {
      history = [];
    }
  }

  async function refreshActiveJob() {
    try {
      let status = await invoke<JobStatus | null>("get_active_job_status");
      if (!status && currentJob?.state === "running") {
        status = await invoke<JobStatus>("get_job_status", { jobId: currentJob.jobId });
      }
      if (!status) return;
      const previousState = currentJob?.state;
      currentJob = status;
      if (status.state === "completed" && status.transcriptAvailable && transcript?.jobId !== status.jobId) {
        await loadTranscript(status.jobId);
      } else if ((status.state === "failed" || status.state === "cancelled") && previousState === "running") {
        errorMessage = status.state === "cancelled" ? "转写已取消。" : status.error ?? "转写失败。";
      }
    } catch {
      // The regular desktop workflow remains usable if shared task polling is unavailable.
    }
  }

  async function chooseModel() {
    errorMessage = "";
    try {
      const path = await open({
        multiple: false,
        directory: true,
        title: "选择 sherpa-onnx 模型文件夹",
      });
      if (typeof path !== "string") return;
      modelBusy = true;
      model = await invoke<ModelSummary>("set_selected_model", { path });
    } catch (error) {
      errorMessage = String(error);
    } finally {
      modelBusy = false;
    }
  }

  async function startTranscription() {
    if (!selected || !model || currentJob?.state === "running") return;
    errorMessage = "";
    transcript = null;
    try {
      currentJob = await invoke<JobStatus>("start_transcription", { path: selected.path });
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function loadTranscript(jobId: string) {
    try {
      transcript = await invoke<Transcript>("get_job_result", { jobId });
      await refreshHistory();
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function openHistoryItem(jobId: string) {
    errorMessage = "";
    currentJob = await invoke<JobStatus>("get_job_status", { jobId }).catch(() => null);
    await loadTranscript(jobId);
  }

  async function copyTranscript() {
    if (!transcript) return;
    try {
      await navigator.clipboard.writeText(transcript.text);
      errorMessage = "已复制转写文本。";
    } catch {
      const textarea = document.createElement("textarea");
      textarea.value = transcript.text;
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.select();
      const copied = document.execCommand("copy");
      textarea.remove();
      errorMessage = copied ? "已复制转写文本。" : "复制失败，请手动选择文本。";
    }
  }

  async function exportTranscript(format: "txt" | "json") {
    if (!transcript) return;
    const baseName = transcript.sourceFileName.replace(/\.[^.]+$/, "") || "transcript";
    try {
      const path = await save({
        defaultPath: `${baseName}.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!path) return;
      const exportedPath = await invoke<string>("export_transcript", {
        jobId: transcript.jobId,
        path,
        format,
      });
      errorMessage = `已导出并核对文件：${exportedPath}`;
    } catch (error) {
      errorMessage = String(error);
    }
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let value = bytes;
    let unitIndex = -1;
    do {
      value /= 1024;
      unitIndex += 1;
    } while (value >= 1024 && unitIndex < units.length - 1);
    return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unitIndex]}`;
  }
</script>

<svelte:head>
  <title>转写工作台</title>
  <meta name="description" content="选择音视频文件并查看转写结果。" />
</svelte:head>

<div class="app-shell" aria-busy={busy || currentJob?.state === "running"}>
  <aside class="sidebar" aria-label="应用导航">
    <div class="brand">
      <div class="brand-mark" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 10v4" />
        </svg>
      </div>
      <span class="brand-name">语音转写</span>
    </div>

    <nav class="primary-nav" aria-label="主导航">
      <a class="nav-item active" href="#transcribe-panel" aria-current="page" aria-label="转写" title="转写">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 10v4" />
        </svg>
        <span>转写</span>
      </a>
      <button class="nav-item" type="button" onclick={chooseModel} disabled={modelBusy || busy || currentJob?.state === "running"} aria-label="模型" title="模型">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="m12 3 8 4.5v9L12 21l-8-4.5v-9L12 3Z" />
          <path d="m4.5 7.8 7.5 4.4 7.5-4.4M12 12.2V21" />
        </svg>
        <span>模型</span>
      </button>
      <a class="nav-item" href="#history-panel" aria-label="历史" title="历史">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M3.5 12a8.5 8.5 0 1 0 2.3-5.8L3.5 8.5" />
          <path d="M3.5 4.5v4h4M12 7.5v5l3.25 2" />
        </svg>
        <span>历史</span>
      </a>
    </nav>

    <div class="sidebar-spacer"></div>
    <span class="prototype-label">界面原型</span>
  </aside>

  <main id="transcribe-panel" class="main-pane">
    <header class="workspace-header">
      <div>
        <p class="eyebrow">WORKSPACE</p>
        <h1>转写工作台</h1>
        <p class="page-subtitle">选择音视频，开始转写，并查看或导出结果。</p>
      </div>
    </header>

    {#if errorMessage}
      <p class="notice" role="alert">{errorMessage}</p>
    {/if}

    <section class="workflow-card" aria-labelledby="file-heading">
      <div class="section-heading">
        <div class="section-title-group">
          <span class="section-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
              <path d="M7 3.75h7l5 5V20a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 20V5.25A1.5 1.5 0 0 1 6.5 3.75Z" />
              <path d="M14 4v5h5M8.5 13h7M8.5 16.5h7" />
            </svg>
          </span>
          <div>
            <p class="section-kicker">SOURCE</p>
            <h2 id="file-heading">音视频文件</h2>
          </div>
        </div>
        <span class="file-status" class:ready={selected !== null}>
          <span class="status-dot" aria-hidden="true"></span>
          {selected ? "已选择" : "待选择"}
        </span>
      </div>

      {#if selected}
        <div class="selected-file">
          <div class="file-type-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
              <path d="M7 3.75h7l5 5V20a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 20V5.25A1.5 1.5 0 0 1 6.5 3.75Z" />
              <path d="M14 4v5h5M9 13.5h.01M12 13.5h.01M15 13.5h.01M9 17h.01M12 17h.01M15 17h.01" />
            </svg>
          </div>
          <div class="selected-file-copy">
            <p class="selected-file-name">{selected.fileName}</p>
            <p class="selected-file-meta">{formatSize(selected.sizeBytes)}</p>
          </div>
          <button class="text-button" type="button" onclick={chooseFile} disabled={busy || currentJob?.state === "running"}>
            更换文件…
          </button>
        </div>
      {:else}
        <div class="file-picker-card">
          <div class="picker-symbol" aria-hidden="true">
            <svg viewBox="0 0 40 40" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M11 4.75h12l8 8V32a3 3 0 0 1-3 3H11a3 3 0 0 1-3-3V7.75a3 3 0 0 1 3-3Z" />
              <path d="M23 5v8h8M20 19v9M15.5 23.5h9" />
            </svg>
          </div>
          <p class="picker-title">选择一份音频或视频文件</p>
          <p class="picker-description">选择后可查看文件信息并开始转写。</p>
          <button class="secondary-button" type="button" onclick={chooseFile} disabled={busy || currentJob?.state === "running"}>
            {busy ? "读取中…" : "选择文件…"}
          </button>
        </div>
      {/if}

      <div class="action-row">
        <button
          class="primary-button"
          type="button"
          onclick={startTranscription}
          disabled={!selected || !model || busy || currentJob?.state === "running"}
        >
          {currentJob?.state === "running" ? "转写中…" : "开始转写"}
        </button>
        {#if !selected}
          <p class="action-hint">先选择一份文件</p>
        {:else if !model}
          <p class="action-hint">先选择一个模型</p>
        {/if}
      </div>
    </section>

    {#if currentJob?.state === "running"}
      <section class="job-card" aria-labelledby="job-heading" aria-live="polite">
        <div class="job-heading-row">
          <div class="job-heading-copy">
            <span class="job-pulse" aria-hidden="true"></span>
            <div>
              <p class="section-kicker">IN PROGRESS</p>
              <h2 id="job-heading">{currentJob.phase}</h2>
            </div>
          </div>
          {#if currentJob.totalChunks && currentJob.totalChunks > 0}
            <span class="chunk-count">{currentJob.completedChunks}/{currentJob.totalChunks} 段</span>
          {/if}
        </div>
        {#if currentJob.totalChunks && currentJob.totalChunks > 0}
          <progress
            class="transcription-progress"
            aria-label="转写进度"
            max={currentJob.totalChunks}
            value={currentJob.completedChunks}
          ></progress>
        {:else}
          <div class="indeterminate-track" aria-hidden="true"><span></span></div>
        {/if}
      </section>
    {/if}

    <section id="results-panel" class="results-card" aria-labelledby="results-heading">
      <div class="section-heading results-heading-row">
        <div class="section-title-group">
          <span class="section-icon result-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
              <path d="M6.5 4h11A1.5 1.5 0 0 1 19 5.5v13a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 18.5v-13A1.5 1.5 0 0 1 6.5 4Z" />
              <path d="M8.5 9h7M8.5 12.5h7M8.5 16h4" />
            </svg>
          </span>
          <div>
            <p class="section-kicker">RESULT</p>
            <h2 id="results-heading">转写结果</h2>
          </div>
        </div>
        {#if transcript}
          <span class="result-ready"><span class="status-dot" aria-hidden="true"></span>已完成</span>
        {/if}
      </div>

      {#if transcript}
        <div class="result-content">
          <div class="result-meta">
            <span class="result-source">{transcript.sourceFileName}</span>
            <span>{transcript.engine}</span>
          </div>
          <pre class="transcript-text">{transcript.text || "没有识别到语音内容。"}</pre>
          <div class="result-actions">
            <button class="secondary-button" type="button" onclick={copyTranscript}>复制文本</button>
            <button class="secondary-button" type="button" onclick={() => exportTranscript("txt")}>导出 TXT…</button>
            <button class="secondary-button" type="button" onclick={() => exportTranscript("json")}>导出 JSON…</button>
          </div>
        </div>
      {:else if currentJob?.state === "running"}
        <div class="result-placeholder compact-placeholder">
          <p>完成后，文字结果会显示在这里。</p>
        </div>
      {:else}
        <div class="result-placeholder">
          <span class="placeholder-icon" aria-hidden="true">
            <svg viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M9 4.75h9l5.25 5.25V25A2.25 2.25 0 0 1 21 27.25H9A2.25 2.25 0 0 1 6.75 25V7A2.25 2.25 0 0 1 9 4.75Z" />
              <path d="M18 5v5h5M11 15h10M11 19h10M11 23h6" />
            </svg>
          </span>
          <div>
            <h3>转写内容会显示在这里</h3>
            <p>选择文件和模型后，开始转写。</p>
          </div>
        </div>
      {/if}
    </section>
  </main>

  <aside class="inspector" aria-label="模型和历史">
    <section class="inspector-section" aria-labelledby="model-heading">
      <div class="inspector-heading">
        <div>
          <p class="section-kicker" id="model-heading">MODEL</p>
        </div>
        <span class="model-state" class:ready={model !== null} aria-label={model ? "模型已加载" : "尚未选择模型"}>
          <span class="status-dot" aria-hidden="true"></span>
        </span>
      </div>

      {#if model}
        <div class="model-card">
          <div class="model-art" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
              <path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 10v4" />
            </svg>
          </div>
          <div class="model-copy">
            <p class="model-name">{model.displayName}</p>
            <p class="model-family">{model.family}</p>
          </div>
          <span class="model-menu-mark" aria-hidden="true">•••</span>
        </div>
        <p class="model-ready"><span class="status-dot" aria-hidden="true"></span>已就绪</p>
      {:else}
        <div class="model-empty">
          <span class="model-empty-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
              <path d="m12 3 8 4.5v9L12 21l-8-4.5v-9L12 3Z" />
              <path d="m4.5 7.8 7.5 4.4 7.5-4.4M12 12.2V21" />
            </svg>
          </span>
          <p>尚未选择模型</p>
        </div>
      {/if}

      <button
        class="secondary-button model-button"
        type="button"
        onclick={chooseModel}
        disabled={modelBusy || busy || currentJob?.state === "running"}
      >
        {modelBusy ? "检查中…" : model ? "更换模型…" : "选择模型…"}
      </button>
    </section>

    <section id="history-panel" class="inspector-section history-section" aria-labelledby="history-heading">
      <div class="inspector-heading history-heading-row">
        <div>
          <p class="section-kicker">RECENT</p>
          <h2 id="history-heading">最近结果</h2>
        </div>
        {#if history.length > 0}<span class="history-count">{history.length}</span>{/if}
      </div>
      {#if history.length > 0}
        <div class="history-list">
          {#each history.slice(0, 5) as item (item.jobId)}
            <button class="history-item" type="button" onclick={() => openHistoryItem(item.jobId)}>
              <span class="history-source">{item.sourceFileName}</span>
              <span class="history-preview">{item.preview || "（空结果）"}</span>
            </button>
          {/each}
        </div>
      {:else}
        <div class="history-empty">完成的转写结果会列在这里。</div>
      {/if}
    </section>
  </aside>
</div>

<style>
  :global(:root) {
    color-scheme: light;
    --canvas: #eef3f9;
    --sidebar: #f4f7fb;
    --surface: #ffffff;
    --surface-soft: #f7f9fc;
    --ink: #182338;
    --body: #526176;
    --line: #d8e1ec;
    --accent: #1265d6;
    --accent-hover: #0a56b4;
    --accent-soft: #e5efff;
    --accent-ink: #0a56b4;
    --accent-contrast: #ffffff;
    --success: #16804b;
    --danger: #8f1d1d;
    --danger-soft: #fff0ef;
    --shadow: 0 8px 24px rgb(31 55 85 / 5%);
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      color-scheme: dark;
      --canvas: #111824;
      --sidebar: #141e2c;
      --surface: #1a2534;
      --surface-soft: #202c3b;
      --ink: #e8edf5;
      --body: #b7c3d2;
      --line: #344354;
      --accent: #64a6ff;
      --accent-hover: #8abbff;
      --accent-soft: #223b5b;
      --accent-ink: #b8d7ff;
      --accent-contrast: #102033;
      --success: #73c89d;
      --danger: #ffb8ab;
      --danger-soft: #40292b;
      --shadow: 0 8px 24px rgb(0 0 0 / 18%);
    }
  }

  :global(*) {
    box-sizing: border-box;
  }

  :global(html) {
    min-width: 320px;
    scroll-behavior: smooth;
  }

  :global(body) {
    min-width: 320px;
    min-height: 100vh;
    margin: 0;
    background: var(--canvas);
    color: var(--ink);
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
    font-size: 14px;
    line-height: 1.5;
  }

  :global(button),
  :global(a) {
    font: inherit;
  }

  :global(button:focus-visible),
  :global(a:focus-visible) {
    outline: 3px solid var(--accent);
    outline-offset: 3px;
  }

  :global(h1),
  :global(h2),
  :global(h3),
  :global(p) {
    margin: 0;
  }

  .app-shell {
    display: grid;
    width: 100%;
    min-height: 100vh;
    min-height: 100dvh;
    grid-template-columns: 174px minmax(0, 1fr) 250px;
    background: var(--canvas);
  }

  .sidebar {
    display: flex;
    min-width: 0;
    min-height: 100vh;
    flex-direction: column;
    padding: 18px 12px 14px;
    border-right: 1px solid var(--line);
    background: var(--sidebar);
  }

  .brand {
    display: flex;
    min-height: 46px;
    align-items: center;
    gap: 10px;
    padding: 0 6px;
  }

  .brand-mark {
    display: grid;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 10px;
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .brand-mark svg {
    width: 21px;
    height: 21px;
  }

  .brand-name {
    color: var(--ink);
    font-size: 13px;
    font-weight: 650;
    line-height: 1.2;
  }

  .primary-nav {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-top: 25px;
  }

  .nav-item {
    display: flex;
    min-width: 0;
    min-height: 42px;
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: transparent;
    color: var(--body);
    cursor: pointer;
    text-align: left;
    text-decoration: none;
    transition: background-color 120ms ease, color 120ms ease;
  }

  .nav-item svg {
    width: 19px;
    height: 19px;
    flex: 0 0 auto;
  }

  .nav-item span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .nav-item:hover:not(:disabled) {
    background: var(--surface-soft);
    color: var(--ink);
  }

  .nav-item.active {
    background: var(--accent-soft);
    color: var(--accent-ink);
    font-weight: 620;
  }

  .nav-item:disabled {
    cursor: default;
    opacity: 0.58;
  }

  .sidebar-spacer {
    flex: 1;
  }

  .prototype-label {
    padding: 12px 9px 2px;
    border-top: 1px solid var(--line);
    color: var(--body);
    font-size: 11px;
  }

  .main-pane {
    display: flex;
    min-width: 0;
    min-height: 100vh;
    flex-direction: column;
    gap: 15px;
    overflow: auto;
    padding: 22px 22px 28px;
  }

  .workspace-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
    padding: 2px 1px 4px;
  }

  .eyebrow,
  .section-kicker {
    color: var(--body);
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.09em;
    line-height: 1.3;
    text-transform: uppercase;
  }

  .workspace-header h1 {
    margin-top: 2px;
    font-size: 23px;
    font-weight: 670;
    letter-spacing: -0.025em;
    line-height: 1.25;
  }

  .page-subtitle {
    margin-top: 5px;
    color: var(--body);
    font-size: 13px;
  }

  .workflow-card,
  .job-card,
  .results-card {
    min-width: 0;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  .workflow-card {
    padding: 17px;
  }

  .section-heading,
  .section-title-group,
  .job-heading-row,
  .job-heading-copy,
  .inspector-heading {
    display: flex;
    min-width: 0;
    align-items: center;
  }

  .section-heading {
    justify-content: space-between;
    gap: 10px;
  }

  .section-title-group {
    gap: 10px;
  }

  .section-icon {
    display: grid;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid var(--line);
    border-radius: 9px;
    background: var(--surface-soft);
    color: var(--accent-ink);
  }

  .section-icon svg {
    width: 19px;
    height: 19px;
  }

  .section-heading h2,
  .inspector-heading h2 {
    margin-top: 2px;
    font-size: 15px;
    font-weight: 650;
    line-height: 1.25;
  }

  .section-kicker {
    font-size: 9px;
  }

  .file-status,
  .result-ready {
    display: inline-flex;
    min-height: 26px;
    align-items: center;
    gap: 7px;
    padding: 3px 9px;
    border: 1px solid var(--line);
    border-radius: 999px;
    color: var(--body);
    font-size: 12px;
    white-space: nowrap;
  }

  .file-status.ready,
  .result-ready {
    border-color: color-mix(in srgb, var(--success) 35%, var(--line));
    background: color-mix(in srgb, var(--success) 8%, var(--surface));
    color: var(--body);
  }

  .status-dot {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: var(--body);
  }

  .file-status.ready .status-dot,
  .result-ready .status-dot,
  .model-ready .status-dot,
  .model-state.ready .status-dot {
    background: var(--success);
  }

  .file-picker-card {
    display: flex;
    min-height: 158px;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    margin-top: 15px;
    padding: 16px;
    border: 1px dashed color-mix(in srgb, var(--accent) 32%, var(--line));
    border-radius: 10px;
    background: var(--surface-soft);
    text-align: center;
  }

  .picker-symbol {
    display: grid;
    width: 46px;
    height: 46px;
    place-items: center;
    margin-bottom: 6px;
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--accent-ink);
  }

  .picker-symbol svg {
    width: 31px;
    height: 31px;
  }

  .picker-title {
    font-size: 14px;
    font-weight: 620;
  }

  .picker-description {
    margin-top: 3px;
    margin-bottom: 10px;
    color: var(--body);
    font-size: 12px;
  }

  .selected-file {
    display: flex;
    min-width: 0;
    min-height: 82px;
    align-items: center;
    gap: 12px;
    margin-top: 15px;
    padding: 13px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface-soft);
  }

  .file-type-icon {
    display: grid;
    width: 42px;
    height: 42px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent-ink);
  }

  .file-type-icon svg {
    width: 23px;
    height: 23px;
  }

  .selected-file-copy {
    min-width: 0;
    flex: 1;
  }

  .selected-file-name {
    overflow-wrap: anywhere;
    color: var(--ink);
    font-size: 13px;
    font-weight: 620;
    line-height: 1.4;
  }

  .selected-file-meta {
    margin-top: 3px;
    color: var(--body);
    font-size: 12px;
  }

  .primary-button,
  .secondary-button,
  .text-button {
    display: inline-flex;
    min-height: 40px;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 0 14px;
    border: 1px solid transparent;
    border-radius: 8px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 620;
    line-height: 1.2;
    text-decoration: none;
    transition: background-color 120ms ease, border-color 120ms ease, color 120ms ease, transform 120ms ease;
  }

  .primary-button {
    min-height: 46px;
    width: 100%;
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .primary-button:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .secondary-button {
    border-color: var(--line);
    background: var(--surface);
    color: var(--ink);
  }

  .secondary-button:hover:not(:disabled),
  .text-button:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent-ink);
  }

  .text-button {
    min-height: 36px;
    flex: 0 0 auto;
    padding: 0 9px;
    background: transparent;
    color: var(--accent-ink);
  }

  .primary-button:active:not(:disabled),
  .secondary-button:active:not(:disabled),
  .text-button:active:not(:disabled),
  .history-item:active {
    transform: translateY(1px);
  }

  .primary-button:disabled,
  .secondary-button:disabled,
  .text-button:disabled,
  .nav-item:disabled {
    cursor: default;
    opacity: 0.55;
  }

  .primary-button:disabled {
    background: var(--accent-soft);
    color: var(--accent-ink);
    opacity: 1;
  }

  .action-row {
    display: flex;
    min-width: 0;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    margin-top: 13px;
  }

  .action-hint {
    min-height: 18px;
    color: var(--body);
    font-size: 12px;
    text-align: center;
  }

  .notice {
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--danger) 26%, var(--line));
    border-radius: 9px;
    background: var(--danger-soft);
    color: var(--danger);
    font-size: 13px;
    overflow-wrap: anywhere;
  }

  .job-card {
    padding: 15px 17px;
  }

  .job-heading-row {
    justify-content: space-between;
    gap: 10px;
  }

  .job-heading-copy {
    min-width: 0;
    gap: 10px;
  }

  .job-heading-copy h2 {
    margin-top: 2px;
    font-size: 14px;
    font-weight: 620;
    line-height: 1.4;
  }

  .job-pulse {
    width: 9px;
    height: 9px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-soft);
  }

  .chunk-count {
    color: var(--body);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .transcription-progress {
    display: block;
    width: 100%;
    height: 8px;
    margin-top: 13px;
    overflow: hidden;
    appearance: none;
    border: 0;
    border-radius: 999px;
    background: var(--surface-soft);
  }

  .transcription-progress::-webkit-progress-bar {
    border-radius: 999px;
    background: var(--surface-soft);
  }

  .transcription-progress::-webkit-progress-value {
    border-radius: 999px;
    background: var(--accent);
  }

  .transcription-progress::-moz-progress-bar {
    border-radius: 999px;
    background: var(--accent);
  }

  .indeterminate-track {
    height: 7px;
    overflow: hidden;
    margin-top: 13px;
    border-radius: 999px;
    background: var(--surface-soft);
  }

  .indeterminate-track span {
    display: block;
    width: 34%;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    animation: progress-travel 1.4s ease-in-out infinite alternate;
  }

  @keyframes progress-travel {
    from { transform: translateX(-12%); }
    to { transform: translateX(205%); }
  }

  .results-card {
    padding: 16px 17px;
  }

  .results-heading-row {
    padding-bottom: 12px;
    border-bottom: 1px solid var(--line);
  }

  .result-icon {
    color: var(--accent-ink);
  }

  .result-content {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 11px;
    padding-top: 12px;
  }

  .result-meta {
    display: flex;
    min-width: 0;
    justify-content: space-between;
    gap: 10px;
    color: var(--body);
    font-size: 11px;
  }

  .result-meta span {
    overflow-wrap: anywhere;
  }

  .result-source {
    min-width: 0;
    font-weight: 600;
  }

  .transcript-text {
    min-height: 95px;
    max-height: 340px;
    overflow: auto;
    margin: 0;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: 9px;
    background: var(--surface-soft);
    color: var(--ink);
    font: inherit;
    font-size: 13px;
    line-height: 1.7;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .result-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
  }

  .result-actions .secondary-button {
    min-height: 36px;
    padding: 0 10px;
    font-size: 12px;
  }

  .result-placeholder {
    display: flex;
    min-height: 132px;
    align-items: center;
    gap: 12px;
    padding: 18px 5px;
    color: var(--body);
  }

  .compact-placeholder {
    min-height: 72px;
    font-size: 13px;
  }

  .placeholder-icon {
    display: grid;
    width: 42px;
    height: 42px;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface-soft);
  }

  .placeholder-icon svg {
    width: 25px;
    height: 25px;
  }

  .result-placeholder h3 {
    color: var(--ink);
    font-size: 13px;
    font-weight: 620;
  }

  .result-placeholder p {
    margin-top: 3px;
    font-size: 12px;
  }

  .inspector {
    min-width: 0;
    min-height: 100vh;
    padding: 21px 14px;
    border-left: 1px solid var(--line);
    background: var(--surface);
  }

  .inspector-section {
    min-width: 0;
  }

  .inspector-heading {
    justify-content: space-between;
    gap: 8px;
  }

  .model-state {
    display: grid;
    width: 24px;
    height: 24px;
    place-items: center;
    border-radius: 50%;
    background: var(--surface-soft);
  }

  .model-state .status-dot {
    width: 7px;
    height: 7px;
  }

  .model-card {
    display: flex;
    min-width: 0;
    min-height: 69px;
    align-items: center;
    gap: 9px;
    margin-top: 14px;
    padding: 9px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface-soft);
  }

  .model-art {
    display: grid;
    width: 36px;
    height: 36px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 9px;
    background: var(--accent-soft);
    color: var(--accent-ink);
  }

  .model-art svg {
    width: 21px;
    height: 21px;
  }

  .model-copy {
    min-width: 0;
    flex: 1;
  }

  .model-name {
    overflow-wrap: anywhere;
    color: var(--ink);
    font-size: 12px;
    font-weight: 620;
    line-height: 1.4;
  }

  .model-family {
    margin-top: 2px;
    color: var(--body);
    font-size: 11px;
  }

  .model-menu-mark {
    flex: 0 0 auto;
    color: var(--body);
    font-size: 14px;
    letter-spacing: 1px;
  }

  .model-ready {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 12px;
    color: var(--body);
    font-size: 12px;
  }

  .model-empty {
    display: flex;
    min-height: 88px;
    align-items: center;
    gap: 10px;
    margin-top: 13px;
    padding: 12px;
    border: 1px dashed var(--line);
    border-radius: 10px;
    color: var(--body);
    font-size: 12px;
  }

  .model-empty-icon {
    display: grid;
    width: 36px;
    height: 36px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 9px;
    background: var(--surface-soft);
    color: var(--body);
  }

  .model-empty-icon svg {
    width: 20px;
    height: 20px;
  }

  .model-button {
    width: 100%;
    margin-top: 12px;
  }

  .history-section {
    margin-top: 22px;
    padding-top: 18px;
    border-top: 1px solid var(--line);
  }

  .history-count {
    display: grid;
    min-width: 22px;
    height: 22px;
    place-items: center;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--body);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 12px;
  }

  .history-item {
    display: flex;
    width: 100%;
    min-width: 0;
    flex-direction: column;
    gap: 3px;
    padding: 9px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--ink);
    cursor: pointer;
    text-align: left;
    transition: background-color 120ms ease, border-color 120ms ease;
  }

  .history-item:hover,
  .history-item:focus-visible {
    border-color: var(--line);
    background: var(--surface-soft);
  }

  .history-source {
    overflow: hidden;
    width: 100%;
    font-size: 12px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .history-preview {
    display: -webkit-box;
    overflow: hidden;
    color: var(--body);
    font-size: 11px;
    line-height: 1.45;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .history-empty {
    margin-top: 12px;
    padding: 11px 10px;
    border-radius: 8px;
    background: var(--surface-soft);
    color: var(--body);
    font-size: 12px;
    line-height: 1.5;
  }

  @media (prefers-contrast: more) {
    :global(:root) {
      --line: #748198;
    }

    .workflow-card,
    .job-card,
    .results-card,
    .model-card {
      border-width: 2px;
    }
  }

  @media (max-width: 860px) {
    .app-shell {
      grid-template-columns: 68px minmax(0, 1fr);
      min-height: 100vh;
      min-height: 100dvh;
    }

    .sidebar {
      grid-row: 1 / span 2;
      align-items: center;
      padding-right: 8px;
      padding-left: 8px;
    }

    .brand {
      padding: 0;
    }

    .brand-name,
    .nav-item span,
    .prototype-label {
      display: none;
    }

    .primary-nav {
      width: 100%;
      align-items: center;
    }

    .nav-item {
      width: 46px;
      justify-content: center;
      padding: 0;
    }

    .main-pane {
      grid-column: 2;
      min-height: 100vh;
      padding-right: 18px;
      padding-left: 18px;
    }

    .inspector {
      display: grid;
      grid-column: 2;
      grid-row: 2;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      align-items: start;
      gap: 18px;
      min-height: 0;
      padding: 18px;
      border-top: 1px solid var(--line);
      border-left: 0;
    }

    .history-section {
      margin-top: 0;
      padding-top: 0;
      border-top: 0;
    }
  }

  @media (max-width: 740px) {
    .app-shell {
      grid-template-columns: 58px minmax(0, 1fr);
    }

    .main-pane {
      gap: 12px;
      padding: 16px 13px 22px;
    }

    .workflow-card,
    .job-card,
    .results-card {
      padding: 13px;
    }

    .inspector {
      gap: 13px;
      padding: 14px 13px;
    }

    .workspace-header h1 {
      font-size: 21px;
    }
  }

  @media (max-width: 560px) {
    .app-shell {
      grid-template-columns: minmax(0, 1fr);
    }

    .sidebar {
      position: static;
      grid-row: auto;
      flex-direction: row;
      justify-content: space-between;
      min-height: 0;
      padding: 9px 12px;
      border-right: 0;
      border-bottom: 1px solid var(--line);
    }

    .brand-name,
    .nav-item span {
      display: inline;
    }

    .brand {
      gap: 8px;
    }

    .brand-mark {
      width: 30px;
      height: 30px;
    }

    .brand-name {
      font-size: 12px;
    }

    .primary-nav {
      width: auto;
      flex-direction: row;
      gap: 2px;
      margin-top: 0;
    }

    .nav-item {
      width: auto;
      min-height: 36px;
      gap: 6px;
      padding: 0 8px;
      font-size: 12px;
    }

    .nav-item svg {
      width: 16px;
      height: 16px;
    }

    .main-pane {
      grid-column: 1;
      min-height: 0;
      padding: 15px 13px 20px;
    }

    .inspector {
      grid-column: 1;
      grid-row: auto;
      grid-template-columns: minmax(0, 1fr);
    }

    .prototype-label {
      display: none;
    }

    .page-subtitle {
      font-size: 12px;
    }

    .file-picker-card {
      min-height: 142px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    :global(*),
    :global(*::before),
    :global(*::after) {
      scroll-behavior: auto !important;
      animation-duration: 0.01ms !important;
      animation-iteration-count: 1 !important;
      transition-duration: 0.01ms !important;
    }
  }
  /* Provisional redesign: a focused transcript desk, not a three-pane dashboard.
     Rank: primary = transcript; secondary = source selection + start; tertiary = model/history.
     Tokens: fog #F3F5F7, ink #17212B, slate #4F5E6B, signal red #B43C2D; dark equivalents below. */
  :global(:root) {
    color-scheme: light dark;
    --canvas: #f3f5f7;
    --sidebar: #f8f9fa;
    --surface: #ffffff;
    --surface-soft: #f7f9fa;
    --ink: #17212b;
    --body: #4f5e6b;
    --line: #d4dde4;
    --accent: #b43c2d;
    --accent-hover: #9d3125;
    --accent-soft: #f7e9e6;
    --accent-ink: #8c2f25;
    --accent-contrast: #ffffff;
    --success: #256b49;
    --danger: #8e2d25;
    --danger-soft: #f8eae8;
    --shadow: 0 10px 28px rgb(23 33 43 / 4%);
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --canvas: #151b20;
      --sidebar: #191f24;
      --surface: #1d252b;
      --surface-soft: #242d34;
      --ink: #edf2f5;
      --body: #bac5cd;
      --line: #414d56;
      --accent: #f08b78;
      --accent-hover: #ffa391;
      --accent-soft: #422a26;
      --accent-ink: #ffc0b5;
      --accent-contrast: #25110e;
      --success: #83d2a4;
      --danger: #ffb9ad;
      --danger-soft: #3a2523;
      --shadow: 0 10px 28px rgb(0 0 0 / 18%);
    }
  }

  .app-shell {
    grid-template-columns: minmax(0, 1fr);
    min-height: 100vh;
    min-height: 100dvh;
  }

  /* Clear column placement left over from the retired three-pane layout. */
  .sidebar, .main-pane, .inspector { grid-column: auto; grid-row: auto; }
  .brand-name, .nav-item span, .prototype-label { display: inline; }

  .sidebar {
    position: relative;
    min-height: 62px;
    height: 62px;
    flex-direction: row;
    align-items: center;
    gap: 20px;
    padding: 0 28px;
    border-right: 0;
    border-bottom: 1px solid var(--line);
    background: var(--sidebar);
  }

  .brand { min-height: 0; padding: 0; }
  .brand-mark { width: 30px; height: 30px; border-radius: 8px; }
  .brand-mark svg { width: 18px; height: 18px; }
  .brand-name { font-size: 13px; }
  .sidebar-spacer { display: none; }

  .primary-nav {
    width: auto;
    flex-direction: row;
    gap: 4px;
    margin: 0 0 0 auto;
  }

  .nav-item {
    width: auto;
    min-height: 36px;
    gap: 7px;
    padding: 0 11px;
    border-radius: 7px;
    font-size: 12px;
  }

  .nav-item svg { width: 16px; height: 16px; }
  .prototype-label {
    padding: 4px 0 4px 14px;
    border-top: 0;
    border-left: 1px solid var(--line);
    font-size: 11px;
    white-space: nowrap;
  }

  .main-pane {
    width: min(100%, 1024px);
    min-height: 0;
    justify-self: center;
    gap: 18px;
    overflow: visible;
    padding: 34px 28px 18px;
  }

  .workspace-header { padding: 0 0 15px; border-bottom: 1px solid var(--line); }
  .workspace-header h1 { margin-top: 4px; font-size: 26px; letter-spacing: -0.03em; }
  .page-subtitle { margin-top: 6px; font-size: 13px; }

  .workflow-card {
    padding: 4px 0 0;
    border: 0;
    border-radius: 0;
    background: transparent;
    box-shadow: none;
  }

  .section-icon { display: none; }
  .section-title-group { gap: 0; }
  .section-heading h2 { font-size: 14px; }
  .file-status, .result-ready { min-height: 28px; }

  .file-picker-card {
    display: grid;
    min-height: 92px;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas: "title action" "description action";
    align-items: center;
    justify-items: start;
    column-gap: 20px;
    margin-top: 12px;
    padding: 17px 18px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface);
    text-align: left;
  }

  .picker-symbol { display: none; }
  .picker-title { grid-area: title; font-size: 14px; font-weight: 650; }
  .picker-description { grid-area: description; margin: 3px 0 0; font-size: 12px; }
  .file-picker-card .secondary-button { grid-area: action; min-height: 40px; }

  .selected-file {
    min-height: 66px;
    margin-top: 12px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 9px;
    background: var(--surface);
  }

  .file-type-icon { width: 36px; height: 36px; border-radius: 8px; }
  .action-row {
    flex-direction: row;
    align-items: center;
    justify-content: flex-start;
    gap: 12px;
    margin-top: 12px;
  }
  .primary-button { width: auto; min-width: 132px; min-height: 42px; border-radius: 8px; }
  .action-hint { text-align: left; }

  .job-card {
    padding: 14px 16px;
    border-radius: 10px;
    box-shadow: none;
  }

  .results-card {
    min-height: 230px;
    padding: 17px 20px 20px;
    border-radius: 12px;
    box-shadow: var(--shadow);
  }

  .results-heading-row { padding-bottom: 13px; }
  .result-content { gap: 12px; padding-top: 14px; }
  .result-meta { font-size: 12px; }
  .transcript-text {
    min-height: 180px;
    max-height: 440px;
    padding: 15px 16px;
    border-color: transparent;
    border-radius: 8px;
    background: var(--surface-soft);
    font-size: 14px;
    line-height: 1.75;
  }
  .result-actions { gap: 8px; }
  .result-actions .secondary-button { min-height: 36px; }
  .result-placeholder { min-height: 174px; padding: 25px 2px; }
  .placeholder-icon { display: none; }
  .result-placeholder h3 { font-size: 15px; }
  .result-placeholder p { font-size: 13px; }

  .inspector {
    display: grid;
    width: min(100%, 1024px);
    min-height: 0;
    grid-template-columns: minmax(220px, 0.8fr) minmax(0, 1.2fr);
    align-items: start;
    gap: 14px;
    justify-self: center;
    padding: 0 28px 30px;
    border: 0;
    background: transparent;
  }

  .inspector-section {
    padding: 15px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface);
  }

  .history-section { margin: 0; padding-top: 15px; border-top: 1px solid var(--line); }
  .history-item { min-height: 48px; }

  @media (max-width: 720px) {
    .sidebar { gap: 10px; padding: 0 16px; }
    .main-pane { padding: 26px 18px 16px; }
    .inspector { grid-template-columns: minmax(0, 1fr); padding: 0 18px 24px; }
  }

  @media (max-width: 560px) {
    .sidebar { min-height: 58px; height: auto; flex-wrap: wrap; gap: 6px; padding: 8px 12px; }
    .primary-nav { margin-left: auto; }
    .nav-item { min-height: 34px; gap: 5px; padding: 0 7px; font-size: 11px; }
    .prototype-label { display: none; }
    .main-pane { grid-column: 1; padding: 20px 13px 14px; }
    .inspector { grid-column: 1; padding: 0 13px 20px; }
    .workspace-header h1 { font-size: 23px; }
    .file-picker-card { min-height: 100px; grid-template-columns: minmax(0, 1fr); grid-template-areas: "title" "description" "action"; row-gap: 3px; }
    .file-picker-card .secondary-button { margin-top: 7px; }
    .action-row { align-items: flex-start; flex-direction: column; }
    .primary-button { width: 100%; }
    .results-card { padding: 14px; }
    .result-meta { flex-direction: column; gap: 3px; }
    .result-actions .secondary-button { flex: 1 1 auto; }
  }
</style>
