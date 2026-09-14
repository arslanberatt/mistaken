//! `SCStreamOutput`/`SCStreamDelegate` Objective-C class.
//!
//! Everything reachable from [`stream_didOutputSampleBuffer_ofType`] runs on
//! the adapter's own serial dispatch queue and must stay allocation-free
//! after [`crate::start`] returns: the two buffers in [`DeliveryState`] are
//! preallocated once when the session is built, and every subsequent
//! callback only writes into them.

use core::ffi::c_void;
use core::mem::{align_of, size_of};
use core::ptr::{self, NonNull};
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol};
use objc2::{define_class, AllocAnyThread, DefinedClass};
use objc2_core_audio_types::{AudioBuffer, AudioBufferList};
use objc2_core_media::CMSampleBuffer;
use objc2_foundation::NSError;
use objc2_screen_capture_kit::{SCStream, SCStreamDelegate, SCStreamOutput, SCStreamOutputType};

use crate::blocks::{feed_frames, BlockAssembler, FrameLayout, MAX_CHANNELS};
use crate::error::{SystemAudioError, SystemAudioErrorKind};
use crate::format::validate_format_description;
use crate::{SystemAudioFormat, SystemAudioSink};

/// One second of frames at the highest sample rate this adapter accepts
/// (48 kHz). Bounds every batch this adapter copies out of a sample buffer
/// in one pass, regardless of layout.
const PER_CHANNEL_CAPACITY_FRAMES: usize = 48_000;

/// Preallocated raw storage for an `AudioBufferList` with up to
/// [`MAX_CHANNELS`] buffers. `AudioBufferList` is a C "flexible array
/// member" type (`mBuffers: [AudioBuffer; 1]`); additional buffers are laid
/// out immediately after the header in one allocation, exactly how Apple's
/// own SDK expects a multi-buffer list to be allocated.
struct AudioBufferListStorage {
    bytes: Box<[u8]>,
}

impl AudioBufferListStorage {
    fn new() -> Self {
        let header_size = size_of::<AudioBufferList>();
        let extra = (MAX_CHANNELS - 1) * size_of::<AudioBuffer>();
        let bytes = vec![0u8; header_size + extra].into_boxed_slice();
        Self { bytes }
    }

    /// Populates the list for one delivery batch. `buffers` is
    /// `(channel_count, byte_len, data_ptr)` per `AudioBuffer` entry.
    fn populate(&mut self, buffers: &[(u32, u32, *mut c_void)]) -> NonNull<AudioBufferList> {
        debug_assert!(!buffers.is_empty() && buffers.len() <= MAX_CHANNELS);
        let list_ptr = self.bytes.as_mut_ptr() as *mut AudioBufferList;
        // SAFETY: `bytes` was allocated with room for `MAX_CHANNELS`
        // `AudioBuffer` entries laid out immediately after the
        // `AudioBufferList` header, and `buffers.len() <= MAX_CHANNELS`.
        unsafe {
            (*list_ptr).mNumberBuffers = buffers.len() as u32;
            let entries_ptr = ptr::addr_of_mut!((*list_ptr).mBuffers) as *mut AudioBuffer;
            for (i, &(channels, byte_len, data)) in buffers.iter().enumerate() {
                entries_ptr.add(i).write(AudioBuffer {
                    mNumberChannels: channels,
                    mDataByteSize: byte_len,
                    mData: data,
                });
            }
        }
        // SAFETY: `bytes` is a non-null heap allocation.
        unsafe { NonNull::new_unchecked(list_ptr) }
    }
}

/// Mutable state touched only from the adapter's serial delivery queue,
/// except for the brief moment `stream_didStopWithError` records into it
/// after the queue has already stopped delivering (see `stop()`).
struct DeliveryState {
    sink: Box<dyn SystemAudioSink>,
    /// The first validated sample rate seen this session. A later buffer
    /// with a different rate ends the session rather than silently mixing
    /// rates, because the downstream recognizer stream is fixed to one
    /// rate for its life.
    session_rate_hz: Option<u32>,
    session_channels: Option<u16>,
    assembler: Option<BlockAssembler>,
    /// One preallocated raw sample buffer, `MAX_CHANNELS *
    /// PER_CHANNEL_CAPACITY_FRAMES` samples, reused for every batch: for
    /// non-interleaved audio, channel `c`'s frames live at
    /// `[c * PER_CHANNEL_CAPACITY_FRAMES, c * PER_CHANNEL_CAPACITY_FRAMES +
    /// batch_frames)`; for interleaved audio the first `batch_frames *
    /// channels` samples hold one interleaved buffer.
    scratch: Box<[f32]>,
    abl_storage: AudioBufferListStorage,
}

impl DeliveryState {
    fn new(sink: Box<dyn SystemAudioSink>) -> Self {
        Self {
            sink,
            session_rate_hz: None,
            session_channels: None,
            assembler: None,
            scratch: vec![0.0_f32; MAX_CHANNELS * PER_CHANNEL_CAPACITY_FRAMES].into_boxed_slice(),
            abl_storage: AudioBufferListStorage::new(),
        }
    }
}

