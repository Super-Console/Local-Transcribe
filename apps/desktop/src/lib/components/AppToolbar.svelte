<script lang="ts">
  import Icon from "./Icon.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { theme } from "$lib/theme.svelte";
  import type { ModelSummary } from "$lib/types";

  let {
    title,
    model,
    sidebarVisible,
    onToggleSidebar,
    onChooseModel,
  }: {
    title: string;
    model: ModelSummary | null;
    sidebarVisible: boolean;
    onToggleSidebar: () => void;
    onChooseModel: () => void;
  } = $props();

  // macOS 上窗口按钮位于内容层之上；侧栏隐藏时给工具栏让出交通灯位置。
  const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
  let inset = $derived(!sidebarVisible && isMac);

  function toggleTheme() {
    theme.preference = theme.resolved === "dark" ? "light" : "dark";
  }

  // Overlay 标题栏下原生拖动被关掉，这里显式发起拖动/双击最大化。
  async function beginDrag(event: MouseEvent) {
    if (event.button !== 0) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("button, a, input, select, textarea")) return;
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
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="toolbar"
  class:inset
  data-tauri-drag-region
  role="toolbar"
  aria-label="转写工具栏"
  tabindex="-1"
  onmousedown={beginDrag}
  ondblclick={toggleMaximize}
>
  <button
    type="button"
    class="icon-btn"
    aria-label={sidebarVisible ? "隐藏侧边栏" : "显示侧边栏"}
    aria-pressed={sidebarVisible}
    title={sidebarVisible ? "隐藏侧边栏 (⌘\\)" : "显示侧边栏 (⌘\\)"}
    onclick={onToggleSidebar}
  >
    <Icon name="panel" />
  </button>
  <h1 class="title">{title}</h1>
  <div class="spacer"></div>
  <button
    type="button"
    class="icon-btn theme"
    aria-label={theme.resolved === "dark" ? "切换到浅色外观" : "切换到深色外观"}
    title={theme.resolved === "dark" ? "切换到浅色外观" : "切换到深色外观"}
    onclick={toggleTheme}
  >
    <Icon name={theme.resolved === "dark" ? "sun" : "moon"} />
  </button>
  <div class="model-pick">
    <span class="label">默认模型</span>
    <button type="button" class="select" onclick={onChooseModel} aria-haspopup="dialog">
      <span class="dot" aria-hidden="true"></span>
      <span class="name" title={model?.displayName ?? "未选择模型"}>{model?.displayName ?? "未选择模型"}</span>
      <Icon name="chevronDown" size={12} stroke={1.6} />
    </button>
  </div>
</div>

<style>
  .toolbar {
    height: 46px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 16px 0 22px;
  }

  .toolbar.inset {
    padding-left: 78px;
  }

  .icon-btn {
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-2);
    display: grid;
    place-items: center;
    padding: 0;
  }

  .icon-btn:hover {
    background: var(--fill);
  }

  .title {
    margin: 0 0 0 4px;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.012em;
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }

  .model-pick {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .label {
    font-size: 11.5px;
    color: var(--text-3);
    letter-spacing: 0.01em;
    white-space: nowrap;
  }

  .select {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 9px 0 10px;
    border-radius: 8px;
    background: var(--panel);
    border: 1px solid var(--hairline-2);
    box-shadow: 0 1px 1.5px rgba(0, 0, 0, 0.045);
    font-size: 12.5px;
    white-space: nowrap;
    max-width: 260px;
  }

  .select:hover {
    background: var(--hover);
  }

  .select .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #c9c9ce;
    flex: none;
  }

  :global(:root[data-theme="dark"]) .select .dot {
    background: #5a5a60;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
