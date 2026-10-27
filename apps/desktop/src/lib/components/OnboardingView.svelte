<script lang="ts">
  // Provisional functional OOBE surface. The final visual design remains open for Timmy's approval.
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { LOCALE_NAMES, LOCALE_OPTIONS, locale, t, type LocalePreference } from "$lib/i18n.svelte";
  import type { MessageKey } from "$lib/i18n/messages/zh";
  import { onboardingSteps, recommendModels } from "$lib/onboarding-logic.mjs";
  import type { CatalogModel, LibraryModelSummary, ModelDownloadEvent } from "$lib/types";

  type Step = "welcome" | "models" | "permissions" | "privacy" | "finish";
  type HardwareInfo = {
    memoryBytes: number | null;
    cpuBrand: string | null;
    appleSilicon: boolean;
    availableBytes: number | null;
  };
  type OnboardingInfo = {
    hardware: HardwareInfo;
    dataPath: string;
    modelPath: string;
    logsPath: string;
    modelBytes: Record<string, number>;
  };
  type PermissionStatus = {
    microphone: "authorized" | "denied" | "restricted" | "not_determined" | "unknown";
    screenCapture: "granted" | "denied" | "unknown";
  };
  type AsrChoice = { id: string; name: string; bytes: number; installed: boolean };

  let {
    platform,
    preferredAsrModelId,
    installedModels,
    catalog,
    downloads,
    onDownload,
    onCancelDownload,
    onComplete,
  }: {
    platform: "macos" | "windows" | "other";
    preferredAsrModelId: string | null;
    installedModels: LibraryModelSummary[];
    catalog: CatalogModel[];
    downloads: Record<string, ModelDownloadEvent>;
    onDownload: (id: string) => Promise<void> | void;
    onCancelDownload: () => Promise<void> | void;
    onComplete: (importFirst: boolean, preferredAsrModelId: string | null) => Promise<void> | void;
  } = $props();

  const GIB = 1024 ** 3;
  const steps = $derived(onboardingSteps(platform) as Step[]);
  let stepIndex = $state(0);
  let step = $derived(steps[stepIndex] ?? "welcome");
  let selectedLanguage = $state<"zh" | "multi">("zh");
  let selectedAsrId = $state<string | null>(null);
  let manuallySelectedAsr = $state(false);
  let info = $state<OnboardingInfo | null>(null);
  let infoLoading = $state(false);
  let infoFailed = $state(false);
  let permissions = $state<PermissionStatus | null>(null);
  let permissionLoading = $state(false);
  let microphoneResult = $state<string | null>(null);
  let networkResult = $state<string | null>(null);
  let actionError = $state("");
  let primaryButton = $state<HTMLButtonElement | undefined>(undefined);
  let punctuationSelected = $state(false);

  const asrModels = $derived(catalog.filter((entry) => entry.kind === "asr"));
  const extraInstalledAsrModels: AsrChoice[] = $derived(
    installedModels
      .filter((model) => !asrModels.some((entry) => entry.id === model.id))
      .map((model) => ({
        id: model.id,
        name: model.displayName,
        bytes: model.bytes,
        installed: true,
      })),
  );
  const availableAsrModels: AsrChoice[] = $derived([
    ...asrModels,
    ...extraInstalledAsrModels,
  ]);
  const punctuationModel = $derived(catalog.find((entry) => entry.kind === "punctuation") ?? null);
  const llmModels = $derived(catalog.filter((entry) => entry.kind === "llm"));
  const recommendation = $derived(
    recommendModels({
      memoryBytes: info?.hardware.memoryBytes,
      availableBytes: info?.hardware.availableBytes,
      language: selectedLanguage,
      modelBytes: info?.modelBytes ?? {},
    }),
  );
  const chosenAsrId = $derived(selectedAsrId ?? recommendation.asrModelId);
  const chosenAsr: AsrChoice | null = $derived.by(() => {
    return availableAsrModels.find((entry) => entry.id === chosenAsrId) ?? null;
  });
  const recommendedLlm = $derived(
    llmModels.find((entry) => entry.id === recommendation.languageModelId) ?? null,
  );
  const chosenModelIsPending = $derived(!!chosenAsr && !chosenAsr.installed);
  const detectedDetails = $derived.by(() => {
    if (!info) return "";
    const parts: string[] = [];
    if (info.hardware.cpuBrand?.trim()) parts.push(info.hardware.cpuBrand.trim());
    if (info.hardware.memoryBytes && info.hardware.memoryBytes > 0) {
      parts.push(t("onboarding.memory", { value: Math.round(info.hardware.memoryBytes / GIB) }));
    }
    if (info.hardware.availableBytes != null) {
      parts.push(t("onboarding.disk", { value: Math.round(info.hardware.availableBytes / GIB) }));
    } else {
      parts.push(t("onboarding.diskUnknown"));
    }
    return parts.join(" · ");
  });

  $effect(() => {
    if (step === "models" && !info && !infoLoading) void loadInfo();
    if (step === "permissions" && !permissions && !permissionLoading) void loadPermissions();
  });

  $effect(() => {
    if (preferredAsrModelId && !manuallySelectedAsr) {
      selectedAsrId = preferredAsrModelId;
      manuallySelectedAsr = true;
    }
  });

  $effect(() => {
    stepIndex;
    void tick().then(() => primaryButton?.focus());
  });

  onMount(() => {
    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void finish(false);
      }
    };
    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });

  async function loadInfo() {
    infoLoading = true;
    infoFailed = false;
    try {
      info = await invoke<OnboardingInfo>("get_onboarding_info");
    } catch {
      infoFailed = true;
      info = null;
    } finally {
      infoLoading = false;
    }
  }

  async function loadPermissions() {
    permissionLoading = true;
    try {
      // Read-only TCC status check; this does not request either permission.
      permissions = await invoke<PermissionStatus>("app_permissions");
    } catch {
      permissions = { microphone: "unknown", screenCapture: "unknown" };
    } finally {
      permissionLoading = false;
    }
  }

  function setLocale(value: string) {
    locale.preference = value as LocalePreference;
  }

  function setLanguage(value: "zh" | "multi") {
    selectedLanguage = value;
    if (info && !manuallySelectedAsr) {
      selectedAsrId = recommendModels({
        memoryBytes: info.hardware.memoryBytes,
        availableBytes: info.hardware.availableBytes,
        language: value,
        modelBytes: info.modelBytes,
      }).asrModelId;
    }
  }

  async function selectAsrModel(id: string) {
    selectedAsrId = id;
    manuallySelectedAsr = true;
    actionError = "";
    try {
      await invoke("set_default_asr_model", { id });
    } catch (issue) {
      actionError = String(issue);
    }
  }

  async function requestMicrophone() {
    actionError = "";
    try {
      microphoneResult = await invoke<string>("request_microphone_permission");
      permissions = {
        microphone: microphoneResult as PermissionStatus["microphone"],
        screenCapture: permissions?.screenCapture ?? "unknown",
      };
    } catch (issue) {
      actionError = String(issue);
    }
  }

  async function requestLocalNetwork() {
    actionError = "";
    try {
      networkResult = await invoke<string>("request_local_network_permission");
    } catch (issue) {
      actionError = String(issue);
    }
  }

  async function openSystemSettings(pane: "microphone" | "screen_capture" | "network") {
    try {
      if (pane === "network") await invoke("open_local_network_settings");
      else await invoke("open_privacy_settings", { pane });
    } catch (issue) {
      actionError = String(issue);
    }
  }

  async function finish(importFirst: boolean) {
    actionError = "";
    try {
      const finalAsrId = selectedAsrId ?? (info ? recommendation.asrModelId : null);
      if (finalAsrId) await invoke("set_default_asr_model", { id: finalAsrId });
      await invoke("complete_onboarding");
      await onComplete(importFirst, finalAsrId ?? preferredAsrModelId);
    } catch (issue) {
      actionError = t("onboarding.error", { message: String(issue) });
    }
  }

  function next() {
    if (stepIndex < steps.length - 1) stepIndex += 1;
    else void finish(false);
  }

  function previous() {
    if (stepIndex > 0) stepIndex -= 1;
  }

  function dragWindow(event: MouseEvent) {
    if (event.button !== 0 || event.detail > 1 || !(event.target instanceof Element)) return;
    if (event.target.closest('button, a, input, select, textarea, label, [contenteditable], [role="button"]')) return;
    event.preventDefault();
    try { void getCurrentWindow().startDragging().catch(() => {}); } catch { /* browser preview */ }
  }

  function modelSize(bytes: number): string {
    if (bytes >= GIB) return `${(bytes / GIB).toFixed(2)} GB`;
    return `${Math.round(bytes / (1024 ** 2))} MB`;
  }

  function reasonKey(id: string): MessageKey {
    if (id === "zipformer-ctc-small-zh") return "onboarding.reasonZipformer";
    if (id === "sensevoice-small") return "onboarding.reasonSenseVoice";
    if (id === "x-asr-zh-en") return "onboarding.reasonXasr";
    return "onboarding.reasonFireRed";
  }

  function allowed(status: string | null): boolean {
    return status === "authorized" || status === "granted";
  }

  function requestStatus(status: string | null): string {
    if (allowed(status)) return t("onboarding.permissionGranted");
    if (status === "denied" || status === "restricted") return t("onboarding.permissionDenied");
    if (status === "no_network") return t("onboarding.noNetwork");
    if (!status || status === "unknown" || status === "not_determined") {
      return t("onboarding.permissionNotRequested");
    }
    return t("onboarding.permissionSkipped");
  }
