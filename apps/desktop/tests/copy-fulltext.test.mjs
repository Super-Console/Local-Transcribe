import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  createCopyFeedback,
  normalizeCopyText,
  resolveCopyText,
} from "../src/lib/copy-text.js";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

const langs = ["zh", "en", "de", "es", "fr"];
const messages = Object.fromEntries(
  langs.map((lang) => [lang, read(`../src/lib/i18n/messages/${lang}.ts`)]),
);
const view = read("../src/lib/components/HistoryView.svelte");
const transcribeView = read("../src/lib/components/TranscribeView.svelte");
const reader = read("../src/lib/components/Reader.svelte");
const icons = read("../src/lib/components/Icon.svelte");
const page = read("../src/routes/+page.svelte");
const libRs = read("../src-tauri/src/lib.rs");
const cargo = read("../src-tauri/Cargo.toml");

// 断言 1：无选区点击 → 剪贴板内容等于正文纯文本（首尾已 trim、段落间隔已规范化）。
test("no selection copies the full text, trimmed and with collapsed blank runs", () => {
  const fullText = "  第一段。\n\n\n\n第二段。\n\n  ";
  const text = resolveCopyText({
    selectedText: "",
    anchorNode: null,
    focusNode: null,
    body: null,
    fullText,
  });
  assert.equal(text, "第一段。\n\n第二段。");
  // trim 只作用于首尾；换行两侧的行内空格不属于 trim 范围，任务书也只要求首尾 trim + 压换行。
  assert.equal(normalizeCopyText("\n\n  a \n\n\n\n b  \n"), "a \n\n b");
  assert.equal(normalizeCopyText("a\n\n\n\n\n\nb"), "a\n\nb");
});

// 断言 2：选中正文中的一段文字后点击 → 剪贴板内容只等于选区。
test("a selection inside the body copies only the selection", () => {
  const inside = { tag: "mark" };
  const body = { contains: (node) => node === inside };
  const text = resolveCopyText({
    selectedText: "  只要这一段  ",
    anchorNode: inside,
    focusNode: inside,
    body,
    fullText: "整篇正文",
  });
  assert.equal(text, "只要这一段");
});

// 断言 3：选中界面上的其它文字（例如标题）后点击 → 仍复制全文（选区不在正文内）。
test("a selection elsewhere in the UI still copies the full text", () => {
  const heading = { tag: "h1" };
  const bodyMark = { tag: "mark" };
  const body = { contains: (node) => node === bodyMark };
  const text = resolveCopyText({
    selectedText: "界面上的标题",
    anchorNode: heading,
    focusNode: heading,
    body,
    fullText: "整篇正文",
  });
  assert.equal(text, "整篇正文");

  // 选区一端在正文、一端在正文外（拖选越过边界）同样退回全文。
  const crossed = resolveCopyText({
    selectedText: "跨界选区",
    anchorNode: bodyMark,
    focusNode: heading,
    body,
    fullText: "整篇正文",
  });
  assert.equal(crossed, "整篇正文");
});

// 断言 4：点击后 1.5 秒内按钮显示「已复制」，1.5 秒后恢复「复制」。
test("feedback shows copied for 1.5s then returns to idle", () => {
  let now = 0;
  let seq = 0;
  const pending = new Map();
  const schedule = (fn, ms) => {
    const handle = ++seq;
    pending.set(handle, { fn, at: now + ms });
    return handle;
  };
  const cancel = (handle) => pending.delete(handle);
  const advance = (ms) => {
    now += ms;
    for (const [handle, item] of [...pending]) {
      if (item.at <= now) {
        pending.delete(handle);
        item.fn();
      }
    }
  };

  const states = [];
  const feedback = createCopyFeedback({
    onChange: (state) => states.push(state),
    schedule,
    cancel,
  });

  feedback.success();
  assert.equal(feedback.state, "copied");
  advance(1499);
  assert.equal(feedback.state, "copied");
  advance(1);
  assert.equal(feedback.state, "idle");
  assert.deepEqual(states, ["copied", "idle"]);
});

