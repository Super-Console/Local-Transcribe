import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { isWithinSidebarDragStrip } from "../src/lib/sidebar-drag.js";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const page = read("../src/routes/+page.svelte");
const sidebar = read("../src/lib/components/AppSidebar.svelte");
const toolbar = read("../src/lib/components/AppToolbar.svelte");
const shell = read("../src/lib/components/AppShell.svelte");

const desktopLockPattern = (version) =>
  new RegExp(`name = "voice-to-text-desktop"\\r?\\nversion = "${version.replaceAll('.', '\\.') }"`);

test("release version check accepts LF and CRLF Cargo.lock checkouts", () => {
  const pattern = desktopLockPattern("1.8.1");
  assert.match('name = "voice-to-text-desktop"\nversion = "1.8.1"', pattern);
  assert.match('name = "voice-to-text-desktop"\r\nversion = "1.8.1"', pattern);
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
  assert.equal(packageJson.version, "1.8.1");
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
  // 2026-11-21：窗口外壳抽成共享 AppShell.svelte（结构 + CSS + 拖拽），断言随之迁移。
  assert.match(shell, /<header class="window-header" role="presentation" onmousedown=\{startWindowDrag\}>/);
  assert.match(shell, /getCurrentWindow\(\)/);
  assert.match(shell, /startDragging\(\)/);
  assert.match(shell, /startWindowDrag[\s\S]*?closest\(/);
  assert.doesNotMatch(shell, /<[a-z][^>]*\sdata-tauri-drag-region/);
  assert.doesNotMatch(toolbar, /<[a-z][^>]*\sdata-tauri-drag-region/);
});

test("window shell is the shared AppShell with header, sidebar header and tools slot", () => {
  // 2026-11-21 UI 验收 3）：.window-header / .sidebar-header / .header-tools / 拖拽
  // 必须住在 lib/components/AppShell.svelte（sparse-checkout 之外也能被 Cloud 复用）。
  assert.match(shell, /\.window-header\s*\{[^}]*height:\s*52px/s);
  assert.match(shell, /\.sidebar-header\s*\{[^}]*padding:\s*0 10px 0 80px/s);
  assert.match(shell, /\.header-tools\s*\{[^}]*flex:\s*1/s);
  assert.match(shell, /\.collapsed \.sidebar-header\s*\{[^}]*width:\s*126px/s);
  assert.match(shell, /<div class="sidebar-header">\s*<\/div>/);
  assert.match(page, /import AppShell\b[^;\n]*from "\$lib\/components\/AppShell\.svelte"/);
  assert.doesNotMatch(page, /\.window-header\s*\{/);
  assert.doesNotMatch(page, /function headerPointerDown/);
});

test("traffic-light clearance is platform-aware through data-os", () => {
  // 2026-11-21 UI 验收 4）：macOS 保留 padding-left: 80px；Windows 没有红绿灯降到 10px。
  // 启动时给 document.documentElement 打 data-os="macos|windows"。
  // 不允许负 margin 或高度补偿（历史 bug：margin-top:-52px + height:calc(100% + 52px)）。
  assert.match(shell, /(:global\()?\[data-os="windows"\]\)? \.sidebar-header\s*\{[^}]*padding-left:\s*10px/s);
  assert.match(shell, /document\.documentElement\.dataset\.os/);
  assert.match(shell, /dataset\.os\s*=|"macos"|"windows"/);
  assert.match(shell, /\.sidebar-header\s*\{[^}]*padding:\s*0 10px 0 80px/s);
  assert.doesNotMatch(shell, /margin-top:\s*-\d|height:\s*calc\(100%\s*\+/);
  assert.doesNotMatch(page, /margin-top:\s*-\d|height:\s*calc\(100%\s*\+/);
});

test("sidebar takes grouped items and renders one section title per group", () => {
  // 2026-11-21 UI 验收 2）：AppSidebar 参数化（分组 + items 数组），每组标题是 .section。
  // Local 传原来的 5 项（工作区：home/record/lan；管理：model/settings）。
  assert.match(sidebar, /groups:/);
  assert.match(sidebar, /\{#each groups as group/);
  assert.match(sidebar, /class="section">\{group\.title\}/);
  assert.match(page, /title:\s*t\("nav\.workspace"\)/);
  assert.match(page, /title:\s*t\("nav\.manage"\)/);
  const groupsBlock = page.match(/sidebarGroups[\s\S]*?\]\);/)?.[0] ?? "";
  for (const view of ["home", "record", "lan", "model", "settings"]) {
    assert.ok(groupsBlock.includes(`view: "${view}"`), `sidebar group includes ${view}`);
  }
  const workspace = groupsBlock.match(/nav\.workspace[\s\S]*?nav\.manage/)?.[0] ?? "";
  assert.match(workspace, /view: "home"/);
  assert.match(workspace, /view: "record"/);
  assert.match(workspace, /view: "lan"/);
});

test("Windows sidebar uses a fixed header overlay and keeps the main below it", () => {
  assert.match(page, /windowsUp=\{onboardingPlatform === "windows" \|\| previewWindowsLayout\}/);
  assert.doesNotMatch(page, /class="workspace" onmousedown=/);
  assert.match(page, /sidebarPointerDown[\s\S]*?isWithinSidebarDragStrip\(event\.clientY\)/);
  assert.match(page, /onSidebarPointerDown=\{onboardingPlatform === "windows" \? sidebarPointerDown : undefined\}/);
  assert.match(sidebar, /use:sidebarDragRegion=\{onSidebarPointerDown\}/);
  assert.match(page, /get\("windowsLayout"\) === "1"/);
  assert.match(shell, /class:windows-sidebar-up=\{windowsUp\}/);
  assert.match(shell, /\.app\.windows-sidebar-up\s*\{[^}]*position:\s*relative/s);
  assert.match(shell, /\.app\.windows-sidebar-up \.window-header\s*\{[^}]*position:\s*absolute;[^}]*left:\s*var\(--sidebar-width\);[^}]*height:\s*52px/s);
  assert.match(shell, /\.app\.windows-sidebar-up \.sidebar-header\s*\{[^}]*width:\s*0;[^}]*padding:\s*0/s);
  assert.match(shell, /\.app\.windows-sidebar-up \.main\s*\{[^}]*padding-top:\s*60px/s);
  assert.match(shell, /\.window-header\s*\{[^}]*height:\s*52px/s);
  assert.match(shell, /\.sidebar-header\s*\{[^}]*padding:\s*0 10px 0 80px/s);
  assert.match(shell, /\.collapsed \.sidebar-header\s*\{[^}]*width:\s*126px/s);
  assert.match(sidebar, /padding:\s*14px 10px 14px/);
});

test("the Windows sidebar drag strip ends at viewport y=52", () => {
  assert.equal(isWithinSidebarDragStrip(0), true);
  assert.equal(isWithinSidebarDragStrip(52), true);
  assert.equal(isWithinSidebarDragStrip(52.01), false);
});

test("sidebar has a visible keyboard focus indicator", () => {
  assert.match(sidebar, /\.nav-item:focus-visible\s*\{[^}]*outline: 2px/);
});
