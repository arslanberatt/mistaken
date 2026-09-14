# windows-system-audio

Standalone Windows-only Rust crate implementing
[Spec 08 — Windows System-Audio Adapter (WASAPI Loopback)](../../docs/specs/spec-08-windows-system-audio-adapter.md).

Captures whatever the **default render endpoint** is playing, through WASAPI
loopback, and delivers bounded mono `f32` blocks through a narrow sink trait
with an honest, continuous timeline. It has no dependency on the Mistaken
application crate, Tauri, serde, or an ASR crate, and it is not wired into
the application yet — that is Spec 09's job.

Building this crate for a non-Windows target is a **compile error**, not a
stub: `src/lib.rs` asserts `#[cfg(not(windows))] compile_error!(...)`.

## Layout

```text
windows-system-audio/
  Cargo.toml / Cargo.lock   own manifest/lockfile, empty [workspace]
  src/
    lib.rs        public API surface, compile_error guard, module wiring
    error.rs      frozen SystemAudioErrorKind taxonomy + HRESULT mapping
    config.rs     SystemAudioConfig (buffer duration, gap-fill cap)
    format.rs     mix-format validation + sample conversion (pure Rust)
    blocks.rs     20 ms mono block assembly (pure Rust)
    timeline.rs   gap measurement, silence synthesis, counters (pure Rust)
    com.rs        COM apartment scope (Windows-only)
    endpoint.rs   default render endpoint resolution + change detection (Windows-only)
    capture.rs    WASAPI init, event-driven capture loop, thread/session (Windows-only)
  examples/
    system_audio_probe.rs   aggregate-only real-hardware probe
  platform-notes-windows.md limitations, version floor, permission finding
```

`format.rs`, `blocks.rs`, and `timeline.rs` contain no Windows-specific code
and are exercised by ordinary `#[cfg(test)]` unit tests with synthetic
values. Because the crate as a whole is gated Windows-only, those tests still
only *run* on a Windows target/host — the separation exists so the logic is
reviewable and testable independently of any live WASAPI stream, not so it
can be tested cross-platform.

## Building

```powershell
cd crates\windows-system-audio
cargo build
cargo build --example system_audio_probe
```

On a non-Windows host, `cargo build`/`cargo check` fail immediately at the
`compile_error!` in `lib.rs` (by design — see Scope section 4 of the spec).
Static type-checking against the real `windows` crate 0.62.2 API surface is
still possible from any host via Rust's cross-compilation target:

```bash
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu --all-targets
cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings
```

This proves every WASAPI/COM call site type-checks against the pinned
`windows` crate version and enabled features. It does **not** prove the code
runs correctly — no linker for the target is installed in this development
environment, and only real Windows hardware can execute WASAPI calls, open a
COM apartment, or observe real audio. Treat a clean cross-check as "compiles
against the real API," never as "verified on Windows."

## Real-hardware verification

Run on real Windows hardware, with networking disabled, from this directory:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --all-targets
cargo run --example system_audio_probe -- 15
```

The probe takes options for the scenarios that need them:

```powershell
cargo run --example system_audio_probe -- 4 --cycles 6
cargo run --example system_audio_probe -- 12 --sink-queue 4 --sink-delay-ms 200
```

`--cycles N` runs N consecutive Start → Stop → Start cycles and reports each
one's start and stop duration; `--sink-queue N` bounds the sink at N pending
blocks so a slow consumer (`--sink-delay-ms M`) makes `on_block` return
`false`, exercising the drop-newest path.

Then follow the developer verification flow in spec section 7: known
playback, idle timeline, resume, long-idle cap, default-endpoint switch,
device unplug, five Start → Stop → Start cycles with handle/thread/working-set
inspection, and (optionally) DRM-protected content to confirm it appears as
silence rather than a diagnosed bug.

**Results actually recorded on real hardware, and the items still not
exercised, are in `platform-notes-windows.md` under "Real-hardware
verification status".** Read that before assuming any step of the flow above
has been run.

The probe never writes audio to disk and never prints a sample value; its
output is aggregate statistics only: sample rate, source encoding branch,
block count and block length, windowed and cumulative peak/RMS, and the full
counter set. Note that the reported `channels` is always `1` — it is the
delivered mono count, not the endpoint's channel count, which no public API
exposes.

## Public API

See `src/lib.rs` and `src/capture.rs` for the exact signatures. Summary:

- `availability() -> Result<(), SystemAudioError>` — version floor, audio
  service, and default render endpoint presence, without starting anything.
- `start(config, sink) -> Result<SystemAudioSession, SystemAudioError>` —
  starts capture on the default render endpoint.
- `SystemAudioSession::stop(&mut self) -> Result<(), SystemAudioError>` —
  idempotent, bounded teardown.
- `SystemAudioSession::counters(&self) -> SystemAudioCounters` — captured,
  silent-flagged, synthesized, discontinuity, truncated-gap, and dropped-block
  counts, always reported separately.
- `SystemAudioSink` — the delivery trait a caller implements; see the
  mapping table onto Spec 03's contract in the spec's section 6, implemented
  by Spec 09, not by this crate.

There is **no permission API**. Windows exposes no per-application permission
gate for render-endpoint loopback, and no error kind maps to
`system_audio_permission_denied`.
