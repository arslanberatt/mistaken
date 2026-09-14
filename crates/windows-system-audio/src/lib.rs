//! Windows WASAPI loopback system-audio capture adapter for Mistaken.
//!
//! Standalone Windows-only crate: no dependency on the Mistaken application
//! crate, Tauri, serde, an ASR crate, or Spec 07's macOS crate. Building this
//! crate for a non-Windows target is a compile error, not a stub — see
//! `docs/specs/spec-08-windows-system-audio-adapter.md` section 4.
//!
//! Captures whatever the **default render endpoint** is playing through
//! WASAPI loopback and delivers bounded mono `f32` blocks with an honest,
//! continuous timeline. There is no permission API: Windows exposes no
//! per-application permission gate for render-endpoint loopback, and this
//! crate must never fabricate one.

#[cfg(not(windows))]
compile_error!(
    "windows-system-audio only builds for Windows targets (WASAPI/COM). \
     See docs/specs/spec-08-windows-system-audio-adapter.md section 4: \
     building this crate on another target is a compile error, not a stub."
);

mod blocks;
#[cfg(windows)]
mod com;
mod config;
#[cfg(windows)]
mod endpoint;
mod error;
mod format;
mod timeline;

#[cfg(windows)]
mod capture;

pub use config::SystemAudioConfig;
pub use error::{SystemAudioError, SystemAudioErrorKind};
pub use format::{SystemAudioFormat, SystemAudioSampleEncoding};
pub use timeline::SystemAudioCounters;

/// Delivery target for captured system audio. Every method is called on the
/// adapter's capture thread and MUST return promptly: no blocking, no
/// contended lock, no allocation, no I/O, no event emission.
pub trait SystemAudioSink: Send + 'static {
    /// One complete 20 ms mono block. Returns `false` if the consumer could
    /// not accept it, which the adapter counts as a drop.
    fn on_block(&mut self, samples: &[f32], format: SystemAudioFormat) -> bool;

    /// Compact terminal or degradation signal. The implementation must only
    /// record state and wake its own worker.
    fn on_error(&mut self, error: SystemAudioError);
}

#[cfg(windows)]
pub use capture::{availability, start, SystemAudioSession};
