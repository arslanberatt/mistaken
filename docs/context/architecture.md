# Architecture Context

## Stack

| Layer | Technology | Role |
| --- | --- | --- |
| Desktop shell | Tauri 2 | Cross-platform macOS/Windows desktop application shell and secure frontend/native bridge |
| Frontend | React + TypeScript | Live transcript UI and session interaction |
| Build tooling | Vite | Frontend development/build pipeline |
| Styling | Tailwind CSS | Utility-first styling using shared design tokens |
| UI primitives | shadcn/ui where useful | Accessible reusable UI primitives; avoid unnecessary component complexity |
| Icons | Lucide React | Simple stroke-based application icons |
| Native core | Rust | Audio lifecycle, ASR lifecycle, platform adapters, normalization, and Tauri commands/events |
| Local ASR runtime | sherpa-onnx | Offline/streaming speech recognition and optional local VAD |
| ASR fallback/benchmark | whisper.cpp | Optional local benchmark/fallback implementation, not a cloud dependency |
| Windows system audio | WASAPI Loopback | Capture system/render audio locally on Windows |
| macOS system audio | ScreenCaptureKit | Capture system audio locally on macOS |
| Microphone | Rust/native audio adapter | Capture selected microphone independently from system audio |
| Session storage | In-memory only | Holds the current transcript and transient runtime state |
| Persistent transcript DB | None | Intentionally not used |
| Auth | None | No accounts or identity system |
| Backend | None | Core application has no server dependency |

## Architectural Objective

Mistaken is a local desktop application, not a web service. The UI is responsible for presentation and user interaction. Native Rust code owns audio capture and local inference. Platform-specific system-audio code is isolated behind a common interface. No core feature may depend on a paid API, cloud service, authentication provider, or database.

## High-Level Data Flow

```text
                         ┌────────────────────┐
                         │      Mistaken      │
                         │   Tauri + React    │
                         └─────────┬──────────┘
                                   │
                 ┌─────────────────┴─────────────────┐
                 │                                   │
                 ▼                                   ▼
        Microphone Adapter                  System Audio Adapter
                 │                         Windows: WASAPI
                 │                         macOS: ScreenCaptureKit
                 ▼                                   ▼
          MIC PCM stream                      SYSTEM PCM stream
                 │                                   │
                 ├──────────────┐     ┌──────────────┤
                 ▼              │     │              ▼
           Normalize/Resample   │     │        Normalize/Resample
                 │              │     │              │
                 ▼              │     │              ▼
          ASR Session A         │     │       ASR Session B
           (microphone)         │     │        (system audio)
                 │              │     │              │
                 └──────────────┴──┬──┴──────────────┘
                                   ▼
                         Transcript Aggregator
                                   │
                    ┌──────────────┴──────────────┐
                    ▼                             ▼
             microphone source              system source
              normal text                   "- " prefix
                    └──────────────┬──────────────┘
                                   ▼
                              React UI
                                   │
                                   ▼
                                Copy All
```

## System Boundaries

Recommended project organization:

- `src/` — React application entry point and top-level client composition.
- `src/features/transcript/` — transcript rendering, formatting, copy/clear behavior, interim/final segment presentation.
- `src/features/audio/` — frontend-facing device selection, capture status, and permission state.
- `src/features/settings/` — local runtime preferences that do not contain transcript history.
- `src/components/ui/` — shared UI primitives only; no audio or ASR business logic.
- `src/lib/tauri/` — typed wrappers around Tauri commands/events; React components must not call arbitrary native commands directly.
- `src/types/` — shared TypeScript domain types.
- `src-tauri/src/audio/` — common Rust audio interfaces, buffering, normalization, resampling, lifecycle.
- `src-tauri/src/audio/windows/` — Windows-specific WASAPI implementation.
- `src-tauri/src/audio/macos/` — macOS-specific ScreenCaptureKit/native bridge implementation.
- `src-tauri/src/asr/` — local ASR abstraction, recognizer session management, model lifecycle.
- `src-tauri/src/asr/sherpa/` — sherpa-onnx adapter.
- `src-tauri/src/asr/whisper/` — optional whisper.cpp benchmark/fallback adapter if introduced.
- `src-tauri/src/transcript/` — native transcript event types and source metadata; no grammar correction.
- `src-tauri/src/commands/` — narrow Tauri command handlers that delegate to domain modules.
- `src-tauri/src/state/` — process-level runtime handles such as active capture sessions; no persistent transcript storage.
- `models/` or platform resource bundle — approved local model artifacts only after license review.

Do not place platform-specific audio code in React.
Do not place UI rendering logic in Rust.
Do not let the ASR runtime know how transcript lines are visually formatted.

## Core Interfaces

The application should depend on internal interfaces instead of concrete audio/ASR implementations.

Conceptual Rust boundary:

```rust
pub enum AudioSource {
    Microphone,
    System,
}

pub struct AudioChunk {
    pub source: AudioSource,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub trait SpeechRecognizer {
    fn start_stream(&mut self, source: AudioSource) -> Result<(), String>;
    fn push_audio(&mut self, chunk: AudioChunk) -> Result<(), String>;
    fn stop_stream(&mut self, source: AudioSource) -> Result<(), String>;
}
```