</script>

{#snippet progressDots()}
  <div class="progress-dots" role="group" aria-label={t("onboarding.step", { current: stepIndex + 1, total: steps.length })}>
    {#each steps as item, index (item)}
      <span class:current={index === stepIndex} class:done={index < stepIndex} aria-hidden="true"></span>
    {/each}
  </div>
{/snippet}

<section class="onboarding-screen" aria-label="Local Transcribe onboarding">
  <header class="topbar" onmousedown={dragWindow} role="presentation">
    <button type="button" class="btn-secondary skip" onclick={() => void finish(false)}>
      {t("onboarding.skip")}
    </button>
  </header>

  <main class="stage">
    {#key step}
    <div class="step-content" aria-live="polite">
      {#if step === "welcome"}
        <h1>{t("onboarding.welcomeTitle")}</h1>
        <p class="lead">{t("onboarding.welcomeBody")}</p>
        <label class="field-label" for="onboarding-locale">{t("onboarding.languageLabel")}</label>
        <select id="onboarding-locale" class="locale-select" value={locale.preference} onchange={(event) => setLocale(event.currentTarget.value)}>
          {#each LOCALE_OPTIONS as option (option)}
            <option value={option}>{option === "system" ? t("common.system") : LOCALE_NAMES[option]}</option>
          {/each}
        </select>
        <div class="welcome-actions">
          <button bind:this={primaryButton} type="button" class="btn-primary welcome-next" onclick={next}>
            {t("onboarding.next")}
          </button>
          {@render progressDots()}
        </div>
      {:else if step === "models"}
        <p class="eyebrow">{t("onboarding.step", { current: stepIndex + 1, total: steps.length })}</p>
        <h1>{t("onboarding.modelsTitle")}</h1>
        <p class="lead">{t("onboarding.modelsBody")}</p>
        <fieldset class="language-choices">
          <legend>{t("settings.audioLangName")}</legend>
          <label class:active={selectedLanguage === "zh"}>
            <input type="radio" name="onboarding-language" checked={selectedLanguage === "zh"} onchange={() => setLanguage("zh")} />
            <span>{t("onboarding.languageZh")}</span>
          </label>
          <label class:active={selectedLanguage === "multi"}>
            <input type="radio" name="onboarding-language" checked={selectedLanguage === "multi"} onchange={() => setLanguage("multi")} />
            <span>{t("onboarding.languageMulti")}</span>
          </label>
        </fieldset>

        {#if infoLoading}
          <p class="muted" role="status">{t("onboarding.loading")}</p>
        {:else if infoFailed || !info || !info.hardware.memoryBytes}
          <p class="notice">{t("onboarding.hardwareUnavailable")}</p>
        {:else}
          <p class="hardware-line">{t("onboarding.detected", { details: detectedDetails })}</p>
        {/if}

        {#if chosenAsr}
          <div class="recommendation" aria-live="polite">
            {#if manuallySelectedAsr || (preferredAsrModelId && selectedAsrId === preferredAsrModelId)}
              <p>{t("onboarding.reasonSelected", { name: chosenAsr.name, size: modelSize(chosenAsr.bytes) })}</p>
            {:else}
              <p>{t("onboarding.recommend", {
                name: chosenAsr.name,
                size: modelSize(chosenAsr.bytes),
                reason: t(reasonKey(chosenAsr.id)),
              })}</p>
            {/if}
            {#if recommendation.diskDowngraded}
              <p class="muted">{t("onboarding.diskDowngrade")}</p>
            {/if}
            <label class="field-label" for="onboarding-asr">{t("onboarding.chooseModel")}</label>
            <select id="onboarding-asr" class="locale-select" value={chosenAsr.id} onchange={(event) => void selectAsrModel(event.currentTarget.value)}>
              {#each availableAsrModels as entry (entry.id)}
                <option value={entry.id}>{entry.name} · {modelSize(entry.bytes)}{entry.installed ? ` · ${t("model.downloaded")}` : ""}</option>
              {/each}
            </select>
          </div>
        {/if}

        <div class="model-optional">
          <div class="optional-title">{t("onboarding.languageModel")}</div>
          {#if recommendedLlm}
            <p>{t("onboarding.languageModelRecommend", { name: recommendedLlm.name, size: modelSize(recommendedLlm.bytes) })}</p>
          {:else}
            <p>{t("onboarding.noLanguageModel")}</p>
          {/if}
        </div>

        {#if punctuationModel}
          {@const punctuationDownload = downloads[punctuationModel.id]}
          <label class="punctuation-choice">
            <input
              type="checkbox"
              checked={punctuationSelected || punctuationModel.installed}
              disabled={punctuationModel.installed || punctuationDownload?.state === "downloading" || punctuationDownload?.state === "installing"}
              onchange={(event) => {
                punctuationSelected = event.currentTarget.checked;
                if (punctuationSelected) void onDownload(punctuationModel.id);
                else void onCancelDownload();
              }}
            />
            <span class="punctuation-copy">
              <strong>{t("onboarding.punctuation")}</strong> <span class="tag">{t("onboarding.optional")}</span>
              <small>{punctuationDownload?.state === "downloading" || punctuationDownload?.state === "installing" ? t("onboarding.downloadStarted") : t("onboarding.punctuationOptIn")}</small>
            </span>
          </label>
        {/if}
      {:else if step === "permissions"}
        <p class="eyebrow">{t("onboarding.step", { current: stepIndex + 1, total: steps.length })}</p>
        <h1>{t("onboarding.permissionTitle")}</h1>
        <p class="lead">{t("onboarding.permissionBody")}</p>
        <div class="permission-list">
          <section class="permission-row">
            <div>
              <h2>{t("onboarding.microphone")}</h2>
              <p>{t("onboarding.microphoneWhy")}</p>
              <p class="muted">{t("onboarding.microphoneNo")}</p>
              {#if permissions}
                <p class="permission-status" role="status">{requestStatus(microphoneResult ?? permissions.microphone)}</p>
              {/if}
            </div>
            <div class="permission-actions">
              {#if !allowed(microphoneResult ?? permissions?.microphone ?? null)}
                {#if (microphoneResult ?? permissions?.microphone) === "denied" || (microphoneResult ?? permissions?.microphone) === "restricted"}
                  <button type="button" class="link-action" onclick={() => void openSystemSettings("microphone")}>{t("onboarding.openSettings")}</button>
                {:else}
                  <button type="button" class="btn-secondary" onclick={() => void requestMicrophone()}>{t("onboarding.allow")}</button>
                {/if}
              {/if}
            </div>
          </section>
          <section class="permission-row">
            <div>
              <h2>{t("onboarding.localNetwork")}</h2>
              <p>{t("onboarding.localNetworkWhy")}</p>
              <p class="muted">{t("onboarding.localNetworkNo")}</p>
              <p class="permission-status" role="status">{requestStatus(networkResult)}</p>
            </div>
            <div class="permission-actions">
              {#if networkResult === "denied"}
                <button type="button" class="link-action" onclick={() => void openSystemSettings("network")}>{t("onboarding.openSettings")}</button>
              {:else if !allowed(networkResult)}
                <button type="button" class="btn-secondary" onclick={() => void requestLocalNetwork()}>{t("onboarding.allow")}</button>
              {/if}
            </div>
          </section>
        </div>
      {:else if step === "privacy"}
        <p class="eyebrow">{t("onboarding.step", { current: stepIndex + 1, total: steps.length })}</p>
        <h1>{t("onboarding.privacyTitle")}</h1>
        <p class="lead">{t("onboarding.privacyBody")}</p>
        {#if info}
          <dl class="paths">
            <div><dt>{t("onboarding.dataPath")}</dt><dd><code>{info.dataPath}</code></dd></div>
            <div><dt>{t("onboarding.modelPath")}</dt><dd><code>{info.modelPath}</code></dd></div>
            <div><dt>{t("onboarding.logsPath")}</dt><dd><code>{info.logsPath}</code></dd></div>
          </dl>
        {:else}
          <p class="notice">{t("onboarding.hardwareUnavailable")}</p>
        {/if}
        <p class="privacy-promise">{t("onboarding.privacyPromise")}</p>
      {:else}
        <p class="eyebrow">{t("onboarding.step", { current: stepIndex + 1, total: steps.length })}</p>
        <h1>{t("onboarding.finishTitle")}</h1>
        <p class="lead">{t("onboarding.finishBody")}</p>
        {#if chosenModelIsPending}
          <p class="notice">{t("onboarding.modelPending")}</p>
        {/if}
        <div class="finish-actions">
          <button bind:this={primaryButton} type="button" class="btn-primary" onclick={() => void finish(true)}>
            {t("onboarding.importFirst")}
          </button>
          <button type="button" class="btn-secondary" onclick={() => void finish(false)}>
            {t("onboarding.lookAround")}
          </button>
        </div>
      {/if}

      {#if actionError}
        <p class="error-message" role="alert">{actionError}</p>
      {/if}
    </div>
    {/key}
  </main>

  {#if step !== "welcome"}
    <footer class="footer">
      <button type="button" class="link-action" onclick={previous}>{t("onboarding.previous")}</button>
      {@render progressDots()}
      {#if step !== "finish"}
        <button bind:this={primaryButton} type="button" class="btn-primary" onclick={next}>{t("onboarding.next")}</button>
      {/if}
    </footer>
  {/if}
</section>

<style>
  .onboarding-screen {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100vh;
    min-height: 520px;
    padding: 22px clamp(22px, 5vw, 64px) 26px;
    overflow: hidden;
    background: var(--bg);
    color: var(--text);
  }

  .topbar,
  .footer {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .topbar { justify-content: flex-end; min-height: 44px; }

  .progress-dots {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 44px;
    justify-content: center;
    flex: none;
  }

  .progress-dots span {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--hairline-2);
  }

  .progress-dots span.current,
  .progress-dots span.done {
    background: var(--accent);
  }

  .skip { min-height: 40px; }

  .stage {
    display: grid;
    flex: 1;
    min-height: 0;
    place-items: center;
    overflow: auto;
  }

  .step-content {
    width: min(100%, 680px);
    padding: 28px 0 34px;
    animation: step-enter 220ms ease-out both;
  }
  @keyframes step-enter {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }
  @media (prefers-reduced-motion: reduce) {
    .step-content { animation: none; }
  }

  .eyebrow {
    margin: 0 0 12px;
    color: var(--text-2);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  h1 {
    max-width: 21em;
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(28px, 4vw, 38px);
    font-weight: 500;
    line-height: 1.16;
    letter-spacing: -0.025em;
  }

  .lead {
    max-width: 58ch;
    margin: 14px 0 28px;
    color: var(--text-2);
    font-size: 15px;
    line-height: 1.65;
  }

  .field-label,
  legend {
    display: block;
    margin: 0 0 8px;
    color: var(--text-2);
    font-size: 13px;
    font-weight: 600;
  }

  .locale-select {
    width: min(100%, 440px);
    min-height: 44px;
    padding: 8px 12px;
    border: 1px solid var(--hairline-2);
    border-radius: 9px;
    background: var(--panel);
    color: var(--text);
    font: inherit;
  }

  .welcome-actions { display: flex; align-items: center; gap: 24px; margin-top: 28px; }
  .welcome-next { min-width: 132px; min-height: 44px; }

  .language-choices {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin: 0 0 22px;
    padding: 0;
    border: 0;
  }

  .language-choices legend { width: 100%; }
  .language-choices label {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 44px;
    padding: 8px 13px;
    border-radius: 9px;
    background: var(--sunken);
    cursor: pointer;
  }
  .language-choices label.active { background: var(--selected); }
  .language-choices input,
  .punctuation-choice input { accent-color: var(--accent); }

  .hardware-line,
  .muted,
  .notice {
    margin: 12px 0;
    color: var(--text-2);
    font-size: 13px;
  }

  .recommendation,
  .model-optional,
  .permission-row,
  .paths {
    margin-top: 18px;
    padding: 17px 18px;
    border-radius: var(--radius);
    background: var(--sunken);
  }

  .recommendation p,
  .model-optional p,
  .permission-row p { margin: 0 0 9px; }
  .recommendation .muted { margin-top: -3px; }
  .recommendation .field-label { margin-top: 16px; }

  .optional-title { margin-bottom: 8px; font-weight: 600; }
  .model-optional p { color: var(--text-2); font-size: 13px; }
  .permission-status { color: var(--text-2); font-size: 13px; }

  .punctuation-choice {
    display: flex;
    align-items: flex-start;
    gap: 11px;
    margin-top: 16px;
    padding: 12px 2px;
    cursor: pointer;
  }
  .punctuation-choice input { margin-top: 4px; width: 17px; height: 17px; flex: none; }
  .punctuation-copy { display: grid; gap: 4px; }
  .punctuation-copy small { color: var(--text-2); }
  .tag {
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--chip);
    color: var(--text-2);
    font-size: 11px;
    font-weight: 500;
  }

  .permission-list { display: grid; gap: 12px; }
  .permission-row { display: flex; align-items: flex-start; justify-content: space-between; gap: 18px; margin: 0; }
  .permission-row h2 { margin: 0 0 8px; font-size: 16px; font-weight: 600; }
  .permission-row p { max-width: 56ch; color: var(--text); font-size: 14px; }
  .permission-row p.muted { color: var(--text-2); }
  .permission-actions { display: flex; align-items: center; min-height: 44px; }

  .paths { display: grid; gap: 14px; margin: 0; }
  .paths div { min-width: 0; }
  .paths dt { margin-bottom: 4px; color: var(--text-2); font-size: 12px; font-weight: 600; }
  .paths dd { margin: 0; min-width: 0; }
  code {
    overflow-wrap: anywhere;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .privacy-promise { margin: 24px 0 0; color: var(--text-2); }

  .finish-actions { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 30px; }
  .finish-actions button { min-height: 44px; }
  .error-message { margin-top: 18px; color: var(--red); font-size: 13px; }
  .footer { display: grid; grid-template-columns: 1fr auto 1fr; padding-top: 14px; }
  .footer .link-action { justify-self: start; min-height: 44px; }
  .footer .btn-primary { justify-self: end; }
  .footer .btn-primary { min-width: 120px; min-height: 44px; }

  @media (max-width: 600px) {
    .onboarding-screen { min-height: 460px; padding-inline: 20px; }
    .step-content { padding-block: 22px; }
    .permission-row { flex-direction: column; }
    .permission-actions { justify-content: flex-start; }
    .finish-actions { flex-direction: column; align-items: stretch; }
  }
</style>
