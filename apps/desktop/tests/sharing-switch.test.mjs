import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

const langs = ["zh", "en", "de", "es", "fr"];
const messages = Object.fromEntries(
  langs.map((lang) => [lang, read(`../src/lib/i18n/messages/${lang}.ts`)]),
);
const lanView = read("../src/lib/components/LanView.svelte");
const toolbar = read("../src/lib/components/AppToolbar.svelte");
const css = read("../src/app.css");

test("the old switch wording and the memory hint are gone from all five locales", () => {
  const keys = [
    "lan.hostActionOn",
    "lan.hostActionOff",
    "lan.hostStateOn",
    "lan.hostStateOff",
    "lan.hostChangingOn",
    "lan.hostChangingOff",
  ];
  for (const lang of langs) {
    const source = messages[lang];
    for (const key of ["lan.hostOn", "lan.hostOff", "lan.hostChanging", "lan.memoryHint"]) {
      assert.doesNotMatch(source, new RegExp(`"${key.replace(".", "\\.")}":`), `${lang} still has ${key}`);
    }
    for (const key of keys) {
      const match = source.match(new RegExp(`"${key.replace(".", "\\.")}": "([^"]*)"`));
      assert.ok(match, `${lang} is missing ${key}`);
      assert.doesNotMatch(match[1], /点击|Turn on|Turn off/);
    }
  }
});

test("the state dot is a shared 7x7 class used by both components", () => {
  assert.match(css, /\.status-dot\s*\{[^}]*width:\s*7px;[^}]*height:\s*7px;[^}]*border-radius:\s*50%;[^}]*background:\s*#cbc5b7;/s);
  assert.match(css, /\.status-dot\.on\s*\{[^}]*background:\s*var\(--green\);/s);
  assert.match(css, /:root\[data-theme="dark"\] \.status-dot\s*\{[^}]*background:\s*#5a5a60;/s);
  assert.match(lanView, /class="status-dot" class:on=\{host\.running\}/);
  assert.match(toolbar, /class="status-dot"/);
  assert.doesNotMatch(toolbar, /^\s*\.dot,?$/m);
  assert.doesNotMatch(toolbar, /\.item-dot/);
});

test("the dot is hidden from screen readers and the button names its state", () => {
  assert.match(lanView, /class="status-dot" class:on=\{host\.running\} aria-hidden="true"/);
  assert.doesNotMatch(lanView, /role="switch"/);
  assert.doesNotMatch(lanView, /aria-checked/);
  assert.match(
    lanView,
    /aria-label=\{`\$\{t\("lan\.hostTitle"\)\}, \$\{host\.running \? t\("lan\.hostStateOn"\) : t\("lan\.hostStateOff"\)\}`\}/,
  );
});

test("the button only says what it will do, and shows progress while busy", () => {
  assert.match(lanView, /t\("lan\.hostActionOff"\) : t\("lan\.hostActionOn"\)/);
  assert.match(lanView, /host\.running \? t\("lan\.hostChangingOff"\) : t\("lan\.hostChangingOn"\)/);
  assert.match(lanView, /aria-busy=\{hostBusy \? "true" : undefined\}/);
  assert.match(lanView, /disabled=\{hostBusy \|\| \(!host\.running && !hostAddress\)\}/);
});

test("the sharing block no longer mentions memory or GB", () => {
  assert.doesNotMatch(lanView, /内存/);
  assert.doesNotMatch(lanView, /memoryHint/);
  // 该区块的文案（标题/说明/按钮/状态）不含内存或容量数字。
  const block = lanView.slice(
    lanView.indexOf('{#if host?.supported}'),
    lanView.indexOf('{#if restartNeeded}'),
  );
  assert.doesNotMatch(block, /GB|内存/);
});
