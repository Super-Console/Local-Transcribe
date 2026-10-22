# ADR 0001: Desktop implementation route

- **Status:** Accepted for current development; not a final visual-design approval.
- **Date:** 2026-10-21
- **Decision owner:** Timmy

## Decision

Timmy explicitly accepts Tauri 2 + Svelte for the VoiceToTextApp desktop application. Develop macOS first in `apps/desktop`, while keeping GUI and shared Rust core contracts in the same repository.

## Consequences and limits

- Use the operating system WebView and Tauri native shell/plugins; the UI should retain platform conventions rather than imitate a browser tab.
- This acceptance is specific to VoiceToTextApp. Do not transfer decisions from MicYou or other projects.
- The placeholder product name and final visual design remain undecided. The present interface is provisional.
- Tauri acceptance does not certify Windows packaging/runtime, performance, accessibility, or offline transcription; those need separate evidence.
- No inference engine or model is selected by this decision. Model weights remain behind explicit approval.
