import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const reader = read('src/lib/components/Reader.svelte');
const transcribe = read('src/lib/components/TranscribeView.svelte');
const icon = read('src/lib/components/Icon.svelte');

test('the reader toolbar switches to icons only when its pane is narrow', () => {
  // 容器查询按工具栏自身可用宽度判断，避免跟随窗口宽度误判（侧栏展开/折叠会改变宽度）。
  assert.match(reader, /\.reader-wrap\s*\{[^}]*container-type:\s*inline-size;/s);
  assert.match(reader, /@container \(max-width:\s*560px\)/);
  assert.match(reader, /@container \(max-width:\s*560px\)\s*\{[\s\S]*?\.reader-bar \.label,[\s\S]*?display:\s*none;/);
  assert.match(reader, /@container \(max-width:\s*560px\)\s*\{[\s\S]*?\.reader-bar kbd\s*\{[\s\S]*?display:\s*none;/);
});

test('labels never wrap into vertical text while they are visible', () => {
  assert.match(reader, /\.reader-bar :global\(button\)\s*\{[^}]*white-space:\s*nowrap;/s);
  assert.match(transcribe, /\.actions button\s*\{[^}]*white-space:\s*nowrap;/s);
});

test('every reader-bar action keeps a meaningful icon when the label is hidden', () => {
  assert.match(reader, /class="back btn-secondary"[\s\S]{0,200}<Icon name="chevronLeft"/);
  assert.match(reader, /class="find-toggle btn-secondary"[\s\S]{0,260}<Icon name="search"/);
  assert.match(reader, /class="ai-toggle btn-secondary"[\s\S]{0,260}<Icon name="sparkle"/);
  assert.match(icon, /sparkle:\s*"/);
  // 标签与快捷键提示都包在可隐藏的节点里，图标本身不受影响。
  for (const label of ['nav.home', 'reader.findOpen', 'reader.aiAsk']) {
    assert.match(reader, new RegExp(`<span class="label">\\{t\\("${label.replace('.', '\\.')}"\\)\\}</span>`));
  }
  assert.match(transcribe, /<Icon name="download" size=\{13\}\s*\/><span class="label">TXT<\/span>/);
  assert.match(transcribe, /<Icon name="download" size=\{13\}\s*\/><span class="label">JSON<\/span>/);
  assert.match(transcribe, /@container \(max-width:\s*560px\)\s*\{[\s\S]*?\.actions \.label\s*\{[\s\S]*?display:\s*none;/);
});
