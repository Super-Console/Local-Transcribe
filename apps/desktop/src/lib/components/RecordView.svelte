<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AudioApp, RecordedTrack, RecordingStatus, TrackSpec } from "$lib/types";

  let { onRecorded }: { onRecorded: (tracks: RecordedTrack[]) => void } = $props();

  type Row = { id: number; key: string };
  type Source = { key: string; kind: "input" | "app"; target: string; label: string };

  let devices = $state<string[]>([]);
  let apps = $state<AudioApp[]>([]);
  let rows = $state<Row[]>([]);
  let running = $state(false);
  let elapsedMs = $state(0);
  let levels = $state<number[]>([]);
  let trackErrors = $state<string[]>([]);
  let busy = $state(false);
  let error = $state("");
  let nextId = 1;

  const sources = $derived<Source[]>([
    ...devices.map((name) => ({
      key: `input:${name}`,
      kind: "input" as const,
      target: name,
      label: name,
    })),
    ...apps.map((app) => ({
      key: `app:${app.pid}`,
      kind: "app" as const,
      target: String(app.pid),
      label: app.name,
    })),
  ]);

  function labelOf(key: string): string {
    return sources.find((source) => source.key === key)?.label ?? key;
  }

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
      // 默认给一条轨道；没有轨道时补一条可用的来源。
      if (rows.length === 0) addTrack();
      else rows = rows.map((row) => (row.key && sources.length > 0 ? row : row));
    } catch (issue) {
      error = String(issue);
    }
  }

  function addTrack() {
    const used = new Set(rows.map((row) => row.key));
    const free = sources.find((source) => !used.has(source.key));
    if (!free) return;
    rows = [...rows, { id: nextId++, key: free.key }];
  }

  function removeTrack(id: number) {
    rows = rows.filter((row) => row.id !== id);
  }

  function setTrackSource(id: number, key: string) {
    rows = rows.map((row) => (row.id === id ? { ...row, key } : row));
  }

  $effect(() => {
    void loadSources();
  });

  $effect(() => {
    if (!running) return;
    const timer = window.setInterval(async () => {
      try {
        const status = await invoke<RecordingStatus>("recording_status");
        elapsedMs = status.elapsedMs;
        levels = status.tracks.map((track) => track.level);
        trackErrors = status.tracks.map((track) => track.error ?? "");
        running = status.running;
        const firstError = trackErrors.find((message) => message.length > 0);
        if (firstError) error = firstError;
      } catch {
        /* transient */
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
        if (result.tracks.length === 0) error = t("record.empty");
        else onRecorded(result.tracks);
      } else {
        if (rows.length === 0) {
          error = t("record.noTracks");
          return;
        }
        const specs: TrackSpec[] = rows.map((row) => {
          const source = sources.find((item) => item.key === row.key);
          return {
            kind: source?.kind ?? "input",
            target: source?.target ?? "",
            label: source?.label ?? "",
          };
        });
        await invoke("start_recording", { tracks: specs });
        running = true;
        elapsedMs = 0;
        levels = specs.map(() => 0);
        trackErrors = specs.map(() => "");
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

  <div class="tracks">
    {#each rows as row, index (row.id)}
      <div class="track" class:hot={(levels[index] ?? 0) > 0.02}>
        <div class="track-head">
          <span class="index">{index + 1}</span>
          <select
            class="pick"
            disabled={running}
            value={row.key}
            onchange={(event) => setTrackSource(row.id, event.currentTarget.value)}
          >
            <optgroup label={t("record.groupInputs")}>
              {#each devices as device (device)}
                <option value={`input:${device}`}>{device}</option>
              {/each}
            </optgroup>
            <optgroup label={t("record.groupApps")}>
              {#each apps as app (app.pid)}
                <option value={`app:${app.pid}`}>{app.name}</option>
              {/each}
            </optgroup>
          </select>
          <button
            type="button"
            class="remove"
            aria-label={t("record.removeTrack")}
            title={t("record.removeTrack")}
            disabled={running}
            onclick={() => removeTrack(row.id)}
          >
            ×
          </button>
        </div>
        <div class="meter">
          <i style:width={`${Math.round((levels[index] ?? 0) * 100)}%`}></i>
        </div>
        {#if trackErrors[index]}
          <p class="track-error">{trackErrors[index]}</p>
        {/if}
      </div>
    {/each}

    {#if !running}
      <button type="button" class="add" onclick={addTrack}>
        <Icon name="plus" size={13} stroke={1.7} />
        {t("record.addTrack")}
      </button>
    {/if}
  </div>

  {#if rows.length === 0}
    <p class="hint">{t("record.noTracks")}</p>
  {:else}
    <p class="hint">{t("record.hint")}</p>
  {/if}

  <div class="transport">
    <button type="button" class="record-btn" class:running disabled={busy} onclick={toggle}>
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

  .tracks {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    max-width: 940px;
    margin: 0 auto;
  }

  .track {
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 13px 16px 14px;
    border: 1px solid var(--hairline-2);
    border-radius: 12px;
    background: var(--card);
    transition: border-color var(--speed-fast) var(--ease-out);
  }

  .track.hot {
    border-color: var(--accent);
  }

  .track-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .index {
    width: 20px;
    flex: none;
    font-size: 12px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }

  .pick {
    flex: 1;
    min-width: 0;
    height: 32px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: var(--fill);
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
  }

  .remove {
    flex: none;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-3);
    font-size: 17px;
    line-height: 1;
  }

  .remove:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
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
    transition: width var(--speed-fast) var(--ease-out);
  }

  .track-error {
    margin: 0;
    font-size: 12px;
    color: var(--red);
  }

  .add {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    align-self: flex-start;
    height: 34px;
    padding: 0 14px;
    border: 1px dashed var(--hairline-2);
    border-radius: 10px;
    background: transparent;
    color: var(--text-2);
    font-size: 13.5px;
  }

  .add:hover {
    border-color: var(--text-3);
    color: var(--text);
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
