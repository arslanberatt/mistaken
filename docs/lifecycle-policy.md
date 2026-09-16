# Capture Lifecycle and Resilience Policy

This document is the single source of truth for Spec 10's frozen recovery
policy, budget/backoff schedule, watchdog bounds, and soak procedure. It is
authored before the classification/supervisor code so implementation follows
a recorded decision rather than inventing one during coding. It restates
`docs/specs/spec-10-capture-lifecycle-resilience.md` section 6 in
implementation-facing terms and records the concrete design decisions made
while implementing it.

## 1. Recovery classification

Recovery is **per source**, **only while the session is otherwise live**,
and **only for transient, source-local, non-user-owned causes**. The
classification is implemented as an exhaustive match over
`RuntimeErrorCode` (`src-tauri/src/audio/supervisor.rs::classify_recovery`)
with no wildcard arm, so a future `RuntimeErrorCode` variant fails to
compile until a deliberate decision is recorded here and in code.

| `RuntimeErrorCode` | Decision | Rationale |
|---|---|---|
| `DeviceDisconnected` | Recoverable | covers Windows `EndpointChanged`/`DeviceInvalidated` and a microphone disconnect; reconnecting the same or a replacement default is often immediate |
| `SystemAudioUnavailable` | Recoverable | covers Windows `AudioServiceDown`/`NoRenderEndpoint` and macOS `StreamStopped`/`NoCaptureContent`; these RuntimeErrorCodes cannot be distinguished further without widening the frozen `AudioErrorKind` enum (consumed unchanged by this spec), so every cause that collapses onto this code is treated uniformly as transient |
| `MicrophoneUnavailable` | Recoverable | symmetric with `DeviceDisconnected`: a momentary "no microphone available" during a recovery attempt (for example immediately after a rapid unplug/replug) is the same physical scenario as a disconnect, just observed at a different point in backend start |
| `CaptureStartFailed` | Recoverable, within budget | **only ever classified while a recovery attempt is already under way** — the initial `start_capture` atomic-rollback path never calls this classifier; a transient failure while re-acquiring a device or stream during a recovery attempt consumes one budget slot exactly like any other recoverable condition |
| `MicrophonePermissionDenied` | Terminal | user-owned decision; retrying would nag |
| `SystemAudioPermissionDenied` | Terminal | user-owned decision; covers macOS `PermissionRequiresRestart` |
| `UnsupportedPlatform` | Terminal | permanent on this host |
| `ModelMissing` | Terminal | requires a file fix; fails the whole session, not one source |
| `ModelLoadFailed` | Terminal | model-level, not source-local |
| `ModelUnsupported` | Terminal | permanent on this host |
| `CaptureStopFailed` | Terminal | already terminal for that source; resources are released regardless |
| `Internal` | Terminal | untrusted state; also the landing code for a contained panic or a poisoned lock |
| `RuntimeUnavailable` | Terminal | never reached as a source-fault reason (pre-backend placeholder); classified defensively |
| `InvalidRequest` | Terminal | command-validation-only; never a source-fault reason |
| `CaptureAlreadyActive` | Terminal | command-validation-only; never a source-fault reason |
| `CaptureNotActive` | Terminal | command-validation-only; never a source-fault reason |
| `AudioQueueOverflow` | Terminal | degradation, not failure; never routed to the fault/recovery path at all (the source stays live) |
| `InferenceLagging` | Terminal | degradation, not failure; never routed to the fault/recovery path at all (the source stays live) |

## 2. Budget, backoff, and reset

Frozen (`RecoveryPolicy::production()`):

- **3 attempts per source per session.** Backoff before attempts 1, 2, 3:
  **500 ms, 2 s, 5 s**.
- The budget **resets** after that source has been continuously `capturing`
  with at least one `receiving` observation for **60 s**.
- Attempts are visible: `capture:error` carries the attempt number in its
  message (`Reconnecting <source>… attempt N of 3`), reusing the same
  `RuntimeErrorCode` the underlying condition already mapped to
  (`recoverable: true`) — no new error code is introduced.
- Budget exhaustion is terminal, with the last underlying `RuntimeError`
  preserved as the reason plus `Automatic reconnection stopped.` appended.
- Recovery never changes the requested source set and never re-enables a
  terminal source.
