<script lang="ts" generics="V extends string">
  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n.svelte";

  /**
   * 参数化侧栏（2026-11-21 UI 验收 2）：分组 + items 由调用方传入。
   * Local 传 5 项（工作区：home/record/lan；管理：model/settings），
   * Cloud 传 6 项（工作区：transcribe/record/history；管理：providers/usage/settings）。
   * 每组标题渲染为 .section 小标题，样式与原实现一致。
   */
  type SidebarItem = { view: V; icon: string; label: string };
  type SidebarGroup = { title: string; items: SidebarItem[] };

  let {
    groups,
    view,
    collapsed,
    onNavigate,
    onSidebarPointerDown,
  }: {
    groups: SidebarGroup[];
    view: V;
    collapsed: boolean;
    onNavigate: (next: V) => void;
    onSidebarPointerDown?: (event: MouseEvent) => void;
  } = $props();

  function sidebarDragRegion(node: HTMLElement, onPointerDown?: (event: MouseEvent) => void) {
    let currentHandler = onPointerDown;
    if (currentHandler) node.addEventListener("mousedown", currentHandler);
    return {
      update(nextHandler?: (event: MouseEvent) => void) {
        if (currentHandler) node.removeEventListener("mousedown", currentHandler);
        currentHandler = nextHandler;
        if (currentHandler) node.addEventListener("mousedown", currentHandler);
      },
      destroy() {
        if (currentHandler) node.removeEventListener("mousedown", currentHandler);
      },
    };
  }

</script>

<aside
  id="app-sidebar"
  class="sidebar"
  class:collapsed
  inert={collapsed}
  aria-label={t("nav.mainLabel")}
  use:sidebarDragRegion={onSidebarPointerDown}
>
  {#each groups as group, index (group.title)}
    <div class="group" class:stacked={index > 0}>
      <div class="section">{group.title}</div>
      <nav class="nav">
        {#each group.items as item (item.view)}
          <button
            type="button"
            class="nav-item"
            class:active={view === item.view}
            aria-current={view === item.view ? "page" : undefined}
            aria-label={item.label}
            title={item.label}
            onclick={() => onNavigate(item.view)}
          >
            <Icon name={item.icon} />
            <span class="label">{item.label}</span>
          </button>
        {/each}
      </nav>
    </div>
  {/each}
</aside>

<style>
  /* 普通 macOS 侧栏：通高平铺列，右缘一条发丝线与内容分开 */
  .sidebar {
    width: var(--sidebar-width, 204px);
    flex: none;
    height: 100%;
    overflow: auto;
    display: flex;
    flex-direction: column;
    /* 顶部留白与红绿灯条下方的首个内容行对齐 */
    padding: 14px 10px 14px;
    background: var(--sidebar);
    border-right: 1px solid var(--hairline);
    transition: width var(--speed) var(--ease-out);
  }

  /* 折叠 = 整列完全消失，内容区拿回全宽 */
  .sidebar.collapsed {
    width: 0;
    padding: 0;
    border-right: 0;
    overflow: hidden;
  }

  /* 参考设计的分组小标题：大写、字距加宽、弱化颜色 */
  .section {
    padding: 0 10px 6px;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-2);
    user-select: none;
    white-space: nowrap;
  }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  /* 第 2 组起与上一组留出组间距（沿用原 .bottom 的 24px 语义） */
  .group.stacked {
    margin-top: 24px;
    padding-top: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 32px;
    flex: none;
    padding: 0 11px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    font-size: 13px;
    line-height: 1;
    color: var(--text);
    text-align: left;
    cursor: default;
    width: 100%;
    white-space: nowrap;
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
    font-weight: 600;
  }

  .nav-item.active :global(svg) {
    opacity: 1;
  }

  .nav-item:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
</style>
