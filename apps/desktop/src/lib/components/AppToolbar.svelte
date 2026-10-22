<script lang="ts">
  import Icon from "./Icon.svelte";
  import { theme } from "$lib/theme.svelte";
  import { t } from "$lib/i18n.svelte";
  import type { LibraryModelSummary, ModelSummary } from "$lib/types";

  let {
    model,
    models = [],
    loadedModelId = null,
    search = "",
    onSearchChange,
    onSelectModel,
    onManageModels,
  }: {
    model: ModelSummary | null;
    models?: LibraryModelSummary[];
    loadedModelId?: string | null;
    search?: string;
    onSearchChange: (value: string) => void;
    onSelectModel: (id: string) => void;
    onManageModels: () => void;
  } = $props();

  let open = $state(false);

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
  <div class="search">
    <Icon name="search" size={14} stroke={1.6} />
    <input
      type="text"
      autocomplete="off"
      autocorrect="off"
      spellcheck="false"
      value={search}
      placeholder={t("search.placeholder")}
      aria-label={t("search.placeholder")}
      oninput={(event) => onSearchChange(event.currentTarget.value)}
    />
    {#if search}
      <button
        type="button"
        class="search-clear"
        aria-label={t("search.clear")}
        title={t("search.clear")}
        onclick={() => onSearchChange("")}
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
        class:on={!!model && model.id === loadedModelId}
        title={model && model.id === loadedModelId ? t("model.loadedHint") : undefined}
        aria-hidden="true"
      ></span>
      <span class="name" title={model?.displayName ?? t("common.noModel")}
        >{model?.displayName ?? t("common.noModel")}</span
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
              <span class="item-name">{entry.displayName}</span>
              <span class="item-meta">{entry.family} · {sizeLabel(entry.bytes)}</span>
            </span>
            {#if entry.selected}
              <span class="check" aria-hidden="true">✓</span>
            {/if}
          </button>
        {:else}
          <div class="item empty">{t("common.noModel")}</div>
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
    height: 52px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 18px;
  }

  .icon-btn {
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-2);
    display: grid;
    place-items: center;
    padding: 0;
  }

  .icon-btn:hover {
    background: var(--fill);
  }

  .spacer {
    flex: 1;
  }

  /* 参考设计的搜索框：圆角、浅底、放大镜 */
  .search {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 240px;
    height: 32px;
    padding: 0 9px;
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
    font-size: 13.5px;
    outline: none;
  }

  .search input::placeholder {
    color: var(--text-3);
  }

  .search :global(svg) {
    flex: none;
  }

  .search-clear {
    border: 0;
    background: transparent;
    color: var(--text-3);
    font-size: 15px;
    line-height: 1;
    padding: 0 2px;
  }

  .search-clear:hover {
    color: var(--text);
  }

  .model-pick {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .select {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    padding: 0 11px 0 12px;
    border-radius: 9px;
    background: var(--fill);
    border: 0;
    font-size: 13.5px;
    white-space: nowrap;
    max-width: 260px;
  }

  .select:hover {
    background: var(--hover);
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
    overflow: hidden;
    text-overflow: ellipsis;
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
    top: 38px;
    right: 0;
    z-index: 25;
    width: 320px;
    max-height: 380px;
    overflow-y: auto;
    padding: 6px;
    border-radius: 11px;
    border: 1px solid var(--hairline-2);
    background: var(--panel);
    box-shadow: 0 12px 32px rgba(40, 28, 16, 0.16), 0 2px 6px rgba(40, 28, 16, 0.06);
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
    background: var(--fill);
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

  .item.manage,
  .item.empty {
    color: var(--text-2);
    font-size: 13px;
  }
</style>
