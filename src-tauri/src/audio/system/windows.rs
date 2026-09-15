//! Windows WASAPI loopback system-audio capture backend.
//!
//! Bridges `crates/windows-system-audio` into Mistaken's native audio pipeline
//! using the frozen mapping table from Spec 08.

#[cfg(target_os = "windows")]
use windows_system_audio::{
    availability, start as windows_start, SystemAudioConfig, SystemAudioCounters, SystemAudioError,
    SystemAudioErrorKind, SystemAudioFormat, SystemAudioSession,
    SystemAudioSink as WindowsSystemAudioSink,
};

#[cfg(target_os = "windows")]
use super::sink::SystemAudioSinkBridge;
use super::SystemAudioBackend;
use crate::audio::{AudioCaptureSession, AudioError, AudioErrorKind, AudioSource, PcmBlockSink};
use crate::state::runtime::{RuntimeError, RuntimeErrorCode, TranscriptSource};

#[cfg(target_os = "windows")]
pub use windows_system_audio::SystemAudioErrorKind as WindowsSystemAudioErrorKind;

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsSystemAudioErrorKind {
    Unsupported,
    NoRenderEndpoint,
    AudioServiceDown,
    UnsupportedFormat,
    StartFailed,
    StopFailed,
    DeviceInvalidated,
    EndpointChanged,
    Internal,
}

/// Maps a native Windows system audio error kind to Mistaken's internal [`AudioErrorKind`].
pub fn map_windows_error_kind(kind: WindowsSystemAudioErrorKind) -> AudioErrorKind {
    match kind {
        WindowsSystemAudioErrorKind::Unsupported => AudioErrorKind::Unavailable,
        WindowsSystemAudioErrorKind::NoRenderEndpoint => AudioErrorKind::Unavailable,
        WindowsSystemAudioErrorKind::AudioServiceDown => AudioErrorKind::Unavailable,
        WindowsSystemAudioErrorKind::UnsupportedFormat => AudioErrorKind::UnsupportedFormat,
        WindowsSystemAudioErrorKind::StartFailed => AudioErrorKind::StartFailed,
        WindowsSystemAudioErrorKind::StopFailed => AudioErrorKind::StopFailed,
        WindowsSystemAudioErrorKind::DeviceInvalidated => AudioErrorKind::DeviceDisconnected,
        WindowsSystemAudioErrorKind::EndpointChanged => AudioErrorKind::DeviceDisconnected,
        WindowsSystemAudioErrorKind::Internal => AudioErrorKind::Internal,
    }
}

/// Maps a native Windows system audio error kind to the user-facing serializable [`RuntimeError`].
pub fn map_windows_runtime_error_kind(kind: WindowsSystemAudioErrorKind) -> RuntimeError {
    let (code, message, recoverable) = match kind {
        WindowsSystemAudioErrorKind::Unsupported => (
            RuntimeErrorCode::UnsupportedPlatform,
            "system audio requires Windows 10 Version 1703 (build 15063) or newer",
            false,
        ),
        WindowsSystemAudioErrorKind::NoRenderEndpoint => (
            RuntimeErrorCode::SystemAudioUnavailable,
            "No audio output device is available. Connect or enable an output device.",
            true,
        ),
        WindowsSystemAudioErrorKind::AudioServiceDown => (
            RuntimeErrorCode::SystemAudioUnavailable,
            "Windows audio service is not running",
            true,
        ),
        WindowsSystemAudioErrorKind::UnsupportedFormat => (
            RuntimeErrorCode::CaptureStartFailed,
            "the audio output device mix format is not supported",
            false,
        ),
        WindowsSystemAudioErrorKind::StartFailed => (
            RuntimeErrorCode::CaptureStartFailed,
            "system audio capture failed to start",
            true,
        ),
        WindowsSystemAudioErrorKind::StopFailed => (
            RuntimeErrorCode::CaptureStopFailed,
            "system audio capture failed to stop cleanly",
            false,
        ),
        WindowsSystemAudioErrorKind::DeviceInvalidated => (
            RuntimeErrorCode::DeviceDisconnected,
            "the audio output device was unplugged or reconfigured",
            true,
        ),
        WindowsSystemAudioErrorKind::EndpointChanged => (
            RuntimeErrorCode::DeviceDisconnected,
            "the default audio output device changed mid-session",
            true,
        ),
        WindowsSystemAudioErrorKind::Internal => (
            RuntimeErrorCode::Internal,
            "an internal system audio error occurred",
            false,
        ),
    };
    RuntimeError::new(code, message, recoverable).with_source(TranscriptSource::System)
}

#[cfg(target_os = "windows")]
pub fn map_windows_error(error: SystemAudioError) -> AudioError {
    AudioError {
        source: AudioSource::System,
        kind: map_windows_error_kind(error.kind),
    }
}

#[cfg(target_os = "windows")]
pub fn map_windows_runtime_error(error: &SystemAudioError) -> RuntimeError {
    map_windows_runtime_error_kind(error.kind)
}

