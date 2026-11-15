STATUS: DONE

# Local Transcribe 1.8.0 UI work

## Implemented

- Windows-only sidebar positioning using the existing runtime `get_onboarding_platform` result. The sidebar rises by 52 px while the toolbar remains 52 px high and in its original position. Existing sidebar 14 px padding and the expanded/collapsed outer shell widths are retained.
- Dragging still uses `startDragging()` and filters interactive controls. Windows drag handling is attached only to the `.sidebar` subtree and only for `event.clientY <= 52`; the workspace/content/reader has no drag listener. In collapsed mode the existing 126 px header segment remains draggable through the unchanged header handler.
- Added static, version-keyed 1.8.0 What's New data and a small, dismissible non-modal card. It is placed at the upper-right below the toolbar to keep it visually associated with the app chrome and avoid a blocking overlay. Escape and the close button dismiss it; dismissal records the current version.
- Trigger behavior: no previous read-version record means no card (fresh install); a recorded version lower than a known current release shows it; closing records current version so it is not repeated.
- Added eight i18n keys in all five locales (zh/en/de/es/fr).

## Verified

- `cd apps/desktop && npm run check`: exit 0; `svelte-check found 0 errors and 1 warning in 1 file`. The warning is `WhatsNewCard.svelte:6:35`, `state_referenced_locally`; it is unrelated to this drag fix.
- `cd apps/desktop && npm test`: 139 passed, 0 failed, 0 skipped. The reader browser geometry test ran and passed; no browser test was skipped.
- `cargo fmt --all -- --check`: exit 0, no output.
- Drag regression coverage: focused sidebar-shell test verifies there is no workspace mousedown handler, Windows attaches only the sidebar action, the 126 px collapsed header/unchanged header drag remain present; pure geometry unit test verifies y=0 and y=52 pass while y=52.01 fails.
- What's New targeted test: `node --test tests/whats-new.test.mjs` — 4 passed, 0 failed.
- `git diff --check`: passed.
- Windows-only rendering is isolated by `class:windows-sidebar-up={onboardingPlatform === "windows"}`. `.window-header` remains 52 px and `.sidebar-header` retains `padding: 0 10px 0 80px`; AppToolbar CSS is untouched. Thus the macOS rendering path keeps the original layout values and does not receive the Windows positioning rule.

## Blocked / not verified

- **Windows real-device visual effect and drag behavior were not verified.** The fix is verified through code-level and geometry tests only.
- macOS GUI rendering was not visually tested or launched, per task constraint.
- `apps/desktop/src-tauri/tauri.conf.json` version and title bar settings were not changed.
