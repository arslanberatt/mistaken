# Windows Platform Notes

Recorded decisions, documented limitations, and open verification items for
`windows-system-audio` (spec section 10, "Platform, Permissions, Offline,
Privacy, and Fallback").

## Minimum Windows version

Two numbers, both enforced by `check_version_floor()`:

- **API floor: Windows 10 version 1703 (build 15063).** Below this build,
  event-driven loopback capture receives no events, and Microsoft's
  documented workaround is a second event-driven render stream purely for
  pacing. This crate refuses that workaround — it would double the audio
  client failure surface for one obsolete range — and instead reports
  `SystemAudioErrorKind::Unsupported` below the floor. The build number is
  read via `RtlGetVersion` (`Wdk::System::SystemServices`), not
  `GetVersionExW`, because `GetVersionExW` is subject to application-manifest
  version lying and `RtlGetVersion` is not.
- **Supported and tested floor: Windows 10 version 22H2 (build 19045) and
  Windows 11.** These are the versions all real-hardware acceptance evidence
  below is produced on. Builds between 15063 and 19045 run the identical code
  path and are explicitly untested — no shim, no special case, no claim of
  support for that range.

**Yielded to the integration owner:** the Windows minimum-version declaration
and its user-facing support statement belong in the application's shared
configuration (Spec 09), not in this crate. Spec 14 (Windows packaging)
consumes the same two numbers for its installer floor.

## Permissions

Windows exposes **no per-application permission** for capturing a render
endpoint through loopback. This crate:

- Requests no permission and presents no prompt anywhere in `availability()`
  or `start()`.
- Exposes no permission API.
- Maps no condition to a `system_audio_permission_denied`-style error kind.

**Desktop microphone privacy setting effect — partially observed on real
hardware (build 19045).** Documentation states the desktop-app microphone
privacy toggle governs *capture* endpoints (microphones), not *render*
endpoints consumed through loopback. Observed on the verification host: both
`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone`
and the matching `HKCU` key read `Allow` throughout every capture run, and
loopback capture succeeded with no prompt of any kind. The *negative*
direction — flipping the toggle to `Deny` and confirming loopback still
captures — was **not exercised**; see "Real-hardware verification status"
below for why. Until it is, this crate claims only that loopback works with
the toggle allowed, not that the toggle is proven irrelevant.

## No extra device or driver

Capture works against only the stock default output device. This crate never
requires, installs, or looks for a "Stereo Mix"-style hardware loopback
device, a virtual audio cable, or a third-party driver — Microsoft's loopback
documentation states these are unnecessary, and requiring one would
contradict the product rule against virtual-cable dependencies.

**Confirmed on real hardware (build 19045).** The verification host's audio
endpoint inventory, enumerated before and after every capture run, was
unchanged and contained no hardware-loopback device: three active render
endpoints (`Kulaklıklar (HyperX Cloud III Wireless)`,
`LS27AG32x (HD Audio Driver for Display Audio)`, `Hoparlör (Realtek(R) Audio)`)
and two active capture endpoints (`Mikrofon (HyperX Cloud III Wireless)`,
`Mikrofon (Realtek(R) Audio)`). No "Stereo Mix" endpoint exists on this host
in either direction, no virtual audio cable is installed, and no third-party
loopback driver was installed or enabled by this crate's operation. Capture
worked against the stock default output device alone.

## Documented capture limitations

These are properties of the WASAPI loopback platform, not defects in this
crate, and the product must state them rather than appear broken:

- **DRM-protected content is not captured.** A trusted audio driver refuses
  loopback capture of protected streams. No WASAPI API distinguishes
  protected-content silence from genuine silence, so this crate reports
  silence and never guesses or diagnoses a cause.
- **Loopback captures the whole session mix**, not one application.
  Everything audible in the audio session — including notification sounds —
  enters the captured stream. Per-application/per-process filtering
  (`ActivateAudioInterfaceAsync` with activation parameters) requires a newer
  Windows floor and is out of scope for V1 per `spec-plan.md`.
- **Remote Desktop** redirects audio through session-specific devices, so a
  Remote Desktop session's captured mix follows that session's endpoint
  rather than the physical machine's speakers.
- Mistaken produces no audio output itself in V1, so self-capture feedback is
  not a present hazard. Unlike ScreenCaptureKit's
  `excludesCurrentProcessAudio` on macOS, WASAPI loopback has **no
  per-process exclusion**, so if Mistaken ever gains audio output, Specs
  09/10 must address feedback explicitly. Recorded here as a forward
  constraint, not solved by this crate.

## Offline and privacy

- This crate has no network code path: no HTTP client, no DNS lookup, no
  remote resource, and no telemetry dependency in `Cargo.toml`.
- No audio is written to disk, no file is created, and no sample value is
  logged anywhere in the crate or its probe example. The probe reports
  aggregate statistics only.
- `captured_frames`, `silent_flag_frames`, and `synthesized_frames` are
  always reported as three separate counters so no report or log line can
  present synthesized silence as captured audio.

## Real-hardware verification status

Verified on real Windows hardware on 2026-09-14. This section records what
was actually executed and what remains unexercised; nothing below is inferred
from documentation or from a cross-compilation result.

### Verification host

| Field | Value |
|---|---|
| Machine | MONSTER ABRA A5 V17.2 |
| CPU | 11th Gen Intel Core i5-11400H @ 2.70 GHz, 6 cores / 12 logical |
| RAM | 15.8 GB |
| OS | Microsoft Windows 10 Education, version 10.0.19045, build **19045**, x64 |
| Toolchain | rustc 1.98.1 (48a229cea 2026-09-01), cargo 1.98.1, host `x86_64-pc-windows-gnu` |
| Linker | MinGW-w64 gcc 15.2.0 (WinLibs UCRT, POSIX threads, SEH) |

Build 19045 is exactly the crate's documented *tested* floor, so
`check_version_floor()` was exercised on its supported-path branch against a
real `RtlGetVersion` result.

**Target note.** Verification ran against the `x86_64-pc-windows-gnu` target,
not `x86_64-pc-windows-msvc`: no MSVC toolchain or Windows SDK is installed on
this host. Both are native Windows targets calling the same WASAPI/COM
exports, so the runtime evidence below is genuine real-Windows behavior; an
MSVC-target build of this crate has still never been produced or run, and
Spec 09/14 should run the check set once more on the MSVC target it ships.

### Default render endpoint used

`GetDefaultAudioEndpoint(eRender, eConsole)` resolved to
`Kulaklıklar (HyperX Cloud III Wireless)`, endpoint id
`{0.0.0.00000000}.{0073df4e-7175-4db4-8ef3-e890953f78f3}`, device state
ACTIVE. The crate itself never logs or exposes this id — it reads it only to
compare against a fresh lookup once per second — so the id above was obtained
from a separate enumeration helper, not from this crate's output.

### Crate check set (all native, all exit 0)

| Command | Result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo check --locked --all-targets` | clean |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | clean, zero warnings |
| `cargo test --locked` | **35 passed, 0 failed, 0 ignored** |
| `cargo build --locked --all-targets` | clean (lib, tests, probe example) |

All five ran with `--locked`, so `Cargo.lock` resolved unchanged: only
`windows` 0.62.2 and its `windows-*` support crates plus the ordinary
proc-macro build crates (`proc-macro2`, `quote`, `syn`, `unicode-ident`). No
`cpal`, no third-party audio crate, no HTTP or telemetry dependency.

The 35 unit tests that were previously executed only in a throwaway scratch
crate on macOS now run natively on Windows as the crate's own tests, covering
every format-validation and conversion branch, block assembly, and the gap /
cap / counter timeline math.

### Real loopback capture — exercised

Known playback: a generated 440 Hz sine, amplitude 0.5, 48 kHz 16-bit stereo
WAV, played through the default endpoint with `System.Media.SoundPlayer`.

- **Observed mix format:** 48 000 Hz, conversion branch **Float32** (the
  `source_encoding` the crate reports). Zero format rejections across every
  run.
- **Block cadence:** exactly **50 blocks per second**, sustained over every
  run (24 s, 18 s, 12 s, 10 s, and 6 × 4 s cycles).
- **Block shape:** `block_len_min = block_len_max = 960` samples = 48000 / 50,
  i.e. exactly 20 ms of mono audio per block. `wrong_length_blocks = 0` and
  `out_of_range_samples = 0` over more than 5 000 delivered blocks — every
  sample finite and within `[-1.0, 1.0]`.
- **Signal during playback:** windowed peak 0.5048 – 0.7734, windowed RMS
  0.3535 – 0.3556 against a theoretical 0.5 / sqrt(2) = 0.35355 for the played
  tone. Far above the criterion-8 threshold of 0.01.
- **Signal with the tone stopped:** windowed peak fell to 0.018 – 0.056 and
  windowed RMS to 0.0015 – 0.0058 within one second, then recovered to
  0.52+ within one second of resuming playback.
- **Discontinuity counter:** `discontinuity_events` incremented exactly once
  per session, on the first packet after `Start()`, and never again — the
  flag is counted and does not synthesize silence for the apparent jump.
- **Silent-flag counter:** `silent_flag_frames` stayed 0 on this host; the
  audio engine delivered real (non-silent-flagged) packets throughout, so the
  `AUDCLNT_BUFFERFLAGS_SILENT` path was not reached by a real packet. It
  remains covered only by unit test
  `silent_flagged_packets_count_separately_from_captured`.

**The peak exceeded the played amplitude** (0.68 – 0.77 observed for a 0.5
tone). This is the endpoint's own DSP in the session mix, not a conversion
defect: RMS tracked the theoretical value to within 0.3 %, which a gain error
or a broken conversion branch could not do. Recorded as an observation, not a
finding.

### Bounded buffering and drop-newest — exercised

Bounded sink, capacity 4 blocks, consumer draining one block every 200 ms
(5 blocks/s consumed against 50 blocks/s produced), 12 s run:

- `dropped_blocks` rose linearly — 41, 86, 131, 177, 222, 267, 312, 357, 402,
  447, 492, **537** — about 45 drops per second, exactly the production /
  consumption difference.
- Block production stayed at **exactly 50 blocks/s for every one of the 12
  seconds**: the refusing sink never stalled the capture loop.
- Working set moved 8.87 → 8.92 MB over the whole run: no internal queueing
  and no memory growth behind a sink that refuses almost everything.
- `captured_frames` continued incrementing normally, and no counter other
  than `dropped_blocks` was touched by the refusals.

### Start → Stop → Start and resource hygiene — exercised

Six consecutive cycles in one process, 4 s of capture each, with capture
confirmed working in every cycle:

| Cycle | `start()` | `stop()` | second `stop()` |
|---|---|---|---|
| 1 | 15.6 ms | 10.3 ms | Ok (idempotent) |
| 2 | 9.6 ms | 10.4 ms | Ok (idempotent) |
| 3 | 6.5 ms | 10.6 ms | Ok (idempotent) |
| 4 | 5.6 ms | 10.5 ms | Ok (idempotent) |
| 5 | 6.5 ms | 10.4 ms | Ok (idempotent) |
| 6 | 7.1 ms | 10.5 ms | Ok (idempotent) |

Every teardown completed in about 10 ms, far inside the one-second budget.
Sampled twice per second across all six cycles:

- **Threads:** 5 while capturing, 4 between cycles, back to 5 on the next
  start — the adapter's one capture thread appears and disappears with the
  session and never accumulates.
- **Handles:** 185 → 186 between cycle 1 and cycle 2, then flat at 186 for
  cycles 2 – 6 (163/164 between cycles). The single one-time increment is
  lazy COM/CRT initialization, not a per-cycle leak: a leaked handle per cycle
  would have shown six increments.
- **Working set:** 8.81 → 9.03 MB across 26 s and six full cycles.
- **CPU:** 0.11 s of processor time over 26 s of wall clock — about 0.4 % of
  one core — and 0.25 s over the 12 s bounded-sink run, which does far more
  per-block work in the sink.

A separate 24 s single-session run held threads at 5, handles at 182, and
working set at 8.84 → 8.90 MB from start to finish.

### Offline and privacy — exercised

While a full capture session ran, the probe process owned **zero TCP
connections and zero UDP endpoints** for its entire lifetime
(`Get-NetTCPConnection` / `Get-NetUDPEndpoint` polled against its pid
throughout). Combined with the `--locked` dependency set above, this is direct
runtime evidence that the crate opens no socket, resolves no name, and emits
no telemetry. No audio file was created and no sample value was printed: the
probe's output is aggregate statistics only.

### Not exercised — BLOCKED

These criteria require actions that were deliberately not performed on this
host. They are **not** passed, and no later document may report them as passed
without a fresh run that actually performs them.

- **AC 16 — default-endpoint switch not exercised.** Detecting the switch
  requires changing the machine's default render endpoint while capture is
  running. The verification host had live voice-call render streams open
  (Discord, Discovery), and moving the default output mid-call would have
  disrupted a real user session, so the switch was not performed. The
  once-per-second `has_default_endpoint_changed` poll therefore ran throughout
  every session and correctly reported *no* change, but the positive
  `EndpointChanged` branch and its detection latency are unmeasured.
- **AC 9 / AC 10 — silence-gap and gap-cap behavior not exercised on real
  hardware.** `synthesized_frames` and `truncated_gap_events` stayed 0 in
  every run because the audio engine never went idle: Discord (pid 7288) and
  Discovery-d (pid 23236) held continuous render streams on the default
  endpoint, so WASAPI delivered real packets without interruption and the
  capture loop's `WAIT_TIMEOUT` synthesis path was never entered. Observing it
  requires a host with no application holding a render stream. Note that what
  *was* observed is still honest — during the quiet phase `captured_frames`
  kept rising with near-zero content rather than being reported as
  synthesized — but the synthesis path itself remains covered only by unit
  tests `idle_gap_between_packets_is_synthesized_and_counted`,
  `idle_wait_timeout_synthesizes_and_advances_expected_position`,
  `gap_longer_than_cap_is_truncated_and_resynchronized`, and
  `idle_wait_timeout_respects_the_cap`.
- **AC 7 / AC 17 — elevated audio-service and device-disable tests not
  exercised.** The verification session did not run as Administrator
  (`WindowsPrincipal.IsInRole(Administrator)` = False). Stopping the
  `Audiosrv` service to induce `AudioServiceDown`, disabling every render
  endpoint to induce `NoRenderEndpoint`, and disabling or unplugging the
  active endpoint to induce `DeviceInvalidated` all require elevation (or
  physical access to the wireless headset's power/dongle). Only
  `availability()`'s success path was exercised; its three failure branches
  and the whole `DeviceInvalidated` recovery-and-retry path are unmeasured.
- **AC 5 (partial) — microphone privacy toggle `Deny` direction not
  exercised.** See "Permissions" above.
- **AC 4 (partial) — the `Unsupported` branch below build 15063 cannot be
  induced** on a build-19045 host. Only the supported-path branch of
  `check_version_floor()` ran, against a real `RtlGetVersion` result.
- **AC 8 / AC 12 (partial) — the endpoint's source channel count was not
  recorded.** `SystemAudioFormat.channels` is hardcoded to `1`: it is the
  *delivered* mono channel count, not the endpoint's. `ValidatedFormat`
  computes and uses `source_channels` internally for the downmix, but no
  public API exposes it, so the probe cannot report it and this evidence
  record must not claim the endpoint was stereo or mono. The raw
  `wFormatTag` / `WAVEFORMATEXTENSIBLE` subformat is likewise not surfaced —
  only the resolved conversion branch (`Float32`) is.
- **AC 19 — DRM-protected content not tested.** The documented limitations
  above are recorded from Microsoft's documentation; no protected stream was
  played against this crate.
