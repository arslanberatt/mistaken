//! macOS ScreenCaptureKit system-audio capture backend.
//!
//! Bridges `crates/macos-system-audio` into Mistaken's native audio pipeline
//! using the frozen mapping table from Spec 07 / `platform-notes-macos.md`.

#![cfg(target_os = "macos")]

use macos_system_audio::{
    permission_status, start as macos_start, ScreenRecordingPermission, SystemAudioConfig,
    SystemAudioError, SystemAudioErrorKind, SystemAudioFormat, SystemAudioSession,
    SystemAudioSink as MacOsSystemAudioSink,
};

use super::sink::SystemAudioSinkBridge;
use super::SystemAudioBackend;
use crate::audio::{AudioCaptureSession, AudioError, AudioErrorKind, AudioSource, PcmBlockSink};
use crate::state::runtime::{RuntimeError, RuntimeErrorCode, TranscriptSource};

/// Maps a native [`SystemAudioError`] to Mistaken's internal [`AudioError`].
pub fn map_macos_error(error: SystemAudioError) -> AudioError {
    let kind = match error.kind {
        SystemAudioErrorKind::PermissionDenied
        | SystemAudioErrorKind::PermissionRequiresRestart => AudioErrorKind::PermissionDenied,
        SystemAudioErrorKind::Unsupported => AudioErrorKind::Unavailable,
        SystemAudioErrorKind::NoCaptureContent => AudioErrorKind::Unavailable,
        SystemAudioErrorKind::UnsupportedFormat => AudioErrorKind::UnsupportedFormat,
        SystemAudioErrorKind::StartFailed => AudioErrorKind::StartFailed,
        SystemAudioErrorKind::StopFailed => AudioErrorKind::StopFailed,
        SystemAudioErrorKind::StreamStopped => AudioErrorKind::Unavailable,
        SystemAudioErrorKind::Internal => AudioErrorKind::Internal,
    };
    AudioError {
        source: AudioSource::System,
        kind,
    }
}

/// Maps a native [`SystemAudioError`] to the user-facing serializable [`RuntimeError`].
pub fn map_macos_runtime_error(error: &SystemAudioError) -> RuntimeError {
    let (code, message, recoverable) = match error.kind {
        SystemAudioErrorKind::PermissionDenied => (
            RuntimeErrorCode::SystemAudioPermissionDenied,
            "macOS grants system audio through Screen Recording. Enable Mistaken in System Settings → Privacy & Security → Screen Recording.",
            true,
        ),
        SystemAudioErrorKind::PermissionRequiresRestart => (
            RuntimeErrorCode::SystemAudioPermissionDenied,
            "macOS grants system audio through Screen Recording. Enable Mistaken in System Settings → Privacy & Security → Screen Recording. Then relaunch Mistaken.",
            true,
        ),
        SystemAudioErrorKind::Unsupported => (
            RuntimeErrorCode::UnsupportedPlatform,
            "system audio requires macOS 13.0 or newer",
            false,
        ),
        SystemAudioErrorKind::NoCaptureContent => (
            RuntimeErrorCode::SystemAudioUnavailable,
            "system audio is unavailable: no capture display found",
            true,
        ),
        SystemAudioErrorKind::UnsupportedFormat => (
            RuntimeErrorCode::CaptureStartFailed,
            "system audio format is not supported",
            false,
        ),
        SystemAudioErrorKind::StartFailed => (
            RuntimeErrorCode::CaptureStartFailed,
            "system audio capture failed to start",
            true,
        ),
        SystemAudioErrorKind::StopFailed => (
            RuntimeErrorCode::CaptureStopFailed,
            "system audio capture failed to stop cleanly",
            false,
        ),
        SystemAudioErrorKind::StreamStopped => (
            RuntimeErrorCode::SystemAudioUnavailable,
            "system audio stream stopped unexpectedly",
            true,
        ),
        SystemAudioErrorKind::Internal => (
            RuntimeErrorCode::Internal,
            "an internal system audio error occurred",
            false,
        ),
    };
    RuntimeError::new(code, message, recoverable).with_source(TranscriptSource::System)
}

struct MacOsSinkAdapter {
    bridge: SystemAudioSinkBridge,
}

impl MacOsSystemAudioSink for MacOsSinkAdapter {
    fn on_block(&mut self, samples: &[f32], format: SystemAudioFormat) -> bool {
        self.bridge
            .handle_block(samples, format.sample_rate_hz, format.channels)
    }

    fn on_error(&mut self, error: SystemAudioError) {
        self.bridge.handle_error(map_macos_error(error));
    }
}

