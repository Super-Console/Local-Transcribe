import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const toolbar = read('../src/lib/components/AppToolbar.svelte');
const page = read('../src/routes/+page.svelte');
const lanView = read('../src/lib/components/LanView.svelte');

test('model menu groups local models and each remote node', () => {
  assert.match(toolbar, /remoteGroups/);
  assert.match(toolbar, /onSelectRemote/);
  assert.match(toolbar, /toolbar\.localSection/);
  assert.match(page, /lan_node_status/);
  assert.match(page, /set_lan_assignment/);
});

test('LAN page no longer prints the model list or the sharing shutdown note', () => {
  assert.doesNotMatch(lanView, /lan\.hostModel/);
  assert.doesNotMatch(lanView, /lan\.hostStopNote/);
  for (const lang of ['zh', 'en', 'de', 'es', 'fr']) {
    const messages = read(`../src/lib/i18n/messages/${lang}.ts`);
    assert.doesNotMatch(messages, /"lan\.hostModel":/);
    assert.doesNotMatch(messages, /"lan\.hostStopNote":/);
  }
});

test('LAN page keeps only node management, sharing and the task list', () => {
  assert.doesNotMatch(lanView, /sendFile|lan_submit_file|lan\.sendFile/);
  assert.doesNotMatch(lanView, /lan\.remoteModel|lan\.remoteDefault|changeRemoteModel/);
  assert.doesNotMatch(lanView, /lan\.modeLabel|set_lan_assignment/);
  assert.match(lanView, /lan\.tasksTitle/);
});

test('toolbar chip dot is green while a remote model is active', () => {
  assert.match(toolbar, /remoteDotOn/);
  assert.doesNotMatch(toolbar, /class:on=\{!!model && model\.id === loadedModelId\}/);
});

test('installed catalog models can be deleted from the model page', () => {
  const modelView = read('../src/lib/components/ModelView.svelte');
  assert.match(modelView, /onDeleteCatalog/);
  assert.match(modelView, /delete_catalog_model|onDeleteCatalog\(entry\)/);
  assert.match(page, /delete_catalog_model/);
});
