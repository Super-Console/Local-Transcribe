<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { Segment } from "$lib/types";

  let {
    jobId,
    segments,
    running = false,
  }: { jobId: string | null; segments: Segment[]; running?: boolean } = $props();

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
    following = true;
    requestAnimationFrame(() => {
      restore();
      if (following && scroller) scrollToBottom("auto");
    });
  });

  // 新段落到达时，仅在跟随状态下滚动到底。
  $effect(() => {
    segments.length;
    if (!following) return;
    requestAnimationFrame(() => scrollToBottom(prefersReducedMotion ? "auto" : "smooth"));
  });

  const showLatest = $derived(!following);
</script>

<div class="reader-wrap">
  <div class="reader" bind:this={scroller} onscroll={onScroll} tabindex="-1" aria-live={running ? "polite" : "off"}>
    {#if segments.length === 0}
      <p class="placeholder">{running ? "正在识别，稍候会逐段出现文字。" : "还没有可显示的文字。"}</p>
    {:else}
      {#each segments as segment, index (index)}
        <p>{segment.text}</p>
      {/each}
    {/if}
  </div>

  {#if showLatest}
    <button type="button" class="latest" onclick={() => { following = true; scrollToBottom(prefersReducedMotion ? "auto" : "smooth"); save(); }}>
      查看最新
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
    font-size: 15px;
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
    font-size: 12px;
    color: var(--text-2);
  }
</style>