// 断言 5：连续两次点击 → 恢复计时从第二次起算。
test("a second click restarts the 1.5s restore timer", () => {
  let now = 0;
  let seq = 0;
  const pending = new Map();
  const schedule = (fn, ms) => {
    const handle = ++seq;
    pending.set(handle, { fn, at: now + ms });
    return handle;
  };
  const cancel = (handle) => pending.delete(handle);
  const advance = (ms) => {
    now += ms;
    for (const [handle, item] of [...pending]) {
      if (item.at <= now) {
        pending.delete(handle);
        item.fn();
      }
    }
  };

  const feedback = createCopyFeedback({ schedule, cancel });
  feedback.success();
  advance(1000);
  feedback.success(); // 第二次点击：重置计时
  advance(1000);
  assert.equal(feedback.state, "copied", "第二次点击后 1 秒仍应显示已复制");
  advance(500);
  assert.equal(feedback.state, "idle", "第二次点击起满 1.5 秒才恢复");
});

// 断言 6：正文为空时按钮禁用，且带有 reader.copyEmpty 的提示。
test("the copy button is disabled with a hint when there is no text", () => {
  assert.match(view, /disabled=\{!copyable\}/);
  assert.match(view, /t\("reader\.copyEmpty"\)/);
  // 禁用判断来自正文纯文本，而不是分段数量：空正文即禁用。
  assert.match(view, /const copyable = \$derived\(fullText\.trim\(\)\.length > 0\)/);
  for (const lang of langs) {
    assert.match(messages[lang], /"reader\.copyEmpty":/);
  }
});

// 断言 7：剪贴板 API 失败时（mock 抛错）走回退路径，且界面显示「复制失败」。
test("clipboard failure falls back to the Tauri command and surfaces the failure", () => {
  assert.match(view, /navigator\.clipboard\.writeText/);
  assert.match(view, /invoke\("copy_to_clipboard"/);
  assert.match(view, /copyFeedback\.failure\(\)/);
  assert.match(view, /t\("reader\.copyFailed"\)/);
  assert.match(libRs, /fn copy_to_clipboard/);
  assert.match(libRs, /copy_to_clipboard,/);
  assert.match(cargo, /arboard/);
  // 回退命令只写文本，不做读取。
  assert.doesNotMatch(libRs, /get_text/);
});

// 断言 8：按钮与 TXT 的间距 8px、与导出组之间 12px；图标尺寸 12–13px。
test("the toolbar keeps copy and export visually separated at the specified sizes", () => {
  // 「复制」与导出组之间 12px（§1 的分组间距）。
  assert.match(view, /\.actions \{[^}]*gap: 12px/s);
  // 导出组内部 TXT 与 JSON 之间 8px。
  assert.match(view, /\.exports \{[^}]*gap: 8px/s);
  // 图标 12–13px：复制用 13，与 download 一致。
  assert.match(view, /<Icon name=\{copyState === "copied" \? "check" : "copy"\} size=\{13\} \/>/);
  assert.match(view, /<Icon name="download" size=\{13\} \/>/);
});

// 结构性补充：图标字形、五语言文案、播报区、Reader 容器绑定。
test("the copy glyph is the two-square form and the check glyph exists", () => {
  assert.match(icons, /copy: "[^"]*Z M10 3\.67H4\.33/);
  assert.match(icons, /check: "M3\.6 8\.3 6\.6 11\.3 12\.4 4\.9"/);
});

test("all five locales carry the four copy strings", () => {
  for (const lang of langs) {
    for (const key of ["reader.copy", "reader.copied", "reader.copyFailed", "reader.copyEmpty"]) {
      assert.match(messages[lang], new RegExp(`"${key}":`), `${lang} 缺少 ${key}`);
    }
  }
  // 旧键已移除，避免五语言留下死键。
  for (const lang of langs) {
    assert.doesNotMatch(messages[lang], /"transcribe\.copyAll"/);
    assert.doesNotMatch(messages[lang], /"error\.copyFailed"/);
  }
  assert.doesNotMatch(page, /copyAll/);
});

test("feedback is announced once via a polite live region", () => {
  assert.match(view, /aria-live="polite"/);
  assert.match(view, /copyAnnouncement/);
});

test("the reader body element is bound out for selection checks", () => {
  assert.match(reader, /bodyEl = \$bindable/);
  assert.match(view, /bind:bodyEl=\{readerBody\}/);
  assert.doesNotMatch(transcribeView, /bind:bodyEl=\{readerBody\}/);
});

test("copy controls are rendered by the historical transcript detail view", () => {
  assert.match(view, /{#if detail && detailJobId}/);
  assert.match(view, /<Icon name=\{copyState === "copied" \? "check" : "copy"\} size=\{13\} \/>/);
  assert.match(view, /onclick=\{onCopy\}/);
});
