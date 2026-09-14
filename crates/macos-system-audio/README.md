# macos-system-audio

Standalone macOS-only Rust crate that captures system audio through Apple
ScreenCaptureKit and delivers bounded mono `f32` PCM through a narrow sink
trait. Built for Mistaken (`docs/specs/spec-07-macos-system-audio-adapter.md`)
but has **no dependency** on the Mistaken application crate, Tauri, serde,
or ASR.

## What this crate does

- Captures whatever the machine is currently playing (system output), not
  the microphone.
- Requests and inspects the macOS **Screen Recording** TCC permission,
  because ScreenCaptureKit has no audio-only permission of its own.
- Registers **only** an audio stream output on a real `SCStream`. No frame,
  pixel buffer, image, or screenshot is ever requested, read, copied, or
  written, even though the OS permission is named "Screen Recording".
- Excludes Mistaken's own process audio from capture
  (`excludesCurrentProcessAudio = true`) so the app can never transcribe
  its own output.
- Validates the *actual* delivered audio format from every sample buffer
  instead of assuming the requested configuration was honored.
- Downmixes to mono and delivers exact 20 ms blocks through
  [`SystemAudioSink`], allocation-free after `start()` returns.
- Tears down deterministically on `stop()`, `Drop`, or process exit, with
  no leaked `SCStream`, dispatch queue, or Objective-C object.

## What this crate does not do

See §4 "Out of scope" in the spec. In short: no microphone, no ASR, no
mixing with another audio source, no video/screen capture in any form, no
disk recording, no networking, and no wiring into the Tauri application —
that is Spec 09's job, using the mapping table below.

## Public API

```rust
pub enum ScreenRecordingPermission { Granted, Denied, Undetermined }
pub fn permission_status() -> ScreenRecordingPermission;   // never prompts
pub fn request_permission() -> ScreenRecordingPermission;  // prompts if undetermined

pub struct SystemAudioConfig { pub sample_rate_hz: u32, pub exclude_current_process_audio: bool }
pub struct SystemAudioFormat { pub sample_rate_hz: u32, pub channels: u16 } // channels always 1

pub trait SystemAudioSink: Send + 'static {
    fn on_block(&mut self, samples: &[f32], format: SystemAudioFormat) -> bool;
    fn on_error(&mut self, error: SystemAudioError);
}

pub fn start(config: SystemAudioConfig, sink: Box<dyn SystemAudioSink>) -> Result<SystemAudioSession, SystemAudioError>;

pub struct SystemAudioSession { /* opaque */ }
impl SystemAudioSession {
    pub fn stop(&mut self) -> Result<(), SystemAudioError>;
    pub fn dropped_blocks(&self) -> u64;
    pub fn non_audio_deliveries(&self) -> u64;
    pub fn format_rejections(&self) -> u64;
    pub fn last_stream_stopped_error(&self) -> Option<SystemAudioError>;
}
```

See `src/lib.rs` for full doc comments and `platform-notes-macos.md` for
the frozen error-mapping table Spec 09 implements.

## Building

macOS only; building for any other target is a `compile_error!`, not a
silent stub. From this directory:

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
```

Pinned toolchain (mirrors the workspace `rust-toolchain.toml`):
`rustc 1.98.1`.

Resolved dependency versions (`Cargo.lock`, this crate's own lockfile):

| Crate | Version |
| --- | --- |
| `objc2` | 0.6.4 |
| `block2` | 0.6.2 |
| `dispatch2` | 0.3.1 |
| `objc2-foundation` | 0.3.2 |
| `objc2-screen-capture-kit` | 0.3.2 |
| `objc2-core-media` | 0.3.2 |
| `objc2-core-graphics` | 0.3.2 |
| `objc2-core-audio-types` | 0.3.2 |
| `objc2-core-foundation` | 0.3.2 |
| `parking_lot` | 0.12.5 |

## Real-hardware probe

```bash
cargo run --example system_audio_probe [seconds_per_window]
```

Starts real capture, prints aggregate peak/RMS/block-count statistics once
per second (never raw samples), exercises Start → Stop → Start → Stop
twice more, and reports resident memory before/after each cycle. See
`platform-notes-macos.md` for the exact verification procedure and the
recorded results from the real host used during implementation.

## Module layout

| File | Owns |
| --- | --- |
| `src/lib.rs` | Public API surface, `SystemAudioSink`/`SystemAudioFormat`, platform gate |
| `src/permission.rs` | `CGPreflightScreenCaptureAccess`/`CGRequestScreenCaptureAccess` |
| `src/config.rs` | macOS-version gate, frozen `SCStreamConfiguration`/`SCContentFilter` builders |
| `src/format.rs` | Runtime `AudioStreamBasicDescription` validation |
| `src/blocks.rs` | Downmix + exact 20 ms mono block assembly (pure, allocation-free) |
| `src/output.rs` | `SCStreamOutput`/`SCStreamDelegate` Objective-C class, delivery state |
| `src/stream.rs` | Permission/shareable-content flow, `SystemAudioSession` lifecycle |
| `src/error.rs` | `SystemAudioErrorKind`/`SystemAudioError` |
| `examples/system_audio_probe.rs` | Real-hardware developer probe |
