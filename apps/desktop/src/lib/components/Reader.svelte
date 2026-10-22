<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n.svelte";
  import type { Segment } from "$lib/types";

  let {
    jobId,
    segments,
    running = false,
    initialFollow = true,
  }: {
    jobId: string | null;
    segments: Segment[];
    running?: boolean;
    /** 历史详情等静态场景传 false：从顶部开始，不自动跟随到底部。 */
    initialFollow?: boolean;
  } = $props();

  let scroller = $state<HTMLDivElement | null>(null);
  let following = $state(true);
  let programmatic = false;
  let restoredFor = $state<string | null>(null);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  const key = $derived(jobId ? `lt.reader.${jobId}` : null);
  const prefersReducedMotion =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  function scrollToBottom(behavior: ScrollBehavior = "smooth") {
    if (!scroller) return;
    programmatic = true;
    scroller.scrollTo({ top: scroller.scrollHeight, behavior });
    window.setTimeout(() => {
      programmatic = false;
    }, 120);
  }

  function save() {
    if (!key || !scroller) return;
    const state = { top: Math.round(scroller.scrollTop), following };
    try {
      localStorage.setItem(key, JSON.stringify(state));
    } catch {
      /* ignore storage errors */
    }
  }

  function restore() {
    if (!key || !scroller) return;
    try {
      const raw = localStorage.getItem(key);
      if (!raw) return;
      const parsed = JSON.parse(raw) as { top?: number; following?: boolean };
      if (typeof parsed.following === "boolean") following = parsed.following;
      if (typeof parsed.top === "number") {
        programmatic = true;
        scroller.scrollTop = parsed.top;
        window.setTimeout(() => {
          programmatic = false;
        }, 120);
      }
    } catch {
      /* ignore malformed entries */
    }
  }

  function onScroll() {
    if (!scroller || programmatic) return;
    const distance = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
    if (distance > 48 && following) {
      following = false;
    } else if (distance <= 8 && !following) {
      following = true;
    }
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(save, 250);
  }

  // 切换任务时恢复上次的阅读位置与自动跟随状态。
  $effect(() => {
    const currentKey = key;
    if (currentKey === restoredFor) return;
    restoredFor = currentKey;
    following = initialFollow;
    requestAnimationFrame(() => {
      restore();
      if (following && scroller) scrollToBottom("auto");
    });
  });

  // 新段落到达时，仅在跟随状态下滚动到底。
  $effect(() => {
    segments.length;
    if (!following || needle) return;
    requestAnimationFrame(() => scrollToBottom(prefersReducedMotion ? "auto" : "smooth"));
  });

  const showLatest = $derived(!following);

  // ---- 本文内搜索：过滤命中段落并高亮 ----
  let query = $state("");
  const needle = $derived(query.trim().toLowerCase());
  const visible = $derived(
    needle ? segments.filter((segment) => segment.text.toLowerCase().includes(needle)) : segments,
  );
  const matchCount = $derived(visible.length);

  function parts(text: string, target: string): Array<{ text: string; hit: boolean }> {
    if (!target) return [{ text, hit: false }];
    const lowered = text.toLowerCase();
    const out: Array<{ text: string; hit: boolean }> = [];
    let index = 0;
    while (index < text.length) {
      const found = lowered.indexOf(target, index);
      if (found === -1) {
        out.push({ text: text.slice(index), hit: false });
        break;
      }
      if (found > index) out.push({ text: text.slice(index, found), hit: false });
      out.push({ text: text.slice(found, found + target.length), hit: true });
      index = found + target.length;
    }
    return out;
  }
</script>

<div class="reader-wrap">
  {#if segments.length > 0}
    <div class="reader-tools">
      {#if needle}
        <span class="count">
          {matchCount === 0 ? t("reader.noMatch") : t("reader.matchCount", { count: matchCount })}
        </span>
      {/if}
      <div class="search">
        <Icon name="search" size={13} stroke={1.6} />
        <input
          type="text"
          autocomplete="off"
          autocorrect="off"
          spellcheck="false"
          value={query}
          placeholder={t("reader.searchPlaceholder")}
          aria-label={t("reader.searchPlaceholder")}
          oninput={(event) => (query = event.currentTarget.value)}
        />
        {#if query}
          <button type="button" class="clear" aria-label={t("search.clear")} title={t("search.clear")} onclick={() => (query = "")}>×</button>
        {/if}
      </div>
    </div>
  {/if}
  <div class="reader" bind:this={scroller} onscroll={onScroll} tabindex="-1" aria-live={running ? "polite" : "off"}>
    {#if segments.length === 0}
      <p class="placeholder">{running ? t("reader.recognizing") : t("reader.noText")}</p>
    {:else if visible.length === 0}
      <p class="placeholder">{t("reader.noMatch")}</p>
    {:else}
      {#each visible as segment, index (index)}
        <p>
          {#each parts(segment.text, needle) as part}
            {#if part.hit}<mark>{part.text}</mark>{:else}{part.text}{/if}
          {/each}
        </p>
      {/each}
    {/if}
  </div>

  {#if showLatest}
    <button type="button" class="latest" onclick={() => { following = true; scrollToBottom(prefersReducedMotion ? "auto" : "smooth"); save(); }}>
      {t("reader.viewLatest")}
      <Icon name="arrowDown" size={12} stroke={1.7} />
    </button>
  {/if}
</div>

<style>
  .reader-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .reader-tools {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    padding: 0 0 8px;
    max-width: 720px;
    width: 100%;
    margin: 0 auto;
  }

  .count {
    font-size: 12.5px;
    color: var(--text-3);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 220px;
    height: 30px;
    padding: 0 8px;
    border-radius: 9px;
    background: var(--fill);
    color: var(--text-3);
  }

  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    outline: none;
  }

  .search input::placeholder {
    color: var(--text-3);
  }

  .search .clear {
    border: 0;
    background: transparent;
    color: var(--text-3);
    font-size: 15px;
    line-height: 1;
    padding: 0 2px;
  }

  .search .clear:hover {
    color: var(--text);
  }

  .reader mark {
    background: color-mix(in srgb, var(--accent) 24%, transparent);
    color: inherit;
    border-radius: 3px;
    padding: 0 1px;
  }

  .reader {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 20px 0 40px;
    max-width: 720px;
    margin: 0 auto;
    width: 100%;
  }

  .reader:focus {
    outline: none;
  }

  .reader p {
    margin: 0 0 16px;
    font-size: 16px;
    line-height: 1.8;
    letter-spacing: 0.002em;
    text-align: left;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .reader p:last-child {
    margin-bottom: 0;
  }

  .placeholder {
    color: var(--text-3);
  }

  .latest {
    position: absolute;
    left: 50%;
    bottom: 14px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 12px;
    border-radius: 14px;
    border: 1px solid var(--hairline-2);
    background: var(--panel);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.12);
    font-size: 13px;
    color: var(--text-2);
  }
</style>
