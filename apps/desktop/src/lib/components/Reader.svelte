<script lang="ts">
  import { tick, type Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import AiAskCard from "./AiAskCard.svelte";
  import { t } from "$lib/i18n.svelte";
  import type { Segment } from "$lib/types";

  let {
    jobId,
    segments,
    running = false,
    initialFollow = true,
    onBack,
    heading,
    actions,
    bodyEl = $bindable(null),
  }: {
    jobId: string | null;
    segments: Segment[];
    running?: boolean;
    /** 历史详情等静态场景传 false：从顶部开始，不自动跟随到底部。 */
    initialFollow?: boolean;
    onBack?: () => void;
    heading?: Snippet;
    actions?: Snippet;
    /** 正文容器向外绑定：详情页复制时用它判断选区是否落在正文内。 */
    bodyEl?: HTMLDivElement | null;
  } = $props();

  let scroller = $state<HTMLDivElement | null>(null);
  let findInput = $state<HTMLInputElement | null>(null);
  let following = $state(true);
  let programmatic = false;
  let restoredFor = $state<string | null>(null);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  let findOpen = $state(false);
  let query = $state("");
  let current = $state(0);

  let atTop = $state(true);
  let atBottom = $state(false);
  let short = $state(false);

  let aiOpen = $state(false);
  let aiCollapsed = $state(false);
  let aiHasAnswer = $state(false);
  let aiSlotEl = $state<HTMLDivElement | null>(null);

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

  function measure() {
    if (!scroller) return;
    const top = scroller.scrollTop;
    const max = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    atTop = top <= 2;
    atBottom = max - top <= 2;
    short = max <= 2;
  }

  function onScroll() {
    measure();
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
      measure();
    });
  });

  // 新段落到达时，仅在跟随状态下滚动到底。
  $effect(() => {
    segments.length;
    if (!following || needle) return;
    requestAnimationFrame(() => scrollToBottom(prefersReducedMotion ? "auto" : "smooth"));
  });

  // 内容或窗口尺寸变化后重新判定两端与是否可滚动。
  $effect(() => {
    segments.length;
    requestAnimationFrame(measure);
    if (typeof window === "undefined") return;
    const onResize = () => measure();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });

  const showLatest = $derived(!following);

  // ---- 本篇内查找：所有命中都保留在正文里，当前命中单独描边 ----
  type Piece = { text: string; hit: boolean; index: number };

  const needle = $derived(query.trim().toLowerCase());
  const model = $derived.by(() => {
    let index = 0;
    const rows = segments.map((segment) => {
      const text = segment.text;
      const pieces: Piece[] = [];
      if (!needle) {
        pieces.push({ text, hit: false, index: -1 });
        return { pieces };
      }
      const lowered = text.toLowerCase();
      let cursor = 0;
      while (cursor < text.length) {
        const found = lowered.indexOf(needle, cursor);
        if (found === -1) {
          pieces.push({ text: text.slice(cursor), hit: false, index: -1 });
          break;
        }
        if (found > cursor) {
          pieces.push({ text: text.slice(cursor, found), hit: false, index: -1 });
        }
        pieces.push({
          text: text.slice(found, found + needle.length),
          hit: true,
          index: index++,
        });
        cursor = found + needle.length;
      }
      return { pieces };
    });
    return { rows, total: index };
  });
  const totalHits = $derived(model.total);

  $effect(() => {
    needle;
    current = 0;
  });

  async function scrollToCurrent() {
    await tick();
    if (!scroller) return;
    const el = bodyEl?.querySelector<HTMLElement>(`mark[data-hit="${current}"]`);
    if (!el) return;
    const mark = el.getBoundingClientRect();
    const frame = scroller.getBoundingClientRect();
    scroller.scrollTop += mark.top - frame.top - scroller.clientHeight / 2 + mark.height / 2;
    measure();
  }

  $effect(() => {
    current;
    totalHits;
    if (findOpen && totalHits > 0) void scrollToCurrent();
  });

  async function openFind() {
    findOpen = true;
    await tick();
    findInput?.focus();
    findInput?.select();
    if (totalHits > 0) void scrollToCurrent();
  }

  function closeFind() {
    findOpen = false;
    query = "";
    current = 0;
    scroller?.focus();
  }

  function step(delta: number) {
    if (totalHits === 0) return;
    current = (current + delta + totalHits) % totalHits;
  }

  function onFindKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeFind();
    } else if (event.key === "Enter") {
      event.preventDefault();
      step(event.shiftKey ? -1 : 1);
    }
  }

  function aiText(): string {
    return segments.map((segment) => segment.text).join("\n\n");
  }

  /** 只认落在正文里的有效选区；点「解释选中的段落」时取用。 */
  function aiSelection(): string | null {
    const value = window.getSelection();
    if (!value || value.isCollapsed || !value.anchorNode || !value.focusNode) {
      return null;
    }
    if (
      !bodyEl ||
      !bodyEl.contains(value.anchorNode) ||
      !bodyEl.contains(value.focusNode)
    ) {
      return null;
    }
    return value.toString().trim() || null;
  }

  function toggleAi() {
    aiOpen = !aiOpen;
  }

  function focusBody() {
    scroller?.focus();
  }

  function onWindowKeydown(event: KeyboardEvent) {
    if (
      (event.metaKey || event.ctrlKey) &&
      event.shiftKey &&
      event.key.toLowerCase() === "a"
    ) {
      event.preventDefault();
      toggleAi();
    } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      void openFind();
    } else if (event.key === "Escape" && aiOpen) {
      // 有回答时 Esc 只收起卡片；否则关闭面板；焦点回正文。
      event.preventDefault();
      if (aiHasAnswer) aiCollapsed = true;
      else aiOpen = false;
      focusBody();
    } else if (event.key === "Escape" && findOpen) {
      event.preventDefault();
      closeFind();
    }
  }

  // Native listener + ResizeObserver: the class toggles must follow the real
  // scroll position even when the reader restores a saved position.
  $effect(() => {
    const element = scroller;
    if (!element) return;
    const onNativeScroll = () => measure();
    element.addEventListener("scroll", onNativeScroll, { passive: true });
    const observer = new ResizeObserver(() => measure());
    observer.observe(element);
    requestAnimationFrame(measure);
    return () => {
      element.removeEventListener("scroll", onNativeScroll);
      observer.disconnect();
    };
  });

  $effect(() => {
    window.addEventListener("keydown", onWindowKeydown);
    return () => window.removeEventListener("keydown", onWindowKeydown);
  });

  // AI 卡片插入/收起/流式增长都改变正文上方的高度：按高度差补偿 scrollTop，
  // 正在读的那一段不跳走；跟随直播到底时交给跟随逻辑。
  $effect(() => {
    const element = aiSlotEl;
    if (!element) return;
    let last = element.offsetHeight;
    const observer = new ResizeObserver(() => {
      const next = element.offsetHeight;
      const delta = next - last;
      last = next;
      if (Math.abs(delta) > 1 && scroller && !following) {
        scroller.scrollTop += delta;
        measure();
      }
    });
    observer.observe(element);
    return () => observer.disconnect();
  });
