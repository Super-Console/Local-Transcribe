# Agent Instructions

1. Read `taskbook/任务书.md`, `docs/architecture.md`, `docs/technical-validation.md`, and `PROJECT_STATUS.md` before implementation. User decisions in the taskbook outrank assistant proposals.
2. Preserve the product boundary: local file transcription only; no live microphone/input-method feature, no cloud transcription, no upload of source media, and no AI summarization/translation in V1.
3. Keep the product name and final visual design undecided until Timmy chooses them. A temporary functional UI is allowed for validation, but must be marked provisional and must not be presented as the final design.
4. Follow test-driven development for behavior changes: write a focused failing test, run it and confirm the expected failure, implement the minimum, then run targeted and full tests.
5. Keep the GUI, CLI, and MCP on a shared application/core contract. MCP must use local stdio and expose only the documented transcription operations; do not add a network listener or unrestricted filesystem tools.
6. Use Tauri 2 + Svelte as the current VoiceToTextApp desktop route; Timmy explicitly accepts Tauri and wants macOS first. Keep the product name/final visual design open and validate Windows packaging on Windows. Treat sherpa-onnx as an inference candidate until exact models, licenses, runtime packaging, and measured size/performance are verified. Do not use the archived third-party `sherpa-rs` binding without a fresh, explicit maintenance review.
7. Do not install global tools, download multi-gigabyte model weights, access paid services, or change machine-wide settings without Timmy's explicit approval. Keep dependencies pinned and project-local where practical.
8. Never commit `GPT聊天记录/`, `.DS_Store`, secrets, personal recordings, model weights, or generated transcription output. Do not edit or relocate the original conversation archive.
9. Windows verification requires a real Windows build/test run (or clearly identified CI run). A macOS cross-compile is not equivalent to Windows UI/runtime acceptance. If the Windows host is unreachable, record the exact criterion as blocked.
10. Do not create Git commits or remotes unless Timmy asks. Stage only reviewed project files if later asked to commit.
11. Update `PROJECT_STATUS.md` when a milestone changes, recording verified facts separately from proposals and blockers.

## Timmy's explicitly requested future release

- Timmy asked for help packaging the macOS version and publishing it on GitHub. Treat this as a pending release task, not authorization to publish immediately. When he asks to continue, inspect the current Git status and GitHub repository/remote first, review the exact release files, build and verify the macOS package, then prepare the GitHub release. Preserve all existing safety rules: never include secrets, `GPT聊天记录/`, `.DS_Store`, personal recordings, model weights, or generated transcription output; do not fabricate signing/notarization or validation status. Confirm any still-unspecified release target or irreversible publication details with Timmy before publishing.
