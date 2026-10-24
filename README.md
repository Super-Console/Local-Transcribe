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
- Models come from a verified in-app list; transcription runs fully offline and never uploads audio.

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
- **Model picker** in the toolbar lists downloaded models, marks the default one, and shows a **green dot only when that model is actually resident in memory** (models load on demand, are reused across jobs, and unload after 120 s idle).
- **Storage** settings show every app directory (models, history results, job records, app data) and offer automatic cleanup by age (1/3/5/7/30 days) or size (1/5/10 GB), off by default; deletions go to the Trash.
- **Interface languages**: 简体中文, English, Español, Français, Deutsch (follows macOS by default).

## LAN Compute V1.1 (Early Beta)

Optional, off by default: transcribe on another computer on your LAN.

- **Node** (`local-transcribe-node`) exposes a local HTTP API (`/api/v1`) with chunked file upload (resume included), a serialized compute queue, result/collect/export APIs and mDNS discovery (`_localtranscribe._tcp.local.`). No pairing or authentication is required.
- **Client** (desktop GUI, LAN view) discovers or manually adds nodes, reads their model lists, chooses a remote model, sends files with progress and resume, tracks task states and fetches results back. The client does not need that model installed locally.
- **Compute modes**: local only / prefer LAN. Legacy `prefer_local` reads as local only. Prefer LAN uses the model chosen for the enabled node; failures retry locally only if a local model is installed and selected. Cancellation never triggers a retry. Fallback retains the job ID and one history entry.
- **Share this Mac**: opt-in on the LAN page. The chosen name, interface and port are remembered, and sharing resumes on the next launch when that interface still exists; if it disappeared, the app does not start the node and asks you to choose another interface (it never silently falls back to `0.0.0.0`). Any non-loopback IPv4 may be used, including Wi-Fi, Ethernet, USB, VPN and Tailscale. The node publishes its installed model library plus the selected external model; clients can choose any listed model. Received media stays in `lan-host` under the app-data directory, with optional cleanup (off by default, terminal tasks only, 1-day minimum, moved to the Trash). Closing the window does not stop a running node while "keep sharing after the window closes" is on; **Quit** asks for confirmation when tasks are running. Desktop hosting is currently macOS-only.
- **Network scope**: sharing may bind any non-loopback IPv4 (Wi-Fi, Ethernet, USB, VPN, Tailscale) or `0.0.0.0` for every interface. Peer filtering follows the mask: `/1`–`/30` allow only the same subnet; `/31` and `/32` point-to-point links (Tailscale) do not filter; `0.0.0.0` is unscoped. The UI states which of the three applies. Transport is HTTP; the app does not configure public access, port mapping or the firewall.
- **Memory**: a node model and a local model are separate processes, so a large remote model plus a large local model each keep their own copy. M2 sampled both processes in the same second: node peak 1.72 GiB, local peak 0.91 GiB, same-second combined peak 2.16 GiB. In a debug M3 run the unload log fired for both after 120 s, but RSS stayed at 872 MB (node) and 484 MB (local) at 260 s; the GUI idled at 42 MB. Treat the unload as cache release, not an immediate RSS guarantee.
- **Tailscale / VPN**: the node can listen on a tailnet address, but it does **not** register mDNS there (multicast does not cross the tailnet), so tailnet nodes never appear in a LAN scan — add their address and port manually. The future mobile app will connect over Tailscale; this does not change the no-auth, private-network-only boundary.
- **Node process**: sharing spawns the bundled `local-transcribe-node` sidecar; the GUI itself never opens the LAN port. The node keeps working when the GUI is closed (or never started) — run the same binary manually, it shares the same data folder and protocol. Stopping sharing (or confirming Quit) stops the child. If a task is running, quitting asks for confirmation first. Closing the window keeps the node alive only while "keep sharing after the window closes" is on and sharing is running; use the menu-bar icon to reopen or quit. A sleeping or lid-closed laptop is unreachable, and clients report it after about five polls (~10 s) — that is expected.
- **Headless node**: `local-transcribe-node --name NAME --data-dir DIR --listen ADDRESS:PORT [--netmask MASK] [--compute on --models-dir APP_DATA]` runs the same node without the GUI. `--listen` is required; a normal LAN address needs `--netmask`, while `0.0.0.0`, loopback and Tailscale `100.64/10` may omit it. `--parent-pid` makes the node exit with its parent; `--idle-unload` accepts 20/120/600/1800/7200 seconds.
- **Data and logs**: app data lives in `~/Library/Application Support/com.timmy.voicetotextapp` (models, history, jobs, `lan-host` received media, settings); node logs live in `~/Library/Logs/com.timmy.voicetotextapp/lan-node.log` (5 MB rotation, one generation).
- **Uninstall residue**: dragging the app to the Trash leaves those two directories behind. Remove them manually after you no longer need the data.
- See `design/windows-early-beta.html` for the frozen Windows Early Beta design and `docs/architecture/windows-lan-review.md` for the design review.