</script>

<div class="reader-wrap">
  <div
    class="reader-shell"
    class:at-top={atTop}
    class:at-bottom={atBottom}
    class:short
  >
    <div
      class="reader-scroll"
      bind:this={scroller}
      onscroll={onScroll}
      tabindex="-1"
      aria-live={running ? "polite" : "off"}
    >
      <div class="reader-content">
      <div class="reader-head">
        {#if heading}
          {@render heading()}
        {/if}
      </div>

      {#if findOpen}
        <div class="find-row" role="search">
          <Icon name="search" size={13} stroke={1.6} />
          <input
            bind:this={findInput}
            type="text"
            autocomplete="off"
            autocorrect="off"
            spellcheck="false"
            value={query}
            placeholder={t("reader.searchPlaceholder")}
            aria-label={t("reader.searchPlaceholder")}
            oninput={(event) => (query = event.currentTarget.value)}
            onkeydown={onFindKeydown}
          />
          <span class="find-count" aria-live="polite">
            {totalHits === 0 ? "0 / 0" : `${current + 1} / ${totalHits}`}
          </span>
          <button
            type="button"
            class="find-btn"
            title={t("reader.findPrevious")}
            aria-label={t("reader.findPrevious")}
            onclick={() => step(-1)}
          >↑</button>
          <button
            type="button"
            class="find-btn"
            title={t("reader.findNext")}
            aria-label={t("reader.findNext")}
            onclick={() => step(1)}
          >↓</button>
          <button
            type="button"
            class="find-btn"
            title={t("reader.findClose")}
            aria-label={t("reader.findClose")}
            onclick={closeFind}
          >×</button>
        </div>
      {/if}

      <div class="ai-slot" bind:this={aiSlotEl}>
        <AiAskCard
          bind:open={aiOpen}
          bind:collapsed={aiCollapsed}
          bind:hasAnswer={aiHasAnswer}
          {jobId}
          text={aiText}
          selection={aiSelection}
          segmentCount={segments.length}
          onFocusBody={focusBody}
        />
      </div>

      <div class="reader-body" bind:this={bodyEl}>
        {#if segments.length === 0}
          <p class="placeholder">{running ? t("reader.recognizing") : t("reader.noText")}</p>
        {:else}
          {#each model.rows as row, index (index)}
            <p>
              {#each row.pieces as piece}
                {#if piece.hit}
                  <mark class:current={piece.index === current} data-hit={piece.index}>{piece.text}</mark>
                {:else}
                  {piece.text}
                {/if}
              {/each}
            </p>
          {/each}
        {/if}
      </div>
      </div>
    </div>

    <div class="reader-fade" aria-hidden="true"></div>
    <div class="fade-bottom" aria-hidden="true"></div>

    <div class="reader-bar">
      {#if onBack}
        <button
          type="button"
          class="back btn-secondary"
          title={t("nav.home")}
          aria-label={t("nav.home")}
          onclick={onBack}
        >
          <Icon name="chevronLeft" size={13} stroke={1.7} />
          <span class="label">{t("nav.home")}</span>
        </button>
      {/if}
      <button
        type="button"
        class="find-toggle btn-secondary"
        class:open={findOpen}
        title={t("reader.findOpen")}
        aria-label={t("reader.findOpen")}
        onclick={() => void openFind()}
      >
        <Icon name="search" size={13} stroke={1.6} />
        <span class="label">{t("reader.findOpen")}</span>
        <kbd>⌘F</kbd>
      </button>
      <button
        type="button"
        class="ai-toggle btn-secondary"
        class:open={aiOpen}
        title={t("reader.aiAsk")}
        aria-label={t("reader.aiAsk")}
        aria-pressed={aiOpen}
        onclick={toggleAi}
      >
        <Icon name="sparkle" size={13} stroke={1.6} />
        <span class="label">{t("reader.aiAsk")}</span>
        <kbd>⌘⇧A</kbd>
      </button>
      <span class="bar-spacer"></span>
      {#if actions}
        {@render actions()}
      {/if}
    </div>
  </div>

  {#if showLatest}
    <button
      type="button"
      class="latest btn-secondary"
      onclick={() => {
        following = true;
        scrollToBottom(prefersReducedMotion ? "auto" : "smooth");
        save();
      }}
    >
      {t("reader.viewLatest")}
      <Icon name="arrowDown" size={12} stroke={1.7} />
    </button>
  {/if}
</div>

<style>
  .reader-wrap {
    position: relative;
    flex: 1;
    /* 工具栏按自身可用宽度决定是否只显示图标，而不是跟着窗口猜。 */
    container-type: inline-size;
    min-height: 0;
    display: flex;
    flex-direction: column;
    pointer-events: none;
  }

  .reader-shell {
    --reader-fade-height: 72px;
    --reader-bar-height: 40px;
    --reader-content-clearance: 8px;
    /* Content starts after (fade height - bar height) + 8px clearance.
       With 72px / 40px this is 40px; changing fade height updates it. */
    --reader-content-padding-top: calc(
      var(--reader-fade-height) - var(--reader-bar-height) + var(--reader-content-clearance)
    );
    isolation: isolate;
    pointer-events: auto;
    position: relative;
    flex: 1;
    min-height: 0;
    width: 100%;
    max-width: 720px;
    margin: 0 auto;
    border-radius: var(--radius);
  }

  .reader-scroll {
    pointer-events: auto;
    position: absolute;
    top: var(--reader-bar-height);
    right: 0;
    bottom: 0;
    left: 0;
    overflow-y: auto;
    padding: 0 0 40px;
    border-radius: var(--radius);
    background: transparent;
  }

  .reader-scroll:focus {
    outline: none;
  }

  .reader-content {
    padding-top: var(--reader-content-padding-top);
  }

  .reader-bar {
    /* Keep --fill opaque here without changing fills elsewhere in the app. */
    --fill: var(--reader-solid-fill);
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 3;
    display: flex;
    align-items: center;
    gap: 8px;
    height: var(--reader-bar-height);
    padding: 0;
    background: transparent;
    border: 0;
    box-shadow: none;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
    pointer-events: none;
  }

  .reader-bar :global(button),
  .reader-bar :global(input) {
    pointer-events: auto;
  }

  .reader-bar :global(button) {
    white-space: nowrap;
    flex: none;
  }

  /* 窗口太窄时只保留图标：文字竖排比丢字更糟，hover/focus 仍有 title 与 aria-label。 */
  @container (max-width: 560px) {
    .reader-bar :global(button) {
      padding-left: 10px;
      padding-right: 10px;
    }

    .reader-bar .label,
    .reader-bar kbd {
      display: none;
    }
  }

  .reader-fade {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: var(--reader-fade-height);
    z-index: 2;
    background: linear-gradient(to bottom,
      var(--reader-fade) 0px,
      var(--reader-fade) var(--reader-bar-height),
      color-mix(in srgb, var(--reader-fade) 80%, transparent)
        calc((var(--reader-bar-height) + var(--reader-fade-height)) / 2),
      transparent var(--reader-fade-height));
    pointer-events: none;
    border-radius: var(--radius) var(--radius) 0 0;
  }

  .fade-bottom {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 24px;
    z-index: 2;
    background: linear-gradient(to bottom, transparent, var(--reader-fade));
    pointer-events: none;
    border-radius: 0 0 var(--radius) var(--radius);
  }

  .reader-shell.at-bottom .fade-bottom,
  .reader-shell.short .fade-bottom {
    height: 8px;
  }

  .bar-spacer {
    flex: 1;
  }

  .back {
    background: var(--capsule-home);
    backdrop-filter: blur(18px) saturate(1.1);
    -webkit-backdrop-filter: blur(18px) saturate(1.1);
  }

  .back:hover {
    background: var(--capsule-home-hover);
  }

  /* Decorative children must not replace the capsule as the hit-test target. */
  .back :global(*),
  .find-toggle :global(*) {
    pointer-events: none;
  }

  .ai-toggle :global(*) {
    pointer-events: none;
  }

  .find-toggle {
    backdrop-filter: blur(18px) saturate(1.1);
    -webkit-backdrop-filter: blur(18px) saturate(1.1);
  }

  .find-toggle.open {
    background: var(--capsule-find-pressed);
    transform: translateY(1px);
  }

  .find-toggle kbd {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-3);
  }

  .ai-toggle {
    backdrop-filter: blur(18px) saturate(1.1);
    -webkit-backdrop-filter: blur(18px) saturate(1.1);
  }

  .ai-toggle.open {
    background: var(--capsule-find-pressed);
    transform: translateY(1px);
  }

  .ai-toggle kbd {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-3);
  }

  .ai-slot {
    position: relative;
  }

  .reader-head {
    padding-top: 0;
  }

  .find-row {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 320px;
    margin: 12px 0 0;
    padding: 3px 5px 3px 8px;
    border-radius: 9px;
    background: var(--fill);
    color: var(--text-3);
  }

  .find-row input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    outline: none;
  }

  .find-row input::placeholder {
    color: var(--text-3);
  }

  .find-count {
    flex: none;
    font-size: 11.5px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }

  .find-btn {
    flex: none;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-3);
    font-size: 13px;
    line-height: 1;
    cursor: default;
  }

  .find-btn:hover {
    background: var(--hover);
    color: var(--text);
  }

  .reader-body {
    padding-top: 16px;
  }

  .reader-body p {
    margin: 0 0 16px;
    font-size: 16px;
    line-height: 1.8;
    letter-spacing: 0.002em;
    text-align: left;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .reader-body p:last-child {
    margin-bottom: 0;
  }

  .reader-body mark {
    background: var(--selected);
    color: inherit;
    border-radius: 3px;
    padding: 0 1px;
  }

  .reader-body mark.current {
    background: color-mix(in srgb, var(--accent) 34%, transparent);
    box-shadow: 0 0 0 1px var(--accent);
  }

  .placeholder {
    color: var(--text-3);
  }

  .latest {
    position: absolute;
    right: 12px;
    bottom: 12px;
    pointer-events: auto;
  }
</style>
