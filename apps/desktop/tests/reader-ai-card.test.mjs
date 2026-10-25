import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { nextAskTurn } from "../src/lib/ai-ask.js";

const here = fileURLToPath(new URL("..", import.meta.url));
const read = (relative) => readFileSync(join(here, relative), "utf8");

const card = read("src/lib/components/AiAskCard.svelte");
const reader = read("src/lib/components/Reader.svelte");

test("1. the answer card is a flat panel: radius 12, hairline, no shadow, 60vh cap", () => {
  assert.match(card, /\.ai-card\s*\{[^}]*border-radius:\s*12px;/s);
  assert.match(card, /\.ai-card\s*\{[^}]*border:\s*1px solid var\(--hairline\);/s);
  assert.match(card, /\.ai-card\s*\{[^}]*background:\s*var\(--panel\);/s);
  assert.match(card, /\.ai-card\s*\{[^}]*max-height:\s*60vh;/s);
  assert.match(card, /\.ai-card\s*\{[^}]*overflow:\s*hidden;/s);
  assert.match(
    card,
    /\.ai-answer\s*\{[^}]*overflow-y:\s*auto;/s,
    "the answer region scrolls inside the card",
  );
  assert.match(card, /\.ai-answer\s*\{[^}]*padding:[^;]*16px/s);
  const cardBlock = card.match(/\.ai-card\s*\{[^}]*\}/s)?.[0] ?? "";
  assert.doesNotMatch(cardBlock, /box-shadow/, "the card body must not carry a shadow");
});

test("2. header actions are plain text: 12px text-2, hover text, 12px gap, no pill", () => {
  assert.match(card, /\.ai-action\s*\{[^}]*font-size:\s*12px;/s);
  assert.match(card, /\.ai-action\s*\{[^}]*color:\s*var\(--text-2\);/s);
  assert.match(card, /\.ai-action:hover[^{]*\{[^}]*color:\s*var\(--text\);/s);
  assert.match(card, /\.ai-actions\s*\{[^}]*gap:\s*12px;/s);
  const actionsBlock = card.match(/\.ai-actions\s*\{[^}]*\}/s)?.[0] ?? "";
  assert.doesNotMatch(actionsBlock, /border-radius/, "actions stay text, never capsules");
});

test("3. answer body: 13px / 1.85, 10px paragraphs, 20px list indent, weight-only emphasis", () => {
  assert.match(card, /\.ai-answer\s*\{[^}]*font-size:\s*13px;/s);
  assert.match(card, /\.ai-answer\s*\{[^}]*line-height:\s*1\.85;/s);
  assert.match(card, /\.ai-answer\s+:global\(p\)\s*\{[^}]*margin[^;]*10px/s);
  assert.match(card, /\.ai-answer\s+:global\(ul\)\s*\{[^}]*padding-left:\s*20px;/s);
  assert.match(card, /\.ai-answer\s+:global\(li\)\s*\{[^}]*margin-bottom:\s*4px;/s);
  assert.match(card, /\.ai-answer\s+:global\(strong\)\s*\{[^}]*font-weight:\s*500;/s);
});

test("4. source line is quiet: 11.5px text-3, 10px spacing, no divider", () => {
  assert.match(card, /\.ai-source\s*\{[^}]*font-size:\s*11\.5px;/s);
  assert.match(card, /\.ai-source\s*\{[^}]*color:\s*var\(--text-3\);/s);
  const sourceBlock = card.match(/\.ai-source\s*\{[^}]*\}/s)?.[0] ?? "";
  assert.doesNotMatch(sourceBlock, /border-top|box-shadow/, "no divider under the answer");
});

test("5. input bar is 34px / radius 10 with the terracotta only on 发送", () => {
  assert.match(card, /\.ai-input\s*\{[^}]*height:\s*34px;/s);
  assert.match(card, /\.ai-input\s*\{[^}]*border-radius:\s*10px;/s);
  assert.match(card, /\.ai-input\s*\{[^}]*background:\s*var\(--fill\);/s);
  assert.match(card, /\.ai-input\s*\{[^}]*padding:[^;]*12px/s);
  assert.match(card, /\.ai-send\s*\{[^}]*height:\s*34px;/s);
  assert.match(card, /\.ai-send\s*\{[^}]*border-radius:\s*10px;/s);
  assert.match(card, /\.ai-send\s*\{[^}]*background:\s*var\(--accent\);/s);
  assert.match(card, /\.ai-send\s*\{[^}]*color:\s*#fffefb;/s);
  assert.match(card, /\.ai-send\.stop\s*\{[^}]*background:\s*transparent;/s);
});

test("6. chips: 28px, full radius, fill background, never terracotta", () => {
  assert.match(card, /\.ai-chip\s*\{[^}]*height:\s*28px;/s);
  assert.match(card, /\.ai-chip\s*\{[^}]*border-radius:\s*999px;/s);
  assert.match(card, /\.ai-chip\s*\{[^}]*background:\s*var\(--fill\);/s);
  assert.match(card, /\.ai-chip\s*\{[^}]*font-size:\s*12px;/s);
  assert.match(card, /\.ai-chips\s*\{[^}]*gap:\s*8px;/s);
  const chipBlock = card.match(/\.ai-chip\s*\{[^}]*\}/s)?.[0] ?? "";
  assert.doesNotMatch(chipBlock, /accent/, "chips never use the terracotta accent");
});

