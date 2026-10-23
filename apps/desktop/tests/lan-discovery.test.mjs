import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const view = read('../src/lib/components/LanView.svelte');

test('LAN scan keeps mDNS first and also probes saved ports over TCP', () => {
  assert.match(view, /scanTimeoutMs\s*=\s*8_000/);
  assert.match(view, /ports:\s*nodes\.map/);
});

test('empty scan result explains the local-network permission risk', () => {
  const zh = read('../src/lib/i18n/messages/zh.ts');
  assert.match(zh, /"lan\.scanEmptyHint":/);
  const en = read('../src/lib/i18n/messages/en.ts');
  assert.match(en, /"lan\.scanEmptyHint":/);
  assert.match(view, /t\("lan\.scanEmptyHint"\)/);
});

test('macOS bundle is re-signed with the stable bundle identifier', () => {
  const pkg = JSON.parse(read('../package.json'));
  const script = pkg.scripts['bundle:mac'] ?? '';
  assert.match(script, /--identifier com\.timmy\.voicetotextapp/);
});

test('LAN page warns when the system never granted local network access', () => {
  assert.match(view, /lan_diagnostics/);
  assert.match(view, /lan\.permissionTitle/);
  assert.match(view, /lan\.permissionAction/);
  for (const lang of ['zh', 'en', 'de', 'es', 'fr']) {
    const messages = read(`../src/lib/i18n/messages/${lang}.ts`);
    assert.match(messages, /"lan\.permissionTitle":/, `${lang} needs the permission banner`);
    assert.match(messages, /"lan\.permissionBody":/, `${lang} needs the permission body`);
  }
});

test('troubleshooting lives in the permission banner and in Settings, not the LAN title row', () => {
  assert.match(view, /lan\.troubleshoot/);
  assert.match(view, /LanDiagnosticsPanel/);
  assert.doesNotMatch(view, /class="diag-open"/);
  const settings = read('../src/lib/components/SettingsView.svelte');
  const trouble = settings.indexOf('settings.troubleshoot');
  const about = settings.indexOf('settings.about');
  assert.ok(trouble >= 0, 'Settings needs a troubleshooting section');
  assert.ok(about > trouble, 'troubleshooting must sit above About');
  const panel = read('../src/lib/components/LanDiagnosticsPanel.svelte');
  assert.match(panel, /lan_diagnostics/);
  assert.match(panel, /restart_app/);
  assert.match(panel, /app_permissions/);
  assert.match(panel, /permissions\.microphone/);
  assert.match(panel, /permissions\.screenCapture/);
  assert.match(panel, /open_privacy_settings/);
});
