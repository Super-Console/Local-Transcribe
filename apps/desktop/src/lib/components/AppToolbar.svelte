<script lang="ts">
  import Icon from "./Icon.svelte";
  import { theme } from "$lib/theme.svelte";
  import { t } from "$lib/i18n.svelte";
  import { prettyName } from "$lib/model-names.svelte";
  import type { LibraryModelSummary, ModelSummary } from "$lib/types";

  let {
    model,
    models = [],
    loadedModelId = null,
    remoteGroups = [],
    remoteLabel = null,
    remoteSelection = null,
    search = "",
    onSearchChange,
    onSelectModel,
    onSelectRemote,
    onManageModels,
  }: {
    model: ModelSummary | null;
    models?: LibraryModelSummary[];
    loadedModelId?: string | null;
    remoteGroups?: Array<{
      id: string;
      name: string;
      models: Array<{ id: string; displayName: string }>;
    }>;
    remoteLabel?: string | null;
    remoteSelection?: { nodeId: string; modelId: string } | null;
    search?: string;
    onSearchChange: (value: string) => void;
    onSelectModel: (id: string) => void;
    onSelectRemote?: (nodeId: string, modelId: string) => void;
    onManageModels: () => void;
  } = $props();

  let open = $state(false);
  // A selected remote model is as "active" as a loaded local model: the dot
  // next to the chip must stay green when the menu closes.
  const remoteDotOn = $derived(remoteSelection !== null);
  // 搜索框默认收起成放大镜，点击展开；有查询词时保持展开。
  let searchOpen = $state(false);
  let searchField = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (search) searchOpen = true;
  });
  $effect(() => {
    if (searchOpen) searchField?.focus();
  });

  function toggleTheme() {
    theme.preference = theme.resolved === "dark" ? "light" : "dark";
  }

  function sizeLabel(bytes: number): string {
    if (!bytes) return "—";
    const gigabytes = bytes / 1_073_741_824;
    if (gigabytes >= 1) return `${gigabytes.toFixed(2)} GB`;
    return `${Math.max(1, Math.round(bytes / 1_048_576))} MB`;
  }
</script>

