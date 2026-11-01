// 1.7.0：白名单从命令行参数变成设置项；启动时按官网清单检查更新。
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (path) => readFile(new URL(path, import.meta.url), 'utf8');

test('the settings page no longer exposes a folder allow-list', async () => {
	const settings = await read('../src/lib/components/SettingsView.svelte');
	assert.doesNotMatch(settings, /access_status|access_add_root|access_remove_root/);
	assert.doesNotMatch(settings, /settings\.accessTitle/);
	const page = await read('../src/routes/+page.svelte');
	assert.doesNotMatch(page, /access_add_root|onFixAccess|blockedPath/);
	const rust = await read('../src-tauri/src/lib.rs');
	assert.doesNotMatch(rust, /access_settings|access::/);
});

test('the home list refreshes when a CLI or MCP job writes a result', async () => {
	const page = await read('../src/routes/+page.svelte');
	assert.match(
		page,
		/listen\("local-transcribe:history-changed", \(\) => \{/,
		'首页必须订阅结果目录变化事件',
	);
	assert.match(page, /if \(view === "home"\) void loadHistory\(\);/);
	const rust = await read('../src-tauri/src/history_watch.rs');
	assert.match(rust, /pub const HISTORY_EVENT: &str = "local-transcribe:history-changed"/);
	assert.match(rust, /pub fn signature/);
});

test('update checks read the product site, not GitHub, and respect the toggle', async () => {
	const page = await read('../src/routes/+page.svelte');
	assert.match(page, /invoke<UpdateInfo \| null>\("check_for_update", \{ force: false \}\)/);
	assert.match(page, /open_external/);
	const settings = await read('../src/lib/components/SettingsView.svelte');
	assert.match(settings, /set_update_settings/);
	assert.match(settings, /force: true/);
	const rust = await read('../src-tauri/src/updates.rs');
	assert.match(rust, /https:\/\/apps\.timhuang\.net\/transcribe\/updates\.json/);
	assert.match(rust, /if !force && !settings\.auto_check/, '关闭后启动检查必须不发请求');
});
