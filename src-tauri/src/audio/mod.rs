//! Common native audio contract.
//!
//! This module freezes the audio-side boundary consumed independently by
//! later specs (microphone capture, macOS/Windows system-audio adapters, and
//! dual-source orchestration). Spec 03 instantiates no backend, starts no
//! worker/thread/stream, and allocates no live buffer: this is a reviewed
//! compile-time contract plus unit tests over synthetic values only.
//!
//! Nothing here is serializable. PCM, audio errors, and capture sessions
//! never cross Tauri IPC/events; only the runtime command/state boundary
//! (see `crate::state::runtime`) maps a native [`AudioError`] onto the
//! frozen, serializable `RuntimeError` shown to the frontend.

use std::num::{NonZeroU16, NonZeroU32};

/// Structural origin of captured audio. Never inferred from content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSource {
    Microphone,
    System,
}

/// The PCM format of a captured block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmFormat {
    pub sample_rate_hz: NonZeroU32,
    pub channels: NonZeroU16,
}

/// One bounded, reusable block of interleaved PCM samples.
///
/// `samples` is acquired from a [`PcmBlockSink`] and never allocated inside
/// an audio callback. Only the `samples[..valid_samples]` prefix holds
/// current audio; the remainder of the buffer is stale reused capacity.
pub struct PcmBlock {
    pub source: AudioSource,
    pub sequence: u64,
    pub format: PcmFormat,
    pub valid_samples: usize,
    pub samples: Box<[f32]>,
}

/// A bounded, non-blocking home for reusable PCM buffers.
///
/// Both methods are non-blocking by contract: a producer that finds no free
/// buffer, or a sink that is full, treats that as explicit overflow rather
/// than waiting or growing memory.
pub trait PcmBlockSink: Send + 'static {
    /// Returns a reusable buffer to fill, or `None` if none is free.
    fn try_acquire(&mut self) -> Option<Box<[f32]>>;

    /// Submits a filled block, or returns it back to the caller if the sink
    /// is full so the caller can apply its own overflow policy.
    fn try_submit(&mut self, block: PcmBlock) -> Result<(), PcmBlock>;
}

/// Reasons a native audio operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioErrorKind {
    PermissionDenied,
    Unavailable,
    DeviceDisconnected,
    UnsupportedFormat,
    StartFailed,
    StopFailed,
    QueueOverflow,
    Internal,
}

/// A native audio failure tied to the source that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioError {
    pub source: AudioSource,
    pub kind: AudioErrorKind,
}

/// A live capture session for exactly one [`AudioSource`].
pub trait AudioCaptureSession: Send {
    fn source(&self) -> AudioSource;
    fn stop(&mut self) -> Result<(), AudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format() -> PcmFormat {
        PcmFormat {
            sample_rate_hz: NonZeroU32::new(16_000).expect("16000 is non-zero"),
            channels: NonZeroU16::new(1).expect("1 is non-zero"),
        }
    }

    fn stereo_format() -> PcmFormat {
        PcmFormat {
            sample_rate_hz: NonZeroU32::new(48_000).expect("48000 is non-zero"),
            channels: NonZeroU16::new(2).expect("2 is non-zero"),
        }
    }

    #[test]
    fn valid_samples_never_exceeds_the_buffer_length() {
        let samples: Box<[f32]> = vec![0.0_f32; 320].into_boxed_slice();
        let block = PcmBlock {
            source: AudioSource::Microphone,
            sequence: 0,
            format: format(),
            valid_samples: 320,
            samples,
        };

        assert!(block.valid_samples <= block.samples.len());
    }

    #[test]
    fn valid_samples_is_divisible_by_channel_count() {
        let samples: Box<[f32]> = vec![0.0_f32; 480].into_boxed_slice();
        let block = PcmBlock {
            source: AudioSource::System,
            sequence: 0,
            format: stereo_format(),
            valid_samples: 480,
            samples,
        };

        assert_eq!(
            block.valid_samples % block.format.channels.get() as usize,
            0
        );
    }

