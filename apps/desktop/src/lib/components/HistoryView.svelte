<script lang="ts">
  import Icon from "./Icon.svelte";
  import Reader from "./Reader.svelte";
  import { t, formatDayLabel, formatClock } from "$lib/i18n.svelte";
  import { prettyName } from "$lib/model-names.svelte";
  import type { HistoryEntry, SearchHit, Transcript } from "$lib/types";

  let {
    entries,
    detail,
    detailJobId = null,
    loading,
    onSelect,
    onBack = () => {},
    onExport,
    onPickFile,
    dropping = false,
    hits = null,
    searchBusy = false,
  }: {
    entries: HistoryEntry[];
    detail?: Transcript | null;
    detailJobId?: string | null;
    loading: boolean;
    onSelect: (jobId: string) => void;
    onBack?: () => void;
    onExport: (format: "txt" | "json") => void;
    onPickFile?: () => void;
    dropping?: boolean;
    /** 非 null 表示正在按查询显示搜索结果。 */
    hits?: SearchHit[] | null;
    searchBusy?: boolean;
  } = $props();

  function dayLabel(timestamp: number): string {
    const date = new Date(timestamp);
    const today = new Date();
    const startOf = (value: Date) =>
      new Date(value.getFullYear(), value.getMonth(), value.getDate()).getTime();
    const diffDays = Math.round((startOf(today) - startOf(date)) / 86_400_000);
    if (diffDays <= 0) return t("time.today");
    if (diffDays === 1) return t("time.yesterday");
    return formatDayLabel(timestamp);
  }

  function timeLabel(timestamp: number): string {
    return formatClock(timestamp);
  }

  function durationLabel(durationMs?: number): string {
    if (!durationMs) return "—";
    const total = Math.round(durationMs / 1000);
    const minutes = Math.floor(total / 60);
    const seconds = total % 60;
    return `${minutes}:${String(seconds).padStart(2, "0")}`;
  }

  // 列表行是 div[role=button]，文本保持可选中；拖选结束时不要误触发跳转
  function activateRow(event: MouseEvent, jobId: string) {
    const selection = window.getSelection();
    if (selection && selection.toString().length > 0) return;
    onSelect(jobId);
  }

  function rowKeydown(event: KeyboardEvent, jobId: string) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onSelect(jobId);
    }
  }

  const groups = $derived.by(() => {
    const result: Array<{ label: string; items: HistoryEntry[] }> = [];
    for (const entry of entries) {
      const label = dayLabel(entry.updatedAtMs);
      const last = result[result.length - 1];
      if (last && last.label === label) last.items.push(entry);
      else result.push({ label, items: [entry] });
    }
    return result;
  });
</script>

