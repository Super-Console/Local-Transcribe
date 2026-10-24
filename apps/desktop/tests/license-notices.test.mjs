import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

const notices = read("../../../THIRD-PARTY-NOTICES.md");
const modelLicenses = read("../../../docs/model-licenses.md");
const rootManifest = read("../../../Cargo.toml");
const tauriConfig = JSON.parse(read("../src-tauri/tauri.conf.json"));
const settings = read("../src/lib/components/SettingsView.svelte");
const page = read("../src/routes/+page.svelte");

test("every crate declares MIT and nothing is UNLICENSED", () => {
  const manifests = [
    "../../../crates/transcription-core/Cargo.toml",
    "../../../crates/transcription-service/Cargo.toml",
    "../../../crates/transcription-cli/Cargo.toml",
    "../../../crates/transcription-mcp/Cargo.toml",
    "../../../crates/lan-compute/Cargo.toml",
  ];
  for (const manifest of manifests) {
    const source = read(manifest);
    assert.match(source, /^license = "MIT"$/m, manifest);
    assert.doesNotMatch(source, /UNLICENSED/, manifest);
  }
  assert.match(rootManifest, /\[workspace\.package\][\s\S]*?license = "MIT"/);
  assert.equal(JSON.parse(read("../package.json")).license, "MIT");
});

test("the bundle carries the copyright and both notice files", () => {
  assert.equal(tauriConfig.bundle.copyright, "Copyright © 2026 TimmySheep");
  const resources = tauriConfig.bundle.resources;
  assert.equal(resources["../../../LICENSE"], "LICENSE");
  assert.equal(resources["../../../THIRD-PARTY-NOTICES.md"], "THIRD-PARTY-NOTICES.md");
});

test("the notices name the two components that need special handling", () => {
  assert.match(notices, /sherpa-onnx/);
  assert.match(notices, /Apache-2\.0/);
  assert.match(notices, /symphonia/i);
  assert.match(notices, /MPL-2\.0/);
  assert.match(notices, /静态链接|statically links/);
  assert.match(notices, /未修改|unmodified/);
});

test("the settings page can open the notices that ship with the app", () => {
  assert.match(settings, /onOpenNotices/);
  assert.match(settings, /settings\.openNotices/);
  assert.match(page, /third_party_notices_path/);
  assert.match(page, /onOpenNotices=\{openThirdPartyNotices\}/);
});

test("both READMEs keep the name and icon out of the MIT grant", () => {
  const en = read("../../../README.md");
  const zh = read("../../../README.zh-CN.md");
  assert.match(en, /name "Local Transcribe" and the application icon are not covered/);
  assert.match(zh, /名称与应用图标不在该许可证范围内/);
});

test("all seven catalog models were checked against their model cards", () => {
  const rows = modelLicenses.split("\n").filter((line) => line.startsWith("| ") && line.includes("huggingface.co"));
  assert.equal(rows.length, 7, "one row per catalog model");
  assert.match(modelLicenses, /未标注/);
  assert.match(modelLicenses, /apache-2\.0/i);
  assert.match(modelLicenses, /2026-10-24/);
  for (const repository of [
    "csukuangfj2/sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03",
    "csukuangfj2/sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26",
    "csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
    "csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
    "csukuangfj/sherpa-onnx-paraformer-zh-2023-09-14",
    "csukuangfj/sherpa-onnx-zipformer-ctc-small-zh-int8-2025-07-16",
    "csukuangfj/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12",
  ]) {
    assert.match(modelLicenses, new RegExp(repository.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
});
