import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const historyView = read('../src/lib/components/HistoryView.svelte');

test('HistoryView 标题由可选 title prop 决定，默认保持 nav.home', () => {
  // 默认值：不传 title 时 Local 首页行为不变
  assert.match(historyView, /title\s*=\s*t\("nav\.home"\)/);
  // 标题渲染 prop 本身，不再硬编码 nav.home
  assert.match(historyView, /<h1 class="page-title">\{title\}<\/h1>/);
  assert.doesNotMatch(historyView, /<h1 class="page-title">\{t\("nav\.home"\)\}<\/h1>/);
});
