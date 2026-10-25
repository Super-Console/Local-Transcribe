<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { tick } from "svelte";
  import { t, locale } from "$lib/i18n.svelte";
  import type { MessageKey } from "$lib/i18n/messages/zh";
  import { classifyLlmError, nextAskTurn, trimHistory } from "$lib/ai-ask.js";
  import { defaultLlm, shortModelLabel } from "$lib/llm-default.svelte";
  import type { CatalogModel } from "$lib/types";

  let {
    open = $bindable(false),
    collapsed = $bindable(false),
    hasAnswer = $bindable(false),
    jobId = null,
    text,
    selection,
    segmentCount = 0,
    onFocusBody = () => {},
  }: {
    open?: boolean;
    collapsed?: boolean;
    hasAnswer?: boolean;
    jobId?: string | null;
    /** 发送时取当前转写全文；调用时快照，不随后续转写变化。 */
    text: () => string;
    /** 正文当前选中段落；无有效选区时返回 null。 */
    selection: () => string | null;
    segmentCount?: number;
    /** Esc 收起后把焦点交回正文滚动容器。 */
    onFocusBody?: () => void;
  } = $props();

  type Action = "summary" | "points" | "todos" | "explain" | "ask";
  type Phase = "idle" | "thinking" | "streaming" | "done" | "error";
  type ErrorKind = "" | "no_model" | "low_memory" | "failed";
  type AskTurn = { question: string; answer: string };
  type GenerationStage = { phase: "reading" | "merging"; current: number; total: number };
  type LlmEvent = { requestId: string; kind: string; data: unknown };
  type LlmStatus = {
    requiredMemoryBytes: number | null;
    availableMemoryBytes: number | null;
  };

  const CHIPS: { action: Action; key: MessageKey }[] = [
    { action: "summary", key: "reader.aiSummarize" },
    { action: "points", key: "reader.aiPoints" },
    { action: "todos", key: "reader.aiTodos" },
    { action: "explain", key: "reader.aiExplain" },
  ];
  const STORE_PREFIX = "lt.reader.ai.";

  let phase = $state<Phase>("idle");
  let generationStage = $state<GenerationStage | null>(null);
  let partialAnswer = $state("");
  let action = $state<Action | null>(null);
  let answer = $state("");
  let question = $state("");
  let lastQuestion = $state("");
  let history = $state<AskTurn[]>([]);
  let turns = $state(0);
  let elapsed = $state(0);
  let errorKind = $state<ErrorKind>("");
  let errorMessage = $state("");
  let turnHint = $state(false);
  let copied = $state(false);
  let strip = $state<"on" | "off" | "hidden">("hidden");
  let downloadBusy = $state(false);
  let downloadStarted = $state(false);
  let status = $state<LlmStatus | null>(null);
  let catalog = $state<CatalogModel[]>([]);

  let inputEl = $state<HTMLTextAreaElement | null>(null);
  let activeId: string | null = null;
  let unlisten: UnlistenFn | null = null;
  let lastRequest: { action: Action; question: string } | null = null;
  let tickHandle: ReturnType<typeof setInterval> | undefined;
  let copyHandle: ReturnType<typeof setTimeout> | undefined;
  let stripHandle: ReturnType<typeof setTimeout> | undefined;
  let saveHandle: ReturnType<typeof setTimeout> | undefined;
  let startedAt = 0;
  let restoredFor: string | null = null;
  let wasOpen = false;

  const busy = $derived(phase === "thinking" || phase === "streaming");
  const showCard = $derived(
    phase === "thinking" || phase === "streaming" || answer.length > 0,
  );
  const llmEntry = $derived(
    catalog.find((entry) => entry.id === defaultLlm.id) ?? null,
  );
  const missingModel = $derived(llmEntry !== null && !llmEntry.installed);
  const modelLabel = $derived(shortModelLabel(llmEntry?.name ?? defaultLlm.id));
  const headerTitle = $derived(
    action === "ask" ? truncate(lastQuestion || question, 40) : actionLabel(action),
  );

  $effect(() => {
    hasAnswer = answer.length > 0;
  });

  // ---------- 每个录音独立记忆：展开/收起与回答不因切页丢失 ----------

  function persist() {
    if (!jobId || restoredFor !== jobId) return;
    try {
      globalThis.localStorage?.setItem(
        STORE_PREFIX + jobId,
        JSON.stringify({
          open,
          collapsed,
          phase: phase === "done" || phase === "error" ? phase : "idle",
          action,
          answer,
          history,
          turns,
          question,
          errorKind,
          errorMessage,
          lastQuestion,
        }),
      );
    } catch {
      // 配额满时放弃持久化，不影响使用。
    }
  }

  function restore(id: string | null) {
    if (!id || restoredFor === id) return;
    restoredFor = id;
    try {
      const raw = globalThis.localStorage?.getItem(STORE_PREFIX + id);
      if (!raw) return;
      const saved = JSON.parse(raw) as Partial<{
        open: boolean;
        collapsed: boolean;
        phase: string;
        action: Action;
        answer: string;
        history: AskTurn[];
        turns: number;
        question: string;
        errorKind: ErrorKind;
        errorMessage: string;
        lastQuestion: string;
      }>;
      answer = typeof saved.answer === "string" ? saved.answer : "";
      action = saved.action ?? null;
      history = Array.isArray(saved.history) ? trimHistory(saved.history) : [];
      turns = Number.isFinite(saved.turns) ? Number(saved.turns) : 0;
      question = typeof saved.question === "string" ? saved.question : "";
      lastQuestion = typeof saved.lastQuestion === "string" ? saved.lastQuestion : "";
      errorKind = saved.errorKind ?? "";
      errorMessage = typeof saved.errorMessage === "string" ? saved.errorMessage : "";
      // 只恢复完成态与错误态；生成中的状态不跨页面恢复。
      phase =
        saved.phase === "done" && answer
          ? "done"
          : saved.phase === "error"
            ? "error"
            : "idle";
      strip = "hidden";
      open = !!saved.open;
      collapsed = !!saved.collapsed && answer.length > 0;
    } catch {
      // 损坏的存档直接忽略。
    }
  }

  $effect(() => {
    restore(jobId);
  });

  $effect(() => {
    void [open, collapsed, phase, action, answer, history, turns, question];
    clearTimeout(saveHandle);
    saveHandle = setTimeout(persist, 300);
  });
  $effect(() => () => clearTimeout(saveHandle));

  // ---------- 打开面板时刷新模型安装与内存状态 ----------

  async function refreshCatalog() {
    try {
      const entries = await invoke<CatalogModel[]>("model_catalog");
      catalog = entries.filter((entry) => entry.kind === "llm");
    } catch {
      catalog = [];
    }
  }

  async function refreshStatus() {
    try {
      status = await invoke<LlmStatus>("status");
    } catch {
      status = null;
    }
  }

  $effect(() => {
    if (open) {
      void refreshCatalog();
      void refreshStatus();
    }
  });

  $effect(() => {
    if (open && !wasOpen && phase === "idle" && !answer) {
      void tick().then(() => inputEl?.focus());
    }
    wasOpen = open;
  });

  // ---------- 生成：invoke 一次，进度靠事件流 ----------

  async function ensureListener() {
    if (unlisten) return;
    unlisten = await listen<LlmEvent>("voice-to-text:llm-event", (event) => {
      const payload = event.payload;
      if (!activeId || payload.requestId !== activeId) return;
      handleEvent(payload.kind, payload.data);
    });
  }

  function handleEvent(kind: string, data: unknown) {
    if (kind === "progress") {
      phase = "thinking";
    } else if (kind === "stage") {
      const stage = (data ?? {}) as Partial<GenerationStage>;
      if (
        (stage.phase === "reading" || stage.phase === "merging") &&
        Number.isFinite(stage.current) &&
        Number.isFinite(stage.total)
      ) {
        if (
          stage.phase === "reading" &&
          generationStage?.phase === "reading" &&
          Number(stage.current) > generationStage.current &&
          partialAnswer
        ) {
          partialAnswer += "\n\n";
        }
        phase = "thinking";
        generationStage = {
          phase: stage.phase,
          current: Number(stage.current),
          total: Number(stage.total),
        };
      }
    } else if (kind === "partial_token") {
      partialAnswer += String(data ?? "");
    } else if (kind === "token") {
      if (phase !== "streaming") {
        phase = "streaming";
        generationStage = null;
        partialAnswer = "";
        strip = "off";
        clearTimeout(stripHandle);
        stripHandle = setTimeout(() => {
          if (strip === "off") strip = "hidden";
        }, 400);
      }
      answer += String(data ?? "");
    } else if (kind === "done") {
      finish((data as { cancelled?: boolean } | null)?.cancelled === true);
    } else if (kind === "error") {
      const payload = (data ?? {}) as { code?: string; message?: string };
      fail(classifyLlmError(String(payload.code ?? "")), String(payload.message ?? ""));
    }
  }

  function finish(cancelled: boolean) {
    activeId = null;
    generationStage = null;
    partialAnswer = "";
    strip = "hidden";
    stopElapsed();
    phase = "done";
    if (!cancelled && action === "ask" && lastQuestion) {
      history = trimHistory([...history, { question: lastQuestion, answer }]);
      lastQuestion = "";
    }
    persist();
  }

  function fail(kind: ErrorKind, message: string) {
    activeId = null;
    generationStage = null;
    partialAnswer = "";
    strip = "hidden";
    stopElapsed();
    phase = "error";
    errorKind = kind;
    errorMessage = message;
    if (kind === "low_memory") void refreshStatus();
    persist();
  }

  function startElapsed() {
    startedAt = Date.now();
    elapsed = 0;
    clearInterval(tickHandle);
    tickHandle = setInterval(() => {
      elapsed = Math.max(1, Math.round((Date.now() - startedAt) / 1000));
    }, 250);
  }

  function stopElapsed() {
    clearInterval(tickHandle);
    elapsed = Math.max(1, Math.round((Date.now() - startedAt) / 1000));
  }

  async function run(next: Action, opts: { question?: string } = {}) {
    const full = text();
    if (!full.trim()) {
      fail("failed", t("reader.aiEmpty"));
      return;
    }
    if (next === "explain" && !selection()) {
      fail("failed", t("reader.aiExplainSelection"));
      return;
    }
    let hist = history;
    if (next === "ask") {
      const guard = nextAskTurn({ turns: history, count: turns });
      turnHint = guard.reset;
      hist = guard.history;
      turns = guard.count;
      lastQuestion = opts.question ?? "";
    } else {
      turnHint = false;
      hist = [];
      turns = 0;
      history = [];
    }
    action = next;
    answer = "";
    partialAnswer = "";
    phase = "thinking";
    generationStage = null;
    errorKind = "";
    errorMessage = "";
    collapsed = false;
    strip = "on";
    startElapsed();
    lastRequest = { action: next, question: opts.question ?? "" };
    try {
      await ensureListener();
      activeId = await invoke<string>("start_generation", {
        request: {
          action: next,
          modelId: defaultLlm.id,
          text: full,
          // 答案语言跟随界面语言，而不是跟随转写内容的语言。
          language: locale.resolved,
          ...(next === "explain" ? { selection: selection() ?? "" } : {}),
          ...(opts.question ? { question: opts.question } : {}),
          history: hist.map((turn) => ({
            question: turn.question,
            answer: turn.answer,
          })),
        },
      });
    } catch (error) {
      activeId = null;
      fail("failed", String(error));
    }
  }

  function onChip(next: Action) {
    if (busy) return;
    void run(next);
  }

  function onSend() {
    const value = question.trim();
    if (!value || busy) return;
    question = "";
    void run("ask", { question: value });
  }

  function onRegenerate() {
    if (!lastRequest || busy) return;
    void run(lastRequest.action, {
      ...(lastRequest.question ? { question: lastRequest.question } : {}),
    });
  }

  async function onStop() {
    try {
      await invoke("cancel");
    } catch {
      // 取消失败时由空闲卸载兜底。
    }
  }

  async function onCopy() {
    try {
      await navigator.clipboard.writeText(answer);
      copied = true;
      clearTimeout(copyHandle);
      copyHandle = setTimeout(() => (copied = false), 1500);
    } catch {
      // 剪贴板不可用时静默。
    }
  }

  async function onDownloadModel() {
    if (!llmEntry || downloadBusy) return;
    downloadBusy = true;
    try {
      await invoke("download_model", { id: llmEntry.id });
      downloadStarted = true;
    } catch (error) {
      errorKind = "failed";
      errorMessage = String(error);
    } finally {
      downloadBusy = false;
    }
  }

  function onInputKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSend();
    }
  }

  // ---------- 渲染辅助 ----------

  function actionLabel(value: Action | null): string {
    switch (value) {
      case "summary":
        return t("reader.aiSummarize");
      case "points":
        return t("reader.aiPoints");
      case "todos":
        return t("reader.aiTodos");
      case "explain":
        return t("reader.aiExplain");
      case "ask":
        return lastQuestion || question;
      default:
        return "";
    }
  }

  function truncate(value: string, max: number): string {
    return value.length > max ? `${value.slice(0, max)}…` : value;
  }

  function sizeLabel(bytes: number): string {
    if (!bytes) return "—";
    const gigabytes = bytes / 1_000_000_000;
    if (gigabytes >= 1) return `${gigabytes.toFixed(2)} GB`;
    return `${Math.max(1, Math.round(bytes / 1_000_000))} MB`;
  }

  function escapeHtml(value: string): string {
    return value
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  /** 模型输出一律按纯文本转义；空行分段、行首列表符号成组，加粗只换字重。 */
  function renderAnswer(value: string): string {
    return value
      .split(/\n{2,}/)
      .map((block) => {
        const lines = block.split("\n");
        const isList =
          lines.length > 0 &&
          lines.every((line) => /^\s*(?:[-•*]|\d+[.、)])\s+/.test(line));
        const body = escapeHtml(block)
          .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
          .replace(/\n/g, "<br>");
        if (isList) {
          const items = lines
            .map(
              (line) =>
                `<li>${escapeHtml(line.replace(/^\s*(?:[-•*]|\d+[.、)])\s+/, ""))
                  .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")}</li>`,
            )
            .join("");
          return `<ul>${items}</ul>`;
        }
        return `<p>${body}</p>`;
      })
      .join("");
  }

  $effect(() => () => {
    clearInterval(tickHandle);
    clearTimeout(copyHandle);
    clearTimeout(stripHandle);
    unlisten?.();
  });
