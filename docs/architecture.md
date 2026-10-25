# Candidate Architecture (Not Yet Frozen)

This diagram captures a testable direction, not a final approval of every component.

## Desktop implementation decision (2026-10-21)

Timmy explicitly accepts Tauri 2 + Svelte for Local Transcribe (the product name, decided 2026-10-24), starting with macOS. Continue in `apps/desktop`; do not replace this project with a SwiftUI frontend. This records the implementation route; the visual design is still evolving, and it is not evidence that Windows packaging, WebView behavior, performance, or the ASR engine has passed acceptance. Do not transfer native-UI choices from separate projects into this repository.

## Current implementation snapshot (2026-10-22)

The desktop and MCP entry points now use the shared Rust core/task/result store. The store is JSON-file based with cross-process `fs2` locks; SQLite is deferred. The MCP control plane and five stdio tools are implemented and protocol-tested. The core contains an in-process sherpa-onnx path for Paraformer/SenseVoice layouts, but no approved model weights have been used for a real inference test. The CLI is not implemented.

## Local language-model runtime spike (2026-10-24)

- The initial implementation uses `llama-cpp-2 = 0.1.160` in-process. macOS builds enable its Metal backend; non-macOS builds pin CPU inference (`n_gpu_layers = 0`). This is a build-verified candidate, not yet the final runtime decision: no GGUF has been loaded or benchmarked.
- The desktop process owns one on-demand LLM cache at `<app-data>/llm`; the supervised LAN node has its own runtime pointed at the same directory. Desktop, service, and node configure `<app-data>/.compute.lock`; ASR and LLM hold its OS advisory lock for the full cache-residency lifetime. The kernel releases it on process exit. This prevents simultaneous ASR/LLM residency across those processes, but does not remotely evict another process's idle cache: a competing load returns `busy` until that process's configured idle-unload timeout expires, and the user retries.
- The node exposes bounded, streamed summary/ask endpoints plus status/load/unload/cancel under the existing user-enabled trusted-LAN HTTP boundary. Tauri commands use events and add no network listener. Long-text chunking and the answer-card UI are subsequent phases.

```mermaid
flowchart TB
  subgraph Entrypoints[User and Agent entry points]
    GUI[Desktop GUI\nTauri 2 + Svelte (accepted route; macOS first)]
    CLI[CLI]
    MCP[Local MCP server\nstdio only]
  end

  GUI --> CORE[Shared Rust core and task/result contract]
  CLI -. planned .-> CORE
  MCP --> CORE

  CORE --> STORE[(Shared JSON task/result files\nfs2 cross-process locks)]
  CORE --> MM[Model catalog and local model store]
  CORE --> RESULT[Versioned transcript result]
  RESULT --> TXT[TXT exporter]
  RESULT --> JSON[JSON exporter]

  CORE --> ENGINE[In-process inference adapter\nreal-model runtime still unverified]
  ENGINE --> SHERPA[sherpa-onnx 1.13.8\nParaformer/SenseVoice code path]
  ENGINE -. fallback only if evidence requires .-> WHISPER[whisper.cpp adapter candidate]

  MEDIA[User-selected audio/video\nread-only input] --> PREP[Media probe / audio extraction\nFFmpeg distribution TBD]
  PREP --> ENGINE
  MODEL[(Separately downloaded local weights)] --> ENGINE
```

## Architectural invariants

- No source media upload and no cloud transcription.
- MCP is launched over local stdio; no TCP/HTTP listener.
- Tauri and MCP currently use the same Rust core/task/result contract; the CLI must join it when implemented.
- Models are loaded only for inference, not at GUI idle/startup.
- Model weights and user media are outside Git and separate from the desktop binary.
- TXT and JSON are the only V1 output formats.

## Decisions requiring a working spike

1. Whether `sherpa-onnx` official Rust crate provides the needed model/platform matrix with acceptable build time, native bundle size, quality and resource use.
2. The current inference path runs in-process; measure real-model memory/failure behavior before deciding whether a supervised child process is needed, while preserving cancellation, accurate progress and recovery.
3. Validate GUI-started real-model task visibility, cancellation and result retrieval through MCP, including the GUI-closed lifecycle; do so without opening a network listener.
4. Whether file-backed JSON status/results remain adequate or should later move to SQLite; define recovery and retention policy.
5. FFmpeg deployment and exact license obligations for the selected build.
6. Final UI composition after Timmy selects a direction; platform title bars and file dialogs should remain native.

The proposed fallback does not mean both ASR engines should be bundled. Add a second engine only when measured evidence shows the first cannot meet an explicit requirement.
