# macOS Platform Notes — `macos-system-audio`

## Minimum macOS version

**13.0**, checked at runtime in `config::check_macos_version()` via
`NSProcessInfo.isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
13, 0, 0 })`, called from `start()` before anything else.

Rationale (Apple documentation, reviewed while authoring Spec 07):
`SCStreamConfiguration` itself is available from macOS 12.3, but every
*audio* property on it — `capturesAudio`, `sampleRate`, `channelCount`,
`excludesCurrentProcessAudio` — is documented as available from macOS
13.0 only. A 12.3 build would compile and even start a stream, but the
audio half would silently not work. Refusing to start below 13.0 with an
explicit `Unsupported` error is the honest behavior; there is no
degraded/best-effort audio mode.

CoreAudio process taps (macOS 14.2+) were considered and rejected: they
would introduce a second, different permission model alongside Screen
Recording for no benefit at the frozen 13.0 floor.

## Permission model

ScreenCaptureKit has no audio-only TCC service. Capturing system audio
requires the **Screen Recording** permission and a real (if unused)
content filter, even though this crate never reads a pixel.

- `permission_status()` calls `CGPreflightScreenCaptureAccess()`, which
  never prompts. That API can only return a boolean, so this function can
  only ever report `Granted` or `Undetermined` — it structurally cannot
  distinguish "the user denied it" from "never asked", and this crate does
  not use any private/undocumented API (e.g. reading `TCC.db`) to fake
  that distinction.
- `request_permission()` calls `CGRequestScreenCaptureAccess()`, which
  prompts only when the state is genuinely undetermined; if already
  determined it returns immediately. Because a determined "no" is only
  observable here, `Denied` is only ever produced by this function, never
  by `permission_status()`.
- `start()` treats the **`SCShareableContent.getShareableContentWithCompletionHandler`**
  result as the authoritative permission signal, exactly as the spec
  requires: an error or empty display list means capture is not actually
  permitted or possible, regardless of what the preflight check said. The
  redacted `getCurrentProcessShareableContentWithCompletionHandler` API is
  never used — Apple documents it as returning content **without user
  consent via TCC**, so treating its success as "permitted" would fabricate
  availability.
- If permission was undetermined and had to be requested inside this
  `start()` call, but the shareable-content query still fails afterward,
  `start()` returns `PermissionRequiresRestart` instead of a generic
  `PermissionDenied`, so a caller can tell the user to relaunch Mistaken.

### Observed on the real implementation host

Screen Recording permission was **already `Granted`** for the terminal
process used to build and run this crate before this spec's verification
session began (a pre-existing grant from prior local development on this
machine, not something this session created). `permission_status()`
correctly reported `Granted` on every real run, and `start()` succeeded
every time without a fresh prompt.

**Recorded limitation:** the `Denied`/prompt path was verified by code
review and the unit-level mapping logic only, not by a fresh live run on
this host, because reproducing "denied" would require revoking the
terminal's existing Screen Recording grant in System Settings — a
system-security change affecting every other tool sharing that grant on a
shared development machine, which this session does not make without
explicit user direction. The `PermissionDenied`/`PermissionRequiresRestart`
mapping is otherwise fully implemented per the flow above and exercised by
`format`/`blocks` unit tests plus manual code review against Apple's
documented `CGRequestScreenCaptureAccess` return contract.

## Real hardware verification

Performed with the crate's own `examples/system_audio_probe.rs`, using
`afplay /System/Library/Sounds/Glass.aiff` as the known local audio
source, from the actual `main` worktree build (not a mock).

| Field | Value |
| --- | --- |
| Date | 2026-09-14 |
| Model | Mac mini (Mac16,10) |
| Chip | Apple M4 (10-core: 4P + 6E) |
| Memory | 16 GB |
| macOS | 15.7.5 (build 24G624) |
| Architecture | `aarch64-apple-darwin` |
| Rust | `rustc 1.98.1`, `cargo 1.98.1` (pinned by workspace `rust-toolchain.toml`) |
| Screen Recording permission | `Granted` (pre-existing on this host) |
| Audio source | `afplay /System/Library/Sounds/Glass.aiff` (system output) |

Observed results (`cargo run --example system_audio_probe 7`, with
`afplay /System/Library/Sounds/Glass.aiff` looping roughly once per second
in the background during cycle 1):

- `permission_status()` reported `Granted`; `start()` succeeded with no
  prompt, on every one of 5 cycles.
- Real PCM arrived at **50–52 blocks/second** (20 ms blocks; matches
  `48000 / 50` exactly) at a negotiated `rate_hz = 48000` for every
  reported second across all 5 capture cycles.