</script>

{#if open || showCard}
  {#if collapsed && showCard}
    <div class="ai-ask">
      <div class="ai-collapsed">
        <span class="ai-collapsed-title">{actionLabel(action)} · {modelLabel}</span>
        <button
          type="button"
          class="ai-action"
          onclick={() => (collapsed = false)}
        >
          {t("reader.aiExpand")}
        </button>
      </div>
    </div>
  {:else}
    <div class="ai-ask" class:with-card={showCard}>
      {#if open}
        <div class="ai-panel">
          <div class="ai-inputrow">
            <textarea
              bind:this={inputEl}
              class="ai-input"
              rows="1"
              spellcheck="false"
              placeholder={t("reader.aiPlaceholder")}
              aria-label={t("reader.aiPlaceholder")}
              bind:value={question}
              onkeydown={onInputKeydown}
            ></textarea>
            {#if busy}
              <button type="button" class="ai-send stop" onclick={onStop}>
                {t("reader.aiStop")}
              </button>
            {:else}
              <button
                type="button"
                class="ai-send"
                disabled={!question.trim()}
                onclick={onSend}
              >
                {t("reader.aiSend")}
              </button>
            {/if}
          </div>
          <div class="ai-chips">
            {#each CHIPS as chip (chip.action)}
              <button
                type="button"
                class="ai-chip"
                disabled={busy}
                onclick={() => onChip(chip.action)}
              >
                {t(chip.key)}
              </button>
            {/each}
          </div>
          {#if turnHint}
            <div class="ai-hint" aria-live="polite">{t("reader.aiTurnLimit")}</div>
          {/if}
          {#if missingModel}
            <div class="ai-empty">
              <div class="ai-empty-text">{t("reader.aiNoModel")}</div>
              <button
                type="button"
                class="btn-secondary"
                disabled={downloadBusy}
                onclick={onDownloadModel}
              >
                {t("reader.aiDownloadModel", {
                  model: modelLabel,
                  size: sizeLabel(llmEntry?.bytes ?? 0),
                })}
              </button>
              {#if downloadStarted}
                <div class="ai-hint">{t("reader.aiDownloadHint")}</div>
              {/if}
            </div>
          {:else if phase === "error" && errorKind === "low_memory"}
            <div class="ai-empty">
              <div class="ai-empty-text">
                {t("reader.aiLowMemory", {
                  required: sizeLabel(status?.requiredMemoryBytes ?? 0),
                  available: sizeLabel(status?.availableMemoryBytes ?? 0),
                })}
              </div>
              <button
                type="button"
                class="btn-secondary"
                onclick={() => lastRequest && onRegenerate()}
              >
                {t("reader.aiUnloadAsr")}
              </button>
            </div>
          {:else if phase === "error" && !answer && errorKind === "failed"}
            <div class="ai-empty">
              <div class="ai-empty-text">
                {t("reader.aiFailed", { reason: errorMessage })}
              </div>
              <button
                type="button"
                class="btn-secondary"
                onclick={() => lastRequest && onRegenerate()}
              >
                {t("reader.aiRetry")}
              </button>
            </div>
          {/if}
        </div>
      {/if}

      {#if showCard}
        <div class="ai-card" role="group" aria-label={headerTitle}>
          {#if strip === "on" || strip === "off"}
            <div class="ai-strip" class:leaving={strip === "off"} aria-hidden="true"></div>
          {/if}
          <div class="ai-head">
            <div class="ai-head-text">
              {#if phase === "thinking"}
                {#if generationStage?.phase === "reading"}
                  {t("reader.aiReadingSegment", {
                    current: generationStage.current,
                    total: generationStage.total,
                  })}
                {:else if generationStage?.phase === "merging"}
                  {t("reader.aiMerging", {
                    current: generationStage.current,
                    total: generationStage.total,
                  })}
                {:else}
                  {t("reader.aiThinking")}
                {/if}
              {:else}
                <span class="ai-title">{headerTitle}</span>
                <span class="ai-sep"> · </span>
                <span>{modelLabel}</span>
                {#if phase !== "error"}
                  <span class="ai-sep"> · </span>
                  <span>{t("reader.aiSeconds", { seconds: elapsed })}</span>
                {/if}
              {/if}
            </div>
            <div class="ai-actions">
              <button
                type="button"
                class="ai-action"
                disabled={!answer}
                onclick={onCopy}
              >
                {copied ? t("reader.aiCopied") : t("reader.aiCopy")}
              </button>
              <button
                type="button"
                class="ai-action"
                disabled={busy}
                onclick={onRegenerate}
              >
                {t("reader.aiRegenerate")}
              </button>
              <button type="button" class="ai-action" onclick={() => (collapsed = true)}>
                {t("reader.aiCollapse")}
              </button>
            </div>
          </div>
          <div class="ai-answer">
            {#if phase === "error"}
              <div class="ai-error">
                {#if errorKind === "no_model"}
                  {t("reader.aiNoModel")}
                {:else if errorKind === "low_memory"}
                  {t("reader.aiLowMemory", {
                    required: sizeLabel(status?.requiredMemoryBytes ?? 0),
                    available: sizeLabel(status?.availableMemoryBytes ?? 0),
                  })}
                {:else}
                  {t("reader.aiFailed", { reason: errorMessage })}
                {/if}
              </div>
            {/if}
            {#if answer}
              {@html renderAnswer(answer)}
            {:else if partialAnswer && phase === "thinking"}
              <div class="ai-partial-label">{t("reader.aiPartialPreview")}</div>
              <div class="ai-partial">{@html renderAnswer(partialAnswer)}</div>
            {/if}
          </div>
          {#if answer && phase !== "error"}
            <div class="ai-source">
              {t("reader.aiSource", { count: segmentCount })}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
{/if}

<style>
  .ai-ask {
    padding: 20px 0 14px;
  }

  .ai-ask.with-card {
    padding-bottom: 16px;
  }

  /* ---------- 打开态：输入条 + 四个固定入口 ---------- */

  .ai-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .ai-inputrow {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .ai-input {
    flex: 1;
    min-width: 0;
    height: 34px;
    padding: 7px 12px;
    border: 1px solid var(--hairline);
    border-radius: 10px;
    background: var(--fill);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    line-height: 18px;
    resize: none;
    overflow-y: auto;
    outline: none;
  }

  .ai-input::placeholder {
    color: var(--text-3);
  }

  .ai-input:focus-visible {
    border-color: var(--hairline-2);
  }

  .ai-send {
    flex: none;
    height: 34px;
    padding: 0 14px;
    border: 1px solid transparent;
    border-radius: 10px;
    background: var(--accent);
    color: #fffefb;
    font: inherit;
    font-size: 13px;
    cursor: default;
  }

  .ai-send:disabled {
    opacity: 0.45;
  }

  .ai-send.stop {
    background: transparent;
    border-color: var(--hairline-2);
    color: var(--text-2);
  }

  .ai-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .ai-chip {
    height: 28px;
    padding: 0 12px;
    border: 0;
    border-radius: 999px;
    background: var(--fill);
    color: var(--text-2);
    font: inherit;
    font-size: 12px;
    cursor: default;
  }

  .ai-chip:hover:not(:disabled) {
    background: var(--chip);
    color: var(--text);
  }

  .ai-chip:disabled {
    opacity: 0.5;
  }

  .ai-hint {
    font-size: 12px;
    color: var(--text-3);
  }

  .ai-empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }

  .ai-empty-text {
    font-size: 13px;
    line-height: 1.6;
    color: var(--text-2);
  }

  /* ---------- 答案卡片：平的面板，不发光、不嵌套 ---------- */

  .ai-card {
    display: flex;
    flex-direction: column;
    max-height: 60vh;
    margin-top: 12px;
    overflow: hidden;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    background: var(--panel);
  }

  .ai-strip {
    position: relative;
    flex: none;
    height: 2px;
    overflow: hidden;
    background: linear-gradient(to right, transparent, var(--accent), transparent);
    opacity: 0.7;
    animation: ai-breath 3s ease-in-out infinite;
  }

  .ai-strip::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to right,
      transparent,
      color-mix(in srgb, var(--accent) 30%, #ffffff),
      transparent
    );
    animation: ai-sweep 8s ease-in-out infinite;
  }

  .ai-strip.leaving {
    animation: none;
    opacity: 0;
    transition: opacity 0.35s ease;
  }

  @keyframes ai-sweep {
    0% {
      transform: translateX(-100%);
    }
    50% {
      transform: translateX(100%);
    }
    100% {
      transform: translateX(-100%);
    }
  }

  @keyframes ai-breath {
    0%,
    100% {
      opacity: 0.55;
    }
    50% {
      opacity: 0.85;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .ai-strip,
    .ai-strip::after {
      animation: none;
    }

    .ai-strip::after {
      background: transparent;
    }
  }

  .ai-head {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 16px 0;
  }

  .ai-head-text {
    min-width: 0;
    font-size: 12px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ai-title {
    color: var(--text-2);
  }

  .ai-actions {
    flex: none;
    display: flex;
    gap: 12px;
  }

  .ai-action {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-2);
    font: inherit;
    font-size: 12px;
    cursor: default;
  }

  .ai-action:hover:not(:disabled) {
    color: var(--text);
  }

  .ai-action:disabled {
    opacity: 0.45;
  }

  .ai-answer {
    flex: 1 1 auto;
    min-height: 0;
    padding: 10px 16px 0;
    overflow-y: auto;
    font-size: 13px;
    line-height: 1.85;
    color: var(--text);
  }

  .ai-partial-label {
    margin-bottom: 6px;
    color: var(--text-3);
    font-size: 11.5px;
  }

  .ai-partial {
    color: var(--text-2);
  }

  .ai-answer :global(p) {
    margin: 0 0 10px;
  }

  .ai-answer :global(p:last-child) {
    margin-bottom: 0;
  }

  .ai-answer :global(ul) {
    margin: 0 0 10px;
    padding-left: 20px;
  }

  .ai-answer :global(li) {
    margin-bottom: 4px;
  }

  .ai-answer :global(li:last-child) {
    margin-bottom: 0;
  }

  .ai-answer :global(strong) {
    font-weight: 500;
  }

  .ai-error {
    margin-bottom: 10px;
    font-size: 13px;
    line-height: 1.6;
    color: var(--text-2);
  }

  .ai-source {
    flex: none;
    padding: 10px 16px 14px;
    font-size: 11.5px;
    color: var(--text-3);
  }

  /* ---------- 收起态：一行、28px、无边框 ---------- */

  .ai-collapsed {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    height: 28px;
    background: transparent;
  }

  .ai-collapsed-title {
    min-width: 0;
    font-size: 12px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
