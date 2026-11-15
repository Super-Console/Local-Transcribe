# Windows service-socket ACL alias normalization

Date: 2026-11-16
Project: VoiceToTextApp (Local Transcribe)
Worktree: `release/1.8.0`
Executor: OpenCode

## Work performed

- Fixed `private_dacl_matches` in `crates/transcription-service/src/windows_ipc.rs` to
  normalize fixed SDDL aliases to their well-known SID before the strict trustee allowlist.
  `LA` is treated as the current user only when the current SID ends in RID `-500`; for other
  users it remains untrusted. `SY` is recognized by alias or SID. Common fixed aliases
  including `BA`, `BU`, `WD`, `OW`, `AN`, `AU`, `BG`, `CG`, `CO`, `IU`, `LS`, `NS`, `NU`, `PU`,
  `RC`, `RD`, `RE`, and `RU` are canonicalized; canonicalization does not grant them access.
- Added macOS-runnable pure-string regressions for the `LA` / current `-500` match, a different
  user SID rejection, and rejection of `BA`, `BU`, and `WD` ACEs. The focused test failed
  before implementation and passed afterwards.
- No version changes, Windows API calls in the new tests, or security-boundary relaxations.

## Verification

- `cargo test -p voice-to-text-service`: 32 passed, 0 failed, 1 ignored (model/media-gated).
- `cargo clippy -p voice-to-text-service --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.

## Platform status

- The failing `windows-2022` CI output was supplied by Timmy. This task's tests ran on macOS;
  Windows CI/MSI confirmation remains pending after the fix is pushed.
