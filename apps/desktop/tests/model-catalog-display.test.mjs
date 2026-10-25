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
  // 2026-10-25：副标题不再带「建议：」前缀，直接显示短语本身。
  assert.doesNotMatch(modelView, /model\.recommendation/);
  assert.match(modelView, /model\.profileQwen4b/);
  assert.match(modelView, /model\.profileQwen8b/);
  assert.match(modelView, /model\.profileQwen17b/);
});

test("every ASR catalog model carries its own distinct suggestion label", () => {
  // 2026-10-25：准确优先/速度优先下的每个模型不能共用「准确优先」「速度优先」
  // 一句占位；各自要有按语言模型写法的独立短语（如「中文均衡」）。
  for (const [id, key] of [
    ["x-asr-zh-en", "model.profileXasr"],
    ["fire-red-asr2", "model.profileFireRed"],
    ["qwen3-asr-0.6b", "model.profileQwen3Asr"],
    ["sensevoice-small", "model.profileSenseVoice"],
    ["paraformer-zh", "model.profileParaformer"],
    ["zipformer-ctc-small-zh", "model.profileZipformerCtc"],
  ]) {
    assert.match(modelView, new RegExp(`case "${id}"[\\s\\S]*?return "${key}"`), id);
  }
  // 分组兜底键保留给清单外未来条目，但清单内六个模型不再落到它们。
  assert.match(modelView, /model\.recommendQuality/);
  assert.match(modelView, /model\.recommendSpeed/);
});

test("language-model guidance is no longer embedded in the model name", () => {
  assert.doesNotMatch(llmCatalog, /name:\s*"Qwen3-(?:4B|8B|1\.7B)[^"]*·/);
  assert.match(modelView, /model\.recommendQuality/);
  assert.match(modelView, /model\.recommendSpeed/);
  assert.match(modelView, /model\.recommendPunctuation/);
});

test("model recommendations are localized and license labels are removed in every locale", () => {
  const keys = [
    "model.recommendQuality",
    "model.recommendSpeed",
    "model.recommendPunctuation",
    "model.recommendTranscription",
    "model.profileQwen4b",
    "model.profileQwen8b",
    "model.profileQwen17b",
    "model.profileLanguageGeneral",
    "model.profileXasr",
    "model.profileFireRed",
    "model.profileQwen3Asr",
    "model.profileSenseVoice",
    "model.profileParaformer",
    "model.profileZipformerCtc",
  ];
  for (const language of locales) {
    const messages = read(`../src/lib/i18n/messages/${language}.ts`);
    for (const key of keys) assert.match(messages, new RegExp(`"${key}"`), `${language}: ${key}`);
    // 「建议：」前缀模板已删除，副标题直接显示短语。
    assert.doesNotMatch(messages, /"model\.recommendation"/, `${language}: recommendation prefix`);
    assert.doesNotMatch(messages, /"model\.license"/);
  }
  const chinese = read("../src/lib/i18n/messages/zh.ts");
  for (const [key, copy] of [
    ["model.profileQwen4b", "中文均衡"],
    ["model.profileQwen8b", "中文大模型"],
    ["model.profileQwen17b", "多语言轻量"],
    ["model.profileXasr", "中英双语最准"],
    ["model.profileFireRed", "中英大模型"],
    ["model.profileQwen3Asr", "千问同款内核"],
    ["model.profileSenseVoice", "五语速转"],
    ["model.profileParaformer", "中文速转"],
    ["model.profileZipformerCtc", "轻量极速"],
  ]) {
    assert.match(chinese, new RegExp(`"${key}": "${copy}"`), key);
  }
});
