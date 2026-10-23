import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const root = fileURLToPath(new URL('../', import.meta.url));
const chrome = process.env.BROWSER_BIN ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';

test('real Svelte reader clicks, focus, hit testing and opaque model menu', {
  skip: !existsSync(chrome) && 'Set BROWSER_BIN to an existing Chromium executable',
  timeout: 90000,
}, async () => {
  const preferred = join(tmpdir(), 'opencode');
  const scratch = mkdtempSync(join(existsSync(preferred) ? preferred : tmpdir(), 'lt-reader-test-'));
  const server = await createServer({
    root, configFile: false,
    cacheDir: join(scratch, 'vite'),
    plugins: [svelte({ configFile: false }), {
      name: 'reader-test-page',
      configureServer(server) {
        server.middlewares.use('/__interaction_test__', (_req, res) => {
          res.setHeader('Content-Type', 'text/html');
          res.end('<!doctype html><html data-glass="on"><head><meta charset="utf-8"></head><body><div id="fixture"></div><pre id="result">pending</pre><script type="module" src="/tests/browser/reader-interaction.ts"></script></body></html>');
        });
      },
    }],
    resolve: { alias: { $lib: `${root}src/lib` } },
    server: { host: '127.0.0.1', port: 0 },
  });
  try {
    await server.listen();
    const port = server.httpServer.address().port;
    const { stdout, stderr } = await promisify(execFile)(chrome, [
      '--headless=new', '--disable-gpu', '--no-first-run', '--no-default-browser-check',
      `--user-data-dir=${join(scratch, 'chrome-profile')}`,
      '--window-size=1000,800', '--virtual-time-budget=12000', '--dump-dom',
      `http://127.0.0.1:${port}/__interaction_test__`,
    ], { maxBuffer: 2 * 1024 * 1024, timeout: 75000 });
    const raw = stdout.match(/<pre id="result">([\s\S]*?)<\/pre>/)?.[1];
    assert.ok(raw && raw !== 'pending', stdout || stderr);
    const report = JSON.parse(raw.replaceAll('&quot;', '"').replaceAll('&gt;', '>').replaceAll('&lt;', '<').replaceAll('&amp;', '&'));
    console.log(JSON.stringify(report, null, 2));
    assert.deepEqual(report.failures, []);
  } finally {
    await server.close();
  }
});
