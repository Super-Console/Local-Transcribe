import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const view = read('../src/lib/components/LanView.svelte');

test('LAN UI removes credential controls and no longer submits jobs or picks models', () => {
  assert.doesNotMatch(view, /hostToken|draft\.token|lan_host_token|nodeFormToken/);
  assert.doesNotMatch(view, /status\.models/);
  assert.doesNotMatch(view, /modelId: selectedNode\?\.model_id/);
  assert.doesNotMatch(view, /changeRemoteModel/);
  assert.match(view, /lan\.tasksTitle/);
});

test('sharing description is concise and does not reference removed authentication', () => {
  assert.match(read('../src/lib/i18n/messages/zh.ts'), /"lan.hostDesc": "开启后，同一局域网子网内的设备可上传媒体并使用本机转写。"/);
});
