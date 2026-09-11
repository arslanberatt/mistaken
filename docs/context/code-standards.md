# Code Standards

## General

- Keep modules small and single-purpose.
- Fix root causes instead of layering workarounds.
- Do not mix unrelated concerns in one component, hook, command, or Rust module.
- Prefer explicit data flow over hidden global behavior.
- Prefer deterministic source metadata over AI inference when the source is already known.
- Keep the core application local-first; do not introduce network dependencies without an explicit product decision.
- Treat audio, ASR, transcript formatting, and UI as separate concerns.
- Fail visibly and predictably. Do not silently fall back to behavior that violates architecture invariants.
- Comments should explain non-obvious constraints or platform behavior, not restate the code.
- Avoid speculative abstraction. Introduce an abstraction when it protects a real system boundary such as platform audio or ASR runtime choice.

## TypeScript

- TypeScript strict mode is required.
- Avoid `any`; use explicit domain types, generics, or `unknown` with narrowing.
- Validate unknown data received across Tauri IPC boundaries before trusting it when appropriate.
- Keep transcript source as a union type, not a free-form string.
- Represent partial/final transcript state explicitly.
- Do not use transcript text itself to infer speaker/source identity.
- Avoid storing native resource handles in frontend state.
- Prefer pure functions for transcript formatting and clipboard serialization.
- React state must contain presentation/domain state, not raw continuous PCM buffers.

Preferred domain types:

```ts
export type TranscriptSource = "microphone" | "system";

export interface TranscriptSegment {
  id: string;
  source: TranscriptSource;
  text: string;
  startedAtMs: number;
  endedAtMs?: number;
  isFinal: boolean;
}

export type CaptureStatus =
  | "idle"
  | "starting"
  | "listening"
  | "stopping"
  | "error";
```

## React

- Components render state; they do not own native audio or ASR implementations.
- Put feature logic under `src/features/*` rather than growing a single application component.
- Keep Tauri event subscription/unsubscription inside dedicated hooks or service modules.
- Every native-event subscription must clean itself up.
- Do not rerender the full transcript unnecessarily on each tiny interim update; keep segment identity stable.
- Final transcript segments are immutable in normal operation.
- Interim segments may be replaced only by newer interim/final results for the same recognizer segment.
- Clipboard formatting belongs in transcript-domain utilities, not button components.
- UI controls must expose disabled/loading states during capture transitions.

## Tauri

- Tauri commands are narrow boundary methods, not business-logic containers.
- Put native business logic in Rust modules and call it from commands.
- Use typed frontend wrappers for commands/events.
- Do not scatter raw command/event string names across React components.
- Native -> frontend live transcript updates should prefer events over polling.
- Never expose arbitrary filesystem or shell access to the frontend for convenience.
- Keep Tauri capabilities/permissions as narrow as practical.

## Rust

- Prefer ownership and typed state over global mutable state.
- Avoid `unwrap()` and `expect()` in production runtime paths unless the condition is a true startup invariant with a clear message.
- Return structured errors from audio/ASR modules and translate them into user-facing states at the application boundary.
- Keep platform code behind traits/enums/modules rather than `cfg` branches spread throughout the codebase.
- Audio callbacks must avoid blocking operations, filesystem work, UI events per sample, or synchronous ASR inference.
- Use bounded channels/ring buffers for streaming audio.
- Make thread/task shutdown explicit so Start -> Stop -> Start cycles do not leak resources.
- Device and capture handles must be released on stop/error.
- Keep FFI boundaries to sherpa-onnx/whisper.cpp isolated in dedicated modules.
- Document any unsafe block with the invariant that makes it safe.

## Audio

- Microphone and system audio are separate streams from capture to transcript result.
- Never pre-mix microphone and system audio for the normal two-source workflow.
- Normalize sample rate/channel format in one dedicated stage.
- Do not assume every input device uses the ASR model's required sample rate.
- Do not accumulate unbounded raw PCM.
- Define chunk/frame sizes in one configuration location.
- Avoid writing raw audio to disk in production V1.
- If temporary debug recording is implemented, keep it behind a development-only flag and never enable it by default.
- Avoid acoustic speaker classification when source metadata already identifies the speaker class.

## ASR

- ASR output is treated as recognized speech, not text to improve stylistically.
- Do not run grammar correction, spell correction, paraphrasing, or LLM cleanup on transcript results.
- Keep `sherpa-onnx` behind an internal recognizer adapter.
- Do not make UI code depend on model-specific token structures.
- Keep interim and final results separate.
- Never modify a final segment solely because a later phrase makes another wording more grammatically plausible.
- Benchmark changes to model/runtime configuration against the Mistaken-specific test corpus.
- Record benchmark configuration so results are reproducible.
- Model license and runtime license must be checked separately before distribution.

## Styling

- Use CSS custom-property tokens defined in `ui-context.md`; no random hardcoded colors inside components.
- Use Tailwind utilities for layout and token-backed styling.
- Follow the radius and spacing conventions in `ui-context.md`.
- Keep the interface visually quiet; transcript content is the primary surface.
- Do not add gradients, glassmorphism, large hero cards, decorative charts, or marketing-style visuals to the core app workspace.
- Listening/error states must be understandable by text and icon, not color alone.
- Maintain keyboard focus visibility.

## Native Commands and Events

Suggested command naming:

```text
list_microphones
get_model_status
start_capture
stop_capture
```

Suggested native event naming:

```text
audio:status
asr:model-status
transcript:partial
transcript:final
capture:error
```

Rules:

- Names are stable contracts between frontend and native code.
- Payloads use explicit typed shapes.
- Do not overload one event with unrelated payload variants.
- Do not emit one event per audio sample/frame to the frontend.
- Only transcript/status/error information should cross to React at normal runtime frequency.

## Data and Storage

- Current transcript belongs in volatile application memory.
- Raw audio buffers are transient and bounded.
- Do not create a transcript database.
- Do not create hidden transcript backup files.
- Do not upload transcript or audio data.
- Local ASR model files are resources, not user transcript data.
- If non-sensitive UI preferences are persisted later, keep them isolated from transcript/session content.

## File Organization

- `src/features/transcript/` — transcript view, domain helpers, copy/clear behavior.
- `src/features/audio/` — frontend audio source selection and capture state.
- `src/features/settings/` — explicitly approved local preferences only.
- `src/components/ui/` — reusable UI primitives.
- `src/lib/tauri/` — typed Tauri command/event wrappers.
- `src/types/` — shared TypeScript domain types.
- `src-tauri/src/audio/` — audio capture abstraction, buffering, normalization.
- `src-tauri/src/audio/windows/` — WASAPI-specific code.
- `src-tauri/src/audio/macos/` — ScreenCaptureKit/native macOS code.
- `src-tauri/src/asr/` — ASR abstraction and model lifecycle.
- `src-tauri/src/commands/` — thin Tauri command handlers.
- `src-tauri/src/state/` — active runtime handles and non-persistent state.

## Testing Priorities

Tests should focus on product invariants, not only generic code coverage.

High-priority cases:

- Microphone segment renders without prefix.
- System segment renders with `- ` prefix.
- Interim segment replacement does not duplicate final text.
- Finalized transcript text is not grammar-corrected by application logic.
- Copy All preserves line ordering and speaker formatting.
- Clear removes all in-memory transcript state.
- Start/Stop can be repeated without leaking a capture session.
- Microphone and system streams cannot be accidentally swapped.
- Network-disabled core workflow still works.
