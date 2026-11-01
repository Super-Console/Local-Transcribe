// The macOS bundle script must rebuild every sidecar from the current source and
// refresh the copies that Tauri's externalBin actually ships. A stale copy caused
// 1.4.0-beta.1 to start the node with --llm-models-dir and fail (exit 2).
//
// 1.6.0 ships the MCP server, the CLI and the local service alongside the GUI so
// an installed app also provides `voice-to-text-mcp` and `local-transcribe`.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const packageJson = JSON.parse(
	await readFile(new URL('../package.json', import.meta.url), 'utf8'),
);
const tauri = JSON.parse(
	await readFile(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'),
);

const script = packageJson.scripts?.['bundle:mac'] ?? '';
const triple = 'aarch64-apple-darwin';

const SIDECARS = [
	'local-transcribe-node',
	'local-transcribe',
	'voice-to-text-mcp',
	'voice-to-text-service',
];

test('bundle:mac rebuilds the LAN node sidecar before bundling', () => {
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

test('bundle:mac ships the MCP server, the CLI and the local service', () => {
	assert.match(
		script,
		/cargo build --release -p voice-to-text-cli -p voice-to-text-mcp -p voice-to-text-service/,
		'bundle:mac must build the three additional entry points from source',
	);
	for (const name of ['local-transcribe', 'voice-to-text-mcp', 'voice-to-text-service']) {
		assert.match(
			script,
			new RegExp(`cp .*target/release/${name} .*src-tauri/binaries/${name}-${triple}`),
			`bundle:mac must stage ${name} for externalBin`,
		);
	}
	const tauriIndex = script.indexOf('tauri build');
	const lastCopyIndex = script.lastIndexOf('src-tauri/binaries/');
	assert.ok(lastCopyIndex < tauriIndex, 'sidecars must be staged before tauri build');
});

test('every sidecar is declared in externalBin and signed with the bundle', () => {
	for (const name of SIDECARS) {
		assert.ok(
			tauri.bundle.externalBin.includes(`binaries/${name}`),
			`externalBin must include ${name}`,
		);
	}
	const signLoop = script.indexOf(
		'for b in local-transcribe-node local-transcribe voice-to-text-mcp voice-to-text-service',
	);
	assert.ok(signLoop >= 0, 'all sidecars must be signed in one loop');
	const signApp = script.lastIndexOf('codesign --force --sign -');
	assert.ok(signApp > signLoop, 'the app bundle is signed after its nested binaries');
});