test("7. loading strip: 2px, 8s sweep, 3s breathing, reduced-motion static, no bounce", () => {
  assert.match(card, /\.ai-strip\s*\{[^}]*height:\s*2px;/s);
  assert.match(card, /8s/, "the sweep runs one 8 second loop");
  assert.match(card, /3s/, "the breathing runs a 3 second round trip");
  assert.match(card, /prefers-reduced-motion:\s*reduce/);
  assert.doesNotMatch(
    card,
    /animation[^;}]*(rotate|bounce|spin|flash|particle)/i,
    "no rotation, bounce, flicker or particles",
  );
  assert.doesNotMatch(card, /\.ai-strip[^{]*\{[^}]*box-shadow/, "the strip is not a glow");
});

test("8. collapsed row: one line with 展开, 28px, no border", () => {
  assert.match(card, /\.ai-collapsed\s*\{[^}]*height:\s*28px;/s);
  const collapsedBlock = card.match(/\.ai-collapsed\s*\{[^}]*\}/s)?.[0] ?? "";
  assert.doesNotMatch(collapsedBlock, /border:/, "the collapsed row stays borderless");
});

test("9. reader owns the card: toolbar entry, ⌘⇧A toggle, scroll compensation", () => {
  assert.match(reader, /AiAskCard/);
  assert.match(reader, /aiAsk/);
  assert.match(reader, /metaKey\s*\|\|\s*.*ctrlKey/);
  assert.match(reader, /shiftKey/);
  assert.match(reader, /scrollTop\s*\+=/, "inserting the card compensates scrollTop");
});

test("10. five-language keys cover the ask flow", () => {
  const languages = ["zh", "en", "de", "es", "fr"];
  const keys = [
    "reader.aiAsk",
    "reader.aiSummarize",
    "reader.aiPoints",
    "reader.aiTodos",
    "reader.aiExplain",
    "reader.aiSource",
    "reader.aiTurnLimit",
    "reader.aiNoModel",
    "reader.aiLowMemory",
  ];
  for (const language of languages) {
    const messages = read(`src/lib/i18n/messages/${language}.ts`);
    for (const key of keys) {
      assert.match(messages, new RegExp(`"${key}"`), `${language} carries ${key}`);
    }
  }
});

test("10b. source row reports transcript sections without claiming factual containment", () => {
  const expected = {
    zh: "基于全文 {count} 个片段生成",
    en: "Generated from {count} transcript sections",
    de: "Aus {count} Abschnitten erzeugt",
    es: "Generado a partir de {count} secciones",
    fr: "Généré à partir de {count} sections",
  };
  for (const [language, source] of Object.entries(expected)) {
    const messages = read(`src/lib/i18n/messages/${language}.ts`);
    const actual = messages.match(/"reader\.aiSource":\s*"([^"]+)"/)?.[1];
    assert.equal(actual, source, `${language} source copy`);
  }
});

test("11. follow-ups stop at three turns and reset with a prompt", () => {
  assert.equal(nextAskTurn({ turns: [], count: 0 }).allowed, true);
  assert.equal(nextAskTurn({ turns: [{ question: "q1", answer: "a1" }], count: 1 }).allowed, true);
  const third = nextAskTurn({
    turns: [
      { question: "q1", answer: "a1" },
      { question: "q2", answer: "a2" },
    ],
    count: 2,
  });
  assert.equal(third.allowed, true);
  const fourth = nextAskTurn({
    turns: [
      { question: "q1", answer: "a1" },
      { question: "q2", answer: "a2" },
      { question: "q3", answer: "a3" },
    ],
    count: 3,
  });
  assert.equal(fourth.allowed, false);
  assert.equal(fourth.reset, true, "the fourth attempt clears the history");
  assert.deepEqual(fourth.history, []);
  assert.equal(fourth.count, 1, "the fresh context counts this question as turn one");
});

test("12. long transcript stages are shown in the answer card in all five languages", () => {
  assert.match(card, /kind === "stage"/);
  assert.match(card, /kind === "partial_token"/);
  assert.match(card, /reader\.aiReadingSegment/);
  assert.match(card, /reader\.aiMerging/);
  assert.match(card, /reader\.aiPartialPreview/);
  for (const language of ["zh", "en", "de", "es", "fr"]) {
    const messages = read(`src/lib/i18n/messages/${language}.ts`);
    assert.match(messages, /"reader\.aiReadingSegment"/);
    assert.match(messages, /"reader\.aiMerging"/);
    assert.match(messages, /"reader\.aiPartialPreview"/);
  }
});

test("13. the ask card sends the resolved UI language with every generation", () => {
  // 2026-10-25：中文界面下 Qwen 用英文总结，因为请求没带语言。
  assert.match(card, /import \{ t, locale \} from "\$lib\/i18n\.svelte"/);
  assert.match(card, /language: locale\.resolved/);

  // Rust 端必须覆盖应用发布的每一种语言；未知语言不强加指令。
  const llm = readFileSync(join(here, "src-tauri/src/llm.rs"), "utf8");
  const directive = llm.match(/fn answer_language_directive[\s\S]*?\n\}/)?.[0] ?? "";
  for (const [code, phrase] of [
    ["zh-CN", "Simplified Chinese"],
    ["en", "English"],
    ["es", "Spanish"],
    ["fr", "French"],
    ["de", "German"],
  ]) {
    assert.ok(
      directive.includes(`Some("${code}")`) && directive.includes(phrase),
      `llm.rs must map ${code} to "${phrase}"`,
    );
  }
});
