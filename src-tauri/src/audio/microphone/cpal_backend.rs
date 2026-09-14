//! The production `MicrophoneBackend`: real CPAL device resolution, format
//! negotiation, and a real-time-safe capture stream.

use std::num::{NonZeroU16, NonZeroU32};
use std::str::FromStr;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, StreamConfig};

use crate::audio::buffer::{block_capacity_for_rate, build_pool};
use crate::audio::{
    AudioCaptureSession, AudioError, AudioErrorKind, AudioSource, PcmBlock, PcmBlockSink, PcmFormat,
};

use super::device::list_devices;
use super::format::downmix_frame;
use super::{MicrophoneBackend, MicrophoneCaptureHandle, NativeMicrophoneDevice, StreamFault};

#[cfg(target_os = "macos")]
use super::permission::{authorization_status, request_access, MicrophoneAuthorization};

/// Accepted negotiated sample-rate bounds (inclusive).
const MIN_SAMPLE_RATE_HZ: u32 = 8_000;
const MAX_SAMPLE_RATE_HZ: u32 = 192_000;
/// Accepted negotiated channel-count bounds (inclusive).
const MIN_CHANNELS: u16 = 1;
const MAX_CHANNELS: u16 = 8;

fn fail(kind: AudioErrorKind) -> AudioError {
    AudioError {
        source: AudioSource::Microphone,
        kind,
    }
}

fn map_cpal_error(error: &cpal::Error) -> AudioError {
    match error.kind() {
        cpal::ErrorKind::DeviceNotAvailable => fail(AudioErrorKind::DeviceDisconnected),
        cpal::ErrorKind::PermissionDenied => fail(AudioErrorKind::PermissionDenied),
        _ => fail(AudioErrorKind::StartFailed),
    }
}

#[derive(Default)]
pub struct CpalMicrophoneBackend;

impl MicrophoneBackend for CpalMicrophoneBackend {
    fn list_devices(&self) -> Result<Vec<NativeMicrophoneDevice>, AudioError> {
        list_devices()
    }

    fn start(&self, device_id: Option<&str>) -> Result<MicrophoneCaptureHandle, AudioError> {
        check_permission()?;

        let host = cpal::default_host();
        let device = resolve_device(&host, device_id)?;

        let supported = device
            .default_input_config()
            .map_err(|e| map_cpal_error(&e))?;
        let sample_rate = supported.sample_rate();
        let channels = supported.channels();
        if !(MIN_SAMPLE_RATE_HZ..=MAX_SAMPLE_RATE_HZ).contains(&sample_rate)
            || !(MIN_CHANNELS..=MAX_CHANNELS).contains(&channels)
        {
            return Err(fail(AudioErrorKind::UnsupportedFormat));
        }

        let format = PcmFormat {
            sample_rate_hz: NonZeroU32::new(sample_rate)
                .ok_or_else(|| fail(AudioErrorKind::UnsupportedFormat))?,
            channels: NonZeroU16::new(1).expect("1 is non-zero"),
        };
        let block_capacity = block_capacity_for_rate(sample_rate);
        let (producer, consumer) = build_pool(block_capacity);

        let fault = Arc::new(StreamFault::default());
        let error_fault = fault.clone();
        let error_callback = move |_err: cpal::Error| {
            // Real-time-adjacent but not the audio callback itself: CPAL
            // calls this from its own backend thread, never the render
            // thread that must stay allocation-free. Storing a flag is the
            // full extent of what happens here; the monitor thread does the
            // actual teardown and reporting.
            error_fault.set();
        };

        let stream_config = StreamConfig {
            channels,
            sample_rate,
            buffer_size: BufferSize::Default,
        };

        let stream = build_stream(
            &device,
            supported.sample_format(),
            stream_config,
            channels as usize,
            format,
            block_capacity,
            Box::new(producer),
            error_callback,
        )?;

        stream
            .play()
            .map_err(|_| fail(AudioErrorKind::StartFailed))?;

        Ok(MicrophoneCaptureHandle {
            session: Box::new(CpalCaptureSession {
                stream: Some(stream),
            }),
            consumer,
            fault,
            format,
        })
    }
}

#[cfg(target_os = "macos")]
fn check_permission() -> Result<(), AudioError> {
    match authorization_status() {
        Some(MicrophoneAuthorization::Authorized) => Ok(()),
        Some(MicrophoneAuthorization::Denied) | Some(MicrophoneAuthorization::Restricted) => {
            Err(fail(AudioErrorKind::PermissionDenied))
        }
        None => match request_access() {
            MicrophoneAuthorization::Authorized => Ok(()),
            MicrophoneAuthorization::Denied | MicrophoneAuthorization::Restricted => {
                Err(fail(AudioErrorKind::PermissionDenied))
            }
        },
    }
}

