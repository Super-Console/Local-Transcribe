import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const root = new URL('../../../', import.meta.url);
const read = (path) => readFile(new URL(path, root), 'utf8');
const targetId = 'net.timhuang.localtranscribe';

const [tauri, packageJson, core, service, serviceMain, cli, mcp, lanSettings, lanService] = await Promise.all([
	read('apps/desktop/src-tauri/tauri.conf.json').then(JSON.parse),
	read('apps/desktop/package.json').then(JSON.parse),
	read('crates/transcription-core/src/lib.rs'),
	read('crates/transcription-service/src/lib.rs'),
	read('crates/transcription-service/src/main.rs'),
	read('crates/transcription-cli/src/main.rs'),
	read('crates/transcription-mcp/src/main.rs'),
	read('crates/lan-compute/src/settings.rs'),
	read('crates/transcription-service/src/lan.rs'),
]);

test('Tauri bundle identity is the canonical application identifier', () => {
	assert.equal(tauri.identifier, targetId);
	assert.match(core, new RegExp(`APP_IDENTIFIER: &str = "${targetId}"`));
});

test('macOS app and all sidecars use the Tauri bundle identifier for code signing', () => {
	const script = packageJson.scripts?.['bundle:mac'] ?? '';
	assert.match(script, /--identifier net\.timhuang\.localtranscribe/);
	assert.doesNotMatch(script, /--identifier com\.timmy\.voicetotextapp/);
});

test('service and LAN compute share the same platform data-directory resolver', () => {
	assert.match(core, /pub fn default_app_data_dir\(\)/);
	assert.match(service, /voice_to_text_core::default_app_data_dir\(\)/);
	assert.match(serviceMain, /default_data_dir\(\)/);
	assert.match(cli, /default_data_dir\(\)/);
	assert.match(mcp, /default_data_dir\(\)/);
	assert.match(lanSettings, /voice_to_text_core::default_app_data_dir\(\)/);
	assert.doesNotMatch(service, /Application Support\/com\.timmy\.voicetotextapp/);
	assert.doesNotMatch(lanSettings, /Application Support\/com\.timmy\.voicetotextapp/);
});

test('LAN-node remote state and log paths point to the fresh app data directory', () => {
	assert.match(lanService, /Application Support\/net\.timhuang\.localtranscribe\/lan-host\/lan-node\.json/);
	assert.match(lanService, /Application Support\/net\.timhuang\.localtranscribe\/lan-host\/lan-node\.log/);
});
