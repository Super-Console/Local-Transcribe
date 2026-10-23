<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { confirm } from "@tauri-apps/plugin-dialog";

  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n.svelte";
  import type { MessageKey } from "$lib/i18n/messages/zh";
  import type {
    LanCollectReport,
    LanHostInterface,
    LanHostStatus,
    LanNodeConfig,
    LanNodeInfo,
    LanNodeStatus,
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
  let collecting = $state(false);
  let testingId = $state<string | null>(null);
  let discovered = $state<LanNodeInfo[]>([]);
  const scanTimeoutMs = 8_000;
  let localAccess = $state<"granted" | "denied" | "no_network" | null>(null);
  let selectedId = $state<string | null>(null);
  let statuses = $state<Record<string, LanNodeStatus>>({});
  let storages = $state<Record<string, LanStorageInfo>>({});
  let tasks = $state<LanTask[]>([]);
  let tasksLoading = $state(false);
  let result = $state<LanTaskResult | null>(null);
  let resultTaskId = $state<string | null>(null);
  let savedNote = $state("");
  let collectDir = $state("");
  let collectReport = $state<LanCollectReport | null>(null);
  let draft = $state({ id: "", name: "", host: "", port: "7878" });
  let host = $state<LanHostStatus | null>(null);
  let hostInterfaces = $state<LanHostInterface[]>([]);
  let hostName = $state("Local Transcribe Mac");
  let hostAddress = $state("");
  let hostPort = $state(7878);
  let hostBusy = $state(false);

  async function refreshHost() {
    if (!tauriAvailable) return;
    try {
      host = await invoke<LanHostStatus>("lan_host_status");
      hostInterfaces = await invoke<LanHostInterface[]>("lan_host_interfaces");
      if (!hostAddress || !hostInterfaces.some((item) => item.address === hostAddress)) {
        hostAddress = hostInterfaces[0]?.address ?? "";
      }
    } catch (issue) { error = String(issue); }
  }

  async function toggleHost() {
    if (hostBusy) return;
    hostBusy = true;
    error = "";
    try {
      host = host?.running
        ? await invoke<LanHostStatus>("stop_lan_host")
        : await invoke<LanHostStatus>("start_lan_host", {
            name: hostName, address: hostAddress, port: Number(hostPort),
          });
    } catch (issue) { error = String(issue); }
    finally { hostBusy = false; }
  }

  const nodes = $derived(settings?.nodes ?? []);
  const selectedNode = $derived(nodes.find((node) => node.id === selectedId) ?? null);

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

  async function scan() {
    error = "";
    scanning = true;
    try {
      // mDNS first (cheap, carries name/port/model); the Rust side then probes
      // the local /24 over TCP for nodes whose multicast never arrives.
      discovered = await invoke<LanNodeInfo[]>("lan_discover", {
        timeoutMs: scanTimeoutMs,
        ports: nodes.map((node) => node.port),
      });
      scanned = true;
    } catch (issue) {
      error = String(issue);
    } finally {
      scanning = false;
      void checkLocalAccess();
    }
  }

  async function checkLocalAccess() {
    try {
      localAccess = await invoke<"granted" | "denied" | "no_network">("lan_local_access");
    } catch {
      localAccess = null;
    }
  }

  async function openLocalNetworkSettings() {
    error = "";
    try {
      await invoke("open_local_network_settings");
    } catch (issue) {
      error = String(issue);
    }
  }

  function fillDraft(node: LanNodeConfig) {
    draft = {
      id: node.id,
      name: node.name,
      host: node.host,
      port: String(node.port),
    };
  }

  function clearDraft() {
    draft = { id: "", name: "", host: "", port: "7878" };
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
        model_id: nodes.find((item) => item.id === draft.id)?.model_id ?? null,
        enabled: draft.id === "" ? true : nodes.find((item) => item.id === draft.id)?.enabled ?? true,
      };
      const nextNodes = draft.id === ""
        ? [...nodes, node]
        : nodes.map((item) => (item.id === draft.id ? node : item));
      await saveNodes(nextNodes);
      clearDraft();
      refreshStatuses();
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
          model_id: null,
          enabled: true,
        },
      ]);
      refreshStatuses();
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
      // One atomic call: storage and capability probes are independent and
      // their failures must never overwrite the reachability result.
      const status = await invoke<LanNodeStatus>("lan_node_status", { nodeId: node.id });
      statuses = { ...statuses, [node.id]: status };
      if (status.storage) {
        storages = { ...storages, [node.id]: status.storage };
      }
      if (status.warning) {
        error = `${node.name}: ${status.warning}`;
      }
    } catch (issue) {
      statuses = { ...statuses, [node.id]: { models: [], reachable: false, capabilities: null, latency_ms: null, storage: null, warning: null } };
      error = String(issue);
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
    void loadStatus(node);
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
    void refreshHost();
    void checkLocalAccess();
    refreshStatuses();
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
    };
  });
