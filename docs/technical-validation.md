# Technical Validation Notes

Separate source evidence from runtime checks. A compile or MCP handshake is not proof that a real model recognizes speech.

## macOS menu-bar icon density and sizing (2026-10-25)

- Source inspection of locked `tray-icon 0.25.1` (`src/platform_impl/macos/mod.rs`) shows that it creates an `NSImage` from the embedded PNG and sets its logical display height to 18 pt. On this 2x Retina display that is a 36-physical-pixel output. The previous macOS embed was only 22×22 pixels; the new build embeds the supplied 44×44 `tray-template@2x.png`, giving the renderer more source pixels and removing the prior low-resolution upscale.
- The tray glyph now comes from the app-logo source alpha mask with uniform scaling. The shipped B scale has 26×20 px opaque bounding box in the 44×44 asset (about 10.6×8.2 pt in the tray's 18 pt square) and 13.4% fully opaque pixels. This is a deliberate size/weight compromise: the source artwork is fuller than the previous hand-drawn outline, so the largest comparison scale looked too heavy.
- Regression checks: Rust `tray_icon_bitmap_is_retina_density` asserts 44×44 at compile/test time; desktop crate 61 passed / 0 failed / 4 ignored; Python `scripts/test-tray-icon.py` 10/10; fmt and strict Clippy passed. The rebuilt debug binary contains the current 44×44 PNG byte-for-byte and not the 22×22 PNG.
- Not verified: the rebuilt debug binary was not running during final checks, and the installed `.app` was not replaced. Therefore the live menu-bar rendering and final optical acceptance remain pending a future real-app launch/installation; asset/build checks are not represented as visual CUA acceptance.

## Local LLM stream filtering (2026-10-25)

- `ThinkTagFilter` runs in the shared `lan-compute` token stream before callbacks, so both desktop events and LAN-node SSE suppress `<think>…</think>` content. It handles delimiters split across UTF-8-safe token chunks, withholds possible partial delimiter prefixes, removes unmatched closing tags, and discards an unterminated thinking block at end-of-generation.
- TDD: three focused tests failed against the initial pass-through filter and pass after implementation (balanced hidden block, delimiters split across chunks, ordinary `<thinking>` text and unterminated block). `cargo test --workspace --offline` passed (220 passed / 0 failed / 11 ignored); `cargo clippy --workspace --all-targets --offline -- -D warnings` and `cargo fmt --all -- --check` passed. Desktop `npm run check` succeeded and `npm test` passed 93/93.
- The Qwen3 `/no_think` prompt directive remains. The existing real-window `track-1.wav` run predates this filter; no post-filter real-model or UI stream run was made. Therefore those checks do not yet prove a real model's final answer is clean; the taskbook's macOS CUA/runtime validation remains outstanding.

## Local LLM long-transcript segmentation (2026-10-25)

- Desktop generation and LAN-node SSE now use `LlmRuntime::generate_segmented_stream`. The loaded model tokenizer measures each complete prompt (system instructions, question/history, wrappers and source segment); the input budget is `floor(8192 × 60%) = 4915` tokens. Segmentation prefers paragraph boundaries, then sentence boundaries; decimals, common English abbreviations and closing quotation marks are protected. A single sentence that cannot fit is rejected instead of being cut.
- Multi-segment generation maps each chunk to a concise grounded partial result (≤512 generated tokens), recursively groups those results using the tokenizer on the complete merge prompt, and streams the final reduction (≤1024 generated tokens). Prompt instructions forbid adding names, numbers, events or conclusions not supported by the current source/partial results. A one-chunk transcript keeps the direct-stream path.
- Partial map output is sent through a separate `partial_token` event as a clearly labelled temporary preview; the final answer stream replaces it. `stage` events carry reading/merging indices for all five localized UI languages. LAN-node partial/stage sends are checked so a disconnected SSE client stops generation instead of silently running the whole map/reduce job.
- Unit coverage includes paragraph/sentence segmentation, no sentence truncation, decimal/abbreviation/quote boundaries, oversized inputs and atomic recursive grouping. `cargo test --workspace --offline`: 231 passed / 0 failed / 11 ignored. Workspace strict Clippy and fmt passed; desktop `npm run check` had 0 errors and `npm test` passed 94/94.
- **Not yet runtime-validated**: no real GGUF was run through this new long-text path, and no post-change ≥30-minute answer/fact check, first-token latency, token/s, RSS, stop, or disconnect measurement was made. The previously observed 9766-token prompt failure predates this implementation. Proceed to taskbook P5 for real macOS UI/model acceptance; code/unit tests are not claimed as that acceptance.

## P5 macOS real-window attempt (2026-10-25 — blocked)

- Launched the current worktree with `npm run tauri -- dev`; the real Tauri window was PID 87937 / window 4503. The same window's AX tree exposed transcript text (including a 143:48 recording) and the model catalog, while its screenshot showed a blank main pane on both the history-detail and model pages. This observation does not establish whether the app failed to paint or the capture was incomplete.
- A summary was started from an approximately 68-minute existing transcript and then stopped during generation. The UI reached a busy state and the stop action returned to a partial/retry state; no complete response was accepted. No reliable timing, throughput, RSS, idle-unload, or name/number fact-check data was collected.
- The seven taskbook UI checks therefore remain unverified. The required three-model, same-transcript/same-four-prompt comparison and menu-bar appearance check were not completed. A CUA legacy-page script request to inspect computed layout was refused because the current permission policy requires unrestricted mode plus trusted launch-time risk acceptance; no permission change or alternate route was used to bypass that refusal.
- Resume P5 only after the blank-pane / AX-versus-screenshot discrepancy can be resolved through an authorized route. Do not treat this attempt as real-model or UI acceptance.

## P5 macOS follow-up: batch-boundary fix and one real long summary (2026-10-25)

- The debug build's first real Qwen3-4B run on the 68-minute transcript aborted in `llama_decode`: runtime log reported `n_ctx=8192`, `n_batch=2048`, `n_ubatch=512`; a segmented prompt can use up to 4915 tokens and was submitted as one logical batch. Upstream llama.cpp defines `n_batch` as the logical maximum submitted to `llama_decode` (see `include/llama.h` and the pinned `llama-cpp-2` API docs).
- `crates/lan-compute/src/llm.rs` now configures `n_batch=CONTEXT_TOKENS` (8192). A focused test failed before the change at the 4915-token segment budget and passed after it. Verification recorded: `cargo test --offline -p voice-to-text-lan` 49/49; `cargo clippy --offline -p voice-to-text-lan --all-targets -- -D warnings`; `cargo fmt --all -- --check`; desktop `npm run check` (0 errors / 0 warnings).
- A real window completed one Qwen3-4B “summarize full transcript” run against a 68:26 Chinese transcript (113 source fragments). The card displayed a complete answer and 138 seconds. Sampled process RSS was 3,704,944 KB during generation and 2,459,504 KB after completion; these are samples, not a guaranteed peak. No crash was observed during this run. First-token latency, cold-load time and token/s were not captured separately.
- The result was in English. A spot-check raises a likely factual error: it says the speaker turned off the computer, while the visible source passage appears to refer to ending remote assistance. No complete three-item name/number audit was done, so the taskbook's fact-consistency criterion remains unverified and the answer is not counted as a pass.
- **Binary provenance remains uncertain:** PID 35398 started at 14:08:19, while the debug executable's filesystem modification time was 14:08:41. The running process may predate that executable; this successful run cannot conclusively prove it used the newly configured `n_batch`.
- The current window screenshot showed both the transcript and answer card rendered, so the earlier blank-pane observation did not reproduce in this view. One full-desktop screenshot showed a quote-shaped menu-bar item, but no tray menu was opened, the exact loaded executable was uncertain, and the CUA connection then exited. Therefore neither current-binary menu-bar acceptance nor the seven-item P5 UI checklist is complete. Only the 4B summary entry ran; the other three prompts and 1.7B/8B comparisons remain outstanding.
- Later process sample at 14:20:09 was 3,746,992 KB RSS while the app was idle, compared with 2,459,504 KB at 14:15:56. RSS was therefore variable across samples; the lower sample is not evidence of stable idle unload or reclaimed memory. `/api/v1/llm/status` was not read back.

## Local LLM P1 code/build validation (2026-10-24)

- Runtime candidate: `llama-cpp-2 = 0.1.160`; `cargo tree -p voice-to-text-lan -e features -i llama-cpp-2` confirmed the macOS target enables `metal`. `cargo check -p voice-to-text-lan --all-targets` passed on this Apple Silicon host. Non-macOS targets use the pinned CPU configuration, but were not built here.
- The LAN node exposes `/api/v1/llm/status`, `/load`, `/unload`, `/cancel`, `/summarize` and `/ask`; request limits, three-turn history, selection validation, missing-model SSE errors and generation-slot release have focused tests. Desktop Tauri status/load/unload/cancel/generation commands compile and have prompt/reference validation tests.
- `<app-data>/.compute.lock` is acquired by ASR and LLM cache owners across the desktop, service and supervised node. Tests confirm both that a second lock owner is rejected until the first guard drops and that the node derives the lock from the application model root rather than its separate `lan-host` task directory. This validates the lock primitive and configured path, not a real two-process model unload/load cycle; competing processes are not remotely preempted and report `busy` until idle unload.
- `cargo test --workspace`: 217 passed, 0 failed, 11 ignored. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check` passed. Frontend `npm run check`: 0 errors / 0 warnings; `npm test`: 82 passed. Re-ran the focused LLM tests at 22:18 CST on the current tree: `cargo test -p voice-to-text-lan llm::tests --offline` (4 passed) and `cargo test -p voice-to-text-desktop llm::tests --offline` (4 passed).
- GGUF loading/generation was not yet tested. Cold-load time, first-token latency, token/s, peak/idle RSS, cancellation against a real model, and answer fact consistency remain unverified. Code/build checks do not constitute model runtime acceptance.

## Local LLM catalog P2 preparation (2026-10-24)

- Read-only Hugging Face checks pinned three official Qwen GGUF repositories to immutable commits and verified the exact filenames, byte lengths, LFS SHA-256 values, `cardData.license`, and repository `LICENSE`; values are recorded in `docs/model-licenses.md`.
- The desktop catalog now groups them under “Language models”, shows file size, estimated memory and Apache-2.0 before download, uses revision-pinned URLs, verifies size and SHA-256, and installs/deletes only the catalogued file (deletion moves it to trash). Focused tests cover the manifest, memory/license row, path, trash move, and digest failures.
- Timmy explicitly authorized all three downloads. On 2026-10-24 the source app was launched in the real desktop window against the existing app-data; the visible catalog showed each file size, estimated memory, and Apache-2.0 before download. The first fixed-revision Qwen3-4B transfer reached 1,024,775,311 / 2,497,280,256 bytes (41.04%) at 22:30 CST, then the primary long-lived connection dropped. The pre-fix retry path truncated the staging file with `File::create` and restarted from zero, reached 52,202,208 bytes, failed again and ended the download task; the app process exited around 22:35 CST with no crash report. The downloader was then changed under TDD to resume with HTTP Range: an existing staging file sends `Range: bytes=<size>-`, a 206 response appends, a 200 response (server ignores Range) or an oversized stale file rewrites from zero with progress reset. Three new local-HTTP-server tests cover interruption/resume, an ignored Range, and a complete-sized stale file; the `model_download` group reports 16 passed / 0 failed / 1 ignored (network), with strict Clippy and fmt clean. `curl -r 52202208-52202209` against the pinned revision URL returned `206` and `Content-Range: bytes 52202208-52202209/2497280256`, confirming host support for resume. The app was restarted at 00:43 CST and resumed the same staging file from 52,202,208 bytes; the UI showed 31% at 00:50 CST matching the byte count. Qwen3-4B finished at 01:09 CST: the install path verified size and SHA-256 before moving the file to `<app-data>/llm/Qwen3-4B-Q4_K_M.gguf` (2,497,280,256 bytes) and trashed the empty staging directory. An independent `shasum -a 256` read returned `7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5`, matching `docs/model-licenses.md`; the real window row then showed “已下载”. The Qwen3-8B transfer started at 01:14 CST and repeatedly stalled on dropped proxy connections (for example 108,917,454 bytes from 01:16 to 01:24 CST with no sockets open), so the downloader gained `fetch_with_fallback`: up to eight automatic attempts alternating the pinned primary and mirror URLs, each resuming from the staged byte count after a short backoff, with cancel still honored and the staging file kept. Two focused tests cover auto-retry resume and attempt exhaustion; the `model_download` group reports 18 passed / 0 failed / 1 ignored, with strict Clippy and fmt clean. After restarting the app at 01:39 CST, the 8B resumed from 376,828,513 bytes and completed at 01:49 CST; SHA-256 verification passed and the file installed to `<app-data>/llm/Qwen3-8B-Q4_K_M.gguf` (5,027,783,488 bytes). An independent `shasum -a 256` returned `d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785`, matching the pinned metadata; the window row showed “已下载”. The Qwen3-1.7B Q8_0 download started at 02:02 CST and completed at 02:21 CST, installing to `<app-data>/llm/Qwen3-1.7B-Q8_0.gguf` (1,834,426,016 bytes) after size and SHA-256 verification; an independent `shasum -a 256` returned `061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a`, matching the pinned metadata, and the real window showed “已下载” for all three language-model tiers with 17 GiB disk free. The P2 download/install scope therefore passes: catalog size/memory/license display, pinned revisions, progress, cancel, trash-based deletion, resumable transfers and all three installs were exercised in the real app. Runtime load/generate metrics and fact-consistency checks belong to the P5 macOS acceptance run and remain unperformed; P2's later stages are not started. Physical memory is 16 GiB.

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