<div class="toolbar" role="toolbar" aria-label={t("toolbar.ariaLabel")} tabindex="-1">
  <div class="spacer"></div>

  <div class="search" class:open={searchOpen}>
    <button
      type="button"
      class="search-icon"
      aria-label={t("search.placeholder")}
      title={t("search.placeholder")}
      onclick={() => {
        if (searchOpen) searchField?.focus();
        else searchOpen = true;
      }}
    >
      <Icon name="search" size={14} stroke={1.6} />
    </button>
    <input
      type="text"
      autocomplete="off"
      autocorrect="off"
      spellcheck="false"
      value={search}
      placeholder={t("search.placeholder")}
      aria-label={t("search.placeholder")}
      bind:this={searchField}
      tabindex={searchOpen ? 0 : -1}
      oninput={(event) => onSearchChange(event.currentTarget.value)}
      onkeydown={(event) => {
        if (event.key === "Escape") {
          onSearchChange("");
          searchOpen = false;
        }
      }}
      onblur={() => {
        if (!search) searchOpen = false;
      }}
    />
    {#if search}
      <button
        type="button"
        class="search-clear"
        aria-label={t("search.clear")}
        title={t("search.clear")}
        onclick={() => {
          onSearchChange("");
          searchOpen = false;
        }}
      >
        ×
      </button>
    {/if}
  </div>
  <button
    type="button"
    class="icon-btn theme"
    aria-label={theme.resolved === "dark" ? t("toolbar.toLight") : t("toolbar.toDark")}
    title={theme.resolved === "dark" ? t("toolbar.toLight") : t("toolbar.toDark")}
    onclick={toggleTheme}
  >
    <Icon name={theme.resolved === "dark" ? "sun" : "moon"} />
  </button>
  <div class="model-pick">
    <button
      type="button"
      class="select"
      aria-haspopup="listbox"
      aria-expanded={open}
      onclick={() => (open = !open)}
    >
      <span
        class="dot"
        class:on={remoteDotOn || (!!model && model.id === loadedModelId)}
        title={remoteDotOn || (model && model.id === loadedModelId) ? t("model.loadedHint") : undefined}
        aria-hidden="true"
      ></span>
      <span class="name" title={remoteLabel ?? prettyName(model?.displayName) ?? t("common.noModel")}
        >{remoteLabel ?? (prettyName(model?.displayName) || t("common.noModel"))}</span
      >
      <Icon name="chevronDown" size={12} stroke={1.6} />
    </button>

    {#if open}
      <button
        type="button"
        class="scrim"
        aria-label={t("common.cancel")}
        onclick={() => (open = false)}
      ></button>
      <div class="menu" role="listbox" aria-label={t("toolbar.defaultModel")}>
        <div class="menu-group">{t("toolbar.localSection")}</div>
        {#each models as entry (entry.id)}
          <button
            type="button"
            class="item"
            role="option"
            aria-selected={entry.selected}
            onclick={() => {
              onSelectModel(entry.id);
              open = false;
            }}
          >
            <span
              class="item-dot"
              class:on={entry.id === loadedModelId}
              title={entry.id === loadedModelId ? t("model.loadedHint") : undefined}
            ></span>
            <span class="item-body">
              <span class="item-name">{prettyName(entry.displayName)}</span>
              <span class="item-meta">{entry.family} · {sizeLabel(entry.bytes)}</span>
            </span>
            {#if entry.selected}
              <span class="check" aria-hidden="true">✓</span>
            {/if}
          </button>
        {:else}
          <div class="item empty">{t("common.noModel")}</div>
        {/each}
        {#each remoteGroups as group (group.id)}
          <div class="menu-sep"></div>
          <div class="menu-group">{group.name}</div>
          {#each group.models as remote (remote.id)}
            {@const active =
              remoteSelection?.nodeId === group.id && remoteSelection?.modelId === remote.id}
            <button
              type="button"
              class="item"
              role="option"
              aria-selected={active}
              onclick={() => {
                onSelectRemote?.(group.id, remote.id);
                open = false;
              }}
            >
              <span class="item-dot on"></span>
              <span class="item-body">
                <span class="item-name">{prettyName(remote.displayName)}</span>
                <span class="item-meta">{t("toolbar.remoteTag", { node: group.name })}</span>
              </span>
              {#if active}
                <span class="check" aria-hidden="true">✓</span>
              {/if}
            </button>
          {/each}
        {/each}
        <div class="menu-sep"></div>
        <button
          type="button"
          class="item manage"
          onclick={() => {
            open = false;
            onManageModels();
          }}
        >
          {t("settings.manageModels")}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .toolbar {
    position: relative;
    z-index: 10;
    height: 52px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 18px;
    background: var(--glass-toolbar);
    border-bottom: 1px solid var(--glass-reader-border);
    box-shadow: inset 0 1px 0 var(--glass-reader-highlight);
    backdrop-filter: blur(20px) saturate(1.1);
    -webkit-backdrop-filter: blur(20px) saturate(1.1);
  }

  .icon-btn {
    flex: none;
    width: 30px;
    height: 30px;
    border: 1px solid var(--capsule-find-border);
    border-radius: 999px;
    background: var(--capsule-find);
    color: var(--text-2);
    display: grid;
    place-items: center;
    padding: 0;
  }

  .icon-btn:hover {
    background: var(--fill);
  }

  /* 月亮与搜索：同尺寸、同圆心高度、常态同底色（只改 theme 一个） */
  .icon-btn.theme {
    border-radius: 999px;
    background: var(--capsule-find);
  }

  .icon-btn.theme:hover {
    background: var(--capsule-find-hover);
  }

  .icon-btn.theme:active {
    transform: translateY(1px);
  }

  .icon-btn.theme:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .spacer {
    flex: 1;
  }

  /* 搜索框：默认收起成放大镜（图标居中），点击后平滑展开 */
  .search {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid var(--capsule-find-border);
    border-radius: 999px;
    background: var(--capsule-find);
    color: var(--text-3);
    overflow: hidden;
    transition:
      width var(--speed) var(--ease-out),
      padding var(--speed) var(--ease-out),
      border-radius var(--speed) var(--ease-out),
      background-color var(--speed-fast) var(--ease-out),
      box-shadow var(--speed) var(--ease-out);
  }

  .search.open {
    width: min(240px, 40%);
    padding: 0 8px;
    gap: 5px;
    justify-content: flex-start;
    border-radius: 999px;
  }

  .search:hover {
    background: var(--capsule-find-hover);
  }

  .search:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .search:active {
    transform: translateY(1px);
  }

  /* 固定尺寸的图标按钮：命中区够大，收起时正好居中 */
  .search-icon {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-3);
    cursor: pointer;
  }

  .search-icon:hover {
    color: var(--text);
  }

  .search input {
    flex: 0 1 auto;
    width: 0;
    min-width: 0;
    height: 100%;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
    outline: none;
    opacity: 0;
    transform: translateX(-6px);
    transition:
      opacity var(--speed-fast) var(--ease-out),
      transform var(--speed) var(--ease-out);
  }

  .search.open input {
    flex: 1;
    width: auto;
    opacity: 1;
    transform: none;
  }

  .search input::placeholder {
    color: var(--text-3);
  }

  .search-clear {
    flex: none;
    border: 0;
    background: transparent;
    color: var(--text-3);
    font-size: 15px;
    line-height: 1;
    padding: 0 2px;
    animation: clear-in var(--speed-fast) var(--ease-spring);
  }

  @keyframes clear-in {
    from {
      opacity: 0;
      transform: scale(0.6);
    }
  }

  .search-clear:hover {
    color: var(--text);
  }

  .model-pick {
    min-width: 0;
    max-width: 260px;
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .select {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    background: var(--capsule-find);
    border: 1px solid var(--capsule-find-border);
    font-size: 12.5px;
    white-space: nowrap;
    max-width: 260px;
  }

  .select:hover {
    background: var(--capsule-find-hover);
  }

  .select:active {
    transform: translateY(1px);
  }

  .select:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .dot,
  .item-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #cbc5b7;
    flex: none;
  }

  .dot.on,
  .item-dot.on {
    background: var(--green);
  }

  :global(:root[data-theme="dark"]) .dot,
  :global(:root[data-theme="dark"]) .item-dot {
    background: #5a5a60;
  }

  :global(:root[data-theme="dark"]) .dot.on,
  :global(:root[data-theme="dark"]) .item-dot.on {
    background: var(--green);
  }

  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .select :global(svg) {
    flex: none;
  }

  .search,
  .icon-btn.theme,
  .select {
    box-shadow: inset 0 1px 0 var(--capsule-highlight);
    backdrop-filter: blur(18px) saturate(1.1);
    -webkit-backdrop-filter: blur(18px) saturate(1.1);
  }

  /* 下拉菜单（参考设计的紧凑浮层） */
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 24;
    background: transparent;
    border: 0;
  }

  .menu {
    position: absolute;
    transform-origin: top right;
    top: 38px;
    right: 0;
    z-index: 40;
    width: 320px;
    max-height: 380px;
    overflow-y: auto;
    padding: 6px;
    border-radius: 12px;
    border: 1px solid var(--hairline);
    background: var(--panel);
    opacity: 1;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
    box-shadow: var(--shadow-panel);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 8px 9px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    text-align: left;
    font-size: 13.5px;
  }

  .item:hover {
    background: var(--hover);
  }

  .item[aria-selected="true"] {
    background: var(--selected);
  }

  :global(:root[data-theme="dark"]) .menu {
    background: var(--bg);
  }

  .item-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .item-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-meta {
    font-size: 11.5px;
    color: var(--text-3);
  }

  .check {
    color: var(--accent);
    font-size: 12px;
  }

  .menu-sep {
    height: 1px;
    margin: 5px 4px;
    background: var(--hairline);
  }

  .menu-group {
    padding: 6px 10px 3px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .item.manage,
  .item.empty {
    color: var(--text-2);
    font-size: 13px;
  }
</style>
