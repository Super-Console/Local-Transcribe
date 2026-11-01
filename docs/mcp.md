# Local MCP server

`voice-to-text-mcp` is a Rust MCP server over **stdio only**. It is a client of the shared
local service: the thirteen tools below submit, observe, cancel, retry, export tasks and manage saved LAN nodes, and no
model ever loads inside the MCP process. See the 2026-10-22 update at the end of this file
for the current contract and the verified end-to-end run.

## Use the installed app (1.6.0+)

The MCP server ships inside the app; there is nothing to build or download:

- macOS: `/Applications/Local Transcribe.app/Contents/MacOS/voice-to-text-mcp`
  (or `~/Applications/...` when installed there)
- Windows: `voice-to-text-mcp.exe` next to `voice-to-text-desktop.exe`

Get a ready-to-paste configuration from **Settings → Integration → Copy config**, or from the
command line:

```sh
local-transcribe mcp-config
```

It prints the JSON to stdout and a short note to stderr, so the output stays valid JSON.
There is no directory allow-list and no extra argument to pass: the server accepts any
absolute media path the current user can read, and the per-user local service socket is the
only access boundary. The app never rewrites an MCP client's own configuration.

The MCP server keeps no whitelist of its own; it only normalizes paths (absolute,
canonicalized, regular file) and forwards the request to the service.

The bundled `voice-to-text-service` must stay next to the MCP binary: the server starts it on
demand over the shared local endpoint. Do not copy the MCP binary alone to another folder.

Run the client on the same Mac/PC as the app. A stdio MCP client on another computer starts
the server there and cannot see this machine's model selection, jobs or media paths.

### Compatibility

- 1.6.0 and later: the DMG/MSI installs `voice-to-text-mcp`, `local-transcribe` and
  `voice-to-text-service`. Reinstalling is enough to gain the three sidecars.
- 1.5.x and earlier: only the GUI and the LAN node were packaged, so `voice-to-text-mcp` had
  to be built from source. Reinstall the 1.6.0 package instead; the data folder is reused and
  no migration is needed.

## Build on macOS

From the project root:

```sh
cargo build --release -p voice-to-text-mcp
```

The executable is `target/release/voice-to-text-mcp`. `sherpa-onnx-sys` needs the matching platform static-library archive during the first build; by default its build script downloads the official release asset. If that download is unavailable, set `SHERPA_ONNX_ARCHIVE_DIR` to a directory containing the exact archive expected by the pinned crate. This is a native runtime library, not a speech model; the MCP server never downloads model weights.

Configure the MCP client to launch the executable directly. For clients whose configuration uses `command` and `args`, the shape is:

```json
{
  "command": "/Users/REDACTED/AWS/VoiceToTextApp/target/release/voice-to-text-mcp",
  "args": []
}
```

Adapt the wrapper to the actual MCP client's configuration. This example does not configure Hermes or any other client. Input and output paths must be absolute; the server canonicalizes them and has no arbitrary output-path or shell-execution tool.

**Host boundary:** stdio `command` normally starts the server on the MCP client's host. To share the desktop model selection, tasks and results, launch it on the same Mac as the Tauri app and local files. A client running on another computer will not automatically see Pro's app data or media paths.

## Shared data and jobs

On macOS, the default data directory is `~/Library/Application Support/net.timhuang.localtranscribe`, matching the Tauri bundle identifier. Both entry points use `selected-model.json`, `jobs/` and `transcripts/` there. `--data-dir PATH` and `VOICE_TO_TEXT_APP_DATA_DIR` can override the MCP data directory for tests or an intentionally separate profile; an override will not share desktop state unless both entry points use the same exact directory. The previous `com.timmy.voicetotextapp` data directory is retained and is not migrated.

Task statuses and results are JSON files guarded by `fs2` cross-process file locks; SQLite remains deferred. GUI and MCP enforce one active transcription task at a time. If a worker exits while marked running, the next status read marks it interrupted. Cancellation is cooperative: media decoding itself is not interruptible; the core checks after decoding and around recognition chunks.

`get_job_result` returns metadata, a preview capped at 1,000 characters, and the local result path. It returns the complete transcript only when `include_text=true` is explicitly supplied.

## Tools

There are thirteen tools. All calls go to the same local service, and none of them opens a
network listener.