- Recovery is cancellable: it is scheduled and gated by the *same*
  `RuntimeManager::generation` counter that already gates every observer
  callback (Spec 03/09's existing mechanism). `stop_capture`/a new
  `start_capture`/shutdown all bump generation; a supervisor thread that
  notices its captured generation is stale aborts before installing
  anything, without adding a second cancellation channel.
- While one source recovers, the other source is untouched: no lock is
  shared across its audio path, and the failing source's teardown +
  recovery attempt run entirely off the audio/ASR threads of the healthy
  source.

## 3. Recovery-safe identity and timeline

- Session id and segment-id format (`mic-<sessionId>-<index>` /
  `sys-<sessionId>-<index>`) are unchanged by a recovery.
- The per-source segment index **continues**: `RuntimeManager` tracks, per
  source, the next segment index and the last emitted `endedAtMs`
  (updated from the existing `on_final` observer callback, never read by
  a second writer). A recovery restart passes these as the new worker's
  starting index/offset floor; only a brand new session resets both to
  zero.
- The failing source's in-flight interim is **abandoned**: its ASR worker
  is torn down through a new `abandon()` path (`AsrChunkFeeder::abandon`,
  `MicrophoneMonitor::abandon`, `SystemAudioMonitor::abandon`) that marks
  the ASR chunk pool `abandoned` instead of `finished`-via-flush; the
  worker observes `AsrChunkConsumer::is_abandoned()` and skips its normal
  `StreamingRecognizer::finish()` drain entirely, so no final is ever
  emitted for that interim and no further partial is emitted for its id.
  Every *user*-initiated `Stop` keeps using the existing graceful
  `stop()`/`finish()` path unchanged (its trailing audio is still flushed
  and finalized, exactly as Spec 06 already behaves).
- Every recovery attempt opens a **fresh** `StreamingRecognizer` stream via
  `RecognizerFactory::open_stream` at the newly negotiated format, never
  reusing the old stream (Spec 06's fatal fixed-sample-rate-per-stream
  constraint).
- The restarted source's new `source_start_offset_ms` is
  `max(session_clock_origin.elapsed(), last_ended_at_ms_for_that_source)`,
  so timestamps stay monotonic per source even in a pathological
  fast-recovery case.
- The audio gap itself is never synthesized; it is simply absent from that
  source's fed frames.

## 4. Sustained-lag detection

- Tracked per source, per source's own ASR worker thread
  (`src-tauri/src/asr/worker.rs`), over discrete, non-overlapping 10 s
  windows measured by counting each window's drained chunks versus its
  dropped (overflow) chunks: `lagging` is entered when
  `dropped / (dropped + drained) > 20%` for a completed window, and left
  when a completed window has a `0%` drop rate.
- The existing throttled `inference_lagging` `capture:error` (≤ 1/s,
  raised on *any* new drop, Spec 06) is **unchanged**.
- Interpretation decision, recorded because the spec text has two framings
  that must be reconciled without widening the frozen event contract
  ("entering/leaving emit one status event each" vs. "the degraded flag is
  derived in the frontend from the throttled `inference_lagging` error
  stream"): a lag-state **transition** (entering or leaving) emits exactly
  one additional `capture:error` with code `inference_lagging`,
  `recoverable: true`, and a transition-specific message (entering:
  `"<Source> is transcribing slower than real time. Some audio is being
  skipped."`; leaving: `"<Source> transcription has recovered."`). This
  reuses the existing code/event; no new payload field or event is added.
  The frontend derives the degraded banner purely from having most
  recently observed an *entering* message for that source, clearing it on
  the matching *leaving* message — it does not need to reimplement the
  10 s/20% window itself.
- No thread count, model configuration, sample rate, endpoint rule, or
  source enablement is ever changed by the app in response to lag.

## 5. Watchdogs

- `starting`: bounded at **10 s** for the automatic, non-interactive parts
  of a start (ASR model load, device acquisition via the microphone
  backend, ASR stream open, monitor/worker construction).
- The macOS system-audio probe/start call is **excluded** from this bound:
  it is the one call in this codebase that can legitimately block on a
  live, user-driven Screen Recording permission dialog (Specs 07/09). If
  the user ignores the dialog, cancellation is via the existing
  generation-bump-on-`Stop` path, not a forced timeout.
- `stopping`: bounded at **3 s**, applied around the blocking teardown
  task `stop_capture` already spawns.
- A watchdog expiry force-commits a real terminal state
  (`capture_start_failed` / `capture_stop_failed`) and never leaves the
  UI in a transitional (`starting`/`stopping`) state; the underlying
  background thread, if still finishing on its own, is not force-killed
  (Rust cannot forcibly terminate a native OS thread safely) but is
  leak-free: it still eventually joins and releases its own resources.

## 6. Panic and lock discipline

- Every native worker body (microphone monitor, system monitor, both ASR
  workers, the supervisor's recovery-attempt thread) runs its body inside
  `std::panic::catch_unwind`. A caught panic is converted to one
  `internal` source-terminal error via the existing observer `on_fault`/
  `on_error` callback; the panic payload itself is never logged, only a
  fixed, sanitized description.
- **Known caveat, deliberately not patched by this spec:**
  `src-tauri/Cargo.toml`'s `[profile.release]` sets `panic = "abort"`,
  which makes `catch_unwind` a no-op and skips `Drop`-based resource
  release in `--release` builds (the process aborts immediately instead of
  unwinding). `Cargo.toml` is listed as "Consumed unchanged" for this
  spec, so it is **not** edited here. Every real-hardware verification run
  and every `cargo test`/`cargo check`/`tauri dev` invocation in this
  spec's evidence uses the default (`unwind`) dev/test profile, where
  panic containment behaves exactly as designed and is exercised for
  real. This is recorded as a yielded requirement for the integration
  owner: change `panic = "abort"` to the default (`unwind`) before any
  `--release` build is treated as resilience-verified (Specs 12-14).
- No lock (`std::sync::Mutex`) is ever `.unwrap()`/`.expect()`-ed; every
  lock accessor already maps poison to a structured `internal` error
  (`RuntimeManager::lock_state`/`lock_session`/`lock_asr_model`,
  unchanged by this spec). A poisoned lock therefore always ends the
  session/source with `internal` rather than reusing untrusted state.
- No lock is held across capture, permission, stream construction,
  play/stop, thread join, backoff sleep, recognizer work, or Tauri
  emission; every status mutation clones a small snapshot, releases the
  lock, and only then emits.

## 7. Shutdown

- One idempotent native `shutdown()` on `RuntimeManager` bumps generation
  (cancelling any pending recovery/backoff), takes the active session, and
  runs the existing `ActiveSession::stop()` teardown synchronously.
- Wired to exactly three entry points: the `main` window's `CloseRequested`
  event, Tauri's `RunEvent::ExitRequested`/`Exit`, and a raw
  `SIGINT`/`SIGTERM` (Unix) or console-control (Windows) handler
  implemented with hand-written `extern "C"` bindings to the platform's
  already-linked system library (no new Cargo dependency — `ctrlc`/
  `signal-hook` were considered and rejected solely because Spec 10 may
  not add a dependency).
- `SIGKILL` and power loss are unhandleable and stated as such; the OS
  releases device handles in that case.

## 8. 60-minute soak procedure

1. Start dual-source capture (real microphone + real system audio) on the
   reference host with networking disabled.
2. Speak intermittently into the microphone and play known audio through
   the system output for the full duration, so both sources stay
   `capturing`/`receiving` rather than idle.
3. Every 5 minutes, sample and record: process RSS, thread count, open
   file/handle count, per-source `recovery_attempts`/
   `recovery_successes`/`lag_windows_entered`/`dropped_capture_blocks`/
   `dropped_inference_blocks`/`watchdog_expiries`/`contained_panics`, and
   each source's audio-time-vs-monotonic-clock drift.
4. After minute 5, RSS growth must stay within 5%; thread and handle
   counts must be exactly flat; any recoveries triggered during the run
   must not ratchet allocation across repeated cycles.
5. Stop capture; record final transcript size (the only permitted
   monotonic growth) and confirm a clean, idempotent `stop_capture`.

## 9. Counters

Per source, per session, recorded in evidence and never persisted:
`recovery_attempts`, `recovery_successes`, `terminal_reason`,
`lag_windows_entered`, `dropped_capture_blocks`, `dropped_inference_blocks`,
`watchdog_expiries`, `contained_panics`, plus the Windows crate's
captured/silent/synthesized frame counters passed through unchanged.