pub struct MacOsCaptureSession {
    session: SystemAudioSession,
}

impl AudioCaptureSession for MacOsCaptureSession {
    fn source(&self) -> AudioSource {
        AudioSource::System
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.session.stop().map_err(map_macos_error)
    }
}

pub struct MacOsSystemAudioBackend;

impl MacOsSystemAudioBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOsSystemAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemAudioBackend for MacOsSystemAudioBackend {
    fn probe(&self) -> Result<(), AudioError> {
        let info = objc2_foundation::NSProcessInfo::processInfo();
        let min_version = objc2_foundation::NSOperatingSystemVersion {
            majorVersion: 13,
            minorVersion: 0,
            patchVersion: 0,
        };
        if !info.isOperatingSystemAtLeastVersion(min_version) {
            return Err(AudioError {
                source: AudioSource::System,
                kind: AudioErrorKind::Unavailable,
            });
        }

        match permission_status() {
            ScreenRecordingPermission::Granted | ScreenRecordingPermission::Undetermined => Ok(()),
            ScreenRecordingPermission::Denied => Err(AudioError {
                source: AudioSource::System,
                kind: AudioErrorKind::PermissionDenied,
            }),
        }
    }

    fn start(
        &self,
        sink: Box<dyn PcmBlockSink>,
        on_error: Box<dyn FnMut(AudioError) + Send>,
    ) -> Result<Box<dyn AudioCaptureSession>, AudioError> {
        let bridge = SystemAudioSinkBridge::new(sink, on_error);
        let adapter = Box::new(MacOsSinkAdapter { bridge });
        let config = SystemAudioConfig::default();
        let session = macos_start(config, adapter).map_err(map_macos_error)?;
        Ok(Box::new(MacOsCaptureSession { session }))
    }

    fn sample_rate(&self) -> u32 {
        48_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_error_mapping_covers_all_kinds_exhaustively() {
        let cases = [
            (
                SystemAudioErrorKind::PermissionDenied,
                AudioErrorKind::PermissionDenied,
                RuntimeErrorCode::SystemAudioPermissionDenied,
                true,
            ),
            (
                SystemAudioErrorKind::PermissionRequiresRestart,
                AudioErrorKind::PermissionDenied,
                RuntimeErrorCode::SystemAudioPermissionDenied,
                true,
            ),
            (
                SystemAudioErrorKind::Unsupported,
                AudioErrorKind::Unavailable,
                RuntimeErrorCode::UnsupportedPlatform,
                false,
            ),
            (
                SystemAudioErrorKind::NoCaptureContent,
                AudioErrorKind::Unavailable,
                RuntimeErrorCode::SystemAudioUnavailable,
                true,
            ),
            (
                SystemAudioErrorKind::UnsupportedFormat,
                AudioErrorKind::UnsupportedFormat,
                RuntimeErrorCode::CaptureStartFailed,
                false,
            ),
            (
                SystemAudioErrorKind::StartFailed,
                AudioErrorKind::StartFailed,
                RuntimeErrorCode::CaptureStartFailed,
                true,
            ),
            (
                SystemAudioErrorKind::StopFailed,
                AudioErrorKind::StopFailed,
                RuntimeErrorCode::CaptureStopFailed,
                false,
            ),
            (
                SystemAudioErrorKind::StreamStopped,
                AudioErrorKind::Unavailable,
                RuntimeErrorCode::SystemAudioUnavailable,
                true,
            ),
            (
                SystemAudioErrorKind::Internal,
                AudioErrorKind::Internal,
                RuntimeErrorCode::Internal,
                false,
            ),
        ];

        for (kind, expected_audio_kind, expected_runtime_code, expected_recoverable) in cases {
            let sys_err = SystemAudioError {
                kind,
                detail: "test diagnostic".into(),
            };
            let audio_err = map_macos_error(sys_err.clone());
            assert_eq!(audio_err.source, AudioSource::System);
            assert_eq!(audio_err.kind, expected_audio_kind);

            let runtime_err = map_macos_runtime_error(&sys_err);
            assert_eq!(runtime_err.code, expected_runtime_code);
            assert_eq!(runtime_err.recoverable, expected_recoverable);
            assert_eq!(runtime_err.source, Some(TranscriptSource::System));
        }
    }

    #[test]
    fn macos_backend_probe_on_current_host() {
        let backend = MacOsSystemAudioBackend::new();
        let result = backend.probe();
        assert!(result.is_ok(), "probe should succeed on supported macOS");
        assert_eq!(backend.sample_rate(), 48_000);
    }
}
