<script module lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const tauriAvailable = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  /** 平台判定：仅 macOS / Windows 两个交付目标。 */
  export function currentPlatform(): "macos" | "windows" {
    return typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent)
      ? "windows"
      : "macos";
  }

  /**
   * 启动时给 <html> 打平台标记（2026-11-21 UI 验收 4）：
   * macOS 有原生红绿灯，.sidebar-header 保留 80px 左侧避让；
   * Windows 没有红绿灯，CSS 用 [data-os="windows"] 把避让降到 10px。
   * 不用负 margin、不做高度补偿。
   */
  if (typeof document !== "undefined") {
    document.documentElement.dataset.os = currentPlatform();
  }

  /**
   * 标题栏拖拽：macOS Overlay 标题栏下 data-tauri-drag-region 不生效（tauri#9503），
   * 且该属性只认直接命中，工具栏等子元素会挡住拖动；改为在标题栏统一显式
   * startDragging()，可交互控件（按钮/输入等）保持原有点击行为。
   * 侧栏顶部拖拽条等复用同一入口。
   */
  export function startWindowDrag(event: MouseEvent): void {
    if (!tauriAvailable || event.button !== 0 || event.detail > 1) return;
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (
      target.closest(
        'a, button, input, select, textarea, label, summary, [contenteditable]:not([contenteditable="false"]), [tabindex]:not([tabindex="-1"]), [role="button"], [role="link"], [role="tab"], [role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"], [role="listbox"], [role="menu"]',
      )
    ) {
      return;
    }
    event.preventDefault();
    try {
      void getCurrentWindow().startDragging().catch(() => {
        /* 权限缺失或非窗口环境时静默降级 */
      });
    } catch {
      /* 非 Tauri 环境没有窗口对象 */
    }
  }
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * 窗口外壳（2026-11-21 UI 验收 3）：结构 + CSS + 拖拽从 routes/+page.svelte
   * 抽出，Local 与 Cloud 共用同一条 52px 顶栏与红绿灯避让布局。
   */
  let {
    collapsed = false,
    windowsUp = false,
    toolbar,
    sidebar,
    overlay,
    children,
  }: {
    collapsed?: boolean;
    windowsUp?: boolean;
    toolbar: Snippet;
    sidebar: Snippet;
    overlay?: Snippet;
    children: Snippet;
  } = $props();
</script>

<div class="app" class:collapsed class:windows-sidebar-up={windowsUp}>
  <header class="window-header" role="presentation" onmousedown={startWindowDrag}>
    <div class="sidebar-header"></div>
    <div class="header-tools">{@render toolbar()}</div>
  </header>
  <div class="workspace">
    {@render sidebar()}
    <main class="main">{@render children()}</main>
  </div>
  {#if overlay}{@render overlay()}{/if}
</div>

<style>
  .app {
    --sidebar-width: 204px;
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    background: var(--bg);
    color: var(--text);
  }

  .app.windows-sidebar-up {
    position: relative;
  }

  .window-header {
    display: flex;
    height: 52px;
    flex: none;
    user-select: none;
  }

  .sidebar-header {
    width: var(--sidebar-width);
    flex: none;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 10px 0 80px;
    background: var(--sidebar);
    border-right: 1px solid var(--hairline);
    transition: width var(--speed) var(--ease-out);
  }

  /* 红绿灯分平台（UI 验收 4）：Windows 无红绿灯，左侧避让从 80px 降到 10px。
   * data-os 打在 <html> 上（AppShell 启动时），祖先选择器需 :global 才不被 scoping 剪掉。 */
  :global([data-os="windows"]) .sidebar-header {
    padding-left: 10px;
  }

  /* Windows keeps the sidebar at y=0 and overlays the reserved titlebar row. */
  .app.windows-sidebar-up .window-header {
    position: absolute;
    top: 0;
    left: var(--sidebar-width);
    right: 0;
    height: 52px;
  }

  .app.windows-sidebar-up .sidebar-header {
    width: 0;
    padding: 0;
    border: 0;
  }

  .app.windows-sidebar-up.collapsed .window-header {
    left: 126px;
  }

  .app.windows-sidebar-up .main {
    padding-top: 60px;
  }

  .collapsed .sidebar-header {
    width: 126px;
    background: var(--bg);
    border-right-color: transparent;
  }

  .header-tools { flex: 1; min-width: 0; }

  .workspace {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  /* 工作区内角用统一 token；原生窗口外角仍由系统管理。 */
  .main {
    border-radius: var(--radius);
    overflow: hidden;
    flex: 1;
    /* 必须可收缩：否则 min-content（MCP 代码块 pre 长行/长路径）会把整条布局顶得比窗口宽 */
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 8px 12px 12px 12px;
    background: var(--bg);
  }
</style>
