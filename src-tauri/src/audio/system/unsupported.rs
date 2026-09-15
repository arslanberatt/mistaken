//! Fallback system audio backend for unsupported platforms.

use super::SystemAudioBackend;
use crate::audio::{AudioCaptureSession, AudioError, AudioErrorKind, AudioSource, PcmBlockSink};

pub struct UnsupportedSystemAudioBackend;

impl UnsupportedSystemAudioBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnsupportedSystemAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemAudioBackend for UnsupportedSystemAudioBackend {
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
    fn unsupported_backend_fails_probe_and_start() {
        let backend = UnsupportedSystemAudioBackend::new();
        let err = backend.probe().unwrap_err();
        assert_eq!(err.kind, AudioErrorKind::Unavailable);
        assert_eq!(err.source, AudioSource::System);
    }
}
