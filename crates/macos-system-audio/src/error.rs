//! Frozen error taxonomy for the macOS system-audio adapter.
//!
//! This crate never depends on the Mistaken application crate or Spec 03's
//! `AudioError`/`RuntimeError` types. Spec 09 implements the mapping table
//! documented in `platform-notes-macos.md` from these values onto that
//! frozen contract.

use std::fmt;

/// Reasons a native system-audio operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemAudioErrorKind {
    /// Screen-recording permission is denied or was never granted.
    PermissionDenied,
    /// Permission was just granted but the shareable-content query still
    /// fails; the running macOS version requires a relaunch before capture
    /// can start.
    PermissionRequiresRestart,
    /// The running macOS version is older than the supported minimum.
    Unsupported,
    /// No display is available to build a content filter.
    NoCaptureContent,
    /// The delivered audio sample buffer is not usable linear-PCM float32.
    UnsupportedFormat,
    /// Starting the `SCStream` failed.
    StartFailed,
    /// Stopping the `SCStream` failed.
    StopFailed,
    /// `SCStreamDelegate` reported that the stream stopped unexpectedly.
    StreamStopped,
    /// An internal invariant was violated; never used as a catch-all for
    /// permission/device/format states that have a specific kind above.
    Internal,
}

/// A native system-audio failure.
#[derive(Debug, Clone)]
pub struct SystemAudioError {
    pub kind: SystemAudioErrorKind,
    /// Sanitized diagnostic for developer evidence. Never user-facing copy,
    /// never a path, never audio content.
    pub detail: String,
}

impl SystemAudioError {
    pub(crate) fn new(kind: SystemAudioErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for SystemAudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for SystemAudioError {}
