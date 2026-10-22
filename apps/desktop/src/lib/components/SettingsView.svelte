<script lang="ts">
  import { theme, type ThemePreference } from "$lib/theme.svelte";
  import type { ModelSummary, ServiceStatus, StorageSummary } from "$lib/types";

  let {
    model,
    storage,
    service,
    onOpenLibrary,
    onOpenHistory,
    onGoModels,
    onRefreshService,
  }: {
    model: ModelSummary | null;
    storage: StorageSummary | null;
    service: ServiceStatus | null;
    onOpenLibrary: () => void;
    onOpenHistory: () => void;
    onGoModels: () => void;
    onRefreshService: () => void;
  } = $props();

  import { onDestroy } from "svelte";

  let copied = $state<"none" | "mcp" | "cli">("none");
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => {
    if (copyTimer) clearTimeout(copyTimer);
  });

  const mcpConfig = $derived(`{
  "mcpServers": {
    "local-transcribe": {
      "command": "<voice-to-text-mcp 可执行文件路径>",
      "args": ["--allow-root", "${storage?.libraryPath ? storage.libraryPath.replace(/\/models$/, "") : "/路径/到/你的/媒体目录"}"]
    }
  }
}`);

  const cliExample = $derived(`local-transcribe models list
local-transcribe jobs add "<媒体文件>" --model <模型 id>
local-transcribe jobs show <job-id>
local-transcribe export <job-id> --format txt --output ~/Desktop/result.txt`);

  async function copy(text: string, which: "mcp" | "cli") {
    try {
      await navigator.clipboard.writeText(text);
      copied = which;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = "none"), 1600);
    } catch {
      copied = "none";
    }
  }

  const options: Array<{ id: ThemePreference; label: string }> = [
    { id: "system", label: "跟随系统" },
    { id: "light", label: "浅色" },
    { id: "dark", label: "深色" },
  ];

  function sizeLabel(bytes?: number): string {
    if (!bytes) return "0 MB";
    const gigabytes = bytes / 1_073_741_824;
    if (gigabytes >= 1) return `${gigabytes.toFixed(2)} GB`;
    return `${Math.max(1, Math.round(bytes / 1_048_576))} MB`;
  }
</script>

