//! Bounded sink bridge connecting platform system-audio crates to Mistaken's
//! [`PcmBlockSink`].
//!
//! Converts delivered mono float samples and sample rate into a [`PcmBlock`]
//! with `source = AudioSource::System`, acquiring reusable buffers from the
//! bounded pool and reporting overflows.

use std::num::{NonZeroU16, NonZeroU32};

use crate::audio::{AudioError, AudioSource, PcmBlock, PcmBlockSink, PcmFormat};

/// A sink bridge holding the underlying [`PcmBlockSink`] and error callback.
pub struct SystemAudioSinkBridge {
    sink: Box<dyn PcmBlockSink>,
    on_error: Box<dyn FnMut(AudioError) + Send>,
    sequence: u64,
}

impl SystemAudioSinkBridge {
    pub fn new(sink: Box<dyn PcmBlockSink>, on_error: Box<dyn FnMut(AudioError) + Send>) -> Self {
        Self {
            sink,
            on_error,
            sequence: 0,
        }
    }

    /// Handles a delivered mono block of float samples and format.
    /// Returns `false` on pool acquisition or submit failure (counted as dropped).
    pub fn handle_block(&mut self, samples: &[f32], sample_rate_hz: u32, channels: u16) -> bool {
        let Some(mut buffer) = self.sink.try_acquire() else {
            return false;
        };
        let valid = samples.len().min(buffer.len());
        buffer[..valid].copy_from_slice(&samples[..valid]);

        let sequence = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);

        let block = PcmBlock {
            source: AudioSource::System,
            sequence,
            format: PcmFormat {
                sample_rate_hz: NonZeroU32::new(sample_rate_hz)
                    .unwrap_or_else(|| NonZeroU32::new(48000).unwrap()),
                channels: NonZeroU16::new(channels).unwrap_or_else(|| NonZeroU16::new(1).unwrap()),
            },
            valid_samples: valid,
            samples: buffer,
        };

        self.sink.try_submit(block).is_ok()
    }

    /// Delivers an audio error.
    pub fn handle_error(&mut self, error: AudioError) {
        (self.on_error)(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::buffer::build_pool;
    use crate::audio::AudioErrorKind;
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn sink_bridge_delivers_system_pcm_blocks() {
        let (producer, mut consumer) = build_pool(960);
        let error_called = Arc::new(AtomicBool::new(false));
        let error_called_clone = error_called.clone();
        let mut bridge = SystemAudioSinkBridge::new(
            Box::new(producer),
            Box::new(move |_| {
                error_called_clone.store(true, Ordering::SeqCst);
            }),
        );

        let input_samples = vec![0.25_f32; 960];
        let accepted = bridge.handle_block(&input_samples, 48000, 1);
        assert!(accepted);

        let block = consumer.try_recv().expect("block should be received");
        assert_eq!(block.source, AudioSource::System);
        assert_eq!(block.format.sample_rate_hz.get(), 48000);
        assert_eq!(block.format.channels.get(), 1);
        assert_eq!(block.valid_samples, 960);
        assert!((block.samples[0] - 0.25).abs() < f32::EPSILON);
        assert_eq!(block.sequence, 0);
        assert!(!error_called.load(Ordering::SeqCst));
    }

    #[test]
    fn sink_bridge_routes_audio_error() {
        let (producer, _) = build_pool(960);
        let error_kind = Arc::new(Mutex::new(None));
        let error_kind_clone = error_kind.clone();
        let mut bridge = SystemAudioSinkBridge::new(
            Box::new(producer),
            Box::new(move |err| {
                *error_kind_clone.lock() = Some(err.kind);
            }),
        );

        bridge.handle_error(AudioError {
            source: AudioSource::System,
            kind: AudioErrorKind::StartFailed,
        });
        assert_eq!(*error_kind.lock(), Some(AudioErrorKind::StartFailed));
    }
}
