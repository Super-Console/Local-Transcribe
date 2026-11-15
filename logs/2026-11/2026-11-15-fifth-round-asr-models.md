# Fifth-round ASR model evaluation and catalog update

Date: 2026-11-15
Project: VoiceToTextApp (Local Transcribe)
Worktree: `research/models-180`
Executor: OpenCode

## Work performed

- Finished the fifth-round CPU/two-thread evaluation of five models on the six existing local corpus clips. FireRedASR2 was rerun after its interrupted initial attempt; the retry completed normally. Qwen3-ASR 1.7B completed all six clips.
- Added the two candidates that passed the stated gate, with pinned model-source revisions, exact file sizes, SHA-256 verification, license-card metadata and observed peak RSS estimates. Updated UI recommendations in five languages and quality/license documentation.
- Kept model weights, corpus and generated transcripts in `~/lt-model-eval/`; none are part of the source changes. No version bump, release, Windows validation or cleanup.
- `npm ci` reused `apps/desktop/package-lock.json` and installed 62 dependencies into the worktree's project-local `node_modules`; no global package/tool was installed. npm reported 5 audit findings (3 low, 2 moderate); no automatic audit fix was run.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo test --locked --workspace`: 277 passed, 0 failed, 11 ignored.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `npm test`: 133 passed, 0 failed.
- `npm run check`: 0 errors, 0 warnings.
- Exact measurements, per-clip CER, download inventory, hashes and limitations are recorded in `.dispatch/REPORT-180-models-v5.md` and `docs/model-download-and-quality.md`.

## Current status

Implementation and verification are complete. One reviewed commit was created on `research/models-180` (`Add verified fifth-round ASR models`). The `.dispatch/` reports remain local because that directory is excluded by this worktree's `.git/info/exclude`.
