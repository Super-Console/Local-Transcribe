# VoiceToTextApp

> Provisional workspace name. The product name (current candidate in the UI: **Local Transcribe**) and the final visual design are intentionally left undecided.

**中文说明见 [README.zh-CN.md](README.zh-CN.md)。**

A local-first desktop tool that transcribes user-selected audio/video files into text — entirely on your own machine. One Rust core serves three equal interfaces: a desktop GUI, a command-line tool, and a local MCP server.

## Product boundary (V1, frozen)

- Local file → on-device transcription → TXT or JSON export.
- Source media stays read-only and is never uploaded.
- No live dictation/subtitles, no cloud ASR, no translation, summarization or note generation.
- Local recording is in scope since 1.0: record inside the app (microphone and/or a chosen meeting app), then transcribe the tracks locally. Nothing is uploaded.
- MCP is local stdio only; no TCP/HTTP listener, no unrestricted filesystem access, no shell execution.
- Model weights are a separate, explicit import; installed models transcribe fully offline.

## Architecture

```
apps/desktop/            Tauri 2 + Svelte desktop app (macOS first)
crates/transcription-core/    audio decoding, model layout inspection, sherpa-onnx inference,
                       cancellable streaming transcription, model library, job/result store
crates/transcription-service/ single local service (Unix socket on macOS/Linux,
                       loopback-only TCP on Windows) shared by GUI / CLI / MCP
crates/transcription-cli/     `local-transcribe` command-line client
crates/transcription-mcp/     MCP server over local stdio (9 tools)
crates/lan-compute/           LAN Compute V1.1: task store, node server, client, mDNS
                       (`local-transcribe-node` binary)
```

All three interfaces talk to the same file-backed service instance, so a job started in the GUI is visible to the CLI and MCP, and vice versa.

## Desktop GUI (1.0.0)

- **Recording** (1.0): record inside the app with the microphone and/or a chosen application (Core Audio process tap) as two separate tracks; each track is transcribed separately and becomes its own record. No virtual audio driver is installed.
- **Home** merges transcription and history: an import tile plus date-grouped transcript cards; opening a record shows the transcript in the reader.
- **Search** in the toolbar matches both file names and transcript text (local JSON, case-insensitive) and shows the matching snippet; the reader has its own in-transcript search with match count and highlighting.
- **Model picker** in the toolbar lists imported models, marks the default one, and shows a **green dot only when that model is actually resident in memory** (models load on demand, are reused across jobs, and unload after 120 s idle).
- **Storage** settings show every app directory (models, history results, job records, app data) and offer automatic cleanup by age (1/3/5/7/30 days) or size (1/5/10 GB), off by default; deletions go to the Trash.
- **Interface languages**: 简体中文, English, Español, Français, Deutsch (follows macOS by default).

## LAN Compute V1.1 (Early Beta)

Optional, off by default: transcribe on another computer on your LAN.

- **Node** (`local-transcribe-node`) exposes a local HTTP API (`/api/v1`, Bearer-token auth) with chunked file upload (resume included), a serialized compute queue, result/collect/export APIs and mDNS discovery (`_localtranscribe._tcp.local.`).
- **Client** (desktop GUI, LAN view) discovers nodes, pairs by token, sends files with progress and resume, tracks task states (init → file_transferring → queued → transcribing → post_processing → completed / failed / cancelled) and fetches results back.
- **Compute modes**: local only / prefer local / prefer LAN. Media is uploaded only to the node you pair with; no cloud service is involved anywhere.
- See `design/windows-early-beta.html` for the frozen Windows Early Beta design and `docs/architecture/windows-lan-review.md` for the design review.

## Supported model families

Imported model folders are validated strictly by on-disk layout; nothing is auto-downloaded.

| Family | Layout | Verified |
|---|---|---|
| Paraformer | `model.onnx` + `tokens.txt` | ✓ real transcription |
| SenseVoice | `model.onnx` + `tokens.txt` | layout only |
| Whisper | `encoder*.onnx` + `decoder*.onnx` + `tokens.txt` | layout only |
| Qwen3-ASR | `conv_frontend.onnx` + encoder/decoder + `tokenizer/` | ✓ real transcription |
| FireRedASR (v1/v2 AED) | `encoder*.onnx` + `decoder*.onnx` + `tokens.txt` | ✓ real transcription |

