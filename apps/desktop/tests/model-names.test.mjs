import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { prettyNameFrom } from "../src/lib/model-names.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const read = (relative) => readFile(path.join(here, relative), "utf8");

const catalog = [
  {
    directory: "sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03",
    name: "X-ASR Zipformer",
  },
  {
    directory: "sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26",
    name: "FireRedASR2 (int8)",
  },
  {
    directory: "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
    name: "Qwen3-ASR 0.6B (int8)",
  },
  {
    directory: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
    name: "SenseVoice small（2025）",
  },
  {
    directory: "sherpa-onnx-paraformer-zh-2023-09-14",
    name: "Paraformer · 2023-09-14",
  },
  {
    directory: "sherpa-onnx-zipformer-ctc-small-zh-int8-2025-07-16",
    name: "Zipformer CTC small（体积最小）",
  },
  {
    directory: "sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12",
    name: "CT-Transformer punctuation",
  },
];

test("the seven catalog models resolve from folder names and ids", () => {
  for (const entry of catalog) {
    assert.equal(prettyNameFrom(entry.directory, catalog), entry.name);
    assert.equal(prettyNameFrom(`paraformer-${entry.directory}`, catalog), entry.name);
  }
});

test("fallback strips sherpa-onnx and a trailing date but never empties", () => {
  assert.equal(prettyNameFrom("sherpa-onnx-unknown-model-2024-01-02", []), "unknown-model");
  assert.equal(
    prettyNameFrom("paraformer-sherpa-onnx-paraformer-zh-2023-09-14", []),
    "paraformer-zh",
  );
  assert.equal(prettyNameFrom("lan-node", []), "lan-node");
  assert.equal(prettyNameFrom("", []), "");
});

test("history renders model names through prettyName, not raw ids", async () => {
  const source = await read("../src/lib/components/HistoryView.svelte");
  assert.match(source, /prettyName\(/);
  assert.doesNotMatch(source, /\{entry\.modelId\}/);
  assert.doesNotMatch(source, /\{hit\.modelId\}/);
});

test("model list keeps languages out of the title and in the subtitle", async () => {
  const source = await read("../src/lib/components/ModelView.svelte");
  assert.match(source, /\{entry\.name\}/);
  assert.match(source, /entry\.languages/);
  assert.doesNotMatch(source, /\{entry\.name\}\s*·\s*\{entry\.languages\}/);
});

test("history renders pretty names instead of raw model ids", async () => {
  const source = await read("../src/lib/components/HistoryView.svelte");
  assert.match(source, /prettyName\(/);
  assert.doesNotMatch(source, /\{detail\.modelId\}/);
});