- While `afplay` was silent, every window reported `peak=0.00000
  rms=0.00000` **while blocks kept arriving at the same ~50/s rate** —
  proving "silent system" (still receiving, genuinely quiet) rather than
  "no audio delivered" (which would show `blocks=0`).
- While `Glass.aiff` played, peak rose to `0.16753`–`0.17534` and RMS to
  `0.01781`–`0.03316`, then decayed back toward the silence floor as the
  chime's reverb tail ended — a real, moving, non-zero signal tied
  directly to the known playback, reproduced on multiple later cycles too
  (cycle 3: peak `0.17534`; cycle 4: peak `0.17534`) since `afplay` kept
  looping across the whole run.
- `dropped_blocks`, `non_audio_deliveries`, and `format_rejections` were
  **`0` for the entire run, every cycle**: the sink never rejected a
  block, no non-audio `SCStreamOutputType` was ever delivered, and every
  observed sample buffer validated as float32 linear PCM at a supported
  rate.
- **Start → Stop, repeated 5 times** (cycles 1–5, exceeding AC16's
  five-cycle minimum) all succeeded with identical behavior; no error, no
  leaked-resource symptom. Per-cycle process inspection via `ps`:

  | Cycle | rss at start (kB) | rss at stop (kB) | threads at start | threads at stop |
  | --- | --- | --- | --- | --- |
  | 1 | 13,088 | 14,064 | 5 | 7 |
  | 2 | 14,288 | 14,544 | 7 | 7 |
  | 3 | 16,160 | 16,224 | 7 | 7 |
  | 4 | 16,304 | 16,368 | 7 | 7 |
  | 5 | 16,528 | 16,480 | 7 | 7 |

  Thread count reached a steady `7` after the first cycle's GCD/ObjC
  runtime warm-up and stayed exactly flat for cycles 2–5 — no per-cycle
  thread growth. Resident memory grew modestly and diminishingly (`14.5 MB
  → 16.2 MB → 16.4 MB → 16.5 MB`, i.e. `+1.7 MB`, `+0.2 MB`, `+0.1 MB`
  cycle-over-cycle from cycle 2 onward) rather than growing linearly per
  cycle, consistent with one-time framework/allocator warm-up rather than
  a per-cycle leak. Mach port count was not measured (no lightweight
  command-line reader was available in this environment); RSS and thread
  count are the two process-inspection metrics this evidence record
  covers.
- Source inspection (`grep -rniE "CGImage|CVPixelBuffer|CVImageBuffer|
  imageBuffer|Screenshot|NSImage|screencapture" src/ examples/`) found
  **zero** matches outside comments naming the `ScreenCaptureKit`
  framework and the `CGPreflightScreenCaptureAccess`/
  `CGRequestScreenCaptureAccess` permission functions themselves,
  corroborating that no frame, pixel buffer, or image API is reachable
  from this crate. `stream.rs` registers exactly one stream output, with
  `SCStreamOutputType::Audio`, and no second output of any other type.
  No file was created anywhere under `/tmp` during any probe run.

## Frozen stream configuration

```text
SCStreamConfiguration:
  capturesAudio                 = true
  sampleRate                    = 48000   (SystemAudioConfig::default(); one of 8000/16000/24000/48000)
  channelCount                  = 2       (REQUESTED_CHANNEL_COUNT; requested, not assumed — see below)
  excludesCurrentProcessAudio   = true
  width, height                 = 2, 2    (nominal; no screen output registered)
  minimumFrameInterval          = 1 fps   (nominal; no frame ever consumed)
  queueDepth                    = 3       (nominal; no frame ever consumed)

SCContentFilter: initWithDisplay:excludingWindows: over the primary display, no exclusions
SCStream outputs: exactly one, SCStreamOutputType::Audio, on this adapter's own serial DispatchQueue
SCStream delegate: this adapter's own StreamOutputHandler, stream:didStopWithError: only
```

The real delivered format is still independently validated per sample
buffer in `format::validate_format_description` from the buffer's actual
`AudioStreamBasicDescription` (not assumed from the request above): linear
PCM, 32-bit float, packed, a rate from `{8000, 16000, 24000, 48000}`, and
1–8 channels. Every real buffer observed on this host validated as
`{sample_rate_hz: 48000, channels: 2, non_interleaved: false}`, i.e. the
framework honored the request exactly.

## Mapping contract for Spec 09

This crate has no dependency on Spec 03's `AudioError`/`RuntimeError`
types and performs none of this mapping itself; Spec 09 implements exactly
this table when wiring the crate in.

