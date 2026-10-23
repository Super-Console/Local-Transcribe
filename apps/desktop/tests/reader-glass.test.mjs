import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const root = new URL('../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const reader = read('src/lib/components/Reader.svelte');
const toolbar = read('src/lib/components/AppToolbar.svelte');
const appCss = read('src/app.css');
const appHtml = read('src/app.html');

test('1. the reader bar is a 40px absolute sibling of the scroller', () => {
  assert.match(reader, /\.reader-bar\s*\{[^}]*position:\s*absolute;/s);
  assert.match(reader, /\.reader-bar\s*\{[^}]*top:\s*0;/s);
  assert.match(reader, /\.reader-bar\s*\{[^}]*height:\s*var\(--reader-bar-height\);/s);
  assert.match(reader, /class="reader-scroll"[\s\S]{0,400}?bind:this=\{scroller\}/);
  assert.match(reader, /\.reader-scroll\s*\{[^}]*top:\s*var\(--reader-bar-height\);/s);
  assert.match(reader, /\.reader-scroll\s*\{[^}]*padding:\s*0 0 40px;/s);
  assert.doesNotMatch(reader, /\.reader-bar\s*\{[^}]*position:\s*sticky;/s);
});

test('2. ⌘F opens the find row; Esc closes, clears and returns focus', () => {
  assert.match(reader, /event\.key\.toLowerCase\(\) === "f"/);
  assert.match(reader, /findOpen = true/);
  assert.match(reader, /findInput\?\.focus\(\)/);
  assert.match(reader, /function closeFind\(\)[\s\S]*?findOpen = false;[\s\S]*?query = "";[\s\S]*?scroller\?\.focus\(\);/);
});

test('3. no match renders 0 / 0 without throwing', () => {
  assert.match(reader, /totalHits === 0 \? "0 \/ 0"/);
});

test('4. every occurrence is counted and the current hit is distinct', () => {
  assert.match(reader, /while \(cursor < text\.length\)/);
  assert.match(reader, /index: index\+\+/);
  assert.match(reader, /mark\.current/);
  assert.match(reader, /piece\.index === current/);
});

test('5. the back button hit area is at least 28px tall', () => {
  assert.match(reader, /class="back btn-secondary"/);
  assert.match(appCss, /\.btn-secondary,[^{]*\{[^}]*height:\s*30px;/s);
  assert.match(appCss, /\.btn-secondary,[^{]*\{[^}]*padding:\s*0 12px;/s);
});

test('6. data-glass=off and reduced transparency remove blur and use solid fill', () => {
  assert.match(appHtml, /data-glass="on"/);
  assert.match(appCss, /html\[data-glass="off"\] \.back/);
  assert.match(appCss, /backdrop-filter:\s*none/);
  assert.match(appCss, /prefers-reduced-transparency:\s*reduce/);
  assert.match(appCss, /background:\s*var\(--panel\)/);
});

test('7. the top fade is a permanent 72px overlay and the scroller has no mask', () => {
  assert.doesNotMatch(reader, /mask-image/);
  assert.match(reader, /\.reader-fade\s*\{[^}]*height:\s*var\(--reader-fade-height\);/s);
  assert.match(reader, /\.reader-fade\s*\{[^}]*pointer-events:\s*none;/s);
  assert.match(reader, /\.fade-bottom\s*\{[^}]*height:\s*24px;/s);
  assert.match(reader, /\.fade-bottom\s*\{[^}]*pointer-events:\s*none;/s);
  assert.doesNotMatch(reader, /\.reader-shell\.at-top \.reader-fade/);
});

test('9a. the bar is clickable above the fade and only the capsules carry glass', () => {
  assert.match(reader, /\.reader-bar\s*\{[^}]*pointer-events:\s*none;/s);
  assert.match(reader, /\.reader-bar\s*\{[^}]*z-index:\s*3;/s);
  assert.match(reader, /\.reader-fade\s*\{[^}]*pointer-events:\s*none;/s);
  assert.match(reader, /class="back btn-secondary"[^>]*onclick=\{onBack\}/s);
  assert.match(reader, /class="find-toggle btn-secondary"[\s\S]{0,240}?onclick=\{\(\) => void openFind\(\)\}/);
  assert.match(reader, /\.reader-bar\s*\{[^}]*background:\s*transparent;/s);
  assert.match(reader, /\.reader-bar\s*\{[^}]*backdrop-filter:\s*none;/s);
  assert.match(appCss, /\.btn-secondary,[^{]*\{[^}]*border-radius:\s*999px;/s);
  assert.match(reader, /\.back\s*\{[^}]*backdrop-filter:\s*blur\(18px\)/s);
  assert.match(reader, /class="find-toggle btn-secondary"/);
  assert.match(reader, /\.find-toggle\s*\{[^}]*backdrop-filter:\s*blur\(18px\)/s);
  assert.match(reader, /\.find-toggle\.open/);
});

test('8. short and bottom states reduce the bottom fade to 8px', () => {
  assert.match(reader, /\.reader-shell\.at-bottom \.fade-bottom/);
  assert.match(reader, /\.reader-shell\.short \.fade-bottom/);
  assert.match(reader, /height:\s*8px/);
});

test('9. only Reader and AppToolbar components declare backdrop-filter', () => {
  const components = join(new URL('src/lib/components/', root).pathname);
  const users = readdirSync(components)
    .filter((file) => file.endsWith('.svelte'))
    .filter((file) => readFileSync(join(components, file), 'utf8').includes('backdrop-filter'));
  assert.deepEqual(users.sort(), ['AppToolbar.svelte', 'Reader.svelte']);
  assert.match(toolbar, /backdrop-filter:\s*blur\(20px\) saturate\(1\.1\)/);
  assert.match(reader, /backdrop-filter:\s*blur\(18px\) saturate\(1\.1\)/);
  assert.doesNotMatch(reader, /blur\((2[1-9]|[3-9]\d)px\)/);
  assert.doesNotMatch(toolbar, /blur\((2[1-9]|[3-9]\d)px\)/);
});

test('9b. the model menu is an opaque content layer above the reader', () => {
  assert.match(toolbar, /\.toolbar\s*\{[^}]*z-index:\s*10;/s);
  assert.match(toolbar, /\.menu\s*\{[^}]*background:\s*var\(--panel\);/s);
  assert.match(toolbar, /\.menu\s*\{[^}]*z-index:\s*40;/s);
  assert.doesNotMatch(toolbar, /\.menu\s*\{[^}]*backdrop-filter:\s*blur/s);
  assert.match(toolbar, /\.menu\s*\{[^}]*box-shadow:\s*var\(--shadow-panel\);/s);
});

test('9c. native thin scrollbars for content and reader', () => {
  assert.match(appCss, /scrollbar-width:\s*thin/);
  assert.match(appCss, /scrollbar-color:\s*rgba\(20, 20, 19, 0\.18\) transparent/);
  assert.match(appCss, /::-webkit-scrollbar\s*\{[^}]*width:\s*8px;/s);
  assert.match(appCss, /::-webkit-scrollbar-thumb\s*\{[^}]*border-radius:\s*4px;/s);
  assert.match(appCss, /rgba\(255, 255, 255, 0\.16\)/);
});

test('10. both search labels are distinct in all five locales', () => {
  for (const lang of ['zh', 'en', 'de', 'es', 'fr']) {
    const messages = read(`src/lib/i18n/messages/${lang}.ts`);
    assert.match(messages, /"search\.placeholder":/, `${lang} needs the toolbar search label`);
    assert.match(messages, /"reader\.searchPlaceholder":/, `${lang} needs the reader search label`);
    for (const key of ['reader.findOpen', 'reader.findNext', 'reader.findPrevious', 'reader.findClose']) {
      assert.match(messages, new RegExp(`"${key.replace('.', '\\.')}":`), `${lang} needs ${key}`);
    }
  }
});
