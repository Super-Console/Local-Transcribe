<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { confirm, open } from "@tauri-apps/plugin-dialog";

  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n.svelte";
  import type { MessageKey } from "$lib/i18n/messages/zh";
  import type {
    LanCollectReport,
    LanMode,
    LanNodeConfig,
    LanNodeInfo,
    LanNodeStatus,
    LanProgressEvent,
    LanSettings,
    LanStorageInfo,
    LanTask,
    LanTaskResult,
    LanTaskState,
    Transcript,
  } from "$lib/types";

  let {
    settings,
    onSettingsSaved,
  }: {
    settings: LanSettings | null;
    onSettingsSaved: (settings: LanSettings) => void;
  } = $props();

  const LAN_PROGRESS_EVENT = "lan://progress";
  const MEDIA_EXTENSIONS = [
    "wav", "wave", "aiff", "aif", "caf", "mp3", "mp2", "mp1", "m4a",
    "mp4", "mov", "mkv", "ogg", "oga", "flac", "aac", "opus", "3gp", "webm",
  ];
  const STATE_LABELS: Record<LanTaskState, MessageKey> = {
    init: "lan.state.init",
    file_transferring: "lan.state.file_transferring",
    queued: "lan.state.queued",
    transcribing: "lan.state.transcribing",
    post_processing: "lan.state.post_processing",
    completed: "lan.state.completed",
    failed: "lan.state.failed",
    cancel_requested: "lan.state.cancel_requested",
    cancelled: "lan.state.cancelled",
  };

  const tauriAvailable = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  let error = $state("");
  let scanning = $state(false);
  let scanned = $state(false);
  let sending = $state(false);
  let collecting = $state(false);
  let testingId = $state<string | null>(null);
  let discovered = $state<LanNodeInfo[]>([]);
  let selectedId = $state<string | null>(null);
  let statuses = $state<Record<string, LanNodeStatus>>({});
  let storages = $state<Record<string, LanStorageInfo>>({});
  let tasks = $state<LanTask[]>([]);
  let tasksLoading = $state(false);
  let transfers = $state<Record<string, { sent: number; total: number }>>({});
  let result = $state<LanTaskResult | null>(null);
  let resultTaskId = $state<string | null>(null);
  let savedNote = $state("");
  let collectDir = $state("");
  let collectReport = $state<LanCollectReport | null>(null);
  let draft = $state({ id: "", name: "", host: "", port: "7878", token: "" });

  const nodes = $derived(settings?.nodes ?? []);
  const selectedNode = $derived(nodes.find((node) => node.id === selectedId) ?? null);

  const modes = $derived.by(() => {
    const list: Array<{ id: LanMode; label: string; desc: string }> = [
      { id: "local_only", label: t("lan.modeLocalOnly"), desc: t("lan.modeLocalOnlyDesc") },
      { id: "prefer_local", label: t("lan.modePreferLocal"), desc: t("lan.modePreferLocalDesc") },
      { id: "prefer_lan", label: t("lan.modePreferLan"), desc: t("lan.modePreferLanDesc") },
    ];
    return list;
  });

  function sizeLabel(bytes?: number): string {
    if (!bytes) return "0 MB";
    const gigabytes = bytes / 1_073_741_824;
    if (gigabytes >= 1) return `${gigabytes.toFixed(2)} GB`;
    return `${Math.max(1, Math.round(bytes / 1_048_576))} MB`;
  }

  function stateLabel(state: LanTaskState): string {
    return t(STATE_LABELS[state]);
  }

  async function saveNodes(nextNodes: LanNodeConfig[]): Promise<void> {
    const saved = await invoke<LanSettings>("set_lan_settings", {
      settings: { mode: settings?.mode ?? "local_only", nodes: nextNodes },
    });
    onSettingsSaved(saved);
  }

  async function changeMode(mode: LanMode) {
    error = "";
    try {
      const saved = await invoke<LanSettings>("set_lan_assignment", { mode });
      onSettingsSaved(saved);
    } catch (issue) {
      error = String(issue);
    }
  }

  async function scan() {
    error = "";
    scanning = true;
    try {
      discovered = await invoke<LanNodeInfo[]>("lan_discover", { timeoutMs: 3000 });
      scanned = true;
    } catch (issue) {
      error = String(issue);
    } finally {
      scanning = false;
    }
  }

  function fillDraft(node: LanNodeConfig) {
    draft = {
      id: node.id,
      name: node.name,
      host: node.host,
      port: String(node.port),
      token: node.token,
    };
  }

  function clearDraft() {
    draft = { id: "", name: "", host: "", port: "7878", token: "" };
  }

  async function submitDraft() {
    error = "";
    const port = Number(draft.port);
    if (
      draft.name.trim().length === 0 ||
      draft.host.trim().length === 0 ||
      !Number.isInteger(port) ||
      port < 1 ||
      port > 65535
    ) {
      error = t("lan.errorForm");
      return;
    }
    try {
      const node: LanNodeConfig = {
        id: draft.id,
        name: draft.name.trim(),
        host: draft.host.trim(),
        port,
        token: draft.token,
        enabled: draft.id === "" ? true : nodes.find((item) => item.id === draft.id)?.enabled ?? true,
      };
      const nextNodes = draft.id === ""
        ? [...nodes, node]
        : nodes.map((item) => (item.id === draft.id ? node : item));
      await saveNodes(nextNodes);
      clearDraft();
    } catch (issue) {
      error = String(issue);
    }
  }

  async function addDiscovered(info: LanNodeInfo) {
    error = "";
    try {
      await saveNodes([
        ...nodes,
        {
          id: "",
          name: info.name,
          host: info.host,
          port: info.port,
          token: "",
          enabled: true,
        },
      ]);
    } catch (issue) {
      error = String(issue);
    }
  }

  async function removeNode(node: LanNodeConfig) {
    const approved = await confirm(t("lan.removeMessage", { name: node.name }), {
      title: t("lan.removeTitle"),
      kind: "warning",
      okLabel: t("common.delete"),
      cancelLabel: t("common.cancel"),
    });
    if (!approved) return;
    error = "";
    try {
      await saveNodes(nodes.filter((item) => item.id !== node.id));
      if (selectedId === node.id) {
        selectedId = null;
        tasks = [];
      }
    } catch (issue) {
      error = String(issue);
    }
  }

  async function toggleNode(node: LanNodeConfig) {
    error = "";
    try {
      await saveNodes(
        nodes.map((item) => (item.id === node.id ? { ...item, enabled: !item.enabled } : item)),
      );
    } catch (issue) {
      error = String(issue);
    }
  }

  async function loadStatus(node: LanNodeConfig) {
    try {
      const status = await invoke<LanNodeStatus>("lan_node_status", { nodeId: node.id });
      statuses = { ...statuses, [node.id]: status };
      if (status.reachable) {
        const storage = await invoke<LanStorageInfo>("lan_storage", { nodeId: node.id });
        storages = { ...storages, [node.id]: storage };
      }
    } catch {
      statuses = { ...statuses, [node.id]: { reachable: false, capabilities: null, latency_ms: null } };
    }
  }

  function refreshStatuses() {
    if (!tauriAvailable) return;
    for (const node of nodes) {
      void loadStatus(node);
    }
  }

  async function testNode(node: LanNodeConfig) {
    error = "";
    testingId = node.id;
    try {
      await loadStatus(node);
    } finally {
      testingId = null;
    }
  }

  async function loadTasks() {
    if (!tauriAvailable || !selectedId) return;
    tasksLoading = tasks.length === 0;
    try {
      tasks = await invoke<LanTask[]>("lan_tasks", { nodeId: selectedId });
    } catch (issue) {
      error = String(issue);
    } finally {
      tasksLoading = false;
    }
  }

  function selectNode(node: LanNodeConfig) {
    selectedId = node.id;
    tasks = [];
    result = null;
    resultTaskId = null;
    savedNote = "";
    collectReport = null;
    void loadTasks();
  }

  async function sendFile() {
    if (!selectedId) return;
    error = "";
    try {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: t("dialog.mediaFilter"), extensions: MEDIA_EXTENSIONS }],
      });
      if (typeof picked !== "string") return;
      sending = true;
      await invoke<string>("lan_submit_file", { nodeId: selectedId, filePath: picked, modelId: null });
      await loadTasks();
    } catch (issue) {
      error = String(issue);
    } finally {
      sending = false;
    }
  }

  async function cancelTask(task: LanTask) {
    if (!selectedId) return;
    error = "";
    try {
      await invoke<LanTask>("lan_cancel_task", { nodeId: selectedId, taskId: task.id });
      await loadTasks();
    } catch (issue) {
      error = String(issue);
    }
  }

  async function fetchResult(task: LanTask) {
    if (!selectedId) return;
    error = "";
    savedNote = "";
    try {
      result = await invoke<LanTaskResult>("lan_fetch_result", { nodeId: selectedId, taskId: task.id });
      resultTaskId = task.id;
    } catch (issue) {
      error = String(issue);
    }
  }

  async function saveResult() {
    if (!selectedId || !resultTaskId) return;
    error = "";
    try {
      await invoke<Transcript>("lan_save_result", { nodeId: selectedId, taskId: resultTaskId });
      savedNote = t("lan.resultSaved");
    } catch (issue) {
      error = String(issue);
    }
  }

  async function runCollect() {
    if (!selectedId) return;
    error = "";
    collecting = true;
    collectReport = null;
    try {
      collectReport = await invoke<LanCollectReport>("lan_collect", {
        nodeId: selectedId,
        outputDir: collectDir,
      });
    } catch (issue) {
      error = String(issue);
    } finally {
      collecting = false;
    }
  }

  onMount(() => {
    if (!tauriAvailable) return;
    refreshStatuses();
    const unlisteners: Array<Promise<() => void>> = [
      listen<LanProgressEvent>(LAN_PROGRESS_EVENT, (event) => {
        const payload = event.payload;
        transfers = {
          ...transfers,
          [payload.task_id]: { sent: payload.sent, total: payload.total },
        };
      }),
    ];
    // 任务状态 2 秒一轮（页面可见时才发请求）；节点在线状态 10 秒一轮。
    const taskTimer = window.setInterval(() => {
      if (document.visibilityState !== "visible") return;
      void loadTasks();
    }, 2000);
    const statusTimer = window.setInterval(() => {
      if (document.visibilityState !== "visible") return;
      refreshStatuses();
    }, 10000);
    return () => {
      window.clearInterval(taskTimer);
      window.clearInterval(statusTimer);
      for (const pending of unlisteners) {
        void pending.then((stop) => stop()).catch(() => {});
      }
    };
  });
