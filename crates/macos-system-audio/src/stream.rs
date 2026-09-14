//! Capture session lifecycle: permission flow, stream creation, start,
//! stop, and idempotent teardown.

use std::sync::mpsc;
use std::sync::Once;
use std::time::Duration;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::AnyThread;
use objc2_foundation::NSError;
use objc2_screen_capture_kit::{SCShareableContent, SCStream, SCStreamOutputType};

use crate::config::{self, SystemAudioConfig};
use crate::error::{SystemAudioError, SystemAudioErrorKind};
use crate::output::StreamOutputHandler;
use crate::permission::{self, ScreenRecordingPermission};
use crate::SystemAudioSink;

/// Bounded wait applied to every ScreenCaptureKit completion handler this
/// adapter calls. None of `getShareableContentWithCompletionHandler`,
/// `startCaptureWithCompletionHandler`, or `stopCaptureWithCompletionHandler`
/// may leave `start`/`stop` hanging indefinitely.
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(10);

fn wait_for_completion<T: Send + 'static>(
    register: impl FnOnce(mpsc::Sender<T>),
) -> Result<T, SystemAudioError> {
    let (tx, rx) = mpsc::channel::<T>();
    register(tx);
    rx.recv_timeout(COMPLETION_TIMEOUT).map_err(|_| {
        SystemAudioError::new(
            SystemAudioErrorKind::Internal,
            "ScreenCaptureKit completion handler did not return in time",
        )
    })
}

fn nserror_detail(error: *mut NSError) -> Option<String> {
    if error.is_null() {
        return None;
    }
    // SAFETY: ScreenCaptureKit completion handlers deliver an unretained
    // (+0) `NSError*`, valid only for the duration of the handler; it is
    // read here and never stored.
    let retained = unsafe { Retained::retain(error) };
    retained.map(|e| e.localizedDescription().to_string())
}

/// `Retained<T>` is not `Send` for most Objective-C classes (they are not
/// documented thread-safe), but `getShareableContentWithCompletionHandler`
/// runs its handler on an internal ScreenCaptureKit queue, not necessarily
/// this thread. This newtype carries the raw `+1` pointer across the
/// `mpsc` channel; only one side ever touches it at a time, so no
/// concurrent access to the underlying object occurs.
struct SendableRetained<T>(*mut T);
// SAFETY: see the type-level doc above.
unsafe impl<T> Send for SendableRetained<T> {}

fn get_shareable_content() -> Result<Retained<SCShareableContent>, SystemAudioError> {
    let result = wait_for_completion::<
        Result<SendableRetained<SCShareableContent>, SystemAudioError>,
    >(|tx| {
        let block = RcBlock::new(
            move |content: *mut SCShareableContent, error: *mut NSError| {
                let outcome = if let Some(detail) = nserror_detail(error) {
                    Err(SystemAudioError::new(
                        SystemAudioErrorKind::NoCaptureContent,
                        detail,
                    ))
                // SAFETY: ScreenCaptureKit completion handlers deliver an
                // unretained (+0) `SCShareableContent*`; `Retained::retain`
                // takes the `+1` this crate then owns via `SendableRetained`.
                } else if let Some(content) = unsafe { Retained::retain(content) } {
                    Ok(SendableRetained(Retained::into_raw(content)))
                } else {
                    Err(SystemAudioError::new(
                        SystemAudioErrorKind::NoCaptureContent,
                        "no shareable content and no error",
                    ))
                };
                let _ = tx.send(outcome);
            },
        );
        // SAFETY: `block` is a valid `Fn(*mut SCShareableContent, *mut
        // NSError)` block kept alive by `RcBlock` until this call returns;
        // ScreenCaptureKit retains it for the duration of the async call.
        unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
    })?;
    match result {
        Ok(SendableRetained(ptr)) => {
            // SAFETY: `ptr` was produced by `Retained::into_raw` above and
            // still carries that `+1` retain count untouched.
            unsafe { Retained::from_raw(ptr) }.ok_or_else(|| {
                SystemAudioError::new(
                    SystemAudioErrorKind::Internal,
                    "null shareable content pointer",
                )
            })
        }
        Err(e) => Err(e),
    }
}

fn start_capture(stream: &SCStream) -> Result<(), SystemAudioError> {
    wait_for_completion::<Result<(), SystemAudioError>>(|tx| {
        let block = RcBlock::new(move |error: *mut NSError| {
            let result = match nserror_detail(error) {
                Some(detail) => Err(SystemAudioError::new(
                    SystemAudioErrorKind::StartFailed,
                    detail,
                )),
                None => Ok(()),
            };
            let _ = tx.send(result);
        });
        // SAFETY: `block` is kept alive by `RcBlock` for the duration of
        // this call; `stream` is a valid, live `SCStream`.
        unsafe { stream.startCaptureWithCompletionHandler(Some(&block)) };
    })?
}

fn stop_capture(stream: &SCStream) -> Result<(), SystemAudioError> {
    wait_for_completion::<Result<(), SystemAudioError>>(|tx| {
        let block = RcBlock::new(move |error: *mut NSError| {
            let result = match nserror_detail(error) {
                Some(detail) => Err(SystemAudioError::new(
                    SystemAudioErrorKind::StopFailed,
                    detail,
                )),
                None => Ok(()),
            };
            let _ = tx.send(result);
        });
        // SAFETY: same as `start_capture` above.
        unsafe { stream.stopCaptureWithCompletionHandler(Some(&block)) };
    })?
}

/// A live capture session for exactly one system-audio stream. Opaque; owns
/// the `SCStream`, its output/delegate handler, and its serial delivery
/// queue.
pub struct SystemAudioSession {
    stream: Retained<SCStream>,
    output: Retained<StreamOutputHandler>,
    _queue: DispatchRetained<DispatchQueue>,
    stopped: Once,
}

