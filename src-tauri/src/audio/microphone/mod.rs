//! Cross-platform microphone capture built on CPAL.
//!
//! This module owns device enumeration, sample-format conversion, and the
//! real-time capture backend. It knows nothing about Tauri, serialization,
//! or transcript state: [`crate::state::runtime`] translates the types here
//! onto the frozen typed IPC contract.

pub mod cpal_backend;
pub mod device;
pub mod format;
#[cfg(target_os = "macos")]
pub mod permission;
pub mod session;

pub use cpal_backend::CpalMicrophoneBackend;
pub use session::MicrophoneMonitor;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::audio::buffer::PoolConsumer;
use crate::audio::{AudioCaptureSession, AudioError, AudioErrorKind};

/// One enumerated native microphone device.
#[derive(Debug, Clone)]
pub struct NativeMicrophoneDevice {
    pub id: cpal::DeviceId,
    pub label: String,
    pub is_default: bool,
}

/// A live, stoppable microphone capture plus the bounded PCM path the
/// monitor drains. The backend that produced this handle already resolved
/// the device, validated its format, and called `Stream::play()`.
pub struct MicrophoneCaptureHandle {
    pub session: Box<dyn AudioCaptureSession>,
    pub consumer: PoolConsumer,
    pub fault: Arc<StreamFault>,
    pub format: crate::audio::PcmFormat,
}

/// Set by a capture backend's stream-error callback and observed by the
/// monitor thread. This is the only channel through which a real-time audio
/// callback communicates a fault: it stores a flag and nothing else.
#[derive(Debug, Default)]
pub struct StreamFault(AtomicBool);

impl StreamFault {
    pub fn set(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// A microphone capture backend. `CpalMicrophoneBackend` is the production
/// implementation; tests use a deterministic fake that never touches a real
/// device.
///
/// `start` resolves the device, negotiates its format, allocates the bounded
/// PCM pool sized for that format, and plays the stream — the pool cannot be
/// sized correctly before the negotiated sample rate is known, so pool
/// construction lives inside the backend rather than being handed in from
/// outside.
pub trait MicrophoneBackend: Send + Sync {
    fn list_devices(&self) -> Result<Vec<NativeMicrophoneDevice>, AudioError>;

    fn start(&self, device_id: Option<&str>) -> Result<MicrophoneCaptureHandle, AudioError>;
}

/// Observed by the monitor thread; implemented by the Tauri-facing runtime
/// layer to update state and emit events. Every method must be cheap,
/// non-blocking, and panic-free: it runs directly on the monitor thread.
pub trait MicrophoneMonitorObserver: Send + Sync + 'static {
    /// The first real (non-silent) PCM block was observed for this session.
    fn on_first_signal(&self);
    /// The pool dropped at least one frame since the last report.
    fn on_overflow(&self);
    /// The stream reported an error (most commonly a disconnect); the
    /// session is being torn down.
    fn on_fault(&self, kind: AudioErrorKind);
}