pub(crate) struct OutputIvars {
    delivery: Mutex<DeliveryState>,
    dropped_blocks: AtomicU64,
    non_audio_deliveries: AtomicU64,
    format_rejections: AtomicU64,
    stream_stopped_error: Mutex<Option<SystemAudioError>>,
}

define_class!(
    // SAFETY: `StreamOutputHandler` subclasses `NSObject` directly, adds no
    // `Drop` requiring specialized deallocation, and its ivars type has no
    // additional safety invariants beyond what `Mutex`/`AtomicU64` already
    // enforce.
    #[unsafe(super(NSObject))]
    #[name = "MistakenSystemAudioStreamOutput"]
    #[ivars = OutputIvars]
    pub(crate) struct StreamOutputHandler;

    // SAFETY: `StreamOutputHandler` implements no methods `NSObjectProtocol`
    // requires beyond the defaults `NSObject` already provides.
    unsafe impl NSObjectProtocol for StreamOutputHandler {}

    // SAFETY: the overridden method below has the exact selector and
    // argument types `SCStreamOutput` declares, and never blocks, retries,
    // or grows memory, satisfying the protocol's real-time delivery
    // contract.
    unsafe impl SCStreamOutput for StreamOutputHandler {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        #[allow(non_snake_case)]
        fn stream_didOutputSampleBuffer_ofType(
            &self,
            _stream: &SCStream,
            sample_buffer: &CMSampleBuffer,
            kind: SCStreamOutputType,
        ) {
            if kind != SCStreamOutputType::Audio {
                // Non-audio deliveries are counted and ignored. This adapter
                // registers only an audio stream output, but the delegate
                // callback signature is shared with video/microphone types.
                self.ivars()
                    .non_audio_deliveries
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            self.handle_audio_sample_buffer(sample_buffer);
        }
    }

    // SAFETY: the overridden method below has the exact selector and
    // argument types `SCStreamDelegate` declares.
    unsafe impl SCStreamDelegate for StreamOutputHandler {
        #[unsafe(method(stream:didStopWithError:))]
        #[allow(non_snake_case)]
        fn stream_didStopWithError(&self, _stream: &SCStream, error: &NSError) {
            let detail = error.localizedDescription().to_string();
            let system_error = SystemAudioError::new(SystemAudioErrorKind::StreamStopped, detail);
            *self.ivars().stream_stopped_error.lock() = Some(system_error.clone());
            self.ivars().delivery.lock().sink.on_error(system_error);
        }
    }
);

impl StreamOutputHandler {
    pub(crate) fn new(sink: Box<dyn SystemAudioSink>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(OutputIvars {
            delivery: Mutex::new(DeliveryState::new(sink)),
            dropped_blocks: AtomicU64::new(0),
            non_audio_deliveries: AtomicU64::new(0),
            format_rejections: AtomicU64::new(0),
            stream_stopped_error: Mutex::new(None),
        });
        // SAFETY: `this` came from `Self::alloc()` immediately above and
        // has not been initialized yet; `[NSObject init]` is the standard
        // designated initializer for a fresh allocation.
        unsafe { objc2::msg_send![super(this), init] }
    }

    pub(crate) fn dropped_blocks(&self) -> u64 {
        self.ivars().dropped_blocks.load(Ordering::Relaxed)
    }

    pub(crate) fn non_audio_deliveries(&self) -> u64 {
        self.ivars().non_audio_deliveries.load(Ordering::Relaxed)
    }

    pub(crate) fn format_rejections(&self) -> u64 {
        self.ivars().format_rejections.load(Ordering::Relaxed)
    }

    pub(crate) fn take_stream_stopped_error(&self) -> Option<SystemAudioError> {
        self.ivars().stream_stopped_error.lock().take()
    }

    /// Discards any partially filled block. Called from `stop()` after the
    /// stream output has been removed, so no callback races this call.
    pub(crate) fn discard_partial_block(&self) {
        let mut delivery = self.ivars().delivery.lock();
        if let Some(assembler) = delivery.assembler.as_mut() {
            assembler.discard_partial();
        }
    }

    fn handle_audio_sample_buffer(&self, sample_buffer: &CMSampleBuffer) {
        let mut delivery_guard = self.ivars().delivery.lock();

        // SAFETY: `sample_buffer` is the live `&CMSampleBuffer` argument
        // delivered by `SCStreamOutput`; it stays valid for this call.
        let Some(format_description) = (unsafe { sample_buffer.format_description() }) else {
            self.ivars()
                .format_rejections
                .fetch_add(1, Ordering::Relaxed);
            return;
        };
        let validated = match validate_format_description(&format_description) {
            Ok(v) => v,
            Err(e) => {
                self.ivars()
                    .format_rejections
                    .fetch_add(1, Ordering::Relaxed);
                delivery_guard.sink.on_error(e);
                return;
            }
        };

        let delivery = &mut *delivery_guard;

        match delivery.session_rate_hz {
            None => {
                delivery.session_rate_hz = Some(validated.sample_rate_hz);
                delivery.session_channels = Some(validated.channels);
                let assembler = BlockAssembler::new(validated.sample_rate_hz as usize / 50);
                debug_assert_eq!(
                    assembler.block_len(),
                    validated.sample_rate_hz as usize / 50,
                    "block length must be exactly 20 ms of mono samples"
                );
                delivery.assembler = Some(assembler);
            }
            Some(rate) if rate != validated.sample_rate_hz => {
                delivery.sink.on_error(SystemAudioError::new(
                    SystemAudioErrorKind::UnsupportedFormat,
                    format!(
                        "sample rate changed mid-session from {rate} to {}",
                        validated.sample_rate_hz
                    ),
                ));
                return;
            }
            Some(_) => {}
        }
        if delivery.session_channels != Some(validated.channels) {
            delivery.sink.on_error(SystemAudioError::new(
                SystemAudioErrorKind::UnsupportedFormat,
                "channel count changed mid-session".to_string(),
            ));
            return;
        }

        let channels = validated.channels as usize;
        // SAFETY: `sample_buffer` is still the same live argument from
        // the callback; `num_samples` is a read-only accessor.
        let total_frames = unsafe { sample_buffer.num_samples() } as usize;
        if total_frames == 0 {
            return;
        }

        let format = SystemAudioFormat {
            sample_rate_hz: validated.sample_rate_hz,
            channels: 1,
        };

        let mut offset = 0usize;
        while offset < total_frames {
            let batch_frames = (total_frames - offset).min(PER_CHANNEL_CAPACITY_FRAMES);

            let list_ptr = if validated.non_interleaved {
                let mut entries = [(0u32, 0u32, ptr::null_mut::<c_void>()); MAX_CHANNELS];
                let scratch_ptr = delivery.scratch.as_mut_ptr();
                for (c, entry) in entries.iter_mut().enumerate().take(channels) {
                    // SAFETY: `c * PER_CHANNEL_CAPACITY_FRAMES + batch_frames`
                    // stays within `scratch`'s `MAX_CHANNELS *
                    // PER_CHANNEL_CAPACITY_FRAMES` length because `channels
                    // <= MAX_CHANNELS` and `batch_frames <=
                    // PER_CHANNEL_CAPACITY_FRAMES`.
                    let base = unsafe { scratch_ptr.add(c * PER_CHANNEL_CAPACITY_FRAMES) };
                    *entry = (
                        1,
                        (batch_frames * size_of::<f32>()) as u32,
                        base as *mut c_void,
                    );
                }
                delivery.abl_storage.populate(&entries[..channels])
            } else {
                let base = delivery.scratch.as_mut_ptr();
                let entries = [(
                    channels as u32,
                    (batch_frames * channels * size_of::<f32>()) as u32,
                    base as *mut c_void,
                )];
                delivery.abl_storage.populate(&entries)
            };

            // SAFETY: `list_ptr` was just populated above with buffers sized
            // for exactly `batch_frames` frames of `channels` channels,
            // matching what `copy_pcm_data_into_audio_buffer_list` requires.
            let status = unsafe {
                sample_buffer.copy_pcm_data_into_audio_buffer_list(
                    offset as i32,
                    batch_frames as i32,
                    list_ptr,
                )
            };
            if status != 0 {
                self.ivars()
                    .format_rejections
                    .fetch_add(1, Ordering::Relaxed);
                delivery.sink.on_error(SystemAudioError::new(
                    SystemAudioErrorKind::UnsupportedFormat,
                    format!(
                        "CMSampleBufferCopyPCMDataIntoAudioBufferList failed with status {status}"
                    ),
                ));
                return;
            }

            let dropped = &self.ivars().dropped_blocks;
            let sink = &mut delivery.sink;
            let assembler = delivery
                .assembler
                .as_mut()
                .expect("assembler initialized above");
            let mut emit = |block: &[f32]| {
                if !sink.on_block(block, format) {
                    dropped.fetch_add(1, Ordering::Relaxed);
                }
            };

            if validated.non_interleaved {
                let mut chan_slices: [&[f32]; MAX_CHANNELS] = [&[]; MAX_CHANNELS];
                for (c, slice) in chan_slices.iter_mut().enumerate().take(channels) {
                    let start = c * PER_CHANNEL_CAPACITY_FRAMES;
                    *slice = &delivery.scratch[start..start + batch_frames];
                }
                feed_frames(
                    FrameLayout::NonInterleaved(&chan_slices[..channels]),
                    batch_frames,
                    channels,
                    assembler,
                    &mut emit,
                );
            } else {
                let data = &delivery.scratch[..batch_frames * channels];
                feed_frames(
                    FrameLayout::Interleaved(data),
                    batch_frames,
                    channels,
                    assembler,
                    &mut emit,
                );
            }

            offset += batch_frames;
        }
    }
}

const _: () = {
    // Compile-time reminder that `AudioBufferListStorage` relies on
    // `AudioBufferList`'s in-memory layout matching Apple's C struct.
    assert!(align_of::<AudioBufferList>() >= align_of::<u32>());
};