</script>

<section class="content">
  <div class="title-row">
    <h1 class="page-title">{t("lan.title")}</h1>
    <span class="temp">{t("lan.tempUi")}</span>
  </div>
  <p class="subtitle">{t("lan.subtitle")}</p>

  {#if error}
    <div class="banner" role="alert">
      <Icon name="warn" size={14} />
      <span>{error}</span>
    </div>
  {/if}

  <div class="group">
    <div class="group-head">
      <div class="group-title">{t("lan.modeLabel")}</div>
    </div>
    <div class="seg" role="radiogroup" aria-label={t("lan.modeLabel")}>
      {#each modes as option (option.id)}
        <button
          type="button"
          role="radio"
          aria-checked={settings?.mode === option.id}
          class:on={settings?.mode === option.id}
          onclick={() => void changeMode(option.id)}
        >
          {option.label}
        </button>
      {/each}
    </div>
    {#each modes as option (option.id)}
      {#if settings?.mode === option.id}
        <p class="note">{option.desc}</p>
      {/if}
    {/each}
  </div>

  <div class="group">
    <div class="group-head">
      <div class="group-title">{t("lan.nodesTitle")}</div>
      <button type="button" class="btn" onclick={() => void scan()} disabled={scanning}>
        <Icon name="search" size={12} stroke={1.8} />
        {scanning ? t("lan.scanning") : t("lan.scan")}
      </button>
    </div>

    {#if discovered.length > 0}
      <div class="list">
        <div class="list-label">{t("lan.scanTitle")}</div>
        {#each discovered as info (`${info.host}:${info.port}`)}
          <div class="row">
            <div class="body">
              <div class="name">{info.name}</div>
              <div class="meta">
                <span>{info.host}:{info.port}</span>
                <span class="sep">·</span>
                <span>{info.compute_enabled ? t("lan.computeOn") : t("lan.computeOff")}</span>
                {#if info.model_id}
                  <span class="sep">·</span>
                  <span>{info.model_id}</span>
                {/if}
              </div>
            </div>
            <div class="row-actions">
              <button type="button" class="act" onclick={() => void addDiscovered(info)}>
                {t("lan.addDiscovered")}
              </button>
            </div>
          </div>
        {/each}
      </div>
    {:else if scanned}
      <p class="note">{t("lan.scanEmpty")}</p>
    {/if}

    <div class="form">
      <div class="form-title">{draft.id === "" ? t("lan.addNode") : t("lan.editNode")}</div>
      <div class="form-grid">
        <label class="field">
          <span>{t("lan.nodeFormName")}</span>
          <input type="text" bind:value={draft.name} placeholder={t("lan.nodeFormName")} />
        </label>
        <label class="field">
          <span>{t("lan.nodeFormHost")}</span>
          <input type="text" bind:value={draft.host} placeholder="REDACTED-LAN-IP" />
        </label>
        <label class="field small">
          <span>{t("lan.nodeFormPort")}</span>
          <input type="text" inputmode="numeric" bind:value={draft.port} placeholder="7878" />
        </label>
        <label class="field">
          <span>{t("lan.nodeFormToken")}</span>
          <input type="password" bind:value={draft.token} placeholder="••••••" />
        </label>
      </div>
      <div class="form-actions">
        <button type="button" class="btn primary" onclick={() => void submitDraft()}>
          {draft.id === "" ? t("lan.saveNode") : t("lan.saveChanges")}
        </button>
        {#if draft.id !== ""}
          <button type="button" class="btn" onclick={() => clearDraft()}>{t("common.cancel")}</button>
        {/if}
      </div>
    </div>

    {#if nodes.length === 0}
      <p class="note">{t("lan.emptyNodes")}</p>
    {:else}
      <div class="list">
        {#each nodes as node (node.id)}
          {@const status = statuses[node.id]}
          {@const storage = storages[node.id]}
          <div class="row card" class:selected={node.id === selectedId}>
            <button type="button" class="pick" onclick={() => selectNode(node)}>
              <div class="name">
                <span class="dot" class:on={status?.reachable}></span>
                {node.name}
                <span class="tag">{node.enabled ? t("lan.enabled") : t("lan.disabled")}</span>
              </div>
              <div class="meta">
                <span>{node.host}:{node.port}</span>
                <span class="sep">·</span>
                <span>
                  {status === undefined
                    ? t("lan.statusUnknown")
                    : status.reachable
                      ? t("lan.statusOnline")
                      : t("lan.statusOffline")}
                </span>
                {#if status?.latency_ms !== undefined && status?.latency_ms !== null}
                  <span class="sep">·</span>
                  <span>{t("lan.latency", { ms: status.latency_ms })}</span>
                {/if}
                {#if status?.capabilities}
                  <span class="sep">·</span>
                  <span>
                    {status.capabilities.compute_enabled ? t("lan.computeOn") : t("lan.computeOff")}
                  </span>
                  {#if status.capabilities.model_id}
                    <span class="sep">·</span>
                    <span>{status.capabilities.model_id}</span>
                  {/if}
                  <span class="sep">·</span>
                  <span>{t("lan.roleLabel")} {status.capabilities.role}</span>
                {/if}
                {#if storage}
                  <span class="sep">·</span>
                  <span>
                    {t("settings.storage")}
                    {t("lan.storageValue", { free: sizeLabel(storage.free_bytes), total: sizeLabel(storage.total_bytes) })}
                  </span>
                {/if}
              </div>
            </button>
            <div class="row-actions">
              <button
                type="button"
                class="act"
                onclick={() => void testNode(node)}
                disabled={testingId === node.id}
              >
                {testingId === node.id ? t("lan.testing") : t("lan.test")}
              </button>
              <button type="button" class="act" onclick={() => fillDraft(node)}>{t("lan.editNode")}</button>
              <button
                type="button"
                class="act"
                aria-label={t("lan.toggleEnable")}
                onclick={() => void toggleNode(node)}
              >
                {node.enabled ? t("lan.disabled") : t("lan.enabled")}
              </button>
              <button type="button" class="act danger" onclick={() => void removeNode(node)}>
                {t("common.delete")}
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div class="group">
    <div class="group-head">
      <div class="group-title">
        {t("lan.tasksTitle")}
        {#if selectedNode}
          <span class="sub">{selectedNode.name}</span>
        {/if}
      </div>
      {#if selectedNode}
        <button type="button" class="btn primary" onclick={() => void sendFile()} disabled={sending}>
          <Icon name="plus" size={12} stroke={1.8} />
          {sending ? t("lan.sending") : t("lan.sendFile")}
        </button>
      {/if}
    </div>

    {#if !selectedNode}
      <p class="note">{t("lan.noNodeSelected")}</p>
    {:else if tasks.length === 0}
      <p class="note">{tasksLoading ? t("history.loading") : t("lan.noTasks")}</p>
    {:else}
      <div class="table" role="table" aria-label={t("lan.tasksTitle")}>
        <div class="tr th" role="row">
          <div class="td" role="columnheader">{t("lan.taskFile")}</div>
          <div class="td" role="columnheader">{t("lan.taskState")}</div>
          <div class="td" role="columnheader">{t("lan.taskProgress")}</div>
          <div class="td right" role="columnheader">{t("lan.taskActions")}</div>
        </div>
        {#each tasks as task (task.id)}
          <div class="tr" role="row">
            <div class="td file" role="cell" title={task.file_name}>{task.file_name}</div>
            <div class="td" role="cell">
              <span class="badge {task.state}">{stateLabel(task.state)}</span>
              {#if task.error}
                <div class="err" title={task.error.message}>{task.error.message}</div>
              {/if}
            </div>
            <div class="td" role="cell">
              {#if task.state === "file_transferring" && transfers[task.id]}
                {t("lan.transferred", {
                  sent: sizeLabel(transfers[task.id].sent),
                  total: sizeLabel(transfers[task.id].total),
                })}
              {:else}
                {t("lan.transferred", {
                  sent: sizeLabel(task.received_bytes),
                  total: sizeLabel(task.file_size),
                })}
              {/if}
            </div>
            <div class="td right" role="cell">
              {#if !["completed", "failed", "cancelled"].includes(task.state) && task.state !== "cancel_requested"}
                <button type="button" class="act" onclick={() => void cancelTask(task)}>
                  {t("lan.actionCancel")}
                </button>
              {/if}
              {#if task.state === "completed"}
                <button type="button" class="act" onclick={() => void fetchResult(task)}>
                  {t("lan.actionFetch")}
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}

    {#if selectedNode}
      <div class="collect">
        <label class="field grow">
          <span>{t("lan.collectLabel")}</span>
          <input type="text" bind:value={collectDir} placeholder={storages[selectedNode.id]?.data_dir ?? "/path/on/node"} />
        </label>
        <button type="button" class="btn" onclick={() => void runCollect()} disabled={collecting}>
          <Icon name="download" size={12} stroke={1.8} />
          {collecting ? t("lan.collecting") : t("lan.collect")}
        </button>
      </div>
      {#if collectReport}
        <p class="note">
          {t("lan.collectReport", { ok: collectReport.exported.length, failed: collectReport.failed.length })}
        </p>
      {/if}
    {/if}

    {#if result}
      <div class="result">
        <div class="form-title">{t("lan.resultTitle")}</div>
        <pre class="snippet">{result.transcript}</pre>
        <div class="form-actions">
          <button type="button" class="btn primary" onclick={() => void saveResult()}>
            {t("lan.actionSave")}
          </button>
          {#if savedNote}
            <span class="note inline">{savedNote}</span>
          {/if}
        </div>
      </div>
    {/if}
  </div>
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
    gap: 12px;
    width: 100%;
    max-width: 940px;
    margin: 10px auto 0;
  }

  .page-title {
    margin: 0;
    font-family: var(--font-display);
    font-size: 34px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  /* 临时 UI 标注：视觉设计未定稿（macOS 设计未冻结是产品约束） */
  .temp {
    margin-left: auto;
    padding: 3px 9px;
    border-radius: 999px;
    background: var(--chip);
    color: var(--text-3);
    font-size: 11px;
    white-space: nowrap;
  }

  .subtitle {
    width: 100%;
    max-width: 940px;
    margin: 4px auto 12px;
    font-size: 13.5px;
    color: var(--text-2);
  }

  .banner {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    max-width: 940px;
    margin: 2px auto 6px;
    padding: 8px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--red) 7%, transparent);
    color: var(--text-2);
    font-size: 12px;
  }

  .group {
    width: 100%;
    max-width: 940px;
    margin: 18px auto 0;
  }

  .group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }

  .group-title {
    font-size: 17px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .group-title .sub {
    margin-left: 8px;
    font-size: 13px;
    font-weight: 400;
    color: var(--text-3);
  }

  .seg {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: 9px;
    background: var(--fill);
  }

  .seg button {
    padding: 5px 12px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    font-size: 12.5px;
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

  .note.inline {
    margin: 0;
    align-self: center;
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

  .list {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--hairline);
  }

  .list-label {
    padding: 10px 2px 4px;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 2px;
    border-top: 1px solid var(--hairline);
    font-size: 13px;
  }

  .row.card {
    padding: 11px 10px;
    border-top: 1px solid var(--hairline);
    border-radius: 10px;
  }

  .row.card.selected {
    background: var(--selected);
  }

  .pick {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    border: 0;
    background: none;
    padding: 0;
    text-align: left;
    color: var(--text);
    cursor: default;
  }

  .body {
    flex: 1;
    min-width: 0;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 14px;
    font-weight: 500;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .meta {
    margin-top: 2px;
    font-size: 12px;
    color: var(--text-3);
    overflow-wrap: anywhere;
  }

  .sep {
    margin: 0 5px;
  }

  .tag {
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--chip);
    color: var(--text-3);
    font-size: 10.5px;
    font-weight: 500;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-3);
    opacity: 0.55;
    flex: none;
  }

  .dot.on {
    background: var(--green);
    opacity: 1;
  }

  .row-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }

  .act {
    border: 0;
    background: none;
    padding: 3px 6px;
    font-size: 12px;
    color: var(--accent-ink);
    cursor: default;
  }

  .act.danger {
    color: var(--red);
  }

  .act:disabled {
    color: var(--text-3);
  }

  .form,
  .result {
    margin-top: 12px;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--sunken);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .form-title {
    font-size: 13.5px;
    font-weight: 600;
  }

  .form-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1 1 180px;
    min-width: 0;
    font-size: 11.5px;
    color: var(--text-3);
  }

  .field.small {
    flex: 0 1 110px;
  }

  .field.grow {
    flex: 1 1 320px;
  }

  .field input {
    height: 30px;
    padding: 0 9px;
    border-radius: 8px;
    border: 1px solid var(--hairline-2);
    background: var(--panel);
    color: var(--text);
    font-size: 13px;
    font-family: inherit;
    width: 100%;
  }

  .form-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .collect {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    margin-top: 12px;
    flex-wrap: wrap;
  }

  .table {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--hairline);
  }

  .tr {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 2px;
    border-bottom: 1px solid var(--hairline);
    font-size: 12.5px;
  }

  .tr.th {
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .td {
    flex: 1;
    min-width: 0;
  }

  .td.file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .td.right {
    flex: none;
    display: flex;
    justify-content: flex-end;
    gap: 4px;
  }

  .err {
    margin-top: 2px;
    font-size: 11px;
    color: var(--red);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 220px;
  }

  /* 九态徽标：颜色只区分“进行中 / 完成 / 失败 / 取消”四类语义 */
  .badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 500;
    background: var(--chip);
    color: var(--text-2);
    white-space: nowrap;
  }

  .badge.init,
  .badge.queued {
    background: var(--chip);
    color: var(--text-2);
  }

  .badge.file_transferring,
  .badge.transcribing,
  .badge.post_processing {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent-ink);
  }

  .badge.completed {
    background: color-mix(in srgb, var(--green) 14%, transparent);
    color: var(--green);
  }

  .badge.failed {
    background: color-mix(in srgb, var(--red) 12%, transparent);
    color: var(--red);
  }

  .badge.cancel_requested,
  .badge.cancelled {
    background: var(--sunken);
    color: var(--text-3);
  }

  .snippet {
    margin: 0;
    padding: 10px 12px;
    border-radius: 8px;
    background: var(--panel);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.7;
    color: var(--text-2);
    max-height: 260px;
    overflow: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
