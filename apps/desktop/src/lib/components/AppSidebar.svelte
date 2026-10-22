<script lang="ts">
  import Icon from "./Icon.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { View } from "$lib/types";

  let {
    view,
    onNavigate,
  }: { view: View; onNavigate: (next: View) => void } = $props();

  // Overlay 标题栏下原生拖动被关闭，侧栏顶部条显式发起拖动。
  async function beginDrag(event: MouseEvent) {
    if (event.button !== 0) return;
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      await getCurrentWindow().startDragging();
    } catch {
      /* 浏览器预览没有窗口 */
    }
  }

  async function toggleMaximize() {
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      await getCurrentWindow().toggleMaximize();
    } catch {
      /* ignore */
    }
  }

  const items: Array<{ id: View; label: string; icon: string }> = [
    { id: "transcribe", label: "转写", icon: "transcribe" },
    { id: "history", label: "历史", icon: "history" },
    { id: "model", label: "模型", icon: "model" },
  ];
</script>

<aside class="sidebar" aria-label="主导航">
  <div
    class="drag-strip"
    data-tauri-drag-region
    role="presentation"
    onmousedown={beginDrag}
    ondblclick={toggleMaximize}
  ></div>
  <div class="brand">Local Transcribe</div>
  <nav class="nav">
    {#each items as item (item.id)}
      <button
        type="button"
        class="nav-item"
        class:active={view === item.id}
        aria-current={view === item.id ? "page" : undefined}
        onclick={() => onNavigate(item.id)}
      >
        <Icon name={item.icon} />
        <span>{item.label}</span>
      </button>
    {/each}
  </nav>
  <div class="bottom">
    <button
      type="button"
      class="nav-item"
      class:active={view === "settings"}
      aria-current={view === "settings" ? "page" : undefined}
      onclick={() => onNavigate("settings")}
    >
      <Icon name="settings" />
      <span>设置</span>
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: 196px;
    flex: none;
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0 10px 10px;
    background: var(--glass);
    backdrop-filter: blur(28px) saturate(180%);
    -webkit-backdrop-filter: blur(28px) saturate(180%);
    border: 1px solid var(--glass-border);
    border-radius: 14px;
    box-shadow: inset 0 1px 0 var(--glass-highlight), var(--glass-shadow);
  }

  /* 原生窗口按钮区域（macOS overlay 标题栏）：只用于拖动，不放控件 */
  .drag-strip {
    height: 40px;
    flex: none;
  }

  .brand {
    font-size: 13px;
    font-weight: 600;
    letter-spacing: -0.01em;
    padding: 0 8px 16px;
    color: var(--text);
    user-select: none;
  }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .bottom {
    margin-top: auto;
    padding-top: 10px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 8px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    font-size: 13px;
    line-height: 1;
    color: var(--text);
    text-align: left;
    cursor: default;
    width: 100%;
  }

  .nav-item :global(svg) {
    opacity: 0.82;
    flex: none;
  }

  .nav-item:hover:not(.active) {
    background: var(--hover);
  }

  .nav-item.active {
    background: var(--selected);
    color: var(--accent-ink);
    font-weight: 500;
  }

  .nav-item.active :global(svg) {
    opacity: 1;
  }
</style>
