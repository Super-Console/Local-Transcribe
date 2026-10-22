<script lang="ts">
  import Icon from "./Icon.svelte";
  import Reader from "./Reader.svelte";
  import type { JobStatus, ModelSummary, Segment, Transcript } from "$lib/types";

  let {
    model,
    fileName,
    job,
    segments,
    transcript,
    error,
    busy,
    dropping,
    onPickFile,
    onChooseModel,
    onCancel,
    onExport,
    onCopy,
  }: {
    model: ModelSummary | null;
    fileName: string | null;
    job: JobStatus | null;
    segments: Segment[];
    transcript: Transcript | null;
    error: string;
    busy: boolean;
    dropping: boolean;
    onPickFile: () => void;
    onChooseModel: () => void;
    onCancel: () => void;
    onExport: (format: "txt" | "json") => void;
    onCopy: () => void;
  } = $props();

  const running = $derived(job?.state === "running");
  const finished = $derived(job?.state === "completed" || !!transcript);
  const stopped = $derived(job?.state === "failed" || job?.state === "cancelled");
  const hasTask = $derived(!!job || !!transcript);
  const percent = $derived(
    job?.totalChunks && job.totalChunks > 0
      ? Math.round((job.completedChunks / job.totalChunks) * 100)
      : null,
  );
  const displayName = $derived(fileName ?? transcript?.sourceFileName ?? "未命名媒体");
  const stateLabel = $derived(
    running
      ? "正在转写"
      : job?.state === "completed" || transcript
        ? "已完成"
        : job?.state === "cancelled"
          ? "已取消"
          : job?.state === "failed"
            ? "失败"
            : "",
  );
  const partial = $derived(stopped && segments.length > 0);
</script>