The exact trait signatures may change during implementation, but the separation of concerns must remain.

## Audio Pipeline

### Independent Sources

Microphone and system audio are independent from capture through ASR.

Never do this:

```text
microphone + system audio -> mix -> one recognizer -> guess speakers
```

Required structure:

```text
microphone -> recognizer A -> source=microphone
system     -> recognizer B -> source=system
```

This is the primary mechanism for speaker separation.

### Normalized ASR Format

The ASR adapter owns any required conversion such as:

- Channel downmixing when required by the selected model
- Sample-rate conversion
- Float/PCM conversion
- Frame sizing
- VAD-compatible chunking

Do not hardcode model-specific audio assumptions throughout the app.

### Backpressure

Audio capture must not block the UI thread.

- Native audio callbacks must do minimal work.
- Use bounded queues/ring buffers where appropriate.
- Define an explicit overflow strategy.
- Do not allow unbounded audio buffers.
- If inference cannot keep up, expose degraded state rather than silently exhausting memory.

## ASR Model Strategy

### Primary Runtime

Use `sherpa-onnx` as the first local ASR runtime to evaluate because it supports native offline inference, streaming ASR, and VAD capabilities.

### Model Selection Is a Benchmark Decision

Do not permanently choose a model based only on popularity.

Candidate models must be tested against a Mistaken-specific corpus that includes:

- Correct conversational English
- Intentionally incorrect grammar
- `went` / `won't`-style confusing words
- Strong and weak accents
- Filler words
- Repetitions
- False starts
- Short responses
- Fast conversational speech
- System-audio playback and real microphone recordings

Evaluate at least:

- Word error rate or equivalent transcript accuracy measure
- Preservation of intentionally incorrect wording
- Streaming latency
- CPU usage
- Memory usage
- Apple Silicon performance
- Windows x64 performance
- Model size / installer impact
- Runtime license
- Model-weight license
- Redistribution rights

### License Rule

The runtime license and model-weight license are separate checks.

A model must not be bundled with Mistaken until its specific weight/license terms have been verified for the intended distribution model.

No runtime download from a paid service is allowed.

## Transcript Aggregation

Domain model:

```ts
type TranscriptSource = "microphone" | "system";

type TranscriptSegment = {
  id: string;
  source: TranscriptSource;
  text: string;
  startedAtMs: number;
  endedAtMs?: number;
  isFinal: boolean;
};
```

Formatting rule:

```ts
function formatSegment(segment: TranscriptSegment): string {
  return segment.source === "system"
    ? `- ${segment.text}`
    : segment.text;
}
```

Source identity is structural metadata. Do not infer source identity from transcript text.

## Tauri IPC Model

Prefer event-driven native -> frontend updates for live transcription.

Examples of native events:

- `audio:status`
- `asr:model-status`
- `transcript:partial`
- `transcript:final`
- `capture:error`

Examples of frontend -> native commands:

- `list_microphones`
- `start_capture`
- `stop_capture`
- `get_model_status`

Command handlers must remain thin and delegate to Rust domain modules.

## Storage Model

- **Current transcript**: React/native process memory only.
- **Audio buffers**: bounded transient memory only; discard after inference unless a future explicit recording feature is approved.
- **ASR model files**: local application resources or another explicitly approved local model location.
- **Preferences**: minimal local configuration may store non-transcript settings such as selected device if later approved. It must not become transcript history.
- **Database**: none.
- **Cloud storage**: none.
- **Transcript files**: none automatically. The user persists content by copying it.

## Auth and Access Model

- No authentication.
- No account ownership model.
- No remote authorization.
- OS permissions are the only access boundary relevant to microphone/system-audio capture.
- Request permissions only when required for the feature being used.

## Error Model

The application must handle these states explicitly:

- Microphone permission denied
- Screen/system audio permission denied
- Selected microphone disconnected
- System audio source unavailable
- Local ASR model missing
- Local ASR model failed to load
- Unsupported CPU/architecture/model combination
- Inference slower than incoming audio
- Native capture failure

Errors must be surfaced to the UI with an actionable local message. Do not fall back to a cloud service.

## Invariants

1. Core transcription must not require a paid API, subscription, account, API key, backend, or internet connection.
2. Microphone and system audio must remain separate sources through the transcription pipeline.
3. Speaker separation for the two-source use case must be based on audio source, not diarization guesses.
4. Mistaken must not run grammar correction, sentence rewriting, or semantic cleanup on transcript text.
5. The application must never silently replace a failing local recognizer with a cloud recognizer.
6. Transcript content must not be persisted to a database.
7. Audio must not be uploaded to a remote service by the core application.
8. Audio capture and ASR inference must not block the React/UI thread.
9. Buffers must be bounded; unbounded accumulation of raw audio is forbidden.
10. Platform-specific capture code must remain behind a common internal boundary.
11. The ASR runtime must remain replaceable behind an internal adapter.
12. A model may be bundled only after its model-weight license and redistribution terms are explicitly verified.
13. Final transcript formatting must preserve source identity: microphone = normal text, system = `- ` prefix.
14. Closing/clearing a session must not depend on deleting database records because no transcript database exists.