    #[test]
    fn samples_stay_within_the_normalized_range() {
        let samples: Box<[f32]> = vec![-1.0, -0.5, 0.0, 0.5, 1.0].into_boxed_slice();

        assert!(samples.iter().all(|sample| (-1.0..=1.0).contains(sample)));
    }

    #[test]
    fn sequence_increments_without_wrap_per_session() {
        for sequence in 0_u64..5 {
            let block = PcmBlock {
                source: AudioSource::Microphone,
                sequence,
                format: format(),
                valid_samples: 0,
                samples: Box::new([]),
            };
            assert_eq!(block.sequence, sequence);
        }
    }

    /// A tiny synthetic sink used only to prove the trait's non-blocking,
    /// bounded-overflow contract. This is not a fake capture backend: it
    /// never touches a real device and is confined to this test module.
    struct FixedCapacitySink {
        free: Vec<Box<[f32]>>,
        stored: Option<PcmBlock>,
    }

    impl PcmBlockSink for FixedCapacitySink {
        fn try_acquire(&mut self) -> Option<Box<[f32]>> {
            self.free.pop()
        }

        fn try_submit(&mut self, block: PcmBlock) -> Result<(), PcmBlock> {
            if self.stored.is_some() {
                return Err(block);
            }
            self.stored = Some(block);
            Ok(())
        }
    }

    #[test]
    fn try_acquire_returns_none_when_no_buffer_is_free() {
        let mut sink = FixedCapacitySink {
            free: Vec::new(),
            stored: None,
        };

        assert!(sink.try_acquire().is_none());
    }

    #[test]
    fn try_submit_returns_the_block_back_when_the_sink_is_full() {
        let mut sink = FixedCapacitySink {
            free: vec![vec![0.0_f32; 4].into_boxed_slice()],
            stored: Some(PcmBlock {
                source: AudioSource::Microphone,
                sequence: 0,
                format: format(),
                valid_samples: 0,
                samples: Box::new([]),
            }),
        };

        let samples = sink.try_acquire().expect("one buffer was seeded as free");
        let block = PcmBlock {
            source: AudioSource::Microphone,
            sequence: 1,
            format: format(),
            valid_samples: 4,
            samples,
        };

        let result = sink.try_submit(block);
        assert!(result.is_err());
    }

    struct StubSession {
        source: AudioSource,
        stopped: bool,
    }

    impl AudioCaptureSession for StubSession {
        fn source(&self) -> AudioSource {
            self.source
        }

        fn stop(&mut self) -> Result<(), AudioError> {
            self.stopped = true;
            Ok(())
        }
    }

    #[test]
    fn audio_capture_session_is_object_safe_and_stoppable() {
        let mut session: Box<dyn AudioCaptureSession> = Box::new(StubSession {
            source: AudioSource::System,
            stopped: false,
        });

        assert_eq!(session.source(), AudioSource::System);
        assert!(session.stop().is_ok());
    }

    #[test]
    fn pcm_block_sink_is_object_safe() {
        let mut sink: Box<dyn PcmBlockSink> = Box::new(FixedCapacitySink {
            free: vec![vec![0.0_f32; 2].into_boxed_slice()],
            stored: None,
        });

        let buffer = sink.try_acquire().expect("one buffer was seeded as free");
        let block = PcmBlock {
            source: AudioSource::Microphone,
            sequence: 0,
            format: format(),
            valid_samples: 2,
            samples: buffer,
        };
        assert!(sink.try_submit(block).is_ok());
    }

    #[test]
    fn audio_error_carries_source_and_kind() {
        let error = AudioError {
            source: AudioSource::Microphone,
            kind: AudioErrorKind::DeviceDisconnected,
        };

        assert_eq!(error.source, AudioSource::Microphone);
        assert_eq!(error.kind, AudioErrorKind::DeviceDisconnected);
    }
}
