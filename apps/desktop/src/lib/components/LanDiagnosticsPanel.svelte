<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  import { t } from "$lib/i18n.svelte";
  import type { AppPermissionStatus, LanDiagnostics } from "$lib/types";

  let { onclose }: { onclose?: () => void } = $props();

  let diag = $state<LanDiagnostics | null>(null);
  let perms = $state<AppPermissionStatus | null>(null);
  let copied = $state(false);

  onMount(() => {
    void load();
  });

  async function load() {
    try {
      diag = await invoke<LanDiagnostics>("lan_diagnostics");
    } catch {
      diag = null;
    }
    try {
      perms = await invoke<AppPermissionStatus>("app_permissions");
    } catch {
      perms = null;
    }
  }

  function localLabel(value?: string): string {
    if (value === "granted") return t("permissions.granted");
    if (value === "denied") return t("permissions.denied");
    if (value === "no_network") return t("permissions.noNetwork");
    return t("permissions.unknown");
  }

  function microphoneLabel(value?: string): string {
    if (value === "authorized") return t("permissions.granted");
    if (value === "denied") return t("permissions.denied");
    if (value === "restricted") return t("permissions.restricted");
    if (value === "not_determined") return t("permissions.notDetermined");
    return t("permissions.unknown");
  }

  function screenLabel(value?: string): string {
    if (value === "granted") return t("permissions.granted");
    if (value === "denied") return t("permissions.denied");
    return t("permissions.unknown");
  }

  async function openPane(pane: string) {
    try {
      if (pane === "local_network") {
        await invoke("open_local_network_settings");
      } else {
        await invoke("open_privacy_settings", { pane });
      }
    } catch {
      /* the settings app is optional */
    }
  }

  const permissionRows = $derived.by(() => [
    {
      label: t("permissions.localNetwork"),
      state: localLabel(diag?.access),
      ok: diag?.access === "granted",
      pane: "local_network",
    },
    {
      label: t("permissions.microphone"),
      state: microphoneLabel(perms?.microphone),
      ok: perms?.microphone === "authorized",
      pane: "microphone",
    },
    {
      label: t("permissions.screenCapture"),
      state: screenLabel(perms?.screenCapture),
      ok: perms?.screenCapture === "granted",
      pane: "screen_capture",
    },
  ]);

  function verdictText(verdict: LanDiagnostics["verdict"]): string {
    if (verdict === "ok") return t("lan.diagVerdictOk");
    if (verdict === "multicast_blocked") return t("lan.diagVerdictMulticast");
    if (verdict === "no_network") return t("lan.diagVerdictNoNetwork");
    return t("lan.diagVerdictPermission");
  }

  async function copy() {
    if (!diag) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(diag, null, 2));
      copied = true;
    } catch {
      copied = false;
    }
  }
</script>

<div class="diag" role="dialog" aria-label={t("lan.diagTitle")}>
  <div class="diag-head">
    <b>{t("lan.diagTitle")}</b>
    {#if onclose}
      <button type="button" class="act" onclick={onclose}>{t("lan.close")}</button>
    {/if}
  </div>
  {#if diag}
    <p class="diag-line">
      {t("lan.diagAccess")}: <code>{diag.access}</code>{#if diag.errno !== null} · errno {diag.errno}{/if}
    </p>
    <p class="diag-line">
      {t("lan.diagTcp")}: <code>{diag.tcpResult}</code>{#if diag.gateway} · {diag.gateway}{/if}{#if diag.tcpErrno !== null} · errno {diag.tcpErrno}{/if}
    </p>
    <p class="diag-line">{t("lan.diagVerdict")}: <b>{verdictText(diag.verdict)}</b></p>
    {#if diag.verdict !== "ok"}
      <p class="diag-line">{t("lan.diagSteps")}</p>
    {/if}
    <p class="diag-line muted">{diag.identifier} · {diag.version}</p>
  {:else}
    <p class="diag-line">{t("lan.diagUnavailable")}</p>
  {/if}

  <div class="permissions">
    <div class="permissions-head">
      <b>{t("permissions.title")}</b>
      <button type="button" class="act" onclick={() => void load()}>
        {t("permissions.refresh")}
      </button>
    </div>
    {#each permissionRows as row}
      <div class="permission-row">
        <span class="permission-name">{row.label}</span>
        <span class:good={row.ok} class:bad={!row.ok}>{row.state}</span>
        <button type="button" class="act" onclick={() => void openPane(row.pane)}>
          {t("permissions.open")}
        </button>
      </div>
    {/each}
    <p class="diag-line muted">{t("permissions.note")}</p>
  </div>
  <div class="actions">
    <button type="button" class="btn primary" onclick={() => void invoke("restart_app")}>
      {t("lan.restartApp")}
    </button>
    <button type="button" class="btn" onclick={() => void invoke("open_local_network_settings")}>
      {t("lan.permissionAction")}
    </button>
    <button type="button" class="btn" onclick={() => void copy()} disabled={!diag}>
      {copied ? t("lan.diagCopied") : t("lan.copyDiag")}
    </button>
  </div>
</div>

<style>
  .diag {
    padding: 14px 16px;
    border-radius: 12px;
    background: color-mix(in srgb, var(--foreground) 5%, transparent);
  }

  .diag-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 6px;
  }

  .act {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font-size: 12px;
    cursor: pointer;
  }

  .diag-line {
    margin: 4px 0;
    font-size: 12.5px;
    line-height: 1.55;
  }

  .diag-line.muted {
    color: var(--text-3);
    word-break: break-all;
  }

  .diag code {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .permissions {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--hairline);
  }

  .permissions-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 4px;
  }

  .permission-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 2px 0;
    font-size: 12.5px;
  }

  .permission-name {
    flex: 1;
  }

  .permission-row .good {
    color: var(--green);
  }

  .permission-row .bad {
    color: var(--red);
  }

  .permissions .act {
    flex: none;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;
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
    font-size: 13px;
    color: var(--text);
    cursor: default;
  }

  .btn.primary {
    border-color: transparent;
    background: var(--accent);
    color: #fff;
    font-weight: 500;
  }

  .btn:disabled {
    opacity: 0.55;
  }
</style>
