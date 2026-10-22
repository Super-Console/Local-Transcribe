# VoiceToTextApp

> Provisional workspace name. The product name (current candidate in the UI: **Local Transcribe**) and the final visual design are intentionally left undecided.

**中文说明见 [README.zh-CN.md](README.zh-CN.md)。**

A local-first desktop tool that transcribes user-selected audio/video files into text — entirely on your own machine. One Rust core serves three equal interfaces: a desktop GUI, a command-line tool, and a local MCP server.

## Product boundary (V1, frozen)

- Local file → on-device transcription → TXT or JSON export.
- Source media stays read-only and is never uploaded.
- No live microphone dictation, no cloud ASR, no translation, summarization or note generation in V1.
- MCP is local stdio only; no TCP/HTTP listener, no unrestricted filesystem access, no shell execution.
- Model weights are a separate, explicit import; installed models transcribe fully offline.

## Architecture

```
apps/desktop/            Tauri 2 + Svelte desktop app (macOS first)
crates/transcription-core/    audio decoding, model layout inspection, sherpa-onnx inference,
                       cancellable streaming transcription, model library, job/result store
crates/transcription-service/ single local service (Unix socket) shared by GUI / CLI / MCP
crates/transcription-cli/     `local-transcribe` command-line client
crates/transcription-mcp/     MCP server over local stdio (9 tools)
```

All three interfaces talk to the same file-backed service instance, so a job started in the GUI is visible to the CLI and MCP, and vice versa.

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

# CLI + service + MCP
cargo build --release -p voice-to-text-cli -p voice-to-text-service -p voice-to-text-mcp
```

Rust stable + Node.js 24 are required. Tests: `cargo test --workspace`, plus `npm run check` in `apps/desktop`.

## Verification status

- Rust workspace, CLI, service and MCP build and pass tests on Apple Silicon (macOS).
- End-to-end transcription verified on macOS with real models (see the table above) through both the CLI and the GUI.
- MCP protocol, tool surface and the `--allow-root` file-access guard were exercised against the live service.
- A Windows CI workflow exists (`.github/workflows/windows-validation.yml`), but real Windows runtime/UI acceptance is **blocked**: the test host's Smart App Control rejects build-script executables (DeviceGuard code-integrity events 3033/3077). No successful Windows Rust build is claimed.
- Release signing/notarization is unresolved. The product name and final visual design remain open decisions.

See [`PROJECT_STATUS.md`](PROJECT_STATUS.md) for the full milestone log with verified facts separated from proposals and blockers, and [`docs/technical-validation.md`](docs/technical-validation.md) for reproducible checks.

## Repository notes

- `GPT聊天记录/` (the original conversation archive) is private, read-only reference material; it is Git-ignored and must never be staged, moved or edited.
- Model weights, media files and generated transcripts are Git-ignored.
- Development logs live under `logs/` (one event per document, append-only).

## License

No license has been chosen yet; all rights are reserved by default until one is added.
