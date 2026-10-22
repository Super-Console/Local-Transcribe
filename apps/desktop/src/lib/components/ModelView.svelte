<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n.svelte";
  import type { LibraryModelSummary } from "$lib/types";

  let {
    models,
    busy,
    onImport,
    onSelect,
    onDelete,
  }: {
    models: LibraryModelSummary[];
    busy: boolean;
    onImport: () => void;
    onSelect: (id: string) => void;
    onDelete: (model: LibraryModelSummary) => void;
  } = $props();

  function sizeLabel(bytes: number): string {
    if (!bytes) return t("model.unknownSize");
    const gigabytes = bytes / 1_073_741_824;
    if (gigabytes >= 1) return `${gigabytes.toFixed(2)} GB`;
    return `${Math.max(1, Math.round(bytes / 1_048_576))} MB`;
  }

  const total = $derived(models.reduce((sum, model) => sum + model.bytes, 0));
</script>

<section class="content">
  <div class="title-row">
    <h1 class="page-title">{t("nav.model")}</h1>
    <button type="button" class="btn primary" onclick={onImport} disabled={busy}>
      <Icon name="plus" size={12} stroke={1.8} />{t("model.import")}
    </button>
  </div>

  {#if models.length === 0}
    <div class="empty">
      <div class="t">{t("model.emptyTitle")}</div>
      <div class="s">
        {t("model.emptyBody")}
      </div>
      <button type="button" class="btn" onclick={onImport} disabled={busy}>
        <Icon name="plus" size={12} stroke={1.8} />{t("model.chooseFolder")}
      </button>
    </div>
  {:else}
    <div class="list">
      {#each models as model (model.id)}
        <div class="row">
          <div class="body">
            <div class="name">
              {model.displayName}
              {#if model.selected}<span class="tag">{t("model.defaultTag")}</span>{/if}
            </div>
            <div class="meta">
              <span>{model.family}</span>
              <span class="sep">·</span>
              <span>{sizeLabel(model.bytes)}</span>
              <span class="sep">·</span>
              <span>{t("model.installedNotLoaded")}</span>
            </div>
          </div>
          <div class="row-actions">
            {#if !model.selected}
              <button type="button" class="act" onclick={() => onSelect(model.id)} disabled={busy}>
                {t("model.setDefault")}
              </button>
              <button type="button" class="act danger" onclick={() => onDelete(model)} disabled={busy}>
                {t("common.delete")}
              </button>
            {:else}
              <span class="hint">{t("model.currentDefault")}</span>
            {/if}
          </div>
        </div>
      {/each}
      <p class="footnote">
        {t("model.footer", { count: models.length, size: sizeLabel(total) })}
      </p>
    </div>
  {/if}
</section>

<style>
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 2px 28px 20px;
  }

  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    max-width: 940px;
    margin: 10px auto 18px;
  }

  .page-title {
    margin: 0;
    font-family: var(--font-display);
    font-size: 34px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 32px;
    padding: 0 14px;
    border-radius: 9px;
    border: 1px solid var(--hairline-2);
    background: var(--panel);
    font-size: 13.5px;
    font-weight: 500;
    flex: none;
  }

  /* 参考设计的主按钮：深色药丸 */
  .btn.primary {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg);
  }

  .btn.primary:hover:not(:disabled) {
    opacity: 0.88;
    background: var(--text);
  }

  .btn:hover:not(:disabled) {
    background: var(--fill);
  }

  .btn:disabled {
    opacity: 0.55;
  }

  .list {
    width: 100%;
    max-width: 940px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 16px;
    border: 1px solid var(--hairline-2);
    border-radius: 12px;
    background: var(--card);
    transition: border-color var(--speed-fast) var(--ease-out);
  }

  .row:hover {
    border-color: color-mix(in srgb, var(--text) 20%, transparent);
  }

  .body {
    min-width: 0;
    flex: 1;
  }

  .name {
    font-size: 15px;
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .tag {
    font-size: 11.5px;
    color: var(--text-2);
    background: var(--sunken);
    border-radius: 4px;
    padding: 1px 5px;
    font-weight: 400;
  }

  .meta {
    margin-top: 3px;
    font-size: 13px;
    color: var(--text-3);
    display: flex;
    gap: 8px;
  }

  .sep {
    color: var(--hairline-2);
  }

  .row-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
  }

  .act {
    height: 26px;
    padding: 0 8px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-2);
    font-size: 13.5px;
  }

  .act:hover:not(:disabled) {
    background: var(--fill);
    color: var(--text);
  }

  .act.danger:hover:not(:disabled) {
    color: var(--red);
  }

  .act:disabled {
    opacity: 0.5;
  }

  .hint {
    font-size: 12.5px;
    color: var(--text-3);
    padding: 0 6px;
  }

  .empty {
    width: 100%;
    max-width: 940px;
    margin: 44px auto 0;
    text-align: center;
    color: var(--text-3);
  }

  .empty .t {
    font-size: 14.5px;
    color: var(--text-2);
  }

  .empty .s {
    margin: 6px auto 14px;
    max-width: 400px;
    font-size: 13.5px;
    line-height: 1.7;
  }

  .footnote {
    margin: 14px 8px 0;
    font-size: 12.5px;
    line-height: 1.7;
    color: var(--text-3);
  }
</style>
