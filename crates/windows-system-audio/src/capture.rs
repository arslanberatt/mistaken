//! Event-driven WASAPI loopback capture: thread/COM ownership, the frozen
//! initialization sequence, the capture loop, and the public
//! `availability()`/`start()`/`SystemAudioSession` surface (spec section 6
//! "Frozen initialization" and section 11 "Resource Lifecycle").
//!
//! Everything in this module except `availability()` and the `start()`
//! caller-side setup runs on one dedicated capture thread that this module
//! spawns. COM is initialized and uninitialized only on that thread.

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Wdk::System::SystemServices::RtlGetVersion;
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Media::Audio::{
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
    WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};
use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use crate::blocks::BlockAssembler;
use crate::com::ComScope;
use crate::config::SystemAudioConfig;
use crate::endpoint::{endpoint_id, has_default_endpoint_changed, resolve_default_render_endpoint};
use crate::error::{map_runtime_error, map_start_error, SystemAudioError, SystemAudioErrorKind};
use crate::format::{
    validate_raw_format, RawMixFormat, SubFormatKind, ValidatedFormat, WAVE_FORMAT_EXTENSIBLE,
};
use crate::timeline::Timeline;
use crate::{SystemAudioCounters, SystemAudioFormat, SystemAudioSink};

/// The API floor: below Windows 10 version 1703 (build 15063), event-driven
/// loopback receives no events (spec section 3/10).
const MIN_SUPPORTED_BUILD: u32 = 15_063;

/// The capture thread's bounded event wait. Short enough that `stop()` and
/// the once-per-second endpoint poll both stay responsive.
const EVENT_WAIT_TIMEOUT_MS: u32 = 200;
const ENDPOINT_POLL_INTERVAL: Duration = Duration::from_secs(1);
const STOP_JOIN_BUDGET: Duration = Duration::from_secs(5);

// The subformat GUIDs from `ksmedia.h`, reconstructed from their canonical
// string form rather than requiring the `Win32_Media_KernelStreaming`
// feature: {00000001-0000-0010-8000-00AA00389B71} and
// {00000003-0000-0010-8000-00AA00389B71}.
const SUBTYPE_PCM: windows::core::GUID =
    windows::core::GUID::from_u128(0x00000001_0000_0010_8000_00AA00389B71);
const SUBTYPE_IEEE_FLOAT: windows::core::GUID =
    windows::core::GUID::from_u128(0x00000003_0000_0010_8000_00AA00389B71);

/// Reports whether loopback capture is possible right now, without starting
/// it: version floor, then the device enumerator and default render endpoint.
/// Performs no `Initialize`, starts no thread, produces no prompt, and
/// captures nothing.
pub fn availability() -> Result<(), SystemAudioError> {
    check_version_floor()?;
    let _scope = ComScope::initialize_mta()
        .map_err(|err| map_start_error("CoInitializeEx(availability)", err))?;
    let enumerator = create_enumerator()?;
    let device = resolve_default_render_endpoint(&enumerator)?;
    drop(device);
    Ok(())
}

/// Starts capture on the default render endpoint. Spawns the adapter's own
/// capture thread, which performs every WASAPI/COM call; this function blocks
/// only until that thread reports its start result.
pub fn start(
    config: SystemAudioConfig,
    sink: Box<dyn SystemAudioSink>,
) -> Result<SystemAudioSession, SystemAudioError> {
    check_version_floor()?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let counters = Arc::new(Mutex::new(SystemAudioCounters::default()));
    let (ready_tx, ready_rx) = mpsc::channel::<Result<SystemAudioFormat, SystemAudioError>>();

    let thread_stop_flag = Arc::clone(&stop_flag);
    let thread_counters = Arc::clone(&counters);
    let join_handle = std::thread::Builder::new()
        .name("mistaken-windows-system-audio".to_string())
        .spawn(move || {
            run_capture_thread(config, sink, thread_stop_flag, thread_counters, ready_tx);
        })
        .map_err(|err| {
            SystemAudioError::new(
                SystemAudioErrorKind::StartFailed,
                format!("failed to spawn capture thread: {err}"),
            )
        })?;

    match ready_rx.recv() {
        Ok(Ok(_format)) => Ok(SystemAudioSession {
            stop_flag,
            join_handle: Some(join_handle),
            counters,
        }),
        Ok(Err(err)) => {
            let _ = join_handle.join();
            Err(err)
        }
        Err(_) => {
            let _ = join_handle.join();
            Err(SystemAudioError::new(
                SystemAudioErrorKind::Internal,
                "capture thread exited before reporting a start result",
            ))
        }
    }
}

