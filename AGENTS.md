# Agent Instructions

1. Read `taskbook/任务书.md`, `docs/architecture.md`, `docs/technical-validation.md`, and `PROJECT_STATUS.md` before implementation. User decisions in the taskbook outrank assistant proposals.
2. Preserve the product boundary: local transcription only (in-app recording of the microphone and a chosen app is in scope); no live dictation/subtitles or input-method feature, no cloud transcription, no upload of source media, and no AI summarization/translation in V1.
3. The product name is **Local Transcribe** (decided 2026-10-24); the final visual design stays open until Timmy approves it. A temporary functional UI is allowed for validation, but must be marked provisional and must not be presented as the final design.
4. Follow test-driven development for behavior changes: write a focused failing test, run it and confirm the expected failure, implement the minimum, then run targeted and full tests.
5. Keep the GUI, CLI, and MCP on a shared application/core contract. MCP must use local stdio and expose only the documented transcription operations; do not add a network listener or unrestricted filesystem tools.
6. Use Tauri 2 + Svelte as the Local Transcribe desktop route; Timmy explicitly accepts Tauri and wants macOS first. Keep the final visual design open and validate Windows packaging on Windows. sherpa-onnx is the inference engine (models, licenses and packaging are documented in `docs/model-licenses.md` and the model table). Do not use the archived third-party `sherpa-rs` binding without a fresh, explicit maintenance review.
7. Do not install global tools, download multi-gigabyte model weights, access paid services, or change machine-wide settings without Timmy's explicit approval. Keep dependencies pinned and project-local where practical.
8. Never commit `GPT聊天记录/`, `.DS_Store`, secrets, personal recordings, model weights, or generated transcription output. Do not edit or relocate the original conversation archive.
9. Windows verification requires a real Windows build/test run (or clearly identified CI run). A macOS cross-compile is not equivalent to Windows UI/runtime acceptance. If the Windows host is unreachable, record the exact criterion as blocked.
10. Do not create Git commits or remotes unless Timmy asks. Stage only reviewed project files if later asked to commit.
11. Update `PROJECT_STATUS.md` when a milestone changes, recording verified facts separately from proposals and blockers.

## Release process

- Releases are GitHub pre-releases on `TimmySheep/Local-Transcribe`: a macOS DMG built locally with `npm run bundle:mac` plus `hdiutil`, and a Windows MSI built by CI (`.github/workflows/windows-validation.yml`) from the same commit.
- Before publishing: bump every version file (`apps/desktop/package.json`, `package-lock.json`, `tauri.conf.json` including the four-part WiX version, the desktop and LAN crate `Cargo.toml`, `Cargo.lock`, and the version test fixture), then run `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `npm run check`, `npm test`, and wait for the Windows CI run to pass.
- Never ship binaries built from uncommitted sources. Do not overwrite published assets: either rebuild and publish a new version, or state plainly that the published binary does not contain the newer commits.
- Never fabricate signing/notarization or validation status. Keep secrets, `GPT聊天记录/`, `.DS_Store`, personal recordings, model weights and generated transcripts out of releases and Git.
