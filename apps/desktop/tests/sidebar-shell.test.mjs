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
  const pattern = desktopLockPattern("1.5.0-beta.3");
  assert.match('name = "voice-to-text-desktop"\nversion = "1.5.0-beta.3"', pattern);
  assert.match('name = "voice-to-text-desktop"\r\nversion = "1.5.0-beta.3"', pattern);
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
  assert.equal(packageJson.version, "1.5.0-beta.3");
  assert.equal(tauri.version, packageJson.version);
  assert.match(cargo, new RegExp(`version = "${packageJson.version.replaceAll('.', '\\.') }"`));
  assert.equal(npmLock.version, packageJson.version);
  assert.equal(npmLock.packages[""].version, packageJson.version);
  assert.match(cargoLock, desktopLockPattern(packageJson.version));
});

test("sidebar toggle lives in the right toolbar capsule group, not the left header", () => {
  // 2026-10-25：折叠按钮曾独占左上角，样式（--hairline-2 圆形）与右侧工具栏的
  // 胶囊按钮不一致；现在它必须进入右侧同一组，复用 icon-btn 的胶囊样式。
  assert.match(page, /const SIDEBAR_COLLAPSE_ENABLED = true/);
  assert.match(toolbar, /sidebarCollapsed/);
  assert.match(toolbar, /onToggleSidebar/);
  assert.match(toolbar, /class="icon-btn panel-toggle"/);
  assert.match(toolbar, /aria-expanded=\{!sidebarCollapsed\}/);
  assert.match(toolbar, /aria-controls="app-sidebar"/);
  assert.match(toolbar, /onclick=\{onToggleSidebar\}/);
  // 折叠按钮必须复用与深色/浅色切换相同的胶囊外观，而不是另起一套。
  const toggleBlock = toolbar.match(/\.icon-btn\.panel-toggle\s*\{[^}]*\}/s)?.[0] ?? "";
  assert.match(toggleBlock, /var\(--capsule-find\)/);
  assert.doesNotMatch(toggleBlock, /border-radius:\s*50%/);
  assert.match(sidebar, /id="app-sidebar"/);
  assert.match(sidebar, /inert=\{collapsed\}/);
});

test("toolbar button order is search, theme toggle, sidebar collapse, model picker", () => {
  // 2026-10-25：用户要求搜索最左，其次深浅色切换，再次折叠，模型选择器保持最右。
  const search = toolbar.indexOf('class="search"');
  const theme = toolbar.indexOf('class="icon-btn theme"');
  const toggle = toolbar.indexOf('class="icon-btn panel-toggle"');
  const picker = toolbar.indexOf('class="model-pick"');
  assert.ok(search >= 0, "search capsule exists");
  assert.ok(theme > search, "theme toggle comes after search");
  assert.ok(toggle > theme, "sidebar collapse comes after theme toggle");
  assert.ok(picker > toggle, "model picker stays rightmost");
});

test("search button icon matches the other toolbar icon buttons", () => {
  // 2026-10-25：搜索按钮曾用 --text-3，视觉上比月亮/折叠（--text-2）浅，用户不要。
  const searchBlock = toolbar.match(/\.search\s*\{[^}]*\}/s)?.[0] ?? "";
  const iconBlock = toolbar.match(/\.search-icon\s*\{[^}]*\}/s)?.[0] ?? "";
  assert.match(searchBlock, /color: var\(--text-2\)/);
  assert.match(iconBlock, /color: var\(--text-2\)/);
  assert.doesNotMatch(searchBlock, /--text-3/);
  assert.doesNotMatch(iconBlock, /--text-3/);
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