/// Opaque session handle: owns the COM scope, WASAPI client, event, and
/// capture thread indirectly through that dedicated thread. A second
/// `start()` on the same handle is impossible by construction — there is no
/// API to restart an existing `SystemAudioSession`, only to create a new one.
pub struct SystemAudioSession {
    stop_flag: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
    counters: Arc<Mutex<SystemAudioCounters>>,
}

impl SystemAudioSession {
    /// Signals the capture thread, joins it within a bounded budget, and
    /// releases every resource on that thread. Idempotent: a second call
    /// returns success without touching already-released objects.
    pub fn stop(&mut self) -> Result<(), SystemAudioError> {
        self.stop_flag.store(true, Ordering::SeqCst);
        let Some(handle) = self.join_handle.take() else {
            return Ok(());
        };
        let deadline = Instant::now() + STOP_JOIN_BUDGET;
        loop {
            if handle.is_finished() {
                return handle.join().map_err(|_| {
                    SystemAudioError::new(
                        SystemAudioErrorKind::StopFailed,
                        "capture thread panicked during stop",
                    )
                });
            }
            if Instant::now() >= deadline {
                return Err(SystemAudioError::new(
                    SystemAudioErrorKind::StopFailed,
                    "capture thread did not exit within the stop budget",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A snapshot of the session's counters to date.
    pub fn counters(&self) -> SystemAudioCounters {
        *self
            .counters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Drop for SystemAudioSession {
    fn drop(&mut self) {
        // Idempotent teardown so an early return cannot leak a thread or a
        // COM reference; `stop()` is a no-op if already stopped.
        let _ = self.stop();
    }
}

fn check_version_floor() -> Result<(), SystemAudioError> {
    let build = running_build_number()?;
    if build < MIN_SUPPORTED_BUILD {
        return Err(SystemAudioError::new(
            SystemAudioErrorKind::Unsupported,
            format!("Windows build {build} is below the supported floor {MIN_SUPPORTED_BUILD}"),
        ));
    }
    Ok(())
}

/// Reads the true running build number via `RtlGetVersion`, which — unlike
/// `GetVersionExW` — is not subject to application-manifest version lying.
fn running_build_number() -> Result<u32, SystemAudioError> {
    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // Safety: `info` is a valid, correctly sized `OSVERSIONINFOW` for the
    // duration of this call, matching the documented `RtlGetVersion` contract.
    let status = unsafe { RtlGetVersion(&mut info as *mut OSVERSIONINFOW) };
    if status.is_ok() {
        Ok(info.dwBuildNumber)
    } else {
        Err(SystemAudioError::new(
            SystemAudioErrorKind::Internal,
            format!("RtlGetVersion failed with NTSTATUS 0x{:08X}", status.0),
        ))
    }
}

fn create_enumerator() -> Result<IMMDeviceEnumerator, SystemAudioError> {
    // Safety: `MMDeviceEnumerator` is the documented class id for
    // `IMMDeviceEnumerator`; no outer object, in-process server context.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
        .map_err(|err| map_start_error("CoCreateInstance(MMDeviceEnumerator)", err))
}

/// Reads the fields this crate needs from a `WAVEFORMATEX`/
/// `WAVEFORMATEXTENSIBLE` pointer returned by `IAudioClient::GetMixFormat`,
/// without transmuting or guessing: the `WAVEFORMATEXTENSIBLE` branch is only
/// taken when `wFormatTag` says so, matching its documented C layout (a
/// `WAVEFORMATEX` prefix followed by the extensible tail).
///
/// # Safety
/// `ptr` must be a valid, non-null pointer to an initialized `WAVEFORMATEX`
/// (or, when `wFormatTag == WAVE_FORMAT_EXTENSIBLE`, a `WAVEFORMATEXTENSIBLE`)
/// for the duration of this call, as `GetMixFormat` guarantees.
unsafe fn read_raw_mix_format(ptr: *const WAVEFORMATEX) -> RawMixFormat {
    let base = &*ptr;
    let format_tag = base.wFormatTag;
    if format_tag == WAVE_FORMAT_EXTENSIBLE {
        // `WAVEFORMATEXTENSIBLE` is `#[repr(C, packed(1))]`: taking a
        // reference to any of its fields (including through the embedded
        // `Format`/`Samples`/`SubFormat`) is undefined behavior even if the
        // reference is never read. `read_unaligned` copies the whole struct
        // into a normally aligned local value, after which every field
        // access below is an ordinary, safe read.
        let ext = core::ptr::read_unaligned(ptr as *const WAVEFORMATEXTENSIBLE);
        let valid_bits = ext.Samples.wValidBitsPerSample;
        let sub_format_guid = ext.SubFormat;
        let sub_format = if sub_format_guid == SUBTYPE_PCM {
            SubFormatKind::Pcm
        } else if sub_format_guid == SUBTYPE_IEEE_FLOAT {
            SubFormatKind::IeeeFloat
        } else {
            SubFormatKind::Other
        };
        RawMixFormat {
            format_tag,
            channels: ext.Format.nChannels,
            samples_per_sec: ext.Format.nSamplesPerSec,
            bits_per_sample: ext.Format.wBitsPerSample,
            block_align: ext.Format.nBlockAlign,
            valid_bits_per_sample: valid_bits,
            sub_format,
        }
    } else {
        RawMixFormat {
            format_tag,
            channels: base.nChannels,
            samples_per_sec: base.nSamplesPerSec,
            bits_per_sample: base.wBitsPerSample,
            block_align: base.nBlockAlign,
            valid_bits_per_sample: base.wBitsPerSample,
            sub_format: SubFormatKind::None,
        }
    }
}

/// The parts of a successfully initialized session that the capture loop and
/// teardown both need.
struct InitializedSession {
    com_scope: ComScope,
    audio_client: IAudioClient,
    capture_client: IAudioCaptureClient,
    event_handle: HANDLE,
    format: ValidatedFormat,
}

/// Performs the frozen initialization sequence (spec section 6) on the
/// calling thread, which must be the adapter's own dedicated capture thread.
/// On any failure, everything allocated so far in this function is released
/// before returning, so a failed start leaks nothing.
fn initialize_session(config: SystemAudioConfig) -> Result<InitializedSession, SystemAudioError> {
    let com_scope = ComScope::initialize_mta()
        .map_err(|err| map_start_error("CoInitializeEx(capture thread)", err))?;

    let enumerator = create_enumerator()?;
    let device = resolve_default_render_endpoint(&enumerator)?;

    // Safety: `device` is a valid `IMMDevice` just resolved above; `CLSCTX_ALL`
    // and no activation params request the default in-process `IAudioClient`.
    let audio_client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }
        .map_err(|err| map_start_error("IMMDevice::Activate(IAudioClient)", err))?;

    // Safety: `GetMixFormat` returns a COM task-allocated, non-null pointer to
    // a valid `WAVEFORMATEX` on success; freed via `CoTaskMemFree` below after
    // `read_raw_mix_format` has copied every field it needs out of it.
    let mix_format_ptr = unsafe { audio_client.GetMixFormat() }
        .map_err(|err| map_start_error("IAudioClient::GetMixFormat", err))?;
    let raw_format = unsafe { read_raw_mix_format(mix_format_ptr) };
    let validated = match validate_raw_format(&raw_format) {
        Ok(validated) => validated,
        Err(err) => {
            unsafe { CoTaskMemFree(Some(mix_format_ptr as *const _)) };
            return Err(err);
        }
    };

    let hns_buffer_duration = (config.buffer_duration_ms as i64) * 10_000;
    // Safety: `mix_format_ptr` is still valid (freed only after this call);
    // `AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK` and
    // shared mode are the frozen initialization per spec section 6.
    let init_result = unsafe {
        audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            hns_buffer_duration,
            0,
            mix_format_ptr,
            None,
        )
    };
    unsafe { CoTaskMemFree(Some(mix_format_ptr as *const _)) };
    init_result.map_err(|err| map_start_error("IAudioClient::Initialize", err))?;

    // Safety: an auto-reset, initially-unsignaled, unnamed event; `HANDLE` is
    // owned by this function from here on and closed on every exit path.
    let event_handle = unsafe { CreateEventW(None, false, false, windows::core::PCWSTR::null()) }
        .map_err(|err| map_start_error("CreateEventW", err))?;

    // Safety: `event_handle` is a valid, freshly created event handle.
    if let Err(err) = unsafe { audio_client.SetEventHandle(event_handle) } {
        unsafe { close_handle_ignoring_error(event_handle) };
        return Err(map_start_error("IAudioClient::SetEventHandle", err));
    }

    // Safety: `audio_client` was just initialized above with the loopback
    // capture stream flags, so `IAudioCaptureClient` is the correct service.
    let capture_client: IAudioCaptureClient = match unsafe { audio_client.GetService() } {
        Ok(client) => client,
        Err(err) => {
            unsafe { close_handle_ignoring_error(event_handle) };
            return Err(map_start_error(
                "IAudioClient::GetService(IAudioCaptureClient)",
                err,
            ));
        }
    };

    // Safety: the client is fully initialized with a service obtained above.
    if let Err(err) = unsafe { audio_client.Start() } {
        unsafe { close_handle_ignoring_error(event_handle) };
        return Err(map_start_error("IAudioClient::Start", err));
    }

    Ok(InitializedSession {
        com_scope,
        audio_client,
        capture_client,
        event_handle,
        format: validated,
    })
}

/// Safety: `handle` must be a valid handle owned by this crate; errors are
/// intentionally swallowed because this only runs on an already-failing
/// teardown path where the original error is what the caller reports.
unsafe fn close_handle_ignoring_error(handle: HANDLE) {
    let _ = windows::Win32::Foundation::CloseHandle(handle);
}

/// Runs entirely on the dedicated capture thread this function is spawned on
/// (`start()`). Initializes COM/WASAPI, reports the start result through
/// `ready_tx`, runs the capture loop until `stop_flag` is set or a terminal
/// condition is reported through `sink.on_error`, then tears everything down
/// in the documented release order: client service, client, device,
/// enumerator (device/enumerator already dropped by `initialize_session`'s
/// own scope), event handle, and finally the COM scope.
fn run_capture_thread(
    config: SystemAudioConfig,
    mut sink: Box<dyn SystemAudioSink>,
    stop_flag: Arc<AtomicBool>,
    counters: Arc<Mutex<SystemAudioCounters>>,
    ready_tx: mpsc::Sender<Result<SystemAudioFormat, SystemAudioError>>,
) {
    let session = match initialize_session(config) {
        Ok(session) => session,
        Err(err) => {
            let _ = ready_tx.send(Err(err));
            return;
        }
    };

    let InitializedSession {
        com_scope,
        audio_client,
        capture_client,
        event_handle,
        format,
    } = session;

    let output_format = format.output_format();
    if ready_tx.send(Ok(output_format)).is_err() {
        // The caller already gave up (dropped the receiver). Tear down and
        // exit without ever entering the capture loop.
        let _ = unsafe { audio_client.Stop() };
        unsafe { close_handle_ignoring_error(event_handle) };
        drop(capture_client);
        drop(audio_client);
        drop(com_scope);
        return;
    }

    capture_loop(
        &audio_client,
        &capture_client,
        event_handle,
        &format,
        config,
        sink.as_mut(),
        &stop_flag,
        &counters,
    );

    let _ = unsafe { audio_client.Stop() };
    unsafe { close_handle_ignoring_error(event_handle) };
    drop(capture_client);
    drop(audio_client);
    drop(com_scope);
}

/// Delivers `samples` (already downmixed mono) through the block assembler to
/// the sink, then republishes the timeline's latest counters. A plain
/// function rather than a captured closure: `sink`/`timeline` are borrowed
/// only for this one call, so the caller remains free to use them again
/// immediately afterward.
fn deliver_samples(
    samples: &[f32],
    format: &ValidatedFormat,
    assembler: &mut BlockAssembler,
    sink: &mut dyn SystemAudioSink,
    timeline: &mut Timeline,
    counters: &Mutex<SystemAudioCounters>,
) {
    let output_format = format.output_format();
    assembler.feed(samples, |block| {
        if !sink.on_block(block, output_format) {
            timeline.note_dropped_block();
        }
    });
    if let Ok(mut guard) = counters.lock() {
        *guard = timeline.snapshot();
    }
}

#[allow(clippy::too_many_arguments)]
fn capture_loop(
    audio_client: &IAudioClient,
    capture_client: &IAudioCaptureClient,
    event_handle: HANDLE,
    format: &ValidatedFormat,
    config: SystemAudioConfig,
    sink: &mut dyn SystemAudioSink,
    stop_flag: &AtomicBool,
    counters: &Mutex<SystemAudioCounters>,
) {
    let _ = audio_client; // kept alive by the caller's scope for the loop's duration
    let block_len = (format.sample_rate_hz / 50).max(1) as usize;
    let mut assembler = BlockAssembler::new(block_len);
    let max_gap_fill_frames = (format.sample_rate_hz as u64 * config.max_gap_fill_ms as u64) / 1000;
    let mut timeline = Timeline::new(max_gap_fill_frames);
    // The one preallocated conversion scratch buffer, sized for one second of
    // mono output; grown only if an unusually large packet ever requires it,
    // which does not happen in normal operation at the configured buffer
    // duration.
    let mut mono_scratch: Vec<f32> = Vec::with_capacity(format.sample_rate_hz as usize);

    // Default-endpoint-change detection needs its own enumerator instance and
    // the endpoint id observed at start, resolved once here (spec section 6:
    // at most once per second, from the adapter's own thread).
    let poll_state = create_enumerator()
        .and_then(|enumerator| {
            let device = resolve_default_render_endpoint(&enumerator)?;
            let id = endpoint_id(&device)?;
            Ok::<_, SystemAudioError>((enumerator, id))
        })
        .ok();
    let mut last_endpoint_poll = Instant::now();

    'capture: loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        if let Some((enumerator, current_id)) = poll_state.as_ref() {
            if last_endpoint_poll.elapsed() >= ENDPOINT_POLL_INTERVAL {
                last_endpoint_poll = Instant::now();
                match has_default_endpoint_changed(enumerator, current_id) {
                    Ok(true) => {
                        sink.on_error(SystemAudioError::new(
                            SystemAudioErrorKind::EndpointChanged,
                            "default render endpoint changed during capture",
                        ));
                        break;
                    }
                    Ok(false) => {}
                    Err(err) => {
                        sink.on_error(err);
                        break;
                    }
                }
            }
        }

        // Safety: `event_handle` remains valid for the capture loop's
        // duration; it is closed only after this loop returns.
        let wait_result = unsafe { WaitForSingleObject(event_handle, EVENT_WAIT_TIMEOUT_MS) };
        if wait_result == WAIT_TIMEOUT {
            let elapsed_frames =
                (EVENT_WAIT_TIMEOUT_MS as u64 * format.sample_rate_hz as u64) / 1000;
            let fill = timeline.note_idle_wait(elapsed_frames);
            if fill.silent_frames_to_emit > 0 {
                mono_scratch.clear();
                mono_scratch.resize(fill.silent_frames_to_emit as usize, 0.0);
                deliver_samples(
                    &mono_scratch,
                    format,
                    &mut assembler,
                    sink,
                    &mut timeline,
                    counters,
                );
            }
            continue;
        } else if wait_result != WAIT_OBJECT_0 {
            sink.on_error(SystemAudioError::new(
                SystemAudioErrorKind::Internal,
                format!("WaitForSingleObject returned unexpected {wait_result:?}"),
            ));
            break;
        }

        loop {
            // Safety: `capture_client` is valid and owned by this loop's
            // caller for its entire duration.
            let packet_frames = match unsafe { capture_client.GetNextPacketSize() } {
                Ok(frames) => frames,
                Err(err) => {
                    sink.on_error(map_runtime_error(
                        "IAudioCaptureClient::GetNextPacketSize",
                        err,
                    ));
                    break 'capture;
                }
            };
            if packet_frames == 0 {
                break;
            }

            let mut data_ptr: *mut u8 = std::ptr::null_mut();
            let mut frames_available: u32 = 0;
            let mut flags: u32 = 0;
            let mut device_position: u64 = 0;
            let mut qpc_position: u64 = 0;
            // Safety: all five out-pointers are valid local `&mut` bindings
            // for the duration of this call, matching `GetBuffer`'s contract.
            let get_result = unsafe {
                capture_client.GetBuffer(
                    &mut data_ptr,
                    &mut frames_available,
                    &mut flags,
                    Some(&mut device_position),
                    Some(&mut qpc_position),
                )
            };
            if let Err(err) = get_result {
                sink.on_error(map_runtime_error("IAudioCaptureClient::GetBuffer", err));
                break 'capture;
            }

            let silent = (flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
            let discontinuity = (flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32) != 0;
            let fill = timeline.observe_packet(
                device_position,
                frames_available as u64,
                silent,
                discontinuity,
            );

            if fill.silent_frames_to_emit > 0 {
                mono_scratch.clear();
                mono_scratch.resize(fill.silent_frames_to_emit as usize, 0.0);
                deliver_samples(
                    &mono_scratch,
                    format,
                    &mut assembler,
                    sink,
                    &mut timeline,
                    counters,
                );
            }

            mono_scratch.clear();
            if silent || data_ptr.is_null() {
                // Silent-flagged packets are treated as zeros without reading
                // the buffer contents (spec section 6).
                mono_scratch.resize(frames_available as usize, 0.0);
            } else {
                let byte_len = frames_available as usize * format.frame_size_bytes();
                // Safety: `data_ptr` is valid for `byte_len` bytes until the
                // matching `ReleaseBuffer` call below, per `GetBuffer`'s
                // documented packet-lifetime contract.
                let bytes = unsafe { std::slice::from_raw_parts(data_ptr, byte_len) };
                crate::format::convert_and_downmix(
                    bytes,
                    frames_available as usize,
                    format,
                    &mut mono_scratch,
                );
            }
            deliver_samples(
                &mono_scratch,
                format,
                &mut assembler,
                sink,
                &mut timeline,
                counters,
            );

            // Safety: releases exactly the packet just read, matching
            // `frames_available` from the paired `GetBuffer` call above;
            // `GetBuffer`/`ReleaseBuffer` alternate strictly on this thread.
            if let Err(err) = unsafe { capture_client.ReleaseBuffer(frames_available) } {
                sink.on_error(map_runtime_error("IAudioCaptureClient::ReleaseBuffer", err));
                break 'capture;
            }
        }
    }

    // A partial tail is discarded at stop and never zero-padded into a short
    // block.
    assembler.discard_partial();
}
