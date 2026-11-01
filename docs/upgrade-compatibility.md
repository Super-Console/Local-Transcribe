# 1.6.0 → 1.7.0 upgrade compatibility plan

Status: **test design and offline source checks only; installer upgrade acceptance is not complete.** Do not install the 1.7.0 candidate on a primary device until all release gates pass.

## Compatibility boundary

- 1.6.0 used Tauri identifier `com.timmy.voicetotextapp`; 1.7.0 intentionally uses `net.timhuang.localtranscribe`.
- The app-data directory therefore changes. On macOS it is `~/Library/Application Support/<identifier>`; on Windows the shared core uses `%LOCALAPPDATA%\<identifier>`. The old data is not migrated, renamed, or cleaned up. A successful update must leave the old directory byte-for-byte unchanged and start with a separate 1.7.0 data directory.
- Windows installer identity is a separate concern. Both versions use product name `Local Transcribe`; neither config pins a custom WiX `upgradeCode`. Tauri 2.10.2 documents that the default WiX UpgradeCode is a deterministic UUIDv5 derived from product name. The local Tauri CLI reports `80e42889-6dff-530e-97fa-62703debbdd1` for 1.7.0, so the expected 1.6.0 code is the same. This is source/tool-derived, **not yet verified against both MSI databases**.
- The app's current model page only selects catalog models in the new app's own library. The previous model directory is not automatically imported or scanned.

## Offline checks already completed

- `1.6.0` tag config and current 1.7.0 config were compared: product name remains `Local Transcribe`; bundle identifier changes; WiX version advances from `1.6.0.0` to `1.7.0.0`.
- `tauri inspect wix-upgrade-code` on the current config returned the expected deterministic code above.
- The existing `bundle-id-data.test.mjs` checks that desktop, core, service, CLI, MCP and LAN node use the 1.7.0 identifier/resolver. Offline Rust tests cover the resolver and the `access.json` first-start/one-time initialization contract using temporary fixtures. The local workspace test suite and strict Clippy passed before this Windows-only test-path correction; the correction changes only a JavaScript test path conversion.
- The actual Windows frontend run found a Windows-only path issue in `reader-glass.test.mjs`; it was fixed to use Node's `fileURLToPath`. On the matching uncommitted 1.7.0 source snapshot, Windows `npm run check` passed (0 diagnostics), `npm test` passed (126 passed, 0 failed, 2 skipped because no Chromium executable was configured), and `npm run build` passed. These are frontend checks only, not Windows Rust/MSI acceptance.

## Installer/runtime acceptance matrix (still pending)

Run only on a disposable Windows test profile or VM, with a known 1.6.0 MSI and the exact 1.7.0 MSI built from the same source snapshot. Do not use real user transcripts, models, or media.

1. Record the 1.6.0 MSI `ProductCode`, `UpgradeCode`, installed version, install directory, and SHA-256 without changing the installation.
2. Seed only synthetic sentinel files under a disposable old-identifier data directory and record their hashes. Install 1.7.0 over 1.6.0; do not use a production account or source data.
3. Confirm MSI upgrade succeeds, one current Add/Remove Programs entry remains, installed files report 1.7.0, and the 1.7.0 MSI has the same `UpgradeCode` while advancing product version/code as expected.
4. Launch the app and confirm the new `%LOCALAPPDATA%\net.timhuang.localtranscribe` directory is used, the old `com.timmy.voicetotextapp` sentinel hashes are unchanged, and no old jobs/settings/model are silently imported.
5. With a separate synthetic fixture only, verify first-start `access.json` initialization is one-time, later user settings are not overwritten, an empty/corrupt list fails closed, and service + CLI/MCP resolve the same new directory.
6. Uninstall/rollback is a separate, explicitly approved test. Do not infer data deletion behavior from MSI upgrade success; confirm that both old and new user data remain untouched.

Required evidence before calling this gate passed: both MSI database identities; installer exit/log; registry and install-path readback; app-data path readback; before/after sentinel hashes; and the one-install inventory. No such end-to-end MSI test has yet run.
