# Technical Validation Notes

Separate source evidence from runtime checks. A compile or MCP handshake is not proof that a real model recognizes speech.

## Current Pro check (2026-10-22)

- Target: Apple Silicon MacBook Pro, macOS 27.0.1; `arm64`.
- Rust/Cargo: 1.97.1; Node: v24.18.0. Non-interactive SSH omits `/opt/homebrew/bin`, so frontend commands were run with `PATH=/opt/homebrew/bin:$PATH`; no global tool was installed.
- `cargo fmt --all -- --check` — passed.
- `cargo test --workspace` — 26 passed, 0 failed; one real-recording inference test ignored because it requires a sherpa-onnx model and local test recording.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo build -p voice-to-text-mcp` — passed; debug executable linked.
- `cargo build -p voice-to-text-desktop` — passed; debug desktop target linked. No `.app` release bundle/signature was validated by these commands.
- `npm run check` and `npm run build` in `apps/desktop` — passed.

## Native dependency used for these builds

- `voice-to-text-core` pins `sherpa-onnx` 1.13.8; `crates/transcription-mcp` pins official Rust SDK `rmcp` 3.6.0 and `schemars` 1.1.0.
- The official release asset `sherpa-onnx-v1.13.8-osx-arm64-static-lib.tar.bz2` was fetched from the `k2-fsa/sherpa-onnx` GitHub release URL to the Pro scratch directory and supplied through `SHERPA_ONNX_ARCHIVE_DIR`. Expected and actual size: 20,965,936 bytes. Local SHA-256: `9091bf160dc7fdacedbc906b212badf53c2993f4e5277a0e03998e96c31d60da`. The bzip2 tar opened successfully and contained 17 entries.
- This archive is the native inference library, not model weights. GitHub's release-metadata API failed with a TLS EOF in the fetch path, so the computed SHA-256 is recorded but was not compared to a publisher-provided digest.

## MCP protocol and security checks

- Started the built executable as a real subprocess and exchanged newline-delimited JSON-RPC over stdin/stdout. `initialize` negotiated protocol `2025-03-26`; `tools/list` returned the exact five tools.
- `list_models` returned an empty catalog for an isolated scratch data directory; no user app data was altered.
- `transcribe_file` was denied when no `--allow-root` was provided. With one authorized root, an outside file was rejected; a valid scratch WAV inside the root reached the explicit no-selected-model error. No transcription was started and no result was fabricated.
- Rust unit tests verified symlink escape rejection and a true parent/child-process file-lock, active-job, single-task and interrupted-worker recovery path.
- The `MODEL` kicker/model-selection control remains; the exact visible Chinese heading `模型信息` is absent from `apps/desktop/src/routes/+page.svelte`.

## Existing source research

- `k2-fsa/sherpa-onnx` is the official, maintained repository and includes Rust API examples. This app has code for Paraformer/SenseVoice model layouts and an inference path, but the existence of the code/examples does not validate an exact model export, accuracy, app size or release readiness.
- `thewh1teagle/sherpa-rs` is a separate third-party binding and was reported archived; it is not used here.
- Vibe is a Tauri transcription app with a distinct sidecar/server architecture; it remains a reference only, with no full security audit or code-reuse decision.
- Tauri uses the platform WebView; neither source documentation nor this build measures the app's performance.

## Not tested / outstanding

- No model weights were downloaded. Real audio/video recognition, model compatibility, text quality, long-file behavior, timestamps, runtime cancellation, peak resources, offline inference and dependency/model license review remain unverified.
- The ignored integration test `transcribes_a_local_recording_without_changing_it` has not run.
- No Hermes/client registration was made. The client must launch the stdio server on the same Mac to share desktop data/media paths; remote-host arrangements require a separate decision.
- Windows compilation and runtime/UI behavior are not verified; use a real Windows host/CI rather than treating an Apple build as equivalent.
- An earlier strict `codesign --verify --deep --strict` attempt failed with `code has no resources but signature indicates they must be present`. Signing/bundling was not revalidated in this pass.

## References

- https://github.com/k2-fsa/sherpa-onnx
- https://github.com/k2-fsa/sherpa-onnx/tree/master/rust-api-examples
- https://github.com/thewh1teagle/sherpa-rs
- https://github.com/thewh1teagle/vibe
- https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.6.0
- https://v2.tauri.app/security/
- https://v2.tauri.app/distribute/windows-installer/
- https://ffmpeg.org/legal.html
