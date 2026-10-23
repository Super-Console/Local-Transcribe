<script lang="ts">
  import { theme, type ThemePreference } from "$lib/theme.svelte";
  import {
    locale,
    t,
    LOCALE_OPTIONS,
    LOCALE_NAMES,
    type LocalePreference,
  } from "$lib/i18n.svelte";
  import type {
    LanMode,
    ModelSummary,
    RetentionPolicy,
    ServiceStatus,
    StorageSummary,
  } from "$lib/types";

  let {
    model,
    storage,
    service,
    appVersion,
    retention,
    lanMode,
    onOpenLibrary,
    onOpenHistory,
    onOpenPath,
    onGoModels,
    onGoLan,
    onRefreshService,
    onRetentionChange,
  }: {
    model: ModelSummary | null;
    storage: StorageSummary | null;
    service: ServiceStatus | null;
    appVersion: string;
    retention: RetentionPolicy | null;
    lanMode: LanMode | null;
    onOpenLibrary: () => void;
    onOpenHistory: () => void;
    onOpenPath: (path: string) => void;
    onGoModels: () => void;
    onGoLan: () => void;
    onRefreshService: () => void;
    onRetentionChange: (policy: RetentionPolicy) => void;
  } = $props();

  const LAN_MODE_LABELS = $derived.by(() => {
    const labels: Record<LanMode, string> = {
      local_only: t("lan.modeLocalOnly"),
      prefer_lan: t("lan.modePreferLan"),
    };
    return labels;
  });

  const AGE_OPTIONS = [1, 3, 5, 7, 30];
  const SIZE_OPTIONS = [1, 5, 10];
  const GiB = 1_073_741_824;

  import { onDestroy } from "svelte";

  let copied = $state<"none" | "mcp" | "cli">("none");
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => {
    if (copyTimer) clearTimeout(copyTimer);
  });

  const mcpConfig = $derived(`{
  "mcpServers": {
    "local-transcribe": {
      "command": "${t("settings.mcpCommandPlaceholder")}",
      "args": ["--allow-root", "${storage?.libraryPath ? storage.libraryPath.replace(/\/models$/, "") : t("settings.mcpPathPlaceholder")}"]
    }
  }
}`);

  const cliExample = $derived(`local-transcribe models list
local-transcribe jobs add "${t("settings.cliMediaPlaceholder")}" --model ${t("settings.cliModelPlaceholder")}
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

  const options: Array<{ id: ThemePreference; label: string }> = $derived([
    { id: "system", label: t("common.system") },
    { id: "light", label: t("settings.themeLight") },
    { id: "dark", label: t("settings.themeDark") },
  ]);

  function sizeLabel(bytes?: number): string {
    if (!bytes) return "0 MB";
    const gigabytes = bytes / 1_073_741_824;
    if (gigabytes >= 1) return `${gigabytes.toFixed(2)} GB`;
    return `${Math.max(1, Math.round(bytes / 1_048_576))} MB`;
  }
</script>

<section class="content">
  <h1 class="page-title">{t("nav.settings")}</h1>
  <div class="set">
    <div class="group">
      <div class="label">{t("settings.general")}</div>
      <div class="row">
        <div>
          <div class="name">{t("settings.themeName")}</div>
          <div class="desc">{t("settings.themeDesc")}</div>
        </div>
        <div class="seg" role="radiogroup" aria-label={t("settings.themeName")}>
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
      <div class="row">
        <div>
          <div class="name">{t("settings.languageName")}</div>
          <div class="desc">{t("settings.languageDesc")}</div>
        </div>
        <div class="value">
          <select
            class="locale-pick"
            aria-label={t("settings.languageName")}
            value={locale.preference}
            onchange={(event) => {
              locale.preference = event.currentTarget.value as LocalePreference;
            }}
          >
            {#each LOCALE_OPTIONS as option (option)}
              <option value={option}>
                {option === "system" ? t("common.system") : LOCALE_NAMES[option]}
              </option>
            {/each}
          </select>
        </div>
      </div>
    </div>

    <div class="group">
      <div class="label">{t("settings.transcription")}</div>
      <div class="row">
        <div>
          <div class="name">{t("toolbar.defaultModel")}</div>
          <div class="desc">{t("settings.defaultModelDesc")}</div>
        </div>
        <div class="value">
          <span class:empty={!model}>{model?.displayName ?? t("common.noModel")}</span>
          <button type="button" class="link" onclick={onGoModels}>{t("settings.manageModels")}</button>
        </div>
      </div>
      <div class="row">
        <div>
          <div class="name">{t("settings.audioLangName")}</div>
          <div class="desc">{t("settings.audioLangDesc")}</div>
        </div>
        <div class="value">{t("settings.audioLangValue")}</div>
      </div>
    </div>

    <div class="group">
      <div class="label">{t("settings.storage")}</div>
      <div class="row">
        <div class="grow">
          <div class="name">{t("settings.libraryDir")}</div>
          <div class="path" title={storage?.libraryPath ?? ""}>
            {storage?.libraryPath ?? "—"}
          </div>
        </div>
        <div class="value">
          <span>{t("settings.countModels", { count: storage?.libraryCount ?? 0, size: sizeLabel(storage?.libraryBytes) })}</span>
          <button type="button" class="link" onclick={onOpenLibrary} disabled={!storage}>{t("common.open")}</button>
        </div>
      </div>
      <div class="row">
        <div class="grow">
          <div class="name">{t("settings.historyDir")}</div>
          <div class="path" title={storage?.historyPath ?? ""}>
            {storage?.historyPath ?? "—"}
          </div>
        </div>
        <div class="value">
          <span>{t("settings.countHistory", { count: storage?.historyCount ?? 0, size: sizeLabel(storage?.historyBytes) })}</span>
          <button type="button" class="link" onclick={onOpenHistory} disabled={!storage}>{t("common.open")}</button>
        </div>
      </div>
      <div class="row">
        <div class="grow">
          <div class="name">{t("settings.jobsDir")}</div>
          <div class="path" title={storage?.jobsPath ?? ""}>{storage?.jobsPath ?? "—"}</div>
        </div>
        <div class="value">
          <span>{t("settings.countHistory", { count: storage?.jobsCount ?? 0, size: sizeLabel(storage?.jobsBytes) })}</span>
          <button
            type="button"
            class="link"
            onclick={() => storage && onOpenPath(storage.jobsPath)}
            disabled={!storage}>{t("common.open")}</button>
        </div>
      </div>
      <div class="row">
        <div class="grow">
          <div class="name">{t("settings.dataDir")}</div>
          <div class="path" title={storage?.dataPath ?? ""}>{storage?.dataPath ?? "—"}</div>
        </div>
        <div class="value">
          <button
            type="button"
            class="link"
            onclick={() => storage && onOpenPath(storage.dataPath)}
            disabled={!storage}>{t("common.open")}</button>
        </div>
      </div>
      <div class="row">
        <div class="grow">
          <div class="name">{t("settings.retentionName")}</div>
          <div class="desc">{t("settings.retentionDesc")}</div>
        </div>
        <div class="value retention">
          <select
            class="locale-pick"
            aria-label={t("settings.retentionName")}
            value={retention?.mode ?? "off"}
            onchange={(event) =>
              onRetentionChange({
                ...(retention ?? { mode: "off", ageDays: 7, maxBytes: GiB }),
                mode: event.currentTarget.value as RetentionPolicy["mode"],
              })}
          >
            <option value="off">{t("settings.retentionOff")}</option>
            <option value="age">{t("settings.retentionAge")}</option>
            <option value="size">{t("settings.retentionSize")}</option>
          </select>
          {#if retention && retention.mode === "age"}
            <select
              class="locale-pick"
              aria-label={t("settings.retentionAge")}
              value={String(retention.ageDays)}
              onchange={(event) =>
                onRetentionChange({
                  ...retention,
                  ageDays: Number(event.currentTarget.value),
                })}
            >
              {#each AGE_OPTIONS as days (days)}
                <option value={String(days)}>{t("settings.retentionDays", { days })}</option>
              {/each}
            </select>
          {:else if retention && retention.mode === "size"}
            <select
              class="locale-pick"
              aria-label={t("settings.retentionSize")}
              value={String(retention.maxBytes)}
              onchange={(event) =>
                onRetentionChange({
                  ...retention,
                  maxBytes: Number(event.currentTarget.value),
                })}
            >
              {#each SIZE_OPTIONS as gib (gib)}
                <option value={String(gib * GiB)}>{gib} GB</option>
              {/each}
            </select>
          {/if}
        </div>
      </div>
      <p class="note">{t("settings.chunkNote")}</p>
      <p class="note">{t("settings.storageNote")}</p>
    </div>

    <div class="group">
      <div class="label">{t("settings.network")}</div>
      <div class="row">
        <div class="grow">
          <div class="name">{t("lan.modeLabel")}</div>
          <div class="desc">{t("settings.networkDesc")}</div>
        </div>
        <div class="value">
          <span>{lanMode ? LAN_MODE_LABELS[lanMode] : "—"}</span>
          <button type="button" class="link" onclick={onGoLan}>{t("settings.openLan")}</button>
        </div>
      </div>
    </div>

    <div class="group">
      <div class="label">{t("settings.integration")}</div>
      <div class="row">
        <div class="grow">
          <div class="name">{t("settings.serviceName")}</div>
          <div class="desc">
            {service?.running ? t("settings.serviceRunning", { pid: service.pid ?? "—" }) : t("settings.serviceStopped")}
          </div>
          <div class="path" title={service?.socketPath ?? ""}>{service?.socketPath ?? "—"}</div>
        </div>
        <div class="value">
          <span class="status-dot" class:on={service?.running}></span>
          <button type="button" class="link" onclick={onRefreshService}>{t("settings.refresh")}</button>
        </div>
      </div>
      <div class="row">
        <div class="grow">
          <div class="name">{t("settings.mcpName")}</div>
          <div class="desc">{t("settings.mcpDesc")}</div>
        </div>
        <div class="value">
          <button type="button" class="link" onclick={() => copy(mcpConfig, "mcp")}>
            {copied === "mcp" ? t("common.copied") : t("settings.copyConfig")}
          </button>
        </div>
      </div>
      <pre class="snippet">{mcpConfig}</pre>
      <div class="row">
        <div class="grow">
          <div class="name">CLI</div>
          <div class="desc">{t("settings.cliDesc")}</div>
        </div>
        <div class="value">
          <button type="button" class="link" onclick={() => copy(cliExample, "cli")}>
            {copied === "cli" ? t("common.copied") : t("settings.copyExample")}
          </button>
        </div>
      </div>
      <pre class="snippet">{cliExample}</pre>
      <p class="note">
        {t("settings.troubleIntro")}
        <code>--allow-root</code>{t("settings.troubleOutro")}
      </p>
    </div>

    <div class="group">
      <div class="label">{t("settings.about")}</div>
      <div class="row">
        <div>
          <div class="name">Local Transcribe</div>
          <div class="desc">{t("settings.aboutDesc", { version: appVersion })}</div>
        </div>
        <div class="value">{t("settings.aboutBuild")}</div>
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

  .page-title {
    width: 100%;
    max-width: 720px;
    margin: 10px auto 6px;
    font-family: var(--font-display);
    font-size: 34px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .set {
    width: 100%;
    max-width: 720px;
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
    font-size: 17px;
    font-weight: 600;
    color: var(--text);
    letter-spacing: -0.01em;
    padding: 0 2px 6px;
    margin-top: 6px;
  }

  .row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    padding: 14px 2px;
    font-size: 14px;
    border-top: 1px solid var(--hairline);
  }

  .grow {
    min-width: 0;
    flex: 1;
  }

  .name {
    font-size: 15px;
    font-weight: 500;
  }

  .desc {
    font-size: 13.5px;
    color: var(--text-3);
    margin-top: 2px;
    line-height: 1.45;
  }

  .path {
    font-family: var(--font-mono);
    font-size: 12px;
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
    font-size: 13px;
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
    font-size: 13px;
    color: var(--accent-ink);
  }

  .locale-pick {
    height: 30px;
    padding: 0 8px;
    border-radius: 8px;
    border: 0;
    background: var(--fill);
    color: var(--text);
    font-size: 13.5px;
    font-family: inherit;
    max-width: 190px;
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
    font-size: 12px;
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
    padding: 3px;
    border-radius: 9px;
    background: var(--fill);
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
