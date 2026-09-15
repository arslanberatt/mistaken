//! System audio capture backend interface and platform dispatch.
//!
//! Exposes [`SystemAudioBackend`], the compile-time platform dispatch,
//! and the sink bridge connecting platform crates to Mistaken's bounded
//! PCM block pool.

use std::sync::Arc;

use crate::audio::{AudioCaptureSession, AudioError, PcmBlockSink};

pub mod session;
pub mod sink;

#[cfg(target_os = "macos")]
pub mod macos;

pub mod unsupported;
pub mod windows;

pub use session::{SystemAudioMonitor, SystemAudioMonitorObserver};
pub use sink::SystemAudioSinkBridge;

/// Common system audio capture backend interface.
pub trait SystemAudioBackend: Send + Sync {
    /// Cheap, non-prompting availability probe. Reports why capture is
    /// impossible right now without allocating a stream.
    fn probe(&self) -> Result<(), AudioError>;

    /// Starts capture. On macOS this is the only place a permission request
    /// may occur, and only because the user explicitly started capture.
    fn start(
        &self,
        sink: Box<dyn PcmBlockSink>,
        on_error: Box<dyn FnMut(AudioError) + Send>,
    ) -> Result<Box<dyn AudioCaptureSession>, AudioError>;

    /// Expected or probed sample rate in Hz. Default 48_000.
    fn sample_rate(&self) -> u32 {
        48_000
    }
}

/// Returns the default compile-time resolved system audio backend for this target.
pub fn default_backend() -> Arc<dyn SystemAudioBackend> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::MacOsSystemAudioBackend::new())
    }
    #[cfg(target_os = "windows")]
    {
        Arc::new(windows::WindowsSystemAudioBackend::new())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Arc::new(unsupported::UnsupportedSystemAudioBackend::new())
    }
}
