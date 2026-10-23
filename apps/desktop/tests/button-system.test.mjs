import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = file => readFileSync(new URL(`../src/${file}`, import.meta.url), 'utf8');
const pages = ['RecordView', 'LanView', 'ModelView', 'SettingsView', 'HistoryView', 'TranscribeView'];

test('button roles are centralized; each page has at most one primary declaration', () => {
  const css = read('app.css');
  for (const role of ['btn-primary', 'btn-secondary', 'btn-danger', 'link-action', 'link-danger']) {
    assert.ok(css.includes(`.${role}`), `${role} shared rule`);
  }
  for (const role of ['btn-primary', 'btn-secondary', 'btn-danger', 'link-action']) {
    for (const state of ['hover', 'active', 'focus-visible']) {
      assert.ok(css.includes(`.${role}:${state}`), `${role} ${state} defined centrally`);
    }
  }
  for (const page of pages) {
    const source = read(`lib/components/${page}.svelte`);
    const classes = [...source.matchAll(/class="([^"]*)"/g)].map(m => m[1].split(/\s+/));
    assert.ok(classes.filter(c => c.includes('btn-primary')).length <= 1, `${page} primary count`);
    assert.doesNotMatch(source, /class="(?:record-btn|add|link|btn)(?:\s|"|$)/, `${page} migrated classes`);
    assert.doesNotMatch(source, /\.(?:record-btn|add|link|btn|act)(?:[\s.:{])[^{}]*\{/, `${page} old local button styles removed`);
  }
});

test('only model download uses a capsule; settings danger only for clear', () => {
  const model = read('lib/components/ModelView.svelte');
  assert.match(model, /class="btn-secondary"[\s\S]{0,160}onDownload/);
  assert.equal((model.match(/class="btn-secondary"/g) ?? []).length, 1);
  assert.match(model, /class="link-action link-danger"[\s\S]{0,120}onDelete/);
  const settings = read('lib/components/SettingsView.svelte');
  assert.equal((settings.match(/class="btn-danger"/g) ?? []).length, 1);
  assert.match(settings, /class="btn-danger"[\s\S]{0,180}settings\.clearDataConfirm/);
  assert.doesNotMatch(settings, /class="btn-(?:primary|secondary)"/);
});
