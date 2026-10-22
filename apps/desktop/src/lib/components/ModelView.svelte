<script lang="ts">
  import Icon from "./Icon.svelte";
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
    if (!bytes) return "大小未知";
    const gigabytes = bytes / 1_073_741_824;
    if (gigabytes >= 1) return `${gigabytes.toFixed(2)} GB`;
    return `${Math.max(1, Math.round(bytes / 1_048_576))} MB`;
  }

  const total = $derived(models.reduce((sum, model) => sum + model.bytes, 0));
</script>

<section class="content">
  <div class="head">
    <button type="button" class="btn" onclick={onImport} disabled={busy}>
      <Icon name="plus" size={12} stroke={1.8} />导入模型
    </button>
  </div>

  {#if models.length === 0}
    <div class="empty">
      <div class="t">还没有导入模型</div>
      <div class="s">
        自己下载 sherpa-onnx 的 Whisper、Qwen3-ASR、Paraformer 或 SenseVoice 导出包，
        然后选择文件夹导入。导入会复制到应用目录，原始下载保持不动。
      </div>
      <button type="button" class="btn" onclick={onImport} disabled={busy}>
        <Icon name="plus" size={12} stroke={1.8} />选择模型文件夹
      </button>
    </div>
  {:else}
    <div class="list">
      {#each models as model (model.id)}
        <div class="row">
          <div class="body">
            <div class="name">
              {model.displayName}
              {#if model.selected}<span class="tag">默认</span>{/if}
            </div>
            <div class="meta">
              <span>{model.family}</span>
              <span class="sep">·</span>
              <span>{sizeLabel(model.bytes)}</span>
              <span class="sep">·</span>
              <span>已安装，未加载</span>
            </div>
          </div>
          <div class="row-actions">
            {#if !model.selected}
              <button type="button" class="act" onclick={() => onSelect(model.id)} disabled={busy}>
                设为默认
              </button>
              <button type="button" class="act danger" onclick={() => onDelete(model)} disabled={busy}>
                删除
              </button>
            {:else}
              <span class="hint">当前默认</span>
            {/if}
          </div>
        </div>
      {/each}
      <p class="footnote">
        共 {models.length} 个模型 · {sizeLabel(total)}。删除只会移除应用内的副本，不会影响你下载的原始文件。
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

  .head {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    width: 100%;
    max-width: 720px;
    margin: 4px auto 10px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 11px;
    border-radius: 7px;
    border: 1px solid var(--hairline-2);
    background: var(--panel);
    font-size: 12.5px;
    font-weight: 500;
    flex: none;
  }

  .btn:hover:not(:disabled) {
    background: var(--fill);
  }

  .btn:disabled {
    opacity: 0.55;
  }

  .list {
    width: 100%;
    max-width: 720px;
    margin: 0 auto;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 8px;
    border-radius: 8px;
  }

  .row:hover {
    background: var(--hover);
  }

  .body {
    min-width: 0;
    flex: 1;
  }

  .name {
    font-size: 13.5px;
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .tag {
    font-size: 10.5px;
    color: var(--text-2);
    background: var(--sunken);
    border-radius: 4px;
    padding: 1px 5px;
    font-weight: 400;
  }

  .meta {
    margin-top: 3px;
    font-size: 11.5px;
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
    font-size: 12.5px;
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
    font-size: 11.5px;
    color: var(--text-3);
    padding: 0 6px;
  }

  .empty {
    width: 100%;
    max-width: 720px;
    margin: 44px auto 0;
    text-align: center;
    color: var(--text-3);
  }

  .empty .t {
    font-size: 13px;
    color: var(--text-2);
  }

  .empty .s {
    margin: 6px auto 14px;
    max-width: 400px;
    font-size: 12px;
    line-height: 1.7;
  }

  .footnote {
    margin: 14px 8px 0;
    font-size: 11.5px;
    line-height: 1.7;
    color: var(--text-3);
  }
</style>