#[cfg(target_os = "windows")]
struct WindowsSinkAdapter {
    bridge: SystemAudioSinkBridge,
}

#[cfg(target_os = "windows")]
impl WindowsSystemAudioSink for WindowsSinkAdapter {
    fn on_block(&mut self, samples: &[f32], format: SystemAudioFormat) -> bool {
        self.bridge
            .handle_block(samples, format.sample_rate_hz, format.channels)
    }

    fn on_error(&mut self, error: SystemAudioError) {
        self.bridge.handle_error(map_windows_error(error));
    }
}

#[cfg(target_os = "windows")]
pub struct WindowsCaptureSession {
    session: SystemAudioSession,
}

#[cfg(target_os = "windows")]
impl AudioCaptureSession for WindowsCaptureSession {
    fn source(&self) -> AudioSource {
        AudioSource::System
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.session.stop().map_err(map_windows_error)
    }
}

pub struct WindowsSystemAudioBackend;

impl WindowsSystemAudioBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsSystemAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
impl SystemAudioBackend for WindowsSystemAudioBackend {
    fn probe(&self) -> Result<(), AudioError> {
        availability().map_err(map_windows_error)
    }

    fn start(
        &self,
        sink: Box<dyn PcmBlockSink>,
        on_error: Box<dyn FnMut(AudioError) + Send>,
    ) -> Result<Box<dyn AudioCaptureSession>, AudioError> {
        let bridge = SystemAudioSinkBridge::new(sink, on_error);
        let adapter = Box::new(WindowsSinkAdapter { bridge });
        let config = SystemAudioConfig::default();
        let session = windows_start(config, adapter).map_err(map_windows_error)?;
        Ok(Box::new(WindowsCaptureSession { session }))
    }

    fn sample_rate(&self) -> u32 {
        48_000
    }
}

#[cfg(not(target_os = "windows"))]
impl SystemAudioBackend for WindowsSystemAudioBackend {
    fn probe(&self) -> Result<(), AudioError> {
        Err(AudioError {
            source: AudioSource::System,
            kind: AudioErrorKind::Unavailable,
        })
    }

    fn start(
        &self,
        _sink: Box<dyn PcmBlockSink>,
        _on_error: Box<dyn FnMut(AudioError) + Send>,
    ) -> Result<Box<dyn AudioCaptureSession>, AudioError> {
        Err(AudioError {
            source: AudioSource::System,
            kind: AudioErrorKind::Unavailable,
        })
    }

    fn sample_rate(&self) -> u32 {
        48_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_error_mapping_covers_all_kinds_exhaustively() {
        let cases = [
            (
                WindowsSystemAudioErrorKind::Unsupported,
                AudioErrorKind::Unavailable,
                RuntimeErrorCode::UnsupportedPlatform,
                false,
            ),
            (
                WindowsSystemAudioErrorKind::NoRenderEndpoint,
                AudioErrorKind::Unavailable,
                RuntimeErrorCode::SystemAudioUnavailable,
                true,
            ),
            (
                WindowsSystemAudioErrorKind::AudioServiceDown,
                AudioErrorKind::Unavailable,
                RuntimeErrorCode::SystemAudioUnavailable,
                true,
            ),
            (
                WindowsSystemAudioErrorKind::UnsupportedFormat,
                AudioErrorKind::UnsupportedFormat,
                RuntimeErrorCode::CaptureStartFailed,
                false,
            ),
            (
                WindowsSystemAudioErrorKind::StartFailed,
                AudioErrorKind::StartFailed,
                RuntimeErrorCode::CaptureStartFailed,
                true,
            ),
            (
                WindowsSystemAudioErrorKind::StopFailed,
                AudioErrorKind::StopFailed,
                RuntimeErrorCode::CaptureStopFailed,
                false,
            ),
            (
                WindowsSystemAudioErrorKind::DeviceInvalidated,
                AudioErrorKind::DeviceDisconnected,
                RuntimeErrorCode::DeviceDisconnected,
                true,
            ),
            (
                WindowsSystemAudioErrorKind::EndpointChanged,
                AudioErrorKind::DeviceDisconnected,
                RuntimeErrorCode::DeviceDisconnected,
                true,
            ),
            (
                WindowsSystemAudioErrorKind::Internal,
                AudioErrorKind::Internal,
                RuntimeErrorCode::Internal,
                false,
            ),
        ];

        for (kind, expected_audio_kind, expected_runtime_code, expected_recoverable) in cases {
            let audio_kind = map_windows_error_kind(kind);
            assert_eq!(audio_kind, expected_audio_kind);

            let runtime_err = map_windows_runtime_error_kind(kind);
            assert_eq!(runtime_err.code, expected_runtime_code);
            assert_eq!(runtime_err.recoverable, expected_recoverable);
            assert_eq!(runtime_err.source, Some(TranscriptSource::System));
            // Crucial Spec 08 assertion: no Windows error ever maps to permission denied!
            assert_ne!(
                runtime_err.code,
                RuntimeErrorCode::SystemAudioPermissionDenied
            );
        }
    }
}