</script>

<section class="content">
  <div class="title-row">
    <h1 class="page-title">{t("lan.title")}</h1>
    <span class="temp">{t("lan.tempUi")}</span>
  </div>
  <p class="subtitle">{t("lan.subtitle")}</p>

  {#if localAccess === "denied"}
    <div class="perm" role="alert">
      <div class="perm-head">
        <Icon name="warn" size={14} />
        <span>{t("lan.permissionTitle")}</span>
      </div>
      <p class="perm-body">{t("lan.permissionBody")}</p>
      <div class="perm-actions">
        <button type="button" class="btn primary" onclick={() => void openLocalNetworkSettings()}>
          {t("lan.permissionAction")}
        </button>
        <button type="button" class="btn" onclick={() => void checkLocalAccess()}>
          {t("lan.permissionRecheck")}
        </button>
      </div>
    </div>
  {/if}

  {#if error}
    <div class="banner" role="alert">
      <Icon name="warn" size={14} />
      <span>{error}</span>
    </div>
  {/if}

  {#if host?.supported}
    <div class="group host-sharing" aria-busy={hostBusy}>
      <div class="group-head">
        <div class="group-title" id="host-sharing-label">{t("lan.hostTitle")}</div>
        <button type="button" class="btn" role="switch"
          aria-labelledby="host-sharing-label" aria-describedby="host-sharing-description"
          aria-checked={host.running} disabled={hostBusy || (!host.running && !hostAddress)}
          onclick={() => void toggleHost()}>
          {hostBusy ? t("lan.hostChanging") : host.running ? t("lan.hostOn") : t("lan.hostOff")}
        </button>
      </div>
      <p class="note" id="host-sharing-description">{t("lan.hostDesc")}</p>
      {#if host.running}
        <div class="meta" role="status">{t("lan.hostRunning", { name: host.name, address: host.address, port: host.port })}</div>
      {:else}
        <div class="form-grid">
          <label class="field"><span>{t("lan.nodeFormName")}</span>
            <input type="text" bind:value={hostName} maxlength="80" disabled={hostBusy} />
          </label>
          <label class="field"><span>{t("lan.hostInterface")}</span>
            <select bind:value={hostAddress} disabled={hostBusy}>
              {#each hostInterfaces as item (item.address)}
                <option value={item.address}>{item.name} · {item.address}</option>
              {/each}
            </select>
          </label>
          <label class="field small"><span>{t("lan.nodeFormPort")}</span>
            <input type="number" min="1" max="65535" bind:value={hostPort} disabled={hostBusy} />
          </label>
        </div>
        <button type="button" class="act" onclick={() => void refreshHost()} disabled={hostBusy}>{t("lan.hostRefresh")}</button>
        {#if hostInterfaces.length === 0}<p class="note">{t("lan.hostNoInterface")}</p>{/if}
      {/if}
    </div>
  {/if}

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
      <p class="note">{t("lan.scanEmptyHint")}</p>
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
    </div>

    {#if !selectedNode}
      <!-- No node selected: the task list stays empty. -->
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
              {t("lan.transferred", {
                sent: sizeLabel(task.received_bytes),
                total: sizeLabel(task.file_size),
              })}
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

  .perm {
    width: 100%;
    max-width: 940px;
    margin: 14px auto 0;
    padding: 12px 14px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--orange, #d98b2b) 10%, transparent);
    color: var(--text-2);
  }

  .perm-head {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .perm-body {
    margin: 6px 0 0;
    font-size: 12.5px;
    line-height: 1.55;
  }

  .perm-actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
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

  .field input,
  .field select {
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


  .host-sharing .group-head { flex-wrap: wrap; gap: 10px; }
  .host-sharing .note { font-size: 13px; color: var(--text-2); }
  .host-sharing .meta { font-size: 12.5px; color: var(--text-2); }
  .host-sharing .field { font-size: 12.5px; color: var(--text-2); }

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
