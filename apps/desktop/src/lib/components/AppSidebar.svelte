<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n.svelte";
  import type { View } from "$lib/types";

  let {
    view,
    collapsed,
    onNavigate,
  }: {
    view: View;
    collapsed: boolean;
    onNavigate: (next: View) => void;
  } = $props();

</script>

<aside class="sidebar" class:collapsed inert={collapsed} aria-label={t("nav.mainLabel")}>
  <div class="brand" title="Local Transcribe">
    <span class="label">Local Transcribe</span>
  </div>
  <div class="section">{t("nav.workspace")}</div>
  <nav class="nav">
    <button
      type="button"
      class="nav-item"
      class:active={view === "home"}
      aria-current={view === "home" ? "page" : undefined}
      aria-label={t("nav.home")}
      title={t("nav.home")}
      onclick={() => onNavigate("home")}
    >
      <Icon name="home" />
      <span class="label">{t("nav.home")}</span>
    </button>
    <button
      type="button"
      class="nav-item"
      class:active={view === "record"}
      aria-current={view === "record" ? "page" : undefined}
      aria-label={t("nav.record")}
      title={t("nav.record")}
      onclick={() => onNavigate("record")}
    >
      <Icon name="mic" />
      <span class="label">{t("nav.record")}</span>
    </button>
  </nav>
  <div class="bottom">
    <div class="section">{t("nav.manage")}</div>
    <button
      type="button"
      class="nav-item"
      class:active={view === "model"}
      aria-current={view === "model" ? "page" : undefined}
      aria-label={t("nav.model")}
      title={t("nav.model")}
      onclick={() => onNavigate("model")}
    >
      <Icon name="model" />
      <span class="label">{t("nav.model")}</span>
    </button>
    <button
      type="button"
      class="nav-item"
      class:active={view === "settings"}
      aria-current={view === "settings" ? "page" : undefined}
      aria-label={t("nav.settings")}
      title={t("nav.settings")}
      onclick={() => onNavigate("settings")}
    >
      <Icon name="settings" />
      <span class="label">{t("nav.settings")}</span>
    </button>
  </div>
</aside>

<style>
  /* 普通 macOS 侧栏：通高平铺列，右缘一条发丝线与内容分开 */
  .sidebar {
    width: 214px;
    flex: none;
    height: 100%;
    display: flex;
    flex-direction: column;
    /* 顶部留白与红绿灯条下方的首个内容行对齐 */
    padding: 14px 10px 10px;
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

  .brand {
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.02em;
    padding: 2px 10px 16px;
    color: var(--text);
    user-select: none;
    white-space: nowrap;
    overflow: hidden;
  }

  /* 参考设计的分组小标题：大写、字距加宽、弱化颜色 */
  .section {
    padding: 0 10px 6px;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    user-select: none;
    white-space: nowrap;
  }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .bottom {
    margin-top: auto;
    padding-top: 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .bottom .section {
    padding: 0 10px 6px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 36px;
    padding: 0 11px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    font-size: 14.5px;
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
    font-weight: 500;
  }

  .nav-item.active :global(svg) {
    opacity: 1;
  }

  /* 参考设计的导航没有焦点环：选中态本身就是指示 */
  .nav-item:focus,
  .nav-item:focus-visible {
    outline: none;
  }
</style>
