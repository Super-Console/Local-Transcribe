<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AudioApp, RecordedTrack, RecordingStatus } from "$lib/types";

  let { onRecorded }: { onRecorded: (tracks: RecordedTrack[]) => void } = $props();

  let devices = $state<string[]>([]);
  let apps = $state<AudioApp[]>([]);
  let mic = $state("");
  let appPid = $state<number | null>(null);
  let running = $state(false);
  let elapsedMs = $state(0);
  let micLevel = $state(0);
  let appLevel = $state(0);
  let busy = $state(false);
  let error = $state("");

  function formatElapsed(ms: number): string {
    const total = Math.floor(ms / 1000);
    const minutes = Math.floor(total / 60);
    const seconds = total % 60;
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }

  async function loadSources() {
    try {
      const [nextDevices, nextApps] = await Promise.all([
        invoke<string[]>("list_audio_devices"),
        invoke<AudioApp[]>("list_audio_apps"),
      ]);
      devices = nextDevices;
      apps = nextApps;
      if (!mic && devices.length > 0) mic = devices[0];
    } catch (issue) {
      error = String(issue);
    }
  }

  $effect(() => {
    void loadSources();
  });

  // 录音中每 200ms 刷新计时与电平。
  $effect(() => {
    if (!running) return;
    const timer = window.setInterval(async () => {
      try {
        const status = await invoke<RecordingStatus>("recording_status");
        elapsedMs = status.elapsedMs;
        micLevel = status.micLevel;
        appLevel = status.appLevel;
        running = status.running;
        if (status.appError) error = status.appError;
      } catch {
        /* ignore transient failures */
      }
    }, 200);
    return () => window.clearInterval(timer);
  });

  async function toggle() {
    if (busy) return;
    busy = true;
    error = "";
    try {
      if (running) {
        const result = await invoke<{ tracks: RecordedTrack[] }>("stop_recording");
        running = false;
        if (result.tracks.length === 0) {
          error = t("record.empty");
        } else {
          onRecorded(result.tracks);
        }
      } else {
        await invoke("start_recording", {
          mic: mic || null,
          appPid: appPid,
        });
        running = true;
        elapsedMs = 0;
        micLevel = 0;
        appLevel = 0;
      }
    } catch (issue) {
      error = String(issue);
    } finally {
      busy = false;
    }
  }
</script>

<section class="content">
  <h1 class="page-title">{t("nav.record")}</h1>
  <p class="subtitle">{t("record.subtitle")}</p>

  {#if error}
    <div class="banner" role="alert">
      <Icon name="warn" size={14} />
      <span>{error}</span>
    </div>
  {/if}

  <div class="sources">
    <div class="source" class:active={micLevel > 0.02}>
      <div class="source-head">
        <Icon name="mic" size={16} stroke={1.5} />
        <span>{t("record.micLabel")}</span>
      </div>
      <select class="pick" bind:value={mic} disabled={running}>
        {#each devices as device (device)}
          <option value={device}>{device}</option>
        {/each}
      </select>
      <div class="meter"><i style:width={`${Math.round(micLevel * 100)}%`}></i></div>
    </div>

    <div class="source" class:active={appLevel > 0.02}>
      <div class="source-head">
        <Icon name="transcribe" size={16} stroke={1.5} />
        <span>{t("record.appLabel")}</span>
      </div>
      <select
        class="pick"
        disabled={running}
        value={appPid === null ? "" : String(appPid)}
        onchange={(event) =>
          (appPid = event.currentTarget.value ? Number(event.currentTarget.value) : null)}
      >
        <option value="">{t("record.appNone")}</option>
        {#each apps as app (app.pid)}
          <option value={String(app.pid)}>{app.name}</option>
        {/each}
      </select>
      <div class="meter"><i style:width={`${Math.round(appLevel * 100)}%`}></i></div>
    </div>
  </div>

  <p class="hint">{t("record.hint")}</p>

  <div class="transport">
    <button
      type="button"
      class="record-btn"
      class:running
      disabled={busy}
      onclick={toggle}
    >
      <span class="dot"></span>
      {running ? t("record.stop") : t("record.start")}
    </button>
    {#if running}
      <span class="timer">{formatElapsed(elapsedMs)}</span>
    {:else}
      <span class="timer idle">{t("record.idle")}</span>
    {/if}
    <div class="spacer"></div>
    {#if !running}
      <button type="button" class="link" onclick={() => void loadSources()}>
        {t("record.refresh")}
      </button>
    {/if}
  </div>

  <p class="note">{t("record.permission")}</p>
</section>

<style>
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 6px 28px 20px;
  }

  .page-title {
    width: 100%;
    max-width: 940px;
    margin: 10px auto 0;
    font-family: var(--font-display);
    font-size: 34px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .subtitle {
    width: 100%;
    max-width: 940px;
    margin: 8px auto 18px;
    font-size: 14.5px;
    color: var(--text-2);
  }

  .banner {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    max-width: 940px;
    margin: 0 auto 12px;
    padding: 9px 12px;
    border-radius: 9px;
    background: color-mix(in srgb, var(--red) 8%, transparent);
    color: var(--text-2);
    font-size: 13px;
  }

  .sources {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 12px;
    width: 100%;
    max-width: 940px;
    margin: 0 auto;
  }

  .source {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 18px 18px;
    border: 1px solid var(--hairline-2);
    border-radius: 12px;
    background: var(--card);
    transition: border-color 140ms ease;
  }

  .source.active {
    border-color: var(--accent);
  }

  .source-head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14.5px;
    font-weight: 600;
    color: var(--text);
  }

  .pick {
    height: 32px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: var(--fill);
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
    max-width: 100%;
  }

  .meter {
    height: 4px;
    border-radius: 2px;
    background: var(--track);
    overflow: hidden;
  }

  .meter i {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--green);
    transition: width 120ms linear;
  }

  .hint {
    width: 100%;
    max-width: 940px;
    margin: 12px auto 0;
    font-size: 12.5px;
    color: var(--text-3);
  }

  .transport {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    max-width: 940px;
    margin: 22px auto 0;
    padding: 14px 16px;
    border-radius: 12px;
    background: var(--fill);
  }

  .record-btn {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    height: 38px;
    padding: 0 18px;
    border: 0;
    border-radius: 10px;
    background: var(--text);
    color: var(--bg);
    font-size: 14px;
    font-weight: 500;
  }

  .record-btn .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--red);
  }

  .record-btn.running .dot {
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }

  .timer {
    font-size: 15px;
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
  }

  .timer.idle {
    color: var(--text-3);
    font-size: 13px;
  }

  .spacer {
    flex: 1;
  }

  .link {
    border: 0;
    background: none;
    padding: 0;
    font-size: 13px;
    color: var(--accent-ink);
  }

  .note {
    width: 100%;
    max-width: 940px;
    margin: 14px auto 0;
    font-size: 12px;
    line-height: 1.6;
    color: var(--text-3);
  }
</style>