| Crate output | Spec 03 native | Frozen `RuntimeError` code |
| --- | --- | --- |
| `SystemAudioSink::on_block(samples, format)` | `PcmBlock { source: AudioSource::System, format: PcmFormat { sample_rate_hz: format.sample_rate_hz, channels: 1 }, valid_samples: samples.len(), .. }` submitted through `PcmBlockSink` | — |
| `on_block` returns `false` | sink-side overflow accounting (mirrors `SystemAudioSession::dropped_blocks()`) | `audio_queue_overflow` |
| `SystemAudioErrorKind::PermissionDenied` | `AudioErrorKind::PermissionDenied` | `system_audio_permission_denied` |
| `SystemAudioErrorKind::PermissionRequiresRestart` | `AudioErrorKind::PermissionDenied` | `system_audio_permission_denied` with a restart instruction in the UI copy |
| `SystemAudioErrorKind::Unsupported` | `AudioErrorKind::Unavailable` | `unsupported_platform` |
| `SystemAudioErrorKind::NoCaptureContent` | `AudioErrorKind::Unavailable` | `system_audio_unavailable` |
| `SystemAudioErrorKind::UnsupportedFormat` | `AudioErrorKind::UnsupportedFormat` | `capture_start_failed` |
| `SystemAudioErrorKind::StartFailed` | `AudioErrorKind::StartFailed` | `capture_start_failed` |
| `SystemAudioErrorKind::StopFailed` | `AudioErrorKind::StopFailed` | `capture_stop_failed` |
| `SystemAudioErrorKind::StreamStopped` | `AudioErrorKind::Unavailable` | `system_audio_unavailable`, recoverable, `source: "system"` |
| `SystemAudioErrorKind::Internal` | `AudioErrorKind::Internal` | `internal` |

`SystemAudioError.detail` is developer evidence only — sanitized, never a
path, never audio content, never user-facing copy. User-facing copy is
Spec 09/11 work keyed off `SystemAudioErrorKind`, never off `detail`.

## Buffering and real-time design notes

- Two buffers are preallocated once per session, before any callback
  fires: a mono accumulator sized `sample_rate_hz / 50` (exactly one 20 ms
  block), and a raw scratch buffer sized `MAX_CHANNELS (8) ×
  PER_CHANNEL_CAPACITY_FRAMES (48_000, one second at the highest supported
  rate)`. A larger-than-one-second sample buffer is processed through the
  scratch in bounded passes (`copy_pcm_data_into_audio_buffer_list` called
  once per pass, `frame_offset` advancing each time) rather than growing
  either buffer.
- `AudioBufferListStorage` is a preallocated raw byte buffer big enough for
  `MAX_CHANNELS` `AudioBuffer` entries laid out after the
  `AudioBufferList` header — the same "flexible array member" layout
  Apple's own SDK expects when malloc'ing a multi-buffer list, avoiding a
  new Objective-C/CoreFoundation allocation per callback.
  `AudioBufferListStorage::populate` only writes into that preallocated
  buffer.
- Delivery-state mutation (`DeliveryState` behind a `parking_lot::Mutex`)
  is touched only by the adapter's own serial delivery `DispatchQueue`
  during normal operation; `stop()` only touches it *after*
  `removeStreamOutput:type:error:` has already returned, which guarantees
  no in-flight callback can still be running, so there is no real
  contention despite the shared lock.
- A `false` return from `SystemAudioSink::on_block` increments an atomic
  drop counter and processing continues immediately; the adapter never
  buffers, retries, or grows memory for a rejected block.
- A mid-session sample-rate or channel-count change (the delivered format
  no longer matching the session's first validated buffer) reports
  `UnsupportedFormat` through `on_error` and stops processing that sample
  buffer, rather than silently mixing formats — was not observed on the
  real host (rate/channels stayed constant at `48000`/`2` for every
  buffer across all three windows).

## Yielded shared-file requirements

Per the spec's ownership boundary, this worktree makes **none** of these
edits. They are recorded here for the integration owner / Spec 09:

1. `src-tauri/tauri.conf.json` — macOS `minimumSystemVersion` must become
   `"13.0"` (see "Minimum macOS version" above).
2. Registration of `macos-system-audio` as a `target_os = "macos"`-only
   dependency of `src-tauri`, and wiring of `macos_system_audio::start()`
   into the application's runtime capture session per the mapping table
   above.
3. Info.plist / hardened-runtime entitlement: Apple documents **no** usage
   description key for the Screen Recording TCC service (unlike
   `NSMicrophoneUsageDescription` for the microphone). This crate's own
   probe never triggered a missing-key denial or termination when run
   directly from a terminal with an existing grant; if the *built and
   packaged* Mistaken `.app` bundle (Spec 13) is denied or terminated for
   a missing key, the exact key and observed behavior must be recorded by
   Spec 13 from that real packaged build — it is not guessed here, because
   TCC attribution and enforcement can differ between an unsigned CLI
   binary and a signed, bundled application.
