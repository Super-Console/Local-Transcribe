import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { catalogEntryFor } from "../src/lib/model-names.js";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

const catalog = [
  {
    directory: "sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03",
    name: "X-ASR Zipformer",
  },
  {
    directory: "sherpa-onnx-paraformer-zh-2023-09-14",
    name: "Paraformer · 2023-09-14",
  },
];

const history = read("../src/lib/components/HistoryView.svelte");
const page = read("../src/routes/+page.svelte");
const modelView = read("../src/lib/components/ModelView.svelte");
const tauri = read("../src-tauri/src/lib.rs");

test("a history card opens a one-item menu from both mouse and keyboard", () => {
  assert.match(history, /oncontextmenu=\{\(event\) => openMenu\(event, entry\)\}/);
  assert.match(history, /event\.key === "F10" && event\.shiftKey/);
  assert.match(history, /event\.key !== "ContextMenu"/);
  assert.match(history, /role="menuitem"/);
  assert.doesNotMatch(history, /role="menu"[\s\S]{0,400}role="menuitem"[\s\S]{0,200}role="menuitem"/);
  assert.match(history, /if \(event\.key === "Escape"\) close\(\);/);
});

test("deleting asks first and states that the source media is untouched", () => {
  for (const lang of ["zh", "en", "de", "es", "fr"]) {
    const messages = read(`../src/lib/i18n/messages/${lang}.ts`);
    assert.match(messages, /"dialog\.deleteHistoryMessage":/);
  }
  const zh = read("../src/lib/i18n/messages/zh.ts");
  assert.match(zh, /不会动原始音视频文件/);
  assert.match(page, /invoke<\{ removed: string\[\]; skipped: string\[\] \}>\(\s*"delete_history_entry"/);
  assert.match(page, /dialog\.deleteHistoryMessage/);
});

test("a running task cannot be deleted from the menu", () => {
  assert.match(history, /disabled=\{menu\.jobId === runningJobId\}/);
  assert.match(history, /t\("history\.cancelFirst"\)/);
  assert.match(
    page,
    /const runningJobId = \$derived\(job\?\.state === "running" \? job\.jobId : null\)/,
  );
  assert.match(page, /\{runningJobId\}/);
});

test("the backend only moves the three record files and never the dispatch lock", () => {
  assert.match(tauri, /fn delete_history_entry_at\(/);
  assert.match(tauri, /format!\("\{job_id\}\.json"\)/);
  assert.match(tauri, /format!\("\{job_id\}\.cancel"\)/);
  const body = tauri.slice(
    tauri.indexOf("fn delete_history_entry_at("),
    tauri.indexOf("fn delete_history_entry("),
  );
  assert.doesNotMatch(body, /\.dispatch\.lock/);
  assert.match(body, /fs::rename\(&path, &target\)/);
});

test("installed manifest models appear once; only extras get their own section", () => {
  assert.doesNotMatch(modelView, /已安装的模型/);
  assert.match(modelView, /model\.otherInstalled/);
  assert.match(modelView, /\{:else if extraModels\.length > 0\}/);
  assert.match(modelView, /models\.filter\(\(model\) => !catalogEntryFor\(model\.id, catalog\)\)/);
  for (const lang of ["zh", "en", "de", "es", "fr"]) {
    const messages = read(`../src/lib/i18n/messages/${lang}.ts`);
    assert.match(messages, /"model\.otherInstalled":/);
    assert.doesNotMatch(messages, /"model\.installedTitle":/);
  }
});

test("the manifest match used for dedup accepts family-prefixed ids", () => {
  assert.equal(
    catalogEntryFor(
      "transducer-sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03",
      catalog,
    ).name,
    "X-ASR Zipformer",
  );
  assert.equal(catalogEntryFor("sherpa-onnx-paraformer-zh-2023-09-14", catalog).name, "Paraformer · 2023-09-14");
  assert.equal(catalogEntryFor("old-model-from-1.0", catalog), null);
  assert.equal(catalogEntryFor("", catalog), null);
});