<section class="content">
  {#if error}
    <div class="banner" role="alert">
      <Icon name="warn" size={14} />
      <span>{error}</span>
    </div>
  {/if}

  {#if !hasTask}
    <div class="intro">
      <h2>开始一段转写</h2>
      <p>拖入音频或视频文件，或使用下方按钮选择；导入后会自动排队。</p>
    </div>
    <div class="center">
      <div
        class="drop"
        class:dropping
        role="button"
        tabindex="0"
        aria-label="拖入音频或视频文件，或按回车选择文件"
        onclick={onPickFile}
        onkeydown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onPickFile();
          }
        }}
      >
        <Icon name="file" size={30} stroke={1.35} />
        <div class="drop-title">拖入音频或视频文件</div>
        <div class="drop-sub">支持常见的音频与视频格式，可一次放入多个文件</div>
        <div class="actions">
          <button
            type="button"
            class="btn"
            onclick={(event) => {
              event.stopPropagation();
              onPickFile();
            }}
          >
            选择文件
          </button>
          <span class="kbd">⌘O</span>
        </div>
      </div>
    </div>
  {:else}
    <div class="task">
      <div class="task-row">
        <div class="file" title={displayName}>{displayName}</div>
        <div class="spacer"></div>
        {#if running}
          <button type="button" class="quiet" onclick={onCancel}>取消</button>
        {:else}
          <div class="actions">
            <button type="button" class="act" onclick={onCopy} disabled={segments.length === 0}>
              <Icon name="copy" size={13} />复制全文
            </button>
            <button type="button" class="act" onclick={() => onExport("txt")} disabled={segments.length === 0 || !transcript}>
              <Icon name="download" size={13} />TXT
            </button>
            <button type="button" class="act" onclick={() => onExport("json")} disabled={segments.length === 0 || !transcript}>
              <Icon name="download" size={13} />JSON
            </button>
          </div>
        {/if}
      </div>
      <div class="meta">
        <span>模型 <b>{model?.displayName ?? "未选择"}</b></span>
        <span class="sep">·</span>
        <span>{stateLabel}</span>
        {#if job?.totalChunks}
          <span class="sep">·</span>
          <span>已转写 <b>{job.completedChunks} / {job.totalChunks}</b> 段</span>
        {/if}
        {#if running && job?.phase}
          <span class="sep">·</span>
          <span>{job.phase}</span>
        {/if}
      </div>
      {#if running}
        <div
          class="bar"
          role="progressbar"
          aria-valuemin="0"
          aria-valuemax="100"
          aria-valuenow={percent ?? undefined}
          aria-valuetext={percent === null ? "正在识别" : `已处理 ${percent}%`}
        >
          <i class:indeterminate={percent === null} style:width={percent === null ? undefined : `${percent}%`}></i>
        </div>
      {/if}
      {#if partial}
        <div class="note">
          <span class="dot red"></span>
          <span>该结果为部分内容，不完整；导出时会再次提示。</span>
        </div>
      {/if}
    </div>

    <Reader jobId={job?.jobId ?? transcript?.jobId ?? null} {segments} {running} />

    {#if finished && busy}
      <p class="hint">正在准备导出…</p>
    {/if}
  {/if}
</section>

<style>
  .content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 2px 28px 0;
  }

  @media (max-width: 760px) {
    .content {
      padding: 2px 18px 0;
    }
  }

  .banner {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 2px 0 6px;
    padding: 8px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--red) 7%, transparent);
    color: var(--text-2);
    font-size: 12px;
  }

  .intro {
    width: 100%;
    max-width: 720px;
    margin: 10px auto 0;
  }

  .intro h2 {
    margin: 0;
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.022em;
  }

  .intro p {
    margin: 8px 0 0;
    font-size: 13px;
    color: var(--text-2);
  }

  .center {
    flex: 1;
    display: grid;
    place-items: center;
    padding-bottom: 30px;
  }

  .drop {
    width: min(462px, 100%);
    padding: 30px 28px 26px;
    border-radius: 11px;
    border: 1px dashed var(--hairline-2);
    background: color-mix(in srgb, var(--text) 1.2%, transparent);
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    color: var(--text-3);
    transition: border-color 120ms ease, background-color 120ms ease;
  }

  .drop:hover {
    border-color: color-mix(in srgb, var(--text) 22%, transparent);
  }

  .drop.dropping {
    border-color: color-mix(in srgb, var(--accent) 65%, transparent);
    background: color-mix(in srgb, var(--accent) 6%, transparent);
  }

  .drop-title {
    margin-top: 12px;
    font-size: 14.5px;
    font-weight: 500;
    letter-spacing: -0.011em;
    color: var(--text);
  }

  .drop-sub {
    margin-top: 5px;
    font-size: 12.5px;
    color: var(--text-2);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 16px;
  }

  .kbd {
    font-size: 11.5px;
    color: var(--text-3);
    letter-spacing: 0.02em;
  }

  .btn {
    height: 30px;
    padding: 0 14px;
    border-radius: 8px;
    background: var(--panel);
    border: 1px solid var(--hairline-2);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
    font-size: 13px;
    font-weight: 500;
  }

  .btn:hover {
    background: var(--fill);
  }

  .task {
    width: 100%;
    max-width: 720px;
    margin: 6px auto 0;
  }

  .task-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .file {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.012em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .spacer {
    flex: 1;
  }

  .quiet {
    font-size: 12.5px;
    color: var(--text-2);
    white-space: nowrap;
    background: none;
    border: 0;
    padding: 2px 0;
  }

  .quiet:hover {
    color: var(--red);
  }

  .act {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 8px;
    border-radius: 6px;
    border: 0;
    background: transparent;
    color: var(--text-2);
    font-size: 12.5px;
  }

  .act:hover:not(:disabled) {
    background: var(--fill);
    color: var(--text);
  }

  .act:disabled {
    opacity: 0.45;
  }

  .meta {
    margin-top: 6px;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-3);
    flex-wrap: wrap;
  }

  .meta b {
    font-weight: 500;
    color: var(--text-2);
  }

  .sep {
    color: var(--hairline-2);
  }

  .bar {
    margin-top: 14px;
    height: 3px;
    border-radius: 2px;
    background: var(--track);
    overflow: hidden;
  }

  .bar i {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transition: width 200ms ease;
  }

  .bar i.indeterminate {
    width: 38%;
    opacity: 0.55;
  }

  .note {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 10px;
    font-size: 11.5px;
    color: var(--text-3);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-3);
  }

  .dot.red {
    background: var(--red);
  }

  .hint {
    margin: 6px 0 0;
    font-size: 11.5px;
    color: var(--text-3);
  }
</style>
