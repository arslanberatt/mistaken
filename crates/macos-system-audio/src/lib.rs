//! Standalone macOS ScreenCaptureKit system-audio capture adapter.
//!
//! This crate has no dependency on the Mistaken application crate, Tauri,
//! serde, or ASR. It captures whatever the machine is playing through
//! ScreenCaptureKit's audio-only stream output and delivers bounded mono
//! `f32` PCM through [`SystemAudioSink`]. See `README.md` and
//! `platform-notes-macos.md` for the frozen contract Spec 09 wires this
//! crate into.
//!
//! Building this crate for a non-macOS target is a compile error, not a
//! silently empty stub.

#[cfg(not(target_os = "macos"))]
compile_error!("macos-system-audio only builds for macOS");

mod blocks;
mod config;
mod error;
mod format;
mod output;
mod permission;
mod stream;

pub use config::SystemAudioConfig;
pub use error::{SystemAudioError, SystemAudioErrorKind};
pub use permission::{permission_status, request_permission, ScreenRecordingPermission};
pub use stream::{start, SystemAudioSession};

/// The format of every block delivered to a [`SystemAudioSink`]: the
/// stream's actual validated sample rate, and always one channel after
/// downmix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemAudioFormat {
    pub sample_rate_hz: u32,
    pub channels: u16,
}

/// Delivery target. Every method is called on the adapter's serial delivery
/// queue and MUST return promptly: no blocking, no locking that can
/// contend with a slow consumer, no allocation, no I/O, no event emission.
pub trait SystemAudioSink: Send + 'static {
    /// One complete 20 ms mono block. Returns `false` if the consumer could
    /// not accept it, which the adapter counts as a drop.
    fn on_block(&mut self, samples: &[f32], format: SystemAudioFormat) -> bool;

    /// Compact terminal or degradation signal. The implementation must only
    /// record state and wake its own worker.
    fn on_error(&mut self, error: SystemAudioError);
}