- `list_models` — list the selected model and structurally valid model folders under `models/`; this does not prove runtime compatibility.
- `list_nodes` — list saved LAN nodes, including whether each answers a 3-second reachability probe.
- `transcribe_file` — start a task from an absolute path under an authorized root, returning a task ID; optional `model_id` must match `list_models`, and optional `nodeId` must be a saved, enabled node (unknown or disabled ids are rejected before any network request).
- `get_job_status` — read durable task state, chunk progress and errors.
- `get_job_result` — read result metadata/preview, with full text opt-in.
- `list_jobs` — active/pending tasks plus only the most recent 200 terminal tasks.
- `cancel_job` — request cooperative cancellation; a task may finish just before the request is observed.
- `retry_job` — retry a failed or cancelled task under the same job id.
- `export_result` — export a finished transcript to an absolute path whose directory exists.
- `add_node` — add a LAN node (`name`, `host`, optional `port`), probe its `/api/v1/capabilities` and save the snapshot. This is the only tool that can introduce a new address; `host` must be a plain host string (no `://`, no whitespace or shell metacharacters).
- `update_node` — change `enabled`, `host`, `port` or `modelId` of a saved node.
- `remove_node` — remove a saved node from the local list only; nothing is sent to the other machine.
- `node_status` — reachability, protocol version and model list for a saved node.

Supported model-folder layouts in the current code are Paraformer and SenseVoice. The user must select an installed model in the desktop app or place a supported model folder under the app-data `models/` directory. No model weights are bundled or downloaded here.

## Verification boundary

On Pro (2026-10-22), the stdio initialize handshake and `tools/list` were exercised against the built executable; all five tools available at that time were listed. Tool calls verified that a valid WAV reaches the explicit “no model selected” guard without fabricating a task or transcript (the deny-path checks run in that revision were removed together with the allow-list). Rust tests also cover symlink escape and cross-process job locking/recovery. This validates protocol/control-plane behavior only—not real speech recognition, model licenses, accuracy, long media, or packaged-client configuration.

## 2026-10-22 update: MCP and CLI are service clients

The earlier five-tool, in-process MCP design is superseded. Both entry points now talk to
the local service over the same Unix socket as everything else; no MCP or CLI process
loads a model by itself.

- MCP tools (13): `list_models`, `list_nodes`, `add_node`, `update_node`,
  `remove_node`, `node_status`, `transcribe_file`, `get_job_status`, `list_jobs`,
  `get_job_result`, `cancel_job`, `retry_job`, `export_result`. `list_nodes` lists saved LAN nodes with a 3-second
  reachability probe. `transcribe_file` accepts an optional `nodeId`; it must
  be a saved, enabled node id (no host/port/url parameters exist), and unknown
  or disabled ids are rejected before any request leaves the machine.
  `list_jobs` returns active/pending plus only the **most recent 200** terminal
  jobs (read from the same persisted statuses as `get_job_status`, so the two
  can never disagree). It is not a full history export.
- File access has no directory allow-list: any absolute path the current user can open is
  accepted. The previous `--allow-root` / `access.json` scheme was removed on 2026-11-01.
- `transcribe_file` returns a job id immediately; use `get_job_status` / `list_jobs`
  to poll and `get_job_result` with `includeText` only when the full text is needed.
- If no service is running, MCP/CLI start one; the single-instance lock prevents a second
  engine.
- CLI: `local-transcribe status | models list|info | jobs add|list|show|cancel|retry|reorder|remove | export`,
  `lan list|nodes|add|remove|enable|disable|set|scan|start|stop|status`, and `mode local|prefer-lan`.
  `jobs add` accepts `--node <id|name> [--lan-model ID] [--fallback-local]`;
  without `--node` every job runs locally and the GUI's `prefer_lan` mode is
  never inherited. Remote failures stay `failed` unless `--fallback-local` is
  explicit (same job id, one history entry). Every command prints JSON; exit
  codes are 0 success, 1 request failure, 2 usage, 4 service unavailable.

Verified on this Mac with the real Qwen3-ASR 0.6B model: CLI submit → 3 chunks →
`completed` → `export --format txt` wrote real Chinese text; MCP handshake negotiated
`2025-03-26`, listed all tools available at that time (nine), returned the same result and exported JSON.

Not yet: GUI talking to the service instead of running inference in-process, and
approval-gated recovery. Two restart behaviours are different: queued (not yet
started) jobs are still lost across a service restart; the finished/failed/cancelled
list is read from disk, so it survives a restart. Do not conflate them.
