// The macOS bundle script must rebuild the LAN node and refresh the sidecar
// copy that Tauri's externalBin actually ships. A stale copy caused
// 1.4.0-beta.1 to start the node with --llm-models-dir and fail (exit 2).
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const packageJson = JSON.parse(
	await readFile(new URL('../package.json', import.meta.url), 'utf8'),
);

test('bundle:mac rebuilds the LAN node sidecar before bundling', () => {
	const script = packageJson.scripts?.['bundle:mac'] ?? '';
	assert.match(
		script,
		/cargo build --release -p voice-to-text-lan --bin local-transcribe-node/,
		'bundle:mac must rebuild local-transcribe-node from the current source',
	);
	assert.match(
		script,
		/cp .*target\/release\/local-transcribe-node .*src-tauri\/binaries\/local-transcribe-node-aarch64-apple-darwin/,
		'bundle:mac must copy the rebuilt node over the sidecar Tauri ships',
	);
	const buildIndex = script.indexOf('cargo build --release -p voice-to-text-lan');
	const tauriIndex = script.indexOf('tauri build');
	assert.ok(buildIndex >= 0 && tauriIndex > buildIndex, 'node build must run before tauri build');
});
