import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { shouldShowWhatsNew, markWhatsNewRead, whatsNewByVersion } from "../src/lib/whats-new.js";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

function storage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
}

test("a fresh install without a previously read version does not show What's New", () => {
  assert.equal(shouldShowWhatsNew(null, "1.8.0"), false);
});

test("an upgrade shows What's New once and closing records the current version", () => {
  const state = storage({ "lt.whats-new.read-version": "1.7.0" });
  assert.equal(shouldShowWhatsNew(state.getItem("lt.whats-new.read-version"), "1.8.0"), true);
  markWhatsNewRead(state, "1.8.0");
  assert.equal(shouldShowWhatsNew(state.getItem("lt.whats-new.read-version"), "1.8.0"), false);
});

test("the current version is not repeated after it has been read", () => {
  assert.equal(shouldShowWhatsNew("1.8.0", "1.8.0"), false);
});

test("1.8.0 What's New contains exactly the three finalized release notes", () => {
  assert.deepEqual(
    whatsNewByVersion["1.8.0"].map(({ titleKey, bodyKey }) => [titleKey, bodyKey]),
    [
      ["whatsNew.windowsTitle", "whatsNew.windowsBody"],
      ["whatsNew.modelsTitle", "whatsNew.modelsBody"],
      ["whatsNew.cardTitle", "whatsNew.cardBody"],
    ],
  );
});

test("1.8.1 What's New describes the Windows layout and concise update setting", () => {
  assert.deepEqual(
    whatsNewByVersion["1.8.1"].map(({ titleKey, bodyKey }) => [titleKey, bodyKey]),
    [
      ["whatsNew.layoutFixTitle", "whatsNew.layoutFixBody"],
      ["whatsNew.updateDescTitle", "whatsNew.updateDescBody"],
    ],
  );
});

test("all What's New keys exist in every supported locale", () => {
  const keys = [
    "whatsNew.title", "whatsNew.close", "whatsNew.windowsTitle", "whatsNew.windowsBody",
    "whatsNew.modelsTitle", "whatsNew.modelsBody", "whatsNew.cardTitle", "whatsNew.cardBody",
    "whatsNew.layoutFixTitle", "whatsNew.layoutFixBody", "whatsNew.updateDescTitle", "whatsNew.updateDescBody",
  ];
  for (const locale of ["zh", "en", "de", "es", "fr"]) {
    const messages = read(`../src/lib/i18n/messages/${locale}.ts`);
    for (const key of keys) assert.ok(messages.includes(`"${key}"`), `${locale} missing ${key}`);
  }
});