<section class="content">
  {#if detail && detailJobId}
    <Reader
      jobId={detailJobId}
      segments={detail.segments && detail.segments.length > 0
        ? detail.segments
        : detail.text
          ? [{ startMs: 0, endMs: 0, text: detail.text }]
          : []}
      initialFollow={false}
      onBack={onBack}
    >
      {#snippet heading()}
        <div class="detail-head">
          <div class="file">{detail.sourceFileName}</div>
          <div class="meta">
            <span><b>{prettyName(detail.modelId)}</b></span>
            <span class="sep">·</span>
            <span>{durationLabel(detail.durationMs)}</span>
            <span class="sep">·</span>
            <span>{timeLabel(entries.find((item) => item.jobId === detailJobId)?.updatedAtMs ?? Date.now())}</span>
          </div>
        </div>
      {/snippet}
      {#snippet actions()}
        <button type="button" class="act btn-secondary" onclick={() => onExport("txt")}>
          <Icon name="download" size={13} />TXT
        </button>
        <button type="button" class="act btn-secondary" onclick={() => onExport("json")}>
          <Icon name="download" size={13} />JSON
        </button>
      {/snippet}
    </Reader>
  {:else}
    <div class="list">
      <h1 class="page-title">{t("nav.home")}</h1>
      <p class="subtitle">{t("home.subtitle")}</p>
      {#if onPickFile}
        <button
          type="button"
          class="import-tile"
          class:dropping
          onclick={onPickFile}
        >
          <Icon name="plus" size={20} stroke={1.6} />
          <span>{t("home.import")}</span>
        </button>
      {/if}
      {#if hits !== null}
        <div class="group">{t("search.results", { count: hits.length })}</div>
        {#if searchBusy}
          <p class="empty">{t("history.loading")}</p>
        {:else if hits.length === 0}
          <p class="empty">{t("search.empty")}</p>
        {:else}
          <div class="cards">
            {#each hits as hit (hit.jobId)}
              <div
                class="card"
                role="button"
                tabindex="0"
                onclick={(event) => activateRow(event, hit.jobId)}
                onkeydown={(event) => rowKeydown(event, hit.jobId)}
              >
                <div class="card-top">
                  <span class="ficon"><Icon name="file" size={15} stroke={1.5} /></span>
                  <span class="name">{hit.sourceFileName}</span>
                </div>
                <div class="hit-snippet">{hit.snippet}</div>
                <div class="card-meta">
                  <span class="model">{prettyName(hit.modelId)}</span>
                  <span class="dur">{durationLabel(hit.durationMs)}</span>
                </div>
                <div class="card-foot">
                  <span class="chip">{t("state.completed")}</span>
                  <span class="when">{formatDayLabel(hit.updatedAtMs)} {timeLabel(hit.updatedAtMs)}</span>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {:else if loading}
        <p class="empty">{t("history.loading")}</p>
      {:else if entries.length === 0}
        <div class="empty-state">
          <div class="t">{t("history.emptyTitle")}</div>
          <div class="s">{t("history.emptyBody")}</div>
        </div>
      {:else}
        {#each groups as group (group.label)}
          <div class="group">{group.label}</div>
          <div class="cards">
            {#each group.items as entry (entry.jobId)}
              <div
                class="card"
                role="button"
                tabindex="0"
                class:selected={detailJobId === entry.jobId}
                onclick={(event) => activateRow(event, entry.jobId)}
                onkeydown={(event) => rowKeydown(event, entry.jobId)}
              >
                <div class="card-top">
                  <span class="ficon"><Icon name="file" size={15} stroke={1.5} /></span>
                  <span class="name">{entry.sourceFileName}</span>
                </div>
                <div class="card-meta">
                  <span class="model">{prettyName(entry.modelId)}</span>
                  <span class="dur">{durationLabel(entry.durationMs)}</span>
                </div>
                <div class="card-foot">
                  <span class="chip">{t("state.completed")}</span>
                  <span class="when">{formatDayLabel(entry.updatedAtMs)} {timeLabel(entry.updatedAtMs)}</span>
                </div>
              </div>
            {/each}
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</section>

<style>
  .content {
    border-radius: var(--radius);
    overflow: hidden;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 2px 28px 0;
  }

  .list {
    border-radius: var(--radius);
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    width: 100%;
    max-width: 940px;
    margin: 0 auto;
    padding-bottom: 24px;
  }

  .page-title {
    margin: 10px 0 18px;
    font-family: var(--font-display);
    font-size: 34px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .subtitle {
    margin: -8px 0 18px;
    font-size: 14.5px;
    color: var(--text-2);
  }

  /* 参考设计的功能卡片（Import 磁贴） */
  .import-tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 168px;
    padding: 20px 16px;
    margin-bottom: 26px;
    border-radius: 14px;
    border: 1px solid transparent;
    background: var(--fill);
    color: var(--text);
    font-size: 14.5px;
    font-weight: 500;
    transition: border-color var(--speed-fast) var(--ease-out), background-color var(--speed-fast) var(--ease-out);
  }

  .import-tile:hover {
    border-color: var(--hairline-2);
  }

  .import-tile.dropping {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
  }

  .import-tile :global(svg) {
    color: var(--text-2);
  }

  .group {
    font-size: 14px;
    font-weight: 500;
    color: var(--text-3);
    margin: 20px 0 10px;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 12px;
  }

  .card {
    user-select: text;
    -webkit-user-select: text;
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 14px 16px;
    border: 1px solid var(--hairline-2);
    border-radius: 12px;
    background: var(--card);
    text-align: left;
    transition: border-color var(--speed-fast) var(--ease-out), box-shadow var(--speed-fast) var(--ease-out);
  }

  .card:hover {
    border-color: color-mix(in srgb, var(--text) 22%, transparent);
    box-shadow: 0 1px 3px rgba(40, 28, 16, 0.06);
  }

  .card.selected {
    border-color: var(--accent);
  }

  .card-top {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .ficon {
    flex: none;
    margin-top: 1px;
    color: var(--text-2);
    display: grid;
    place-items: center;
  }

  .name {
    flex: 1;
    min-width: 0;
    font-size: 15px;
    font-weight: 600;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }

  /* 搜索结果里的命中上下文 */
  .hit-snippet {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    border-radius: 7px;
    padding: 6px 9px;
    overflow-wrap: anywhere;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .card-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 13px;
    color: var(--text-3);
  }

  .model {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dur {
    flex: none;
    font-variant-numeric: tabular-nums;
  }

  .card-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .chip {
    flex: none;
    font-size: 12px;
    color: var(--text-2);
    background: var(--chip);
    border-radius: 6px;
    padding: 3px 8px;
  }

  .when {
    font-size: 12.5px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }

  .empty,
  .empty-state {
    color: var(--text-3);
    font-size: 12px;
    text-align: center;
    padding: 60px 0;
  }

  .empty-state .t {
    font-size: 13px;
    color: var(--text-2);
  }

  .empty-state .s {
    margin-top: 4px;
  }

  .detail-head {
    width: 100%;
    max-width: 720px;
    margin: 8px auto 0;
  }

  .file {
    font-size: 16.5px;
    font-weight: 600;
    letter-spacing: -0.012em;
  }

  .meta {
    margin-top: 6px;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-3);
  }

  .meta b {
    font-weight: 500;
    color: var(--text-2);
  }

  .sep {
    color: var(--hairline-2);
  }

</style>