## Supported models

The model page offers only the measured list (3 accuracy-first + 3 speed-first + Chinese punctuation);
arbitrary imports are no longer accepted. Every listed model was scored on the same Chinese test audio
on Apple Silicon; see `docs/model-download-and-quality.md` for the raw numbers.

| Family | Layout | Measured CER |
|---|---|---|
| X-ASR Zipformer (transducer) | `encoder*` + `decoder*` + `joiner*` + `tokens.txt` | 0.84% |
| FireRedASR2 (AED) | `encoder*.onnx` + `decoder*.onnx` + `tokens.txt` | 1.29% |
| Qwen3-ASR 0.6B | `conv_frontend.onnx` + encoder/decoder + `tokenizer/` | 1.80% |
| SenseVoice small (2025) | `model.onnx` + `tokens.txt` | 1.67% |
| Paraformer zh | `model.onnx` + `tokens.txt` | 1.74% |
| Zipformer CTC small zh | `model.onnx` + `tokens.txt` | 2.57% |

Inference runs on sherpa-onnx 1.13.8 (CPU).

## Interfaces

### Desktop

Tauri 2 + Svelte. Drag-and-drop or file picker, incremental transcription display, job queue, history, model library, light/dark theme. The current UI is a functional provisional design, not final approval. The two glass bars (reader tool row and app toolbar) are CSS-only; set `<html data-glass="off">` to disable them (Reduced Transparency does the same automatically).

### CLI

```sh
local-transcribe status
local-transcribe models list
local-transcribe jobs add <media> [--model ID] [--root DIR]
local-transcribe jobs list | show | cancel | retry | remove <job-id>
local-transcribe export <job-id> --format txt|json --output PATH

local-transcribe lan list | nodes
local-transcribe lan add <name> <host>[:port] [--force]
local-transcribe lan remove <id|name>
local-transcribe lan enable|disable <id|name>
local-transcribe lan set <id|name> [--host H] [--port P] [--model ID] [--name N]
local-transcribe lan scan [--timeout 8]
local-transcribe lan start [<id|name>] | stop [<id|name>]
local-transcribe lan status
local-transcribe mode local|prefer-lan
```

All commands print JSON. Exit codes: `0` success, `1` the service reported an error (for
example `lan stop` with no running node: `not_running`), `2` bad arguments, `4` the local
service could not be reached.

`lan add` accepts any plain host string — IP, `.local`, tailnet name or DNS name — and
connects to `http://{host}:{port}` directly; it is never parsed as an IPv4 literal. A
node that does not answer is refused with a readable error unless `--force` is given.

`lan start <node>` / `lan stop <node>` start or stop a node on another Mac over **plain
SSH** using the system's `sshd` and your existing key login (`BatchMode=yes`, no password
prompt). It installs no daemon, LaunchAgent, scheduled task or startup item; the only
remote command is `<node_binary> --from-settings`. The optional `ssh` field of a node is
filled **per initiating machine** — on the Air `ssh pro` means the Pro, on the Pro `ssh
air` means the Air; do not assume the same aliases exist on both ends. `lan start` without
an id starts the node on this Mac and shares the same settings as the GUI switch.

### MCP

`voice-to-text-mcp` speaks the MCP protocol over local stdio and exposes exactly thirteen tools: `transcribe_file`, `get_job_status`, `get_job_result`, `cancel_job`, `retry_job`, `list_jobs`, `list_models`, `export_result`, `list_nodes`, `add_node`, `update_node`, `remove_node`, `node_status`. File access requires explicit `--allow-root DIR` arguments; without any root, all file reads are denied. `add_node` is the only tool that can introduce a new address; every other node tool works only on a node already in `lan-settings.json`, and no tool accepts a command, path or URL scheme. See [`docs/mcp.md`](docs/mcp.md).

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
- LAN Compute V1.1: 61 automated tests cover the task state machine, persistence/crash recovery, no-authentication private-network access, resumable upload, collect path-traversal refusal and ZIP export with sha256 manifest. Cross-device runtime verification is recorded in `docs/testing/`.
- Release signing/notarization is unresolved (unsigned Early Beta; Windows SmartScreen first-run warning applies). The product name and final macOS visual design remain open decisions.

See [`PROJECT_STATUS.md`](PROJECT_STATUS.md) for the full milestone log with verified facts separated from proposals and blockers, and [`docs/technical-validation.md`](docs/technical-validation.md) for reproducible checks.


## License

Code is licensed under the MIT License — see [LICENSE](LICENSE).

The name "Local Transcribe" and the application icon are not covered by this
license; they remain the property of the author.

Third-party components keep their own licenses; see
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
