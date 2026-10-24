import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { prettyNameFrom } from "../src/lib/model-names.js";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

// 与 apps/desktop/src-tauri/src/model_download.rs 的 CATALOG 同名同目录。
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
];

// 节点上的模型 id 是 `family-目录名`（inspect_model_directory），不是本地清单 id。
const remoteId =
  "transducer-sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03";
const localId = "paraformer-sherpa-onnx-paraformer-zh-2023-09-14";

const page = read("../src/routes/+page.svelte");
const transcribe = read("../src/lib/components/TranscribeView.svelte");
const history = read("../src/lib/components/HistoryView.svelte");
const tauri = read("../src-tauri/src/lib.rs");

test("a remote task shows its own model even when no local model is selected", () => {
  // 断言 1：记录里的 model_id 命中清单名；当前选中为空也显示模型名。
  assert.equal(prettyNameFrom(remoteId, catalog), "X-ASR Zipformer");
  assert.match(
    page,
    /const taskModelId = \$derived\(transcript\?\.modelId\?\.trim\(\) \|\| startedModelId\)/,
  );
  assert.match(page, /modelId=\{taskModelId\}/);
  assert.match(transcribe, /prettyName\(modelId\)/);
  assert.doesNotMatch(transcribe, /common\.notSelected/);
  assert.doesNotMatch(transcribe, /model\?\.displayName/);
});

test("an empty model id shows the unknown label, never 未选择", () => {
  // 断言 2：记录里也没有 model_id 时是「未知模型」。
  assert.equal(prettyNameFrom("", catalog), "");
  assert.match(
    transcribe,
    /prettyName\(modelId\) \|\| t\("common\.unknownModel"\)/,
  );
  assert.doesNotMatch(transcribe, /common\.notSelected/);
  const labels = [
    ["zh", "未知模型"],
    ["en", "Unknown model"],
    ["de", "Unbekanntes Modell"],
    ["es", "Modelo desconocido"],
    ["fr", "Modèle inconnu"],
  ];
  for (const [lang, value] of labels) {
    const messages = read(`../src/lib/i18n/messages/${lang}.ts`);
    assert.match(messages, new RegExp(`"common\\.unknownModel": "${value}"`));
  }
  // 旧 key 保留，只用于"还没选模型、也没有任务"的空状态。
  assert.match(read("../src/lib/i18n/messages/zh.ts"), /"common\.notSelected": "未选择"/);
});

test("a local transcription keeps showing the model it was started with", () => {
  // 断言 3：本机任务的 model_id 就是选中模型，展示不变。
  assert.equal(prettyNameFrom(localId, catalog), "Paraformer · 2023-09-14");
  assert.match(
    page,
    /startedModelId = remoteSelection\?\.modelId \?\? model\?\.id \?\? null/,
  );
});

test("history cards and detail show the recorded model, not 未选择", () => {
  // 断言 4：远程任务的历史卡片显示远端模型名。
  assert.match(history, /prettyName\(entry\.modelId\) \|\| t\("common\.unknownModel"\)/);
  assert.match(history, /prettyName\(hit\.modelId\) \|\| t\("common\.unknownModel"\)/);
  assert.match(history, /prettyName\(detail\.modelId\) \|\| t\("common\.unknownModel"\)/);
  assert.doesNotMatch(history, /common\.notSelected/);
});

test("the exported JSON carries the record's model id", () => {
  // 断言 5：导出以 jobId 读回已保存的转写记录，JSON 直接由记录序列化。
  assert.match(page, /const jobId = transcript\?\.jobId \?\? job\?\.jobId/);
  assert.match(tauri, /let transcript = load_transcript\(&app, state\.inner\(\), &job_id\)\?/);
  assert.match(tauri, /"json" => transcript[\s\S]{0,40}\.to_json\(\)/);
  // 服务端导出同一条记录；Rust 测试 export_json_uses_the_recording_model_not_a_current_selection。
  assert.match(
    read("../../../crates/transcription-service/src/lib.rs"),
    /assert_eq!\(exported\["modelId"\], model_id\)/,
  );
});

test("the toolbar and settings keep describing the current selection", () => {
  // 断言 6：顶栏与设置页仍拿"当前选中"，只改描述结果的地方。
  assert.match(page, /<AppToolbar\s+[\s\S]{0,40}\{model\}/);
  assert.match(page, /<SettingsView\s+[\s\S]{0,40}\{model\}/);
  assert.doesNotMatch(page, /<TranscribeView\s+model=/);
  assert.doesNotMatch(page, /prettyName\(model\?\.displayName\)/);
});
