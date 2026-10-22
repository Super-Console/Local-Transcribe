<script lang="ts">
  import Icon from "./Icon.svelte";
  import Reader from "./Reader.svelte";
  import { t } from "$lib/i18n.svelte";
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
    onBack,
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
    onBack: () => void;
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
  const displayName = $derived(fileName ?? transcript?.sourceFileName ?? t("transcribe.untitled"));
  const stateLabel = $derived(
    running
      ? t("state.running")
      : job?.state === "completed" || transcript
        ? t("state.completed")
        : job?.state === "cancelled"
          ? t("state.cancelled")
          : job?.state === "failed"
            ? t("state.failed")
            : "",
  );
  const partial = $derived(stopped && segments.length > 0);

  // 实时倍速与剩余时间：每秒刷新时钟，用后端给出的开始时间与音频总长推算。
  let nowMs = $state(Date.now());
  $effect(() => {
    if (!running) return;
    const timer = window.setInterval(() => (nowMs = Date.now()), 1000);
    return () => window.clearInterval(timer);
  });

  const audioRatio = $derived(
    job?.totalChunks && job.totalChunks > 0 ? job.completedChunks / job.totalChunks : null,
  );
  /** 已处理音频时长 / 已耗时 = 实时倍速（1.0× 表示与音频等速）。 */
  const speed = $derived.by(() => {
    const started = job?.startedAtMs;
    const duration = job?.totalDurationMs;
    const ratio = audioRatio;
    if (!running || !started || !duration || ratio === null || ratio <= 0) return null;
    const elapsed = nowMs - started;
    // 前 2 秒样本太少，不显示会跳动的数字。
    if (elapsed < 2000) return null;
    const value = (duration * ratio) / elapsed;
    return Number.isFinite(value) && value > 0 ? value : null;
  });
  const etaMs = $derived.by(() => {
    const duration = job?.totalDurationMs;
    const ratio = audioRatio;
    const current = speed;
    if (!running || !duration || ratio === null || !current) return null;
    return (duration * (1 - ratio)) / current;
  });

  function etaLabel(ms: number): string {
    const seconds = Math.max(1, Math.round(ms / 1000));
    if (seconds < 60) return t("transcribe.etaSeconds", { seconds });
    const minutes = Math.round(seconds / 60);
    if (minutes < 60) return t("transcribe.etaMinutes", { minutes });
    return t("transcribe.etaHours", { hours: Math.floor(minutes / 60), minutes: minutes % 60 });
  }
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
      <h2>{t("transcribe.introTitle")}</h2>
      <p>{t("transcribe.introBody")}</p>
    </div>
    <div class="center">
      <div
        class="drop"
        class:dropping
        role="button"
        tabindex="0"
        aria-label={t("transcribe.dropAria")}
        onclick={onPickFile}
        onkeydown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onPickFile();
          }
        }}
      >
        <Icon name="file" size={30} stroke={1.35} />
        <div class="drop-title">{t("transcribe.dropTitle")}</div>
        <div class="drop-sub">{t("transcribe.dropSub")}</div>
        <div class="actions">
          <button
            type="button"
            class="btn"
            onclick={(event) => {
              event.stopPropagation();
              onPickFile();
            }}
          >
            {t("transcribe.chooseFile")}
          </button>
          <span class="kbd">⌘O</span>
        </div>
      </div>
    </div>
  {:else}
    {#if !running}
      <div class="back-bar">
        <button type="button" class="back" onclick={onBack}>
          <Icon name="chevronLeft" size={12} stroke={1.7} />{t("nav.home")}
        </button>
      </div>
    {/if}
    <div class="task">
      <div class="task-row">
        <div class="file" title={displayName}>{displayName}</div>
        <div class="spacer"></div>
        {#if running}
          <button type="button" class="quiet" onclick={onCancel}>{t("common.cancel")}</button>
        {:else}
          <div class="actions">
            <button type="button" class="act" onclick={onCopy} disabled={segments.length === 0}>
              <Icon name="copy" size={13} />{t("transcribe.copyAll")}
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
        <span>{t("transcribe.model")} <b>{model?.displayName ?? t("common.notSelected")}</b></span>
        <span class="sep">·</span>
        <span>{stateLabel}</span>
        {#if job?.totalChunks}
          <span class="sep">·</span>
          <span>{t("transcribe.chunksBefore")} <b>{job.completedChunks} / {job.totalChunks}</b> {t("transcribe.chunksAfter", { count: job.totalChunks })}</span>
        {/if}
        {#if running && job?.phase}
          <span class="sep">·</span>
          <span>{job.phase}</span>
        {/if}
        {#if running && speed !== null}
          <span class="sep">·</span>
          <span>{t("transcribe.speed", { value: speed.toFixed(1) })}</span>
        {/if}
        {#if running && etaMs !== null}
          <span class="sep">·</span>
          <span>{etaLabel(etaMs)}</span>
        {/if}
      </div>
      {#if running}
        <div
          class="bar"
          role="progressbar"
          aria-valuemin="0"
          aria-valuemax="100"
          aria-valuenow={percent ?? undefined}
          aria-valuetext={percent === null ? t("transcribe.progressIdle") : t("transcribe.progressPercent", { percent })}
        >
          <i class:indeterminate={percent === null} style:width={percent === null ? undefined : `${percent}%`}></i>
        </div>
      {/if}
      {#if partial}
        <div class="note">
          <span class="dot red"></span>
          <span>{t("transcribe.partialNote")}</span>
        </div>
      {/if}
    </div>

    <Reader jobId={job?.jobId ?? transcript?.jobId ?? null} {segments} {running} />

    {#if finished && busy}
      <p class="hint">{t("transcribe.preparingExport")}</p>
    {/if}
  {/if}
</section>

<style>
  .back-bar {
    display: flex;
    width: 100%;
    margin: 2px 0 8px;
    animation: back-in var(--speed) var(--ease-out);
  }

  @keyframes back-in {
    from {
      opacity: 0;
      transform: translateX(-4px);
    }
  }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0 8px 0 3px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    font-size: 12.5px;
    color: var(--text-3);
  }

  .back:hover {
    background: var(--hover);
    color: var(--text);
  }

  .content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 6px 28px 0;
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
    max-width: 940px;
    margin: 10px auto 0;
  }

  /* 参考设计的大标题：衬线字体 */
  .intro h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 36px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .intro p {
    margin: 8px 0 0;
    font-size: 14.5px;
    color: var(--text-2);
  }

  .center {
    flex: 1;
    display: grid;
    place-items: start center;
    padding-top: 34px;
  }

  .drop {
    width: min(520px, 100%);
    padding: 34px 30px 28px;
    border-radius: 14px;
    border: 1px solid transparent;
    background: var(--fill);
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    color: var(--text-3);
    transition: border-color var(--speed-fast) var(--ease-out), background-color var(--speed-fast) var(--ease-out);
  }

  .drop:hover {
    border-color: var(--hairline-2);
  }

  .drop.dropping {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
  }

  .drop-title {
    margin-top: 12px;
    font-size: 16px;
    font-weight: 500;
    letter-spacing: -0.011em;
    color: var(--text);
  }

  .drop-sub {
    margin-top: 5px;
    font-size: 13.5px;
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

  /* 主按钮：深色药丸（参考设计的 Upgrade to Pro 样式） */
  .btn {
    height: 32px;
    padding: 0 16px;
    border-radius: 9px;
    background: var(--text);
    border: 0;
    color: var(--bg);
    font-size: 14px;
    font-weight: 500;
  }

  .btn:hover {
    opacity: 0.88;
  }

  .task {
    width: 100%;
    max-width: 940px;
    margin: 8px auto 0;
  }

  .task-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .file {
    font-size: 16.5px;
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
    font-size: 13.5px;
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
    font-size: 13.5px;
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
    font-size: 13px;
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
    transition: width var(--speed) var(--ease-out);
  }

  /* 总段数未知时（刚开始解码）用滑动的扫描条，而不是一条卡在中间的静态条。 */
  .bar i.indeterminate {
    width: 34%;
    opacity: 0.6;
    animation: sweep 1.5s ease-in-out infinite;
  }

  @keyframes sweep {
    0% {
      margin-left: -34%;
    }
    100% {
      margin-left: 100%;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .bar i.indeterminate {
      animation-duration: 4s;
    }
  }

  .note {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 10px;
    font-size: 12.5px;
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
    font-size: 12.5px;
    color: var(--text-3);
  }
</style>
