import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const page = read("../src/routes/+page.svelte");
const sidebar = read("../src/lib/components/AppSidebar.svelte");

const desktopLockPattern = (version) =>
  new RegExp(`name = "voice-to-text-desktop"\\r?\\nversion = "${version.replaceAll('.', '\\.') }"`);

test("release version check accepts LF and CRLF Cargo.lock checkouts", () => {
  const pattern = desktopLockPattern("1.1.0-beta.2");
  assert.match('name = "voice-to-text-desktop"\nversion = "1.1.0-beta.2"', pattern);
  assert.match('name = "voice-to-text-desktop"\r\nversion = "1.1.0-beta.2"', pattern);
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
  assert.equal(packageJson.version, "1.1.0-beta.2");
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
  assert.match(page, /data-tauri-drag-region/);
  assert.match(sidebar, /id="app-sidebar"/);
  assert.match(sidebar, /inert=\{collapsed\}/);
});

test("navigation preserves all existing destinations", () => {
  for (const view of ["home", "record", "lan", "model", "settings"]) {
    assert.ok(sidebar.includes(`onNavigate("${view}")`));
  }
});

test("sidebar has a visible keyboard focus indicator", () => {
  assert.match(sidebar, /\.nav-item:focus-visible\s*\{[^}]*outline: 2px/);
});
