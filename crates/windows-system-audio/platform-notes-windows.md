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

**Desktop microphone privacy setting effect — pending real-host
observation.** Documentation states the desktop-app microphone privacy
toggle governs *capture* endpoints (microphones), not *render* endpoints
consumed through loopback. Whether the tested build's toggle has any
observable effect on this crate's loopback capture is **Pending**: it must
be measured by toggling the setting on real Windows hardware while capture
is active, and the observed result recorded here verbatim — in either
direction — rather than assumed from documentation alone.

## No extra device or driver

Capture works against only the stock default output device. This crate never
requires, installs, or looks for a "Stereo Mix"-style hardware loopback
device, a virtual audio cable, or a third-party driver — Microsoft's loopback
documentation states these are unnecessary, and requiring one would
contradict the product rule against virtual-cable dependencies. Real-host
verification must confirm no such device was installed or enabled by this
crate's operation.

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

See `docs/specs/spec-08-windows-system-audio-adapter.md` section 16 for the
full evidence template. Summary as of this implementation pass:

- **Static verification (performed on a macOS development host):** the crate
  and every module (including its `#[cfg(test)]` unit tests and the probe
  example) type-check cleanly against `windows` 0.62.2 for the
  `x86_64-pc-windows-gnu` target via `cargo check --all-targets` and pass
  `cargo clippy --all-targets -- -D warnings` for that target. The
  platform-independent modules (`format.rs`, `blocks.rs`, `timeline.rs`) were
  additionally copied into a throwaway, non-Windows-gated scratch crate and
  their 35 unit tests were executed and passed natively, confirming the mix-
  format validation, every sample-conversion branch (float32, PCM16/24/32,
  `WAVE_FORMAT_EXTENSIBLE` subformat discrimination), downmix/clamp/
  non-finite handling, block assembly across arbitrary packet sizes, and
  gap/cap/counter timeline math.
- **Real Windows hardware verification: BLOCKED.** No Windows host was
  available in this environment (macOS development workstation, no physical
  or virtual Windows machine, no remote Windows access). Every acceptance
  criterion that requires a real WASAPI stream, a real COM apartment, real
  playback, a real default-endpoint switch, a real device unplug, or real
  process/handle/thread inspection (spec section 12, criteria 5-9 and 16-20)
  is **not exercised** and must not be reported as passed. This file's
  "Windows edition/version/build," "default output device class," and every
  other real-host evidence field stay `Pending` until this crate is run on
  actual Windows hardware.