<section class="content">
  <div class="set">
    <div class="group">
      <div class="label">通用</div>
      <div class="row">
        <div>
          <div class="name">主题</div>
          <div class="desc">跟随系统时，浅色与深色随外观自动切换</div>
        </div>
        <div class="seg" role="radiogroup" aria-label="主题">
          {#each options as option (option.id)}
            <button
              type="button"
              role="radio"
              aria-checked={theme.preference === option.id}
              class:on={theme.preference === option.id}
              onclick={() => (theme.preference = option.id)}
            >
              {option.label}
            </button>
          {/each}
        </div>
      </div>
    </div>

    <div class="group">
      <div class="label">转写</div>
      <div class="row">
        <div>
          <div class="name">默认模型</div>
          <div class="desc">只影响之后新建的任务</div>
        </div>
        <div class="value">
          <span class:empty={!model}>{model?.displayName ?? "未选择模型"}</span>
          <button type="button" class="link" onclick={onGoModels}>管理模型</button>
        </div>
      </div>
      <div class="row">
        <div>
          <div class="name">语言</div>
          <div class="desc">由模型自动检测，结果保留原语言</div>
        </div>
        <div class="value">自动检测</div>
      </div>
    </div>

    <div class="group">
      <div class="label">存储</div>
      <div class="row">
        <div class="grow">
          <div class="name">模型目录</div>
          <div class="path" title={storage?.libraryPath ?? ""}>
            {storage?.libraryPath ?? "—"}
          </div>
        </div>
        <div class="value">
          <span>{storage?.libraryCount ?? 0} 个 · {sizeLabel(storage?.libraryBytes)}</span>
          <button type="button" class="link" onclick={onOpenLibrary} disabled={!storage}>打开</button>
        </div>
      </div>
      <div class="row">
        <div class="grow">
          <div class="name">历史结果</div>
          <div class="path" title={storage?.historyPath ?? ""}>
            {storage?.historyPath ?? "—"}
          </div>
        </div>
        <div class="value">
          <span>{storage?.historyCount ?? 0} 条 · {sizeLabel(storage?.historyBytes)}</span>
          <button type="button" class="link" onclick={onOpenHistory} disabled={!storage}>打开</button>
        </div>
      </div>
      <p class="note">迁移模型目录需要先做空间预检与回滚方案，尚未开放；删除模型只会移除应用内副本。</p>
    </div>

    <div class="group">
      <div class="label">集成</div>
      <div class="row">
        <div class="grow">
          <div class="name">本地服务</div>
          <div class="desc">
            {service?.running ? `运行中 · pid ${service.pid}` : "未运行：CLI 或 MCP 调用时会自动启动"}
          </div>
          <div class="path" title={service?.socketPath ?? ""}>{service?.socketPath ?? "—"}</div>
        </div>
        <div class="value">
          <span class="status-dot" class:on={service?.running}></span>
          <button type="button" class="link" onclick={onRefreshService}>刷新</button>
        </div>
      </div>
      <div class="row">
        <div class="grow">
          <div class="name">MCP（stdio）</div>
          <div class="desc">9 个工具：模型、提交、状态、结果、队列、取消、重试、导出</div>
        </div>
        <div class="value">
          <button type="button" class="link" onclick={() => copy(mcpConfig, "mcp")}>
            {copied === "mcp" ? "已复制" : "复制配置"}
          </button>
        </div>
      </div>
      <pre class="snippet">{mcpConfig}</pre>
      <div class="row">
        <div class="grow">
          <div class="name">CLI</div>
          <div class="desc">与 MCP 共用同一服务与队列</div>
        </div>
        <div class="value">
          <button type="button" class="link" onclick={() => copy(cliExample, "cli")}>
            {copied === "cli" ? "已复制" : "复制示例"}
          </button>
        </div>
      </div>
      <pre class="snippet">{cliExample}</pre>
      <p class="note">
        故障排查：服务只监听本机 Unix socket（权限 0600）；文件访问需在启动时用
        <code>--allow-root</code> 授权；日志在终端输出，不记录音频内容。
      </p>
    </div>

    <div class="group">
      <div class="label">关于</div>
      <div class="row">
        <div>
          <div class="name">Local Transcribe</div>
          <div class="desc">本地音视频转写 · 预览版本 0.1.0</div>
        </div>
        <div class="value">未签名开发构建</div>
      </div>
    </div>
  </div>
</section>

<style>
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 2px 28px 20px;
  }

  .set {
    width: 100%;
    max-width: 648px;
    margin: 0 auto;
    padding: 0 0 16px;
  }

  .group {
    margin-top: 11px;
  }

  .group:first-child {
    margin-top: 4px;
  }

  .label {
    font-size: 11px;
    color: var(--text-3);
    letter-spacing: 0.03em;
    padding: 0 2px 2px;
  }

  .row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    padding: 6.5px 2px;
    font-size: 12.5px;
  }

  .grow {
    min-width: 0;
    flex: 1;
  }

  .name {
    font-size: 13px;
  }

  .desc {
    font-size: 11px;
    color: var(--text-3);
    margin-top: 1px;
  }

  .path {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-3);
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .value {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-2);
    /* 窄窗口下允许收缩与折行，长模型 id 不再把行顶出窗口 */
    flex: 0 1 auto;
    min-width: 0;
    overflow-wrap: anywhere;
    max-width: 100%;
  }

  .value .empty {
    color: var(--text-3);
  }

  .link {
    border: 0;
    background: none;
    padding: 0;
    font-size: 12px;
    color: var(--accent-ink);
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-3);
    opacity: 0.6;
  }

  .status-dot.on {
    background: var(--green);
    opacity: 1;
  }

  .snippet {
    margin: 2px 0 10px;
    padding: 10px 12px;
    border-radius: 8px;
    background: var(--sunken);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.6;
    color: var(--text-2);
    overflow-x: auto;
    white-space: pre;
  }

  .link:disabled {
    color: var(--text-3);
  }

  .seg {
    margin-left: auto;
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 7px;
    background: var(--sunken);
  }

  .seg button {
    padding: 3px 9px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    font-size: 11.5px;
    color: var(--text-2);
  }

  .seg button.on {
    background: var(--panel);
    color: var(--text);
    box-shadow: 0 1px 1px rgba(0, 0, 0, 0.08);
  }

  :global(:root[data-theme="dark"]) .seg button.on {
    background: #3a3a3e;
  }

  .note {
    margin: 8px 2px 0;
    font-size: 11px;
    line-height: 1.7;
    color: var(--text-3);
  }
</style>
