import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const page = read("../src/routes/+page.svelte");
const sidebar = read("../src/lib/components/AppSidebar.svelte");
const toolbar = read("../src/lib/components/AppToolbar.svelte");

const desktopLockPattern = (version) =>
  new RegExp(`name = "voice-to-text-desktop"\\r?\\nversion = "${version.replaceAll('.', '\\.') }"`);

test("release version check accepts LF and CRLF Cargo.lock checkouts", () => {
  const pattern = desktopLockPattern("1.1.0-beta.3");
  assert.match('name = "voice-to-text-desktop"\nversion = "1.1.0-beta.3"', pattern);
  assert.match('name = "voice-to-text-desktop"\r\nversion = "1.1.0-beta.3"', pattern);
});

test("macOS content extends into the native title bar", () => {
  const config = JSON.parse(read("../src-tauri/tauri.conf.json"));
  assert.equal(config.app.windows[0].titleBarStyle, "Overlay");
  assert.deepEqual(config.app.windows[0].trafficLightPosition, { x: 16, y: 28 });
  assert.equal(config.bundle.macOS.minimumSystemVersion, "14.2");
});

test("desktop release versions stay in sync", () => {
  const packageJson = JSON.parse(read("../package.json"));
  const tauri = JSON.parse(read("../src-tauri/tauri.conf.json"));
  const cargo = read("../src-tauri/Cargo.toml");
  const cargoLock = read("../../../Cargo.lock");
  const npmLock = JSON.parse(read("../package-lock.json"));
  assert.equal(packageJson.version, "1.1.0-beta.3");
  assert.equal(tauri.version, packageJson.version);
  assert.match(cargo, new RegExp(`version = "${packageJson.version.replaceAll('.', '\\.') }"`));
  assert.equal(npmLock.version, packageJson.version);
  assert.equal(npmLock.packages[""].version, packageJson.version);
  assert.match(cargoLock, desktopLockPattern(packageJson.version));
});

test("sidebar toggle remains outside the collapsible navigation", () => {
  assert.match(page, /const SIDEBAR_COLLAPSE_ENABLED = true/);
  assert.match(page, /aria-expanded=\{!sidebarCollapsed\}/);
  assert.match(page, /aria-controls="app-sidebar"/);
  assert.match(page, /onclick=\{toggleSidebar\}/);
  assert.match(page, /onmousedown=\{headerPointerDown\}/);
  assert.match(sidebar, /id="app-sidebar"/);
  assert.match(sidebar, /inert=\{collapsed\}/);
});

test("window header starts dragging programmatically for non-interactive targets", () => {
  // macOS Overlay 标题栏下 data-tauri-drag-region 不生效（tauri#9503），且其属性值
  // 只认直接命中；标题栏必须改用显式 mousedown -> startDragging()，并放行可交互控件。
  assert.match(page, /<header class="window-header" role="presentation" onmousedown=\{headerPointerDown\}>/);
  assert.match(page, /getCurrentWindow\(\)/);
  assert.match(page, /startDragging\(\)/);
  assert.match(page, /headerPointerDown[\s\S]*?closest\(/);
  assert.doesNotMatch(page, /<[a-z][^>]*\sdata-tauri-drag-region/);
  assert.doesNotMatch(toolbar, /<[a-z][^>]*\sdata-tauri-drag-region/);
});

test("navigation preserves all existing destinations", () => {
  for (const view of ["home", "record", "lan", "model", "settings"]) {
    assert.ok(sidebar.includes(`onNavigate("${view}")`));
  }
});

test("sidebar has a visible keyboard focus indicator", () => {
  assert.match(sidebar, /\.nav-item:focus-visible\s*\{[^}]*outline: 2px/);
});
