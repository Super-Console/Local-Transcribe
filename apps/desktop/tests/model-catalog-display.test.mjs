import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

const modelView = read("../src/lib/components/ModelView.svelte");
const modelDownload = read("../src-tauri/src/model_download.rs");
const desktopCommands = read("../src-tauri/src/lib.rs");
const llmCatalog = read("../../../crates/lan-compute/src/llm.rs");
const locales = ["zh", "en", "de", "es", "fr"];

test("model catalog does not display per-model license notices", () => {
  assert.doesNotMatch(modelView, /model\.license|entry\.license/);
});

test("ASR, punctuation, and language-model rows all expose memory estimates", () => {
  assert.match(modelView, /model\.estimatedMemory/);
  assert.match(modelDownload, /estimated_asr_memory_bytes/);
  assert.match(desktopCommands, /estimated_asr_memory_bytes/);
  assert.match(modelDownload, /estimated_memory_bytes:\s*u64/);
});

test("each catalog row includes a localized usage recommendation in its subtitle", () => {
  assert.match(modelView, /recommendationFor/);
  assert.match(modelView, /recommendationForInstalledModel/);
  assert.match(modelView, /model\.recommendation/);
  assert.match(modelView, /model\.profileQwen4b/);
  assert.match(modelView, /model\.profileQwen8b/);
  assert.match(modelView, /model\.profileQwen17b/);
});

test("language-model guidance is no longer embedded in the model name", () => {
  assert.doesNotMatch(llmCatalog, /name:\s*"Qwen3-(?:4B|8B|1\.7B)[^"]*·/);
  assert.match(modelView, /model\.recommendQuality/);
  assert.match(modelView, /model\.recommendSpeed/);
  assert.match(modelView, /model\.recommendPunctuation/);
});

test("model recommendations are localized and license labels are removed in every locale", () => {
  const keys = [
    "model.recommendation",
    "model.recommendQuality",
    "model.recommendSpeed",
    "model.recommendPunctuation",
    "model.recommendTranscription",
    "model.profileQwen4b",
    "model.profileQwen8b",
    "model.profileQwen17b",
    "model.profileLanguageGeneral",
  ];
  for (const language of locales) {
    const messages = read(`../src/lib/i18n/messages/${language}.ts`);
    for (const key of keys) assert.match(messages, new RegExp(`"${key}"`), `${language}: ${key}`);
    assert.doesNotMatch(messages, /"model\.license"/);
  }
  const chinese = read("../src/lib/i18n/messages/zh.ts");
  for (const [key, copy] of [
    ["model.profileQwen4b", "中文默认"],
    ["model.profileQwen8b", "中文大模型"],
    ["model.profileQwen17b", "多语言轻量"],
  ]) {
    assert.match(chinese, new RegExp(`"${key}": "${copy}"`));
  }
});
