<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { whatsNewByVersion } from "$lib/whats-new.js";

  let { version, onClose }: { version: string; onClose: () => void } = $props();
  const items = whatsNewByVersion[version] ?? [];
</script>

<aside class="whats-new" aria-labelledby="whats-new-title" aria-label={t("whatsNew.title")}>
  <header>
    <div>
      <h2 id="whats-new-title">{t("whatsNew.title")}</h2>
      <span class="version">{version}</span>
    </div>
    <button type="button" aria-label={t("whatsNew.close")} title={t("whatsNew.close")} onclick={onClose}>×</button>
  </header>
  <ul>
    {#each items as item (item.titleKey)}
      <li>
        <strong>{t(item.titleKey as never)}</strong>
        <span>{t(item.bodyKey as never)}</span>
      </li>
    {/each}
  </ul>
</aside>

<style>
  .whats-new {
    position: fixed;
    z-index: 30;
    top: 64px;
    right: 18px;
    width: min(340px, calc(100vw - 36px));
    padding: 14px 16px;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-panel);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 650;
  }

  .version {
    display: block;
    margin-top: 3px;
    color: var(--text-3);
    font-size: 11px;
  }

  button {
    flex: none;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-2);
    font-size: 20px;
    line-height: 1;
    cursor: pointer;
  }

  button:hover { background: var(--hover); color: var(--text); }
  button:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }

  ul {
    display: grid;
    gap: 11px;
    margin: 13px 0 0;
    padding: 0;
    list-style: none;
  }

  li { display: grid; gap: 2px; }
  li strong { font-size: 12px; font-weight: 600; }
  li span { color: var(--text-2); font-size: 11.5px; line-height: 1.45; }

  :global(:root[data-theme="dark"]) .whats-new { background: var(--panel); }
</style>
