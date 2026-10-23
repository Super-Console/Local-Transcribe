<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import type { CatalogModel, LibraryModelSummary, ModelDownloadEvent } from "$lib/types";

  let {
    models,
    catalog,
    downloads,
    busy,
    onSelect,
    onDelete,
    onDeleteCatalog,
    onDownload,
    onCancelDownload,
  }: {
    models: LibraryModelSummary[];
    catalog: CatalogModel[];
    downloads: Record<string, ModelDownloadEvent>;
    busy: boolean;
    onSelect: (id: string) => void;
    onDelete: (model: LibraryModelSummary) => void;
    onDeleteCatalog: (entry: CatalogModel) => void;
    onDownload: (id: string) => void;
    onCancelDownload: () => void;
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
  </div>

  {#if catalog.length > 0}
    <div class="list catalog">
      <div class="section-label">{t("model.downloadTitle")}</div>
      {#each ["quality", "speed", "punctuation"] as tier (tier)}
        {@const entries = catalog.filter((entry) => entry.tier === tier)}
        {#if entries.length > 0}
          <div class="tier-label">
            {t(tier === "quality" ? "model.tierQuality" : tier === "speed" ? "model.tierSpeed" : "model.tierPunctuation")}
          </div>
          {#each entries as entry (entry.id)}
            {@const dl = downloads[entry.id]}
            {@const active = !!dl && (dl.state === "downloading" || dl.state === "installing")}
            {@const percent =
              dl && dl.total > 0 ? Math.min(99, Math.round((dl.received / dl.total) * 100)) : 0}
            <div class="row">
              <div class="body">
                <div class="name">
                  {entry.name}
                  {#if entry.kind === "punctuation"}
                    <span class="tag">{t("model.punctuationTag")}</span>
                  {/if}
                </div>
                <div class="meta">
                  <span>{entry.family}</span>
                  <span class="sep">·</span>
                  <span>{sizeLabel(entry.bytes)}</span>
                  {#if active}
                    <span class="sep">·</span>
                    <span>{dl?.state === "installing" ? t("model.installing") : `${percent}%`}</span>
                  {/if}
                </div>
                {#if dl?.state === "error" && dl.error}
                  <div class="error-line">{dl.error}</div>
                  <div class="error-line hint">{t("model.downloadNetworkHint")}</div>
                {/if}
                {#if active}
                  <div class="progress">
                    <div class="progress-fill" style:width={`${percent}%`}></div>
                  </div>
                {/if}
              </div>
              <div class="row-actions">
                {#if entry.installed}
                  <span class="hint">{t("model.downloaded")}</span>
                  <button
                    type="button"
                    class="act danger"
                    onclick={() => onDeleteCatalog(entry)}
                    disabled={busy}
                  >
                    {t("common.delete")}
                  </button>
                {:else if active}
                  <button type="button" class="act" onclick={onCancelDownload}>
                    {t("model.downloadCancel")}
                  </button>
                {:else}
                  <button
                    type="button"
                    class="act"
                    onclick={() => onDownload(entry.id)}
                    disabled={busy}
                  >
                    {t("model.download")}
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        {/if}
      {/each}
    </div>
  {/if}

  {#if models.length === 0}
    <div class="empty">
      <div class="t">{t("model.emptyTitle")}</div>
      <div class="s">
        {t("model.emptyBody")}
      </div>
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
            {:else}
              <span class="hint">{t("model.currentDefault")}</span>
            {/if}
            <button type="button" class="act danger" onclick={() => onDelete(model)} disabled={busy}>
              {t("common.delete")}
            </button>
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

  .list {
    width: 100%;
    max-width: 940px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .list.catalog {
    margin-bottom: 26px;
  }

  .section-label {
    margin: 2px 4px 2px;
    font-size: 12.5px;
    font-weight: 500;
    letter-spacing: 0.01em;
    color: var(--text-3);
  }

  .tier-label {
    margin: 14px 4px 4px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-3);
  }

  .error-line {
    margin-top: 4px;
    font-size: 12.5px;
    color: var(--red);
    line-height: 1.5;
  }

  .progress {
    margin-top: 8px;
    height: 4px;
    border-radius: 2px;
    background: var(--sunken);
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--text);
    transition: width var(--speed-fast) var(--ease-out);
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
