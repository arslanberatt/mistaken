//! Frozen error taxonomy for the Windows system-audio adapter (spec section 6/11).
//!
//! `SystemAudioErrorKind` is the crate's only error vocabulary. Spec 09 maps each
//! kind onto Spec 03's `AudioErrorKind`/`RuntimeError` one-to-one; this crate
//! never imports or produces Spec 03's types itself.

use core::fmt;

/// The complete, frozen set of conditions this crate can report.
///
/// No kind means "permission denied": Windows exposes no per-application
/// permission gate for render-endpoint loopback, so no variant here may be
/// interpreted as one, and none maps to `system_audio_permission_denied`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemAudioErrorKind {
    /// The running Windows build is older than the API floor (build 15063).
    Unsupported,
    /// No default render endpoint exists (e.g. no output device is installed).
    NoRenderEndpoint,
    /// The Windows audio service is not running.
    AudioServiceDown,
    /// The endpoint's mix format cannot be validated/converted, or changed
    /// mid-session.
    UnsupportedFormat,
    /// `Initialize`, `SetEventHandle`, `GetService`, or `Start` failed.
    StartFailed,
    /// `Stop` or the capture-thread join failed or timed out.
    StopFailed,
    /// `AUDCLNT_E_DEVICE_INVALIDATED` / `AUDCLNT_E_RESOURCES_INVALIDATED`: the
    /// endpoint was unplugged or reconfigured.
    DeviceInvalidated,
    /// The default render endpoint changed to a different device mid-session.
    EndpointChanged,
    /// An internal invariant was violated (an unexpected HRESULT, a broken
    /// accumulator/counter/timeline invariant, or a poisoned internal state).
    /// The session always ends safely rather than continuing on unverified
    /// state.
    Internal,
}

/// A structured error with a sanitized diagnostic detail.
///
/// `detail` is developer evidence only: an HRESULT name/value and a short
/// cause. It is never user-facing copy, never a path, and never audio
/// content. User-facing copy is Spec 09/11 work, keyed off `kind`.
#[derive(Debug, Clone)]
pub struct SystemAudioError {
    pub kind: SystemAudioErrorKind,
    pub detail: String,
}

impl SystemAudioError {
    pub fn new(kind: SystemAudioErrorKind, detail: impl Into<String>) -> Self {
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

#[cfg(windows)]
mod hresult_mapping {
    use super::{SystemAudioError, SystemAudioErrorKind};
    use windows::core::Error as WinError;

    /// Sanitized detail for a Windows `Error`/`HRESULT` observed in `context`:
    /// hex code plus the system-provided message. Never a path, never audio
    /// content.
    fn sanitized_detail(context: &str, err: &WinError) -> String {
        format!(
            "{context}: HRESULT 0x{:08X} ({})",
            err.code().0 as u32,
            err.message()
        )
    }

    /// Maps a `windows::core::Error` encountered while starting the client
    /// (`Initialize`/`SetEventHandle`/`GetService`/`Start`, or endpoint/service
    /// resolution) onto the frozen taxonomy.
    pub(crate) fn map_start_error(context: &str, err: WinError) -> SystemAudioError {
        use windows::Win32::Media::Audio::{
            AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_RESOURCES_INVALIDATED,
            AUDCLNT_E_SERVICE_NOT_RUNNING, AUDCLNT_E_UNSUPPORTED_FORMAT,
        };
        let code = err.code();
        let kind = if code == AUDCLNT_E_SERVICE_NOT_RUNNING {
            SystemAudioErrorKind::AudioServiceDown
        } else if code == AUDCLNT_E_DEVICE_INVALIDATED || code == AUDCLNT_E_RESOURCES_INVALIDATED {
            SystemAudioErrorKind::DeviceInvalidated
        } else if code == AUDCLNT_E_UNSUPPORTED_FORMAT {
            SystemAudioErrorKind::UnsupportedFormat
        } else {
            SystemAudioErrorKind::StartFailed
        };
        SystemAudioError::new(kind, sanitized_detail(context, &err))
    }

    /// Maps a `windows::core::Error` encountered while the stream is already
    /// running (`GetNextPacketSize`/`GetBuffer`/`ReleaseBuffer`) onto the
    /// frozen taxonomy.
    pub(crate) fn map_runtime_error(context: &str, err: WinError) -> SystemAudioError {
        use windows::Win32::Media::Audio::{
            AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_RESOURCES_INVALIDATED,
        };
        let code = err.code();
        let kind =
            if code == AUDCLNT_E_DEVICE_INVALIDATED || code == AUDCLNT_E_RESOURCES_INVALIDATED {
                SystemAudioErrorKind::DeviceInvalidated
            } else {
                SystemAudioErrorKind::Internal
            };
        SystemAudioError::new(kind, sanitized_detail(context, &err))
    }
}

#[cfg(windows)]
pub(crate) use hresult_mapping::{map_runtime_error, map_start_error};