#[cfg(not(target_os = "macos"))]
fn check_permission() -> Result<(), AudioError> {
    // Windows desktop apps have no per-application microphone consent API;
    // access is governed only by the OS-wide "Let desktop apps access your
    // microphone" privacy toggle, which CPAL/WASAPI itself enforces at
    // stream-build time. See `map_cpal_error` for how a denial surfaces.
    Ok(())
}

fn resolve_device(host: &cpal::Host, device_id: Option<&str>) -> Result<cpal::Device, AudioError> {
    match device_id {
        Some(id) => {
            let parsed = cpal::DeviceId::from_str(id)
                .map_err(|_| fail(AudioErrorKind::DeviceDisconnected))?;
            host.device_by_id(&parsed)
                .ok_or_else(|| fail(AudioErrorKind::DeviceDisconnected))
        }
        None => host
            .default_input_device()
            .ok_or_else(|| fail(AudioErrorKind::Unavailable)),
    }
}

struct CpalCaptureSession {
    stream: Option<cpal::Stream>,
}

impl AudioCaptureSession for CpalCaptureSession {
    fn source(&self) -> AudioSource {
        AudioSource::Microphone
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        // Dropping the stream pauses and releases the native device/thread.
        // This is idempotent: a second call finds `None` and is a no-op.
        self.stream.take();
        Ok(())
    }
}

/// Builds and returns a playing-but-not-yet-started input stream for the
/// concrete sample type matching `sample_format`. Every branch shares the
/// same generic real-time callback body: acquire a recycled buffer, downmix
/// each incoming frame into it with no allocation, and submit completed
/// blocks non-blockingly.
#[allow(clippy::too_many_arguments)]
fn build_stream<E>(
    device: &cpal::Device,
    sample_format: SampleFormat,
    config: StreamConfig,
    channels: usize,
    format: PcmFormat,
    block_capacity: usize,
    sink: Box<dyn PcmBlockSink>,
    error_callback: E,
) -> Result<cpal::Stream, AudioError>
where
    E: FnMut(cpal::Error) + Send + 'static,
{
    macro_rules! build_for {
        ($sample_ty:ty) => {{
            let mut sink = sink;
            let mut current: Option<Box<[f32]>> = None;
            let mut cursor = 0usize;
            let mut discard_current = false;

            device
                .build_input_stream::<$sample_ty, _, _>(
                    config,
                    move |data: &[$sample_ty], _info| {
                        for frame in data.chunks_exact(channels) {
                            if current.is_none() {
                                current = sink.try_acquire();
                                cursor = 0;
                                discard_current = false;
                            }
                            let Some(buffer) = current.as_deref_mut() else {
                                // No free buffer: the newest frame is
                                // dropped and the pool's own overflow
                                // counter already recorded it.
                                continue;
                            };

                            if !discard_current {
                                match downmix_frame::<$sample_ty>(frame) {
                                    Some(sample) => {
                                        buffer[cursor] = sample;
                                        cursor += 1;
                                    }
                                    None => {
                                        // A non-finite conversion voids the
                                        // whole in-progress block rather
                                        // than writing bad data into it.
                                        discard_current = true;
                                    }
                                }
                            }

                            if cursor == block_capacity {
                                let filled = current.take().expect("block just reached capacity");
                                if discard_current {
                                    // Recycle the buffer directly back into
                                    // circulation by treating it as a fresh
                                    // acquisition target on the next frame.
                                    current = Some(filled);
                                    cursor = 0;
                                    discard_current = false;
                                } else {
                                    let block = PcmBlock {
                                        source: AudioSource::Microphone,
                                        sequence: 0,
                                        format,
                                        valid_samples: block_capacity,
                                        samples: filled,
                                    };
                                    match sink.try_submit(block) {
                                        Ok(()) => {}
                                        Err(returned) => {
                                            // Filled queue is full: drop this
                                            // block's content (already
                                            // counted as overflow inside the
                                            // sink) but keep its buffer in
                                            // circulation instead of leaking
                                            // it, so the pool never shrinks.
                                            current = Some(returned.samples);
                                            cursor = 0;
                                        }
                                    }
                                }
                            }
                        }
                    },
                    error_callback,
                    None,
                )
                .map_err(|_| fail(AudioErrorKind::StartFailed))
        }};
    }

    match sample_format {
        SampleFormat::I8 => build_for!(i8),
        SampleFormat::I16 => build_for!(i16),
        SampleFormat::I24 => build_for!(cpal::I24),
        SampleFormat::I32 => build_for!(i32),
        SampleFormat::I64 => build_for!(i64),
        SampleFormat::U8 => build_for!(u8),
        SampleFormat::U16 => build_for!(u16),
        SampleFormat::U24 => build_for!(cpal::U24),
        SampleFormat::U32 => build_for!(u32),
        SampleFormat::U64 => build_for!(u64),
        SampleFormat::F32 => build_for!(f32),
        SampleFormat::F64 => build_for!(f64),
        _ => Err(fail(AudioErrorKind::UnsupportedFormat)),
    }
}
