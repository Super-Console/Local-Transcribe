import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { recommendModels, onboardingSteps } from "../src/lib/onboarding-logic.mjs";

const gib = 1024 ** 3;
const models = {
  "zipformer-ctc-small-zh": 60 * 1024 ** 2,
  "sensevoice-small": 226 * 1024 ** 2,
  "x-asr-zh-en": 168 * 1024 ** 2,
  "fire-red-asr2": 1.15 * gib,
  "qwen3-4b": 2.5 * gib,
  "qwen3-8b": 5 * gib,
};

test("hardware tiers recommend the taskbook's Chinese and multilingual models", () => {
  assert.deepEqual(recommendModels({ memoryBytes: 8 * gib, availableBytes: 50 * gib, language: "zh", modelBytes: models }), {
    asrModelId: "zipformer-ctc-small-zh",
    languageModelId: null,
    diskDowngraded: false,
  });
  assert.deepEqual(recommendModels({ memoryBytes: 8 * gib, availableBytes: 50 * gib, language: "multi", modelBytes: models }), {
    asrModelId: "sensevoice-small",
    languageModelId: null,
    diskDowngraded: false,
  });
  assert.deepEqual(recommendModels({ memoryBytes: 16 * gib, availableBytes: 50 * gib, language: "zh", modelBytes: models }), {
    asrModelId: "x-asr-zh-en",
    languageModelId: "qwen3-4b",
    diskDowngraded: false,
  });
  assert.deepEqual(recommendModels({ memoryBytes: 24 * gib, availableBytes: 50 * gib, language: "multi", modelBytes: models }), {
    asrModelId: "x-asr-zh-en",
    languageModelId: "qwen3-8b",
    diskDowngraded: false,
  });
  assert.equal(
    recommendModels({ memoryBytes: 24 * gib, availableBytes: 50 * gib, language: "zh", modelBytes: models }).asrModelId,
    "fire-red-asr2",
  );
});

test("low disk space drops at most one tier and never removes the minimum ASR choice", () => {
  const result = recommendModels({
    memoryBytes: 24 * gib,
    availableBytes: 200 * 1024 ** 2,
    language: "zh",
    modelBytes: models,
  });
  assert.equal(result.asrModelId, "x-asr-zh-en");
  assert.equal(result.languageModelId, "qwen3-4b");
  assert.equal(result.diskDowngraded, true);

  const minimum = recommendModels({
    memoryBytes: 8 * gib,
    availableBytes: 1,
    language: "zh",
    modelBytes: models,
  });
  assert.equal(minimum.asrModelId, "zipformer-ctc-small-zh");
  assert.equal(minimum.diskDowngraded, true);
});

test("language-model disk downgrade is independent from the ASR recommendation", () => {
  const result = recommendModels({
    memoryBytes: 24 * gib,
    availableBytes: 6 * gib,
    language: "zh",
    modelBytes: models,
  });
  assert.equal(result.asrModelId, "fire-red-asr2");
  assert.equal(result.languageModelId, "qwen3-4b");
  assert.equal(result.diskDowngraded, true);
});

test("onboarding uses the disk-aware language-model recommendation and does not infer transcription language from UI locale", () => {
  const component = readFileSync(
    new URL("../src/lib/components/OnboardingView.svelte", import.meta.url),
    "utf8",
  );
  assert.match(component, /recommendation\.languageModelId/);
  assert.doesNotMatch(component, /locale\.resolved\s*===\s*"zh-CN"/);
});

test("permissions step is omitted on Windows while other onboarding steps remain", () => {
  assert.deepEqual(onboardingSteps("macos"), ["welcome", "models", "permissions", "privacy", "finish"]);
  assert.deepEqual(onboardingSteps("windows"), ["welcome", "models", "privacy", "finish"]);
});

test("OOBE stays skippable, localized, and can be restarted from settings", () => {
  const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
  const onboarding = read("../src/lib/components/OnboardingView.svelte");
  const page = read("../src/routes/+page.svelte");
  const settings = read("../src/lib/components/SettingsView.svelte");

  assert.match(onboarding, /onboarding\.skip/);
  assert.match(onboarding, /onboarding\.previous/);
  assert.match(onboarding, /onboarding\.step/);
  assert.match(page, /get_onboarding_state/);
  assert.match(onboarding, /complete_onboarding/);
  assert.match(settings, /onboarding\.rerun/);
});

test("the previous-step action stays available on the final screen", () => {
  const onboarding = readFileSync(
    new URL("../src/lib/components/OnboardingView.svelte", import.meta.url),
    "utf8",
  );
  assert.match(onboarding, /\{#if step !== "welcome"\}[\s\S]*?onboarding\.previous/);
});

test("an uninstalled local preference does not block reachable LAN transcription", () => {
  const source = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
  const remoteDispatch = source.indexOf("if let Some(node) = dispatch_node");
  const localPreferenceGuard = source.indexOf(
    "if let Some(preferred_id) = onboarding::read_state(&data_dir)?.preferred_asr_model_id",
  );
  assert.ok(remoteDispatch >= 0, "LAN dispatch branch exists");
  assert.ok(localPreferenceGuard > remoteDispatch, "local preference guard only applies after LAN dispatch");
});

test("every onboarding message exists in all five locale catalogs", () => {
  const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
  const locales = ["zh", "en", "de", "es", "fr"];
  const keysByLocale = new Map(
    locales.map((language) => {
      const source = read(`../src/lib/i18n/messages/${language}.ts`);
      return [language, new Set(source.match(/"onboarding\.[\w]+"/g) ?? [])];
    }),
  );
  const expected = keysByLocale.get("zh");
  for (const [language, keys] of keysByLocale) {
    assert.deepEqual([...keys].sort(), [...expected].sort(), `${language} onboarding translations differ`);
  }
});

test("hardware recommendation code performs no network request", () => {
  const logic = readFileSync(new URL("../src/lib/onboarding-logic.mjs", import.meta.url), "utf8");
  const hardware = readFileSync(new URL("../src-tauri/src/onboarding.rs", import.meta.url), "utf8");
  assert.doesNotMatch(logic, /fetch\s*\(|XMLHttpRequest|https?:\/\//);
  assert.doesNotMatch(hardware, /reqwest|TcpStream|UdpSocket|https?:\/\//);
});