Verified on Apple Silicon with the official sherpa-onnx int8 packages of **Qwen3-ASR 0.6B**, **FireRedASR2-AED zh+en** and **Paraformer-zh**, including mixed Chinese/English speech and Sichuan/Henan dialect samples. Inference runs on sherpa-onnx 1.13.8 (CPU).

## Interfaces

### Desktop

Tauri 2 + Svelte. Drag-and-drop or file picker, incremental transcription display, job queue, history, model library, light/dark theme. The current UI is a functional provisional design, not final approval.

### CLI

```sh
local-transcribe status
local-transcribe models list
local-transcribe models import <model-dir>
local-transcribe jobs add <media> [--model ID] [--root DIR]
local-transcribe jobs list | show | cancel | retry | remove <job-id>
local-transcribe export <job-id> --format txt|json --output PATH
```

All commands print JSON. The shared service starts on demand.

### MCP

`voice-to-text-mcp` speaks the MCP protocol over local stdio and exposes exactly nine tools: `transcribe_file`, `get_job_status`, `get_job_result`, `cancel_job`, `retry_job`, `list_jobs`, `list_models`, `import_model`, `export_result`. File access requires explicit `--allow-root DIR` arguments; without any root, all file reads are denied. See [`docs/mcp.md`](docs/mcp.md).

## Build from source (macOS, Apple Silicon)

```sh
# Desktop app (unsigned local build)
cd apps/desktop && npm install && npm run tauri -- build --bundles app --no-sign

# CLI + service + MCP + LAN node
cargo build --release -p voice-to-text-cli -p voice-to-text-service -p voice-to-text-mcp -p voice-to-text-lan
```

Rust stable + Node.js 24 are required. Tests: `cargo test --workspace`, plus `npm run check` in `apps/desktop`.

Windows: the workspace compiles `x86_64-pc-windows-msvc` (verified via CI on `windows-2022`, which also builds the MSI and ships `local-transcribe-node.exe` / `local-transcribe.exe` / `voice-to-text-service.exe` as artifacts). In-app recording captures the microphone on Windows; per-application audio capture is macOS-only (Core Audio process tap).

## Verification status

- Rust workspace, CLI, service and MCP build and pass tests on Apple Silicon (macOS).
- End-to-end transcription verified on macOS with real models (see the table above) through both the CLI and the GUI.
- MCP protocol, tool surface and the `--allow-root` file-access guard were exercised against the live service.
- Windows: the full pipeline (fmt, tests, strict clippy, release build, npm check, MSI) is green on real `windows-2022` CI runners, and the workspace is cross-checked locally against `x86_64-pc-windows-msvc`. Real-machine install/launch acceptance is recorded in `docs/testing/`; runtime claims are limited to what that record actually verified. (Earlier blockers: Smart App Control rejected *compiling* on the test host; CI compilation and MSI install are separate paths.)
- LAN Compute V1.1: 23 automated tests cover the task state machine, persistence/crash recovery, token auth, resumable upload, collect path-traversal refusal and ZIP export with sha256 manifest. Cross-device runtime verification is recorded in `docs/testing/`.
- Release signing/notarization is unresolved (unsigned Early Beta; Windows SmartScreen first-run warning applies). The product name and final macOS visual design remain open decisions.

See [`PROJECT_STATUS.md`](PROJECT_STATUS.md) for the full milestone log with verified facts separated from proposals and blockers, and [`docs/technical-validation.md`](docs/technical-validation.md) for reproducible checks.

## Repository notes

- `GPT聊天记录/` (the original conversation archive) is private, read-only reference material; it is Git-ignored and must never be staged, moved or edited.
- Model weights, media files and generated transcripts are Git-ignored.
- Development logs live under `logs/` (one event per document, append-only).

## License

MIT — see [LICENSE](LICENSE). Third-party components keep their own licenses.
