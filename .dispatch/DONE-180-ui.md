STATUS: BLOCKED

# Local Transcribe 1.8.0 UI work

## Implemented

- Windows-only sidebar positioning using the existing runtime `get_onboarding_platform` result. The sidebar rises by 52 px while the toolbar remains 52 px high and in its original position. Existing sidebar 14 px padding and the expanded/collapsed outer shell widths are retained.
- Dragging still uses `startDragging()` and filters interactive controls. The Windows workspace also handles blank-area mousedown so the lifted sidebar does not create an undraggable blank strip.
- Added static, version-keyed 1.8.0 What's New data and a small, dismissible non-modal card. It is placed at the upper-right below the toolbar to keep it visually associated with the app chrome and avoid a blocking overlay. Escape and the close button dismiss it; dismissal records the current version.
- Trigger behavior: no previous read-version record means no card (fresh install); a recorded version lower than a known current release shows it; closing records current version so it is not repeated.
- Added eight i18n keys in all five locales (zh/en/de/es/fr).

## Verified

- `cargo fmt --all -- --check`: passed (no output).
- Sidebar targeted test: `node --test tests/sidebar-shell.test.mjs` — 10 passed, 0 failed.
- What's New targeted test: `node --test tests/whats-new.test.mjs` — 4 passed, 0 failed.
- `git diff --check`: passed.
- Windows-only rendering is isolated by `class:windows-sidebar-up={onboardingPlatform === "windows"}`. `.window-header` remains 52 px and `.sidebar-header` retains `padding: 0 10px 0 80px`; AppToolbar CSS is untouched. Thus the macOS rendering path keeps the original layout values and does not receive the Windows positioning rule.

## Blocked / not verified

- `npm run check` was attempted and could not start: `svelte-kit: command not found`.
- `npm test` ran 138 tests: 136 passed; two reader test modules failed to load because the package `vite` is missing. The focused new tests passed. This worktree has no `node_modules`; no dependency installation was authorized or performed.
- **Windows real-device visual effect and drag behavior were not verified.**
- macOS GUI rendering was not visually tested or launched, per task constraint.
- `apps/desktop/src-tauri/tauri.conf.json` version and title bar settings were not changed.