// SAFETY: `SCStream`, its delegate/output object, and its dispatch queue
// are all documented by Apple/objc2 as safe to release from any thread;
// nothing here is `!Send` beyond the raw Objective-C object pointers, which
// `Retained` already asserts `Send`/`Sync` for.
unsafe impl Send for SystemAudioSession {}

impl SystemAudioSession {
    /// Stops capture idempotently. Safe to call more than once; only the
    /// first call performs teardown.
    pub fn stop(&mut self) -> Result<(), SystemAudioError> {
        let mut result = Ok(());
        self.stopped.call_once(|| {
            let output_protocol = ProtocolObject::from_ref(&*self.output);
            // Removing the stream output first guarantees no further
            // `stream:didOutputSampleBuffer:ofType:` callback can be
            // in-flight once this returns, so touching `self.output`'s
            // accumulator afterward is race-free.
            // SAFETY: `output_protocol` was added on this same stream in
            // `start()`; `self.stream` is still a valid, live `SCStream`.
            let _ = unsafe {
                self.stream
                    .removeStreamOutput_type_error(output_protocol, SCStreamOutputType::Audio)
            };
            result = stop_capture(&self.stream);
            self.output.discard_partial_block();
        });
        result
    }

    /// The most recent `SCStreamDelegate` stream-stopped error observed,
    /// if any. Cleared once read. Exposed for probe/evidence use; the
    /// authoritative delivery to a consumer is the matching
    /// `SystemAudioSink::on_error` call, not this accessor.
    pub fn last_stream_stopped_error(&self) -> Option<SystemAudioError> {
        self.output.take_stream_stopped_error()
    }

    /// Total 20 ms mono blocks dropped because the sink rejected them
    /// (`SystemAudioSink::on_block` returned `false`).
    pub fn dropped_blocks(&self) -> u64 {
        self.output.dropped_blocks()
    }

    /// Non-audio sample-buffer deliveries observed and ignored. Should
    /// remain `0` in this audio-only adapter; exposed for probe evidence.
    pub fn non_audio_deliveries(&self) -> u64 {
        self.output.non_audio_deliveries()
    }

    /// Sample buffers rejected by runtime format validation.
    pub fn format_rejections(&self) -> u64 {
        self.output.format_rejections()
    }
}

impl Drop for SystemAudioSession {
    fn drop(&mut self) {
        // A panic or early return elsewhere must not leak the stream,
        // delegate, or dispatch queue: idempotent `stop()` runs here too.
        let _ = self.stop();
    }
}

/// Starts capture. Fails without allocating a stream when unsupported,
/// unpermitted, or unable to build a content filter.
pub fn start(
    config: SystemAudioConfig,
    sink: Box<dyn SystemAudioSink>,
) -> Result<SystemAudioSession, SystemAudioError> {
    config::check_macos_version()?;

    let was_undetermined = permission::permission_status() != ScreenRecordingPermission::Granted;
    if was_undetermined && permission::request_permission() == ScreenRecordingPermission::Denied {
        return Err(SystemAudioError::new(
            SystemAudioErrorKind::PermissionDenied,
            "Screen Recording access was denied",
        ));
    }

    let content = match get_shareable_content() {
        Ok(content) => content,
        Err(e) => {
            return Err(if was_undetermined {
                SystemAudioError::new(SystemAudioErrorKind::PermissionRequiresRestart, e.detail)
            } else {
                SystemAudioError::new(SystemAudioErrorKind::PermissionDenied, e.detail)
            });
        }
    };

    // SAFETY: `content` is a live `Retained<SCShareableContent>` owned by
    // this function; `displays` is a read-only accessor.
    let displays = unsafe { content.displays() };
    let Some(display) = displays.firstObject() else {
        return Err(SystemAudioError::new(
            SystemAudioErrorKind::NoCaptureContent,
            "no display available to build a content filter",
        ));
    };

    let filter = config::build_content_filter(&display);
    let stream_config = config::build_stream_configuration(&config);

    let output = StreamOutputHandler::new(sink);
    let delegate_protocol = ProtocolObject::from_ref(&*output);
    // SAFETY: `SCStream::alloc()` is a fresh, uninitialized allocation
    // consumed exactly once here; `filter`, `stream_config`, and
    // `delegate_protocol` (backed by `output`, which this function keeps
    // alive in the returned session) are all valid live references.
    let stream = unsafe {
        SCStream::initWithFilter_configuration_delegate(
            SCStream::alloc(),
            &filter,
            &stream_config,
            Some(delegate_protocol),
        )
    };

    let queue = DispatchQueue::new("com.mistaken.macos-system-audio.delivery", None);
    let output_protocol = ProtocolObject::from_ref(&*output);
    // SAFETY: `stream` was just initialized above; `output_protocol` is
    // backed by `output`, which this function keeps alive; `queue` is a
    // live serial `DispatchQueue`.
    if let Err(e) = unsafe {
        stream.addStreamOutput_type_sampleHandlerQueue_error(
            output_protocol,
            SCStreamOutputType::Audio,
            Some(&queue),
        )
    } {
        let detail = e.localizedDescription().to_string();
        return Err(SystemAudioError::new(
            SystemAudioErrorKind::StartFailed,
            detail,
        ));
    }

    if let Err(e) = start_capture(&stream) {
        // SAFETY: `output_protocol` was just added on this same `stream`
        // above; both are still valid, live objects.
        let _ = unsafe {
            stream.removeStreamOutput_type_error(output_protocol, SCStreamOutputType::Audio)
        };
        return Err(e);
    }

    Ok(SystemAudioSession {
        stream,
        output,
        _queue: queue,
        stopped: Once::new(),
    })
}
