<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { HistoryEntry, Transcript } from "$lib/types";

  let {
    entries,
    detail,
    detailJobId,
    loading,
    onSelect,
    onBack,
    onExport,
  }: {
    entries: HistoryEntry[];
    detail: Transcript | null;
    detailJobId: string | null;
    loading: boolean;
    onSelect: (jobId: string) => void;
    onBack: () => void;
    onExport: (format: "txt" | "json") => void;
  } = $props();

  function dayLabel(timestamp: number): string {
    const date = new Date(timestamp);
    const today = new Date();
    const startOf = (value: Date) =>
      new Date(value.getFullYear(), value.getMonth(), value.getDate()).getTime();
    const diffDays = Math.round((startOf(today) - startOf(date)) / 86_400_000);
    if (diffDays <= 0) return "今天";
    if (diffDays === 1) return "昨天";
    return `${date.getMonth() + 1} 月 ${date.getDate()} 日`;
  }

  function timeLabel(timestamp: number): string {
    return new Date(timestamp).toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    });
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
    <div class="detail-bar">
      <button type="button" class="back" onclick={onBack}>
        <Icon name="chevronLeft" size={12} stroke={1.7} />历史
      </button>
      <div class="spacer"></div>
      <button type="button" class="act" onclick={() => onExport("txt")}>
        <Icon name="download" size={13} />TXT
      </button>
      <button type="button" class="act" onclick={() => onExport("json")}>
        <Icon name="download" size={13} />JSON
      </button>
    </div>
    <div class="detail-head">
      <div class="file">{detail.sourceFileName}</div>
      <div class="meta">
        <span><b>{detail.modelId}</b></span>
        <span class="sep">·</span>
        <span>{durationLabel(detail.durationMs)}</span>
        <span class="sep">·</span>
        <span>{timeLabel(entries.find((item) => item.jobId === detailJobId)?.updatedAtMs ?? Date.now())}</span>
      </div>
    </div>
    <div class="reader">
      {#each (detail.segments ?? [{ text: detail.text }]) as segment}
        <p>{segment.text}</p>
      {/each}
    </div>
  {:else}
    <div class="list">
      {#if loading}
        <p class="empty">正在读取历史…</p>
      {:else if entries.length === 0}
        <div class="empty-state">
          <div class="t">还没有历史记录</div>
          <div class="s">完成一次转写后，结果会出现在这里。</div>
        </div>
      {:else}
        {#each groups as group (group.label)}
          <div class="group">{group.label}</div>
          {#each group.items as entry (entry.jobId)}
            <div
              class="row"
              role="button"
              tabindex="0"
              class:selected={detailJobId === entry.jobId}
              onclick={(event) => activateRow(event, entry.jobId)}
              onkeydown={(event) => rowKeydown(event, entry.jobId)}
            >
              <span class="line">
                <span class="name">{entry.sourceFileName}</span>
                <span class="chip">{durationLabel(entry.durationMs)}</span>
                <span class="status"><span class="dot ok"></span>已完成</span>
                <span class="when">{timeLabel(entry.updatedAtMs)}</span>
              </span>
              {#if entry.preview}
                <span class="preview">{entry.preview}</span>
              {/if}
            </div>
          {/each}
        {/each}
      {/if}
    </div>
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

  .spacer {
    flex: 1;
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    width: 100%;
    max-width: 720px;
    margin: 2px auto 0;
  }

  .group {
    font-size: 11.5px;
    color: var(--text-3);
    padding: 14px 8px 5px;
    letter-spacing: 0.02em;
  }

  .row {
    user-select: text;
    -webkit-user-select: text;
    display: block;
    width: 100%;
    padding: 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    font-size: 13px;
    text-align: left;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
  }

  .preview {
    display: block;
    margin-top: 4px;
    font-size: 11.5px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row:hover {
    background: var(--hover);
  }

  .row.selected {
    background: var(--selected);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .when {
    font-size: 11.5px;
    color: var(--text-3);
    flex: none;
    font-variant-numeric: tabular-nums;
  }

  .chip {
    flex: none;
    font-size: 10.5px;
    color: var(--text-2);
    background: var(--sunken);
    border-radius: 5px;
    padding: 1px 6px;
    font-variant-numeric: tabular-nums;
  }

  .status {
    width: 76px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 5px;
    font-size: 11.5px;
    color: var(--text-2);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-3);
  }

  .dot.ok {
    background: var(--green);
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

  .detail-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    max-width: 720px;
    margin: 2px auto 0;
  }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-2);
    background: none;
    border: 0;
    padding: 0;
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

  .act:hover {
    background: var(--fill);
    color: var(--text);
  }

  .detail-head {
    width: 100%;
    max-width: 720px;
    margin: 8px auto 0;
  }

  .file {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.012em;
  }

  .meta {
    margin-top: 6px;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-3);
  }

  .meta b {
    font-weight: 500;
    color: var(--text-2);
  }

  .sep {
    color: var(--hairline-2);
  }

  .reader {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    width: 100%;
    max-width: 720px;
    margin: 0 auto;
    padding: 20px 0 24px;
  }

  .reader p {
    margin: 0 0 16px;
    font-size: 15px;
    line-height: 1.8;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
