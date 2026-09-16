//! `RuntimeManager`: orchestrates the real microphone + system-audio + local
//! development ASR lifecycle behind the typed runtime commands.
//!
//! Owns the active dual-source session, atomic start with full rollback,
//! survivor continuity on single-source mid-session failure, bounded
//! per-source recovery (Spec 10), watchdog-bounded transitions, one
//! idempotent shutdown path, and event emission.

use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use tauri::{AppHandle, Runtime};

use crate::asr::chunk_pool::{asr_chunk_capacity_for_rate, build_asr_pool, AsrChunkFeeder};
use crate::asr::manifest::{resolve_model_dir, DEVELOPMENT_MANIFEST};
use crate::asr::worker::{AsrWorker, AsrWorkerObserver};
use crate::asr::{AsrError, AsrErrorKind, AsrModelLoader, RecognizerFactory};
use crate::audio::buffer::{block_capacity_for_rate, build_pool};
use crate::audio::microphone::{
    MicrophoneBackend, MicrophoneCaptureHandle, MicrophoneMonitor, MicrophoneMonitorObserver,
};
use crate::audio::supervisor::{
    classify_recovery, interruptible_wait, RecoveryDecision, RecoveryPolicy,
};
use crate::audio::system::{SystemAudioBackend, SystemAudioMonitor, SystemAudioMonitorObserver};
use crate::audio::{AudioError, AudioErrorKind, AudioSource, PcmFormat};
use crate::events;

use super::runtime::{
    Activity, AudioSourceStatus, CaptureStatus, MicrophoneDevice, ModelStatus, RuntimeError,
    RuntimeErrorCode, RuntimeSnapshot, RuntimeState, TranscriptSegment, TranscriptSource,
};

fn map_audio_error(error: AudioError) -> RuntimeError {
    let source = TranscriptSource::from(error.source);
    match error.source {
        AudioSource::Microphone => {
            let (code, message, recoverable): (RuntimeErrorCode, &str, bool) = match error.kind {
                AudioErrorKind::PermissionDenied => (
                    RuntimeErrorCode::MicrophonePermissionDenied,
                    "microphone access is off",
                    true,
                ),
                AudioErrorKind::Unavailable => (
                    RuntimeErrorCode::MicrophoneUnavailable,
                    "no microphone is available",
                    true,
                ),
                AudioErrorKind::DeviceDisconnected => (
                    RuntimeErrorCode::DeviceDisconnected,
                    "the selected microphone was disconnected",
                    true,
                ),
                AudioErrorKind::UnsupportedFormat => (
                    RuntimeErrorCode::CaptureStartFailed,
                    "the microphone's audio format is not supported",
                    false,
                ),
                AudioErrorKind::StartFailed => (
                    RuntimeErrorCode::CaptureStartFailed,
                    "the microphone could not be started",
                    true,
                ),
                AudioErrorKind::StopFailed => (
                    RuntimeErrorCode::CaptureStopFailed,
                    "the microphone could not be stopped cleanly",
                    false,
                ),
                AudioErrorKind::QueueOverflow => (
                    RuntimeErrorCode::AudioQueueOverflow,
                    "microphone input is delayed; some audio was dropped",
                    true,
                ),
                AudioErrorKind::Internal => (
                    RuntimeErrorCode::Internal,
                    "an internal microphone error occurred",
                    false,
                ),
            };
            RuntimeError::new(code, message, recoverable).with_source(source)
        }
        AudioSource::System => {
            #[cfg(target_os = "macos")]
            {
                let (code, message, recoverable) = match error.kind {
                    AudioErrorKind::PermissionDenied => (
                        RuntimeErrorCode::SystemAudioPermissionDenied,
                        "macOS grants system audio through Screen Recording. Enable Mistaken in System Settings → Privacy & Security → Screen Recording.",
                        true,
                    ),
                    AudioErrorKind::Unavailable => {
                        let info = objc2_foundation::NSProcessInfo::processInfo();
                        let min_version = objc2_foundation::NSOperatingSystemVersion {
                            majorVersion: 13,
                            minorVersion: 0,
                            patchVersion: 0,
                        };
                        if !info.isOperatingSystemAtLeastVersion(min_version) {
                            (
                                RuntimeErrorCode::UnsupportedPlatform,
                                "system audio requires macOS 13.0 or newer",
                                false,
                            )
                        } else {
                            (
                                RuntimeErrorCode::SystemAudioUnavailable,
                                "system audio is unavailable",
                                true,
                            )
                        }
                    }
                    AudioErrorKind::DeviceDisconnected => (
                        RuntimeErrorCode::DeviceDisconnected,
                        "system audio output device disconnected",
                        true,
                    ),
                    AudioErrorKind::UnsupportedFormat => (
                        RuntimeErrorCode::CaptureStartFailed,
                        "system audio format is not supported",
                        false,
                    ),
                    AudioErrorKind::StartFailed => (
                        RuntimeErrorCode::CaptureStartFailed,
                        "system audio capture failed to start",
                        true,
                    ),
                    AudioErrorKind::StopFailed => (
                        RuntimeErrorCode::CaptureStopFailed,
                        "system audio capture failed to stop cleanly",
                        false,
                    ),
                    AudioErrorKind::QueueOverflow => (
                        RuntimeErrorCode::AudioQueueOverflow,
                        "system audio input is delayed; some audio was dropped",
                        true,
                    ),
                    AudioErrorKind::Internal => (
                        RuntimeErrorCode::Internal,
                        "an internal system audio error occurred",
                        false,
                    ),
                };
                RuntimeError::new(code, message, recoverable).with_source(source)
            }
            #[cfg(target_os = "windows")]
            {
                let (code, message, recoverable) = match error.kind {
                    AudioErrorKind::PermissionDenied => (
                        RuntimeErrorCode::Internal,
                        "unexpected permission error on Windows",
                        false,
                    ),
                    AudioErrorKind::Unavailable => (
                        RuntimeErrorCode::SystemAudioUnavailable,
                        "No audio output device is available. Connect or enable an output device.",
                        true,
                    ),
                    AudioErrorKind::DeviceDisconnected => (
                        RuntimeErrorCode::DeviceDisconnected,
                        "the audio output device was unplugged or reconfigured",
                        true,
                    ),
                    AudioErrorKind::UnsupportedFormat => (
                        RuntimeErrorCode::CaptureStartFailed,
                        "the audio output device mix format is not supported",
                        false,
                    ),
                    AudioErrorKind::StartFailed => (
                        RuntimeErrorCode::CaptureStartFailed,
                        "system audio capture failed to start",
                        true,
                    ),
                    AudioErrorKind::StopFailed => (
                        RuntimeErrorCode::CaptureStopFailed,
                        "system audio capture failed to stop cleanly",
                        false,
                    ),
                    AudioErrorKind::QueueOverflow => (
                        RuntimeErrorCode::AudioQueueOverflow,
                        "system audio input is delayed; some audio was dropped",
                        true,
                    ),
                    AudioErrorKind::Internal => (
                        RuntimeErrorCode::Internal,
                        "an internal system audio error occurred",
                        false,
                    ),
                };
                RuntimeError::new(code, message, recoverable).with_source(source)
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                RuntimeError::new(
                    RuntimeErrorCode::UnsupportedPlatform,
                    "system audio capture is not supported on this operating system",
                    false,
                )
                .with_source(source)
            }
        }
    }
}

fn map_asr_error(error: &AsrError, source: Option<TranscriptSource>) -> RuntimeError {
    let (code, recoverable): (RuntimeErrorCode, bool) = match error.kind {
        AsrErrorKind::ModelMissing => (RuntimeErrorCode::ModelMissing, true),
        AsrErrorKind::ModelUnsupported => (RuntimeErrorCode::ModelUnsupported, false),
        AsrErrorKind::ModelLoadFailed => (RuntimeErrorCode::ModelLoadFailed, true),
        AsrErrorKind::StreamCreateFailed => (RuntimeErrorCode::CaptureStartFailed, true),
        AsrErrorKind::DecodeFailed | AsrErrorKind::Internal => (RuntimeErrorCode::Internal, false),
    };
    let mut err = RuntimeError::new(code, error.detail.clone(), recoverable);
    if let Some(src) = source {
        err = err.with_source(src);
    }
    err
}

fn model_status_for_asr_error(error: &AsrError) -> ModelStatus {
    match error.kind {
        AsrErrorKind::ModelMissing => ModelStatus::Missing,
        AsrErrorKind::ModelUnsupported => ModelStatus::Unsupported {
            error: map_asr_error(error, None),
        },
        _ => ModelStatus::Failed {
            error: map_asr_error(error, None),
        },
    }
}

/// Human-readable, lowercase source label for recovery/lag message text.
fn source_label(source: AudioSource) -> &'static str {
    match source {
        AudioSource::Microphone => "microphone",
        AudioSource::System => "system audio",
    }
}

/// Same as [`source_label`] but capitalized for sentence-initial use.
fn source_label_capitalized(source: AudioSource) -> &'static str {
    match source {
        AudioSource::Microphone => "Microphone",
        AudioSource::System => "System audio",
    }
}

struct ActiveMicrophoneSession {
    monitor: MicrophoneMonitor,
    asr_worker: AsrWorker,
}

struct ActiveSystemSession {
    monitor: SystemAudioMonitor,
    asr_worker: AsrWorker,
}

/// Per-source recovery bookkeeping that must survive across a recovery
/// restart even while `mic`/`sys` is momentarily `None` (backing off).
/// Reset only by a brand new session — never by a recovery.
#[derive(Debug, Clone)]
struct RecoveryState {
    /// Attempts consumed against the frozen 3-attempt budget.
    attempts_used: u8,
    /// Set on this source's most recent `on_first_signal`; cleared on
    /// every fault. Used to decide whether the 60 s healthy-reset window
    /// has elapsed the next time this source faults.
    healthy_since: Option<Instant>,
    /// The next segment index a freshly (re)started worker must use, so
    /// segment ids stay unique for the session across a recovery.
    next_segment_index: u64,
    /// The latest `endedAtMs` this source has emitted; a recovery restart
    /// clamps its new start offset to at least this value.
    last_ended_at_ms: u64,
}

impl RecoveryState {
    fn new() -> Self {
        Self {
            attempts_used: 0,
            healthy_since: None,
            next_segment_index: 0,
            last_ended_at_ms: 0,
        }
    }
}

struct ActiveSession {
    mic: Option<ActiveMicrophoneSession>,
    sys: Option<ActiveSystemSession>,
    requested_microphone_device_id: Option<String>,
    session_id: u64,
    session_clock_origin: Instant,
    mic_recovery: RecoveryState,
    sys_recovery: RecoveryState,
}

impl ActiveSession {
    /// Graceful, whole-session teardown used only by a user-initiated
    /// Stop and by `shutdown()`. Each in-flight interim is finalized
    /// normally (Spec 06's existing Stop behavior), unlike a fault-driven
    /// per-source `abandon()`.
    fn stop(&mut self) {
        if let Some(mut mic) = self.mic.take() {
            mic.monitor.stop();
            mic.asr_worker.stop();
        }
        if let Some(mut sys) = self.sys.take() {
            sys.monitor.stop();
            sys.asr_worker.stop();
        }
    }
}

/// One restarted source, returned by a recovery attempt before it is
/// installed back into the active session.
enum RestartedSource {
    Mic(ActiveMicrophoneSession),
    Sys(ActiveSystemSession),
}

/// Process-lifetime (not persisted) recovery/lifecycle counters, recorded
/// only for evidence — never part of the frozen IPC contract. See
/// `docs/lifecycle-policy.md` section 9.
#[derive(Debug, Default)]
struct RecoveryCounters {
    mic_recovery_attempts: AtomicU32,
    mic_recovery_successes: AtomicU32,
    mic_lag_windows_entered: AtomicU32,
    mic_watchdog_expiries: AtomicU32,
    mic_contained_panics: AtomicU32,
    sys_recovery_attempts: AtomicU32,
    sys_recovery_successes: AtomicU32,
    sys_lag_windows_entered: AtomicU32,
    sys_watchdog_expiries: AtomicU32,
    sys_contained_panics: AtomicU32,
}

impl RecoveryCounters {
    fn record_attempt(&self, source: AudioSource) {
        let counter = match source {
            AudioSource::Microphone => &self.mic_recovery_attempts,
            AudioSource::System => &self.sys_recovery_attempts,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
    fn record_success(&self, source: AudioSource) {
        let counter = match source {
            AudioSource::Microphone => &self.mic_recovery_successes,
            AudioSource::System => &self.sys_recovery_successes,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
    fn record_lag_window(&self, source: AudioSource) {
        let counter = match source {
            AudioSource::Microphone => &self.mic_lag_windows_entered,
            AudioSource::System => &self.sys_lag_windows_entered,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
    fn record_watchdog_expiry(&self, source: AudioSource) {
        let counter = match source {
            AudioSource::Microphone => &self.mic_watchdog_expiries,
            AudioSource::System => &self.sys_watchdog_expiries,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
    fn record_panic(&self, source: AudioSource) {
        let counter = match source {
            AudioSource::Microphone => &self.mic_contained_panics,
            AudioSource::System => &self.sys_contained_panics,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// A read-only snapshot of one source's lifecycle counters, for evidence
/// and tests only — never crosses Tauri IPC.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecoveryCounterSnapshot {
    pub recovery_attempts: u32,
    pub recovery_successes: u32,
    pub lag_windows_entered: u32,
    pub watchdog_expiries: u32,
    pub contained_panics: u32,
}

/// The single process-managed runtime handle. `Arc`-wrapped by Tauri's
/// managed state so background work (the async command's blocking task and
/// the monitor's/worker's observers) can hold a cheap clone.
pub struct RuntimeManager<R: Runtime = tauri::Wry> {
    state: Mutex<RuntimeState>,
    backend: Arc<dyn MicrophoneBackend>,
    system_backend: Arc<dyn SystemAudioBackend>,
    asr_loader: Arc<dyn AsrModelLoader>,
    asr_model: Mutex<Option<Arc<dyn RecognizerFactory>>>,
    session: Mutex<Option<ActiveSession>>,
    generation: AtomicU64,
    session_counter: AtomicU64,
    recovery_policy: RecoveryPolicy,
    counters: RecoveryCounters,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> RuntimeManager<R> {
    pub fn new(
        backend: Arc<dyn MicrophoneBackend>,
        system_backend: Arc<dyn SystemAudioBackend>,
        asr_loader: Arc<dyn AsrModelLoader>,
    ) -> Self {
        Self::new_with_policy(
            backend,
            system_backend,
            asr_loader,
            RecoveryPolicy::production(),
        )
    }

    /// Constructor variant with an injectable recovery policy. Production
    /// always uses `RecoveryPolicy::production()` via [`Self::new`]; tests
    /// use a millisecond-scale policy so recovery/backoff/watchdog
    /// behavior can be exercised without slow real-time sleeps.
    pub(crate) fn new_with_policy(
        backend: Arc<dyn MicrophoneBackend>,
        system_backend: Arc<dyn SystemAudioBackend>,
        asr_loader: Arc<dyn AsrModelLoader>,
        recovery_policy: RecoveryPolicy,
    ) -> Self {
        Self {
            state: Mutex::new(RuntimeState::new()),
            backend,
            system_backend,
            asr_loader,
            asr_model: Mutex::new(None),
            session: Mutex::new(None),
            generation: AtomicU64::new(0),
            session_counter: AtomicU64::new(0),
            recovery_policy,
            counters: RecoveryCounters::default(),
            _runtime: std::marker::PhantomData,
        }
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, RuntimeState>, RuntimeError> {
        self.state
            .lock()
            .map_err(|_| RuntimeError::internal("runtime state mutex was poisoned"))
    }

    fn lock_session(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<ActiveSession>>, RuntimeError> {
        self.session
            .lock()
            .map_err(|_| RuntimeError::internal("session mutex was poisoned"))
    }

    fn lock_asr_model(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<Arc<dyn RecognizerFactory>>>, RuntimeError> {
        self.asr_model
            .lock()
            .map_err(|_| RuntimeError::internal("asr model cache mutex was poisoned"))
    }

    pub fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        Ok(self.lock_state()?.snapshot())
    }

    /// Per-source lifecycle counters recorded for evidence (Spec 10). Not
    /// part of the frozen IPC contract; never persisted.
    pub fn recovery_counters(&self, source: AudioSource) -> RecoveryCounterSnapshot {
        match source {
            AudioSource::Microphone => RecoveryCounterSnapshot {
                recovery_attempts: self.counters.mic_recovery_attempts.load(Ordering::Relaxed),
                recovery_successes: self.counters.mic_recovery_successes.load(Ordering::Relaxed),
                lag_windows_entered: self
                    .counters
                    .mic_lag_windows_entered
                    .load(Ordering::Relaxed),
                watchdog_expiries: self.counters.mic_watchdog_expiries.load(Ordering::Relaxed),
                contained_panics: self.counters.mic_contained_panics.load(Ordering::Relaxed),
            },
            AudioSource::System => RecoveryCounterSnapshot {
                recovery_attempts: self.counters.sys_recovery_attempts.load(Ordering::Relaxed),
                recovery_successes: self.counters.sys_recovery_successes.load(Ordering::Relaxed),
                lag_windows_entered: self
                    .counters
                    .sys_lag_windows_entered
                    .load(Ordering::Relaxed),
                watchdog_expiries: self.counters.sys_watchdog_expiries.load(Ordering::Relaxed),
                contained_panics: self.counters.sys_contained_panics.load(Ordering::Relaxed),
            },
        }
    }

    fn emit_capture_status(&self, app: &AppHandle<R>) -> Result<(), RuntimeError> {
        let snapshot = self.snapshot()?;
        events::emit_capture_status(app, snapshot)
    }

    fn emit_audio_status(
        &self,
        app: &AppHandle<R>,
        source: TranscriptSource,
    ) -> Result<(), RuntimeError> {
        let snapshot = self.snapshot()?;
        events::emit_audio_status(app, source, snapshot)
    }

    fn emit_model_status(&self, app: &AppHandle<R>) -> Result<(), RuntimeError> {
        let snapshot = self.snapshot()?;
        events::emit_model_status(app, snapshot)
    }

    fn set_model_status(
        &self,
        app: &AppHandle<R>,
        status: ModelStatus,
    ) -> Result<(), RuntimeError> {
        self.lock_state()?.apply_model_status(status)?;
        self.emit_model_status(app)
    }

    /// Launch-time, metadata-only model presence report.
    pub fn initialize_model_presence(&self, app: &AppHandle<R>) {
        let status = match resolve_model_dir(app) {
            Ok(dir) => match crate::asr::manifest::check_presence(&dir, &DEVELOPMENT_MANIFEST) {
                Ok(()) => ModelStatus::Ready {
                    model_id: DEVELOPMENT_MANIFEST.model_id.to_string(),
                },
                Err(_) => ModelStatus::Missing,
            },
            Err(_) => ModelStatus::Missing,
        };
        if let Ok(mut state) = self.state.lock() {
            let _ = state.apply_model_status(status);
        }
    }

    /// Launch-time system audio probe report.
    pub fn initialize_system_audio_presence(&self, app: &AppHandle<R>) {
        let status = match self.system_backend.probe() {
            Ok(()) => AudioSourceStatus::Idle,
            Err(audio_error) => {
                let runtime_err = map_audio_error(audio_error);
                AudioSourceStatus::Unavailable { error: runtime_err }
            }
        };
        if let Ok(mut state) = self.state.lock() {
            let _ = state.apply_system_audio_status(status);
        }
        let _ = self.emit_audio_status(app, TranscriptSource::System);
    }

    /// Lists real devices and reconciles microphone availability.
    pub fn list_microphones(
        &self,
        app: &AppHandle<R>,
    ) -> Result<Vec<MicrophoneDevice>, RuntimeError> {
        let devices = self.backend.list_devices().map_err(map_audio_error)?;
        let dtos: Vec<MicrophoneDevice> = devices
            .iter()
            .map(|device| MicrophoneDevice {
                id: device.id.to_string(),
                label: device.label.clone(),
                is_default: device.is_default,
            })
            .collect();

        let mut changed = false;
        {
            let mut state = self.lock_state()?;
            let currently_unavailable = matches!(
                state.snapshot().microphone,
                AudioSourceStatus::Unavailable { .. }
            );
            let has_devices = !dtos.is_empty();

            if currently_unavailable && has_devices {
                state.apply_microphone_status(AudioSourceStatus::Idle)?;
                changed = true;
            } else if !currently_unavailable
                && !has_devices
                && state.capture_status() == CaptureStatus::Idle
            {
                state.apply_microphone_status(AudioSourceStatus::Unavailable {
                    error: RuntimeError::new(
                        RuntimeErrorCode::MicrophoneUnavailable,
                        "no microphone is available",
                        true,
                    ),
                })?;
                changed = true;
            }
        }
        if changed {
            self.emit_audio_status(app, TranscriptSource::Microphone)?;
        }

        Ok(dtos)
    }

    async fn ensure_asr_model_loaded(
        self: &Arc<Self>,
        app: &AppHandle<R>,
    ) -> Result<Arc<dyn RecognizerFactory>, AsrError> {
        let cached = self
            .lock_asr_model()
            .map_err(|_| {
                AsrError::new(AsrErrorKind::Internal, "asr model cache mutex was poisoned")
            })?
            .clone();
        if let Some(factory) = cached {
            return Ok(factory);
        }

        let _ = self.set_model_status(
            app,
            ModelStatus::Loading {
                model_id: Some(DEVELOPMENT_MANIFEST.model_id.to_string()),
            },
        );

        let dir = resolve_model_dir(app)?;
        let loader = self.asr_loader.clone();
        let load_result = tauri::async_runtime::spawn_blocking(move || loader.load(&dir)).await;

        let result = match load_result {
            Ok(inner) => inner,
            Err(_join_error) => Err(AsrError::new(
                AsrErrorKind::Internal,
                "model load task panicked",
            )),
        };

        match &result {
            Ok(factory) => {
                *self
                    .lock_asr_model()
                    .map_err(|e| AsrError::new(AsrErrorKind::Internal, e.message))? =
                    Some(factory.clone());
                let _ = self.set_model_status(
                    app,
                    ModelStatus::Ready {
                        model_id: DEVELOPMENT_MANIFEST.model_id.to_string(),
                    },
                );
            }
            Err(error) => {
                let _ = self.set_model_status(app, model_status_for_asr_error(error));
            }
        }

        result
    }

    /// Starts one microphone source: acquires the device, opens a fresh
    /// ASR stream, and spawns its monitor/worker. Self-contained: on its
    /// own failure it releases whatever it already created before
    /// returning `Err`. Used both by the initial atomic `start_capture`
    /// (`segment_index_start`/`min_start_offset_ms` both `0`) and by a
    /// Spec 10 recovery restart (continued index/clamped offset).
    #[allow(clippy::too_many_arguments)]
    async fn start_microphone_source(
        self: &Arc<Self>,
        app: &AppHandle<R>,
        generation: u64,
        device_id: &str,
        factory: &Arc<dyn RecognizerFactory>,
        session_id: u64,
        session_clock_origin: Instant,
        segment_index_start: u64,
        min_start_offset_ms: u64,
    ) -> Result<ActiveMicrophoneSession, RuntimeError> {
        let backend = self.backend.clone();
        let device_for_blocking = device_id.to_string();
        let start_result =
            tauri::async_runtime::spawn_blocking(move || backend.start(Some(&device_for_blocking)))
                .await;

        let capture = match start_result {
            Ok(Ok(capture)) => capture,
            Ok(Err(audio_error)) => return Err(map_audio_error(audio_error)),
            Err(_join_error) => {
                return Err(RuntimeError::internal("microphone start task panicked")
                    .with_source(TranscriptSource::Microphone));
            }
        };

        if self.generation.load(Ordering::SeqCst) != generation {
            let MicrophoneCaptureHandle { mut session, .. } = capture;
            let _ = session.stop();
            return Err(RuntimeError::capture_not_active());
        }

        let format = capture.format;
        let candidate_offset_ms = session_clock_origin.elapsed().as_millis() as u64;
        let mic_start_offset_ms = candidate_offset_ms.max(min_start_offset_ms);
        let stream = match factory.open_stream(format) {
            Ok(stream) => stream,
            Err(asr_error) => {
                let MicrophoneCaptureHandle { mut session, .. } = capture;
                let _ = session.stop();
                return Err(map_asr_error(
                    &asr_error,
                    Some(TranscriptSource::Microphone),
                ));
            }
        };

        let chunk_capacity =
            crate::asr::chunk_pool::asr_chunk_capacity_for_rate(format.sample_rate_hz.get());
        let (asr_producer, asr_consumer) = build_asr_pool(chunk_capacity);
        let asr_feeder = AsrChunkFeeder::new(asr_producer, chunk_capacity);

        let asr_observer: Arc<dyn AsrWorkerObserver> = Arc::new(RuntimeAsrObserver {
            manager: self.clone(),
            app: app.clone(),
            generation,
        });
        let asr_worker = AsrWorker::spawn(
            AudioSource::Microphone,
            asr_consumer,
            stream,
            format.sample_rate_hz.get(),
            session_id,
            mic_start_offset_ms,
            segment_index_start,
            asr_observer,
        );

        let monitor_observer: Arc<dyn MicrophoneMonitorObserver> =
            Arc::new(RuntimeMicrophoneMonitorObserver {
                manager: self.clone(),
                app: app.clone(),
                generation,
            });
        let monitor = MicrophoneMonitor::spawn(capture, monitor_observer, asr_feeder);
        Ok(ActiveMicrophoneSession {
            monitor,
            asr_worker,
        })
    }

    /// Starts the system-audio source, symmetric to
    /// [`Self::start_microphone_source`].
    #[allow(clippy::too_many_arguments)]
    async fn start_system_source(
        self: &Arc<Self>,
        app: &AppHandle<R>,
        generation: u64,
        factory: &Arc<dyn RecognizerFactory>,
        session_id: u64,
        session_clock_origin: Instant,
        segment_index_start: u64,
        min_start_offset_ms: u64,
    ) -> Result<ActiveSystemSession, RuntimeError> {
        let sys_rate = self.system_backend.sample_rate();
        let sys_format = PcmFormat {
            sample_rate_hz: std::num::NonZeroU32::new(sys_rate)
                .unwrap_or_else(|| std::num::NonZeroU32::new(48000).unwrap()),
            channels: std::num::NonZeroU16::new(1).unwrap(),
        };

        let sys_stream = factory
            .open_stream(sys_format)
            .map_err(|asr_error| map_asr_error(&asr_error, Some(TranscriptSource::System)))?;

        let candidate_offset_ms = session_clock_origin.elapsed().as_millis() as u64;
        let sys_start_offset_ms = candidate_offset_ms.max(min_start_offset_ms);
        let (sys_pool_producer, sys_pool_consumer) = build_pool(block_capacity_for_rate(sys_rate));

        let sys_chunk_capacity = asr_chunk_capacity_for_rate(sys_rate);
        let (sys_asr_producer, sys_asr_consumer) = build_asr_pool(sys_chunk_capacity);
        let sys_asr_feeder = AsrChunkFeeder::new(sys_asr_producer, sys_chunk_capacity);

        let asr_observer: Arc<dyn AsrWorkerObserver> = Arc::new(RuntimeAsrObserver {
            manager: self.clone(),
            app: app.clone(),
            generation,
        });
        let mut asr_worker = AsrWorker::spawn(
            AudioSource::System,
            sys_asr_consumer,
            sys_stream,
            sys_rate,
            session_id,
            sys_start_offset_ms,
            segment_index_start,
            asr_observer,
        );

        let manager_for_err = self.clone();
        let app_for_err = app.clone();
        let on_error: Box<dyn FnMut(AudioError) + Send> = Box::new(move |audio_error| {
            manager_for_err.handle_source_fault(
                &app_for_err,
                generation,
                AudioSource::System,
                audio_error.kind,
            );
        });

        let sys_backend = self.system_backend.clone();
        let start_result = tauri::async_runtime::spawn_blocking(move || {
            sys_backend.start(Box::new(sys_pool_producer), on_error)
        })
        .await;

        let sys_session = match start_result {
            Ok(Ok(session)) => session,
            Ok(Err(audio_error)) => {
                drop(sys_asr_feeder);
                asr_worker.stop();
                return Err(map_audio_error(audio_error));
            }
            Err(_join_error) => {
                drop(sys_asr_feeder);
                asr_worker.stop();
                return Err(RuntimeError::internal("system audio start task panicked")
                    .with_source(TranscriptSource::System));
            }
        };

        if self.generation.load(Ordering::SeqCst) != generation {
            let mut session = sys_session;
            let _ = session.stop();
            drop(sys_asr_feeder);
            asr_worker.stop();
            return Err(RuntimeError::capture_not_active());
        }

        let monitor_observer: Arc<dyn SystemAudioMonitorObserver> =
            Arc::new(RuntimeSystemAudioMonitorObserver {
                manager: self.clone(),
                app: app.clone(),
                generation,
            });
        let monitor = SystemAudioMonitor::spawn(
            sys_session,
            sys_pool_consumer,
            monitor_observer,
            sys_asr_feeder,
        );
        Ok(ActiveSystemSession {
            monitor,
            asr_worker,
        })
    }

    /// Starts dual-source capture with atomic start semantics.
    pub async fn start_capture(
        self: &Arc<Self>,
        app: AppHandle<R>,
        microphone_device_id: Option<String>,
        system_audio_enabled: bool,
    ) -> Result<CaptureStatus, RuntimeError> {
        if microphone_device_id.is_none() && !system_audio_enabled {
            return Err(RuntimeError::invalid_request(
                "start_capture requires a microphone device or system audio to be enabled",
            ));
        }
        if let Some(device_id) = &microphone_device_id {
            if device_id.trim().is_empty() {
                return Err(RuntimeError::invalid_request(
                    "microphoneDeviceId must contain at least one non-whitespace character",
                ));
            }
        }

        let generation = {
            let mut state = self.lock_state()?;
            if matches!(
                state.capture_status(),
                CaptureStatus::Starting | CaptureStatus::Listening | CaptureStatus::Stopping
            ) {
                return Err(RuntimeError::new(
                    RuntimeErrorCode::CaptureAlreadyActive,
                    "capture is already starting, listening, or stopping",
                    true,
                ));
            }
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            state.apply_capture_status(CaptureStatus::Starting)?;
            if microphone_device_id.is_some() {
                state.apply_microphone_status(AudioSourceStatus::Starting {
                    device_id: microphone_device_id.clone(),
                })?;
            }
            if system_audio_enabled {
                state.apply_system_audio_status(AudioSourceStatus::Starting { device_id: None })?;
            }
            generation
        };
        self.emit_capture_status(&app)?;
        if microphone_device_id.is_some() {
            self.emit_audio_status(&app, TranscriptSource::Microphone)?;
        }
        if system_audio_enabled {
            self.emit_audio_status(&app, TranscriptSource::System)?;
        }

        let factory = match self.ensure_asr_model_loaded(&app).await {
            Ok(factory) => factory,
            Err(asr_error) => {
                return self
                    .fail_starting(&app, generation, map_asr_error(&asr_error, None))
                    .await;
            }
        };

        if self.generation.load(Ordering::SeqCst) != generation {
            return Err(RuntimeError::capture_not_active());
        }

        let session_clock_origin = Instant::now();
        let session_id = self.session_counter.fetch_add(1, Ordering::SeqCst) + 1;

        // Starting watchdog (Spec 10): bounds the automatic, non-
        // interactive part of a start (device acquisition, ASR stream
        // open, monitor/worker construction). The system-audio
        // probe/start call below is deliberately left unwatched — it is
        // the one call that can legitimately block on a live, user-driven
        // macOS Screen Recording permission dialog. See
        // `docs/lifecycle-policy.md` section 5.
        let watchdog_completed = Arc::new(AtomicBool::new(false));
        if microphone_device_id.is_some() {
            self.arm_starting_watchdog(app.clone(), generation, watchdog_completed.clone());
        }

        let mut active_mic: Option<ActiveMicrophoneSession> = None;
        if let Some(device_id) = &microphone_device_id {
            match self
                .start_microphone_source(
                    &app,
                    generation,
                    device_id,
                    &factory,
                    session_id,
                    session_clock_origin,
                    0,
                    0,
                )
                .await
            {
                Ok(session) => active_mic = Some(session),
                Err(error) => {
                    watchdog_completed.store(true, Ordering::Release);
                    return self.fail_starting(&app, generation, error).await;
                }
            }
        }
        watchdog_completed.store(true, Ordering::Release);

        let mut active_sys: Option<ActiveSystemSession> = None;
        if system_audio_enabled {
            if let Err(audio_error) = self.system_backend.probe() {
                if let Some(mut mic) = active_mic.take() {
                    mic.monitor.stop();
                    mic.asr_worker.stop();
                }
                return self
                    .fail_starting(&app, generation, map_audio_error(audio_error))
                    .await;
            }

            match self
                .start_system_source(
                    &app,
                    generation,
                    &factory,
                    session_id,
                    session_clock_origin,
                    0,
                    0,
                )
                .await
            {
                Ok(session) => active_sys = Some(session),
                Err(error) => {
                    if let Some(mut mic) = active_mic.take() {
                        mic.monitor.stop();
                        mic.asr_worker.stop();
                    }
                    return self.fail_starting(&app, generation, error).await;
                }
            }
        }

        if self.generation.load(Ordering::SeqCst) != generation {
            if let Some(mut mic) = active_mic.take() {
                mic.monitor.stop();
                mic.asr_worker.stop();
            }
            if let Some(mut sys) = active_sys.take() {
                sys.monitor.stop();
                sys.asr_worker.stop();
            }
            return Err(RuntimeError::capture_not_active());
        }

        *self.lock_session()? = Some(ActiveSession {
            mic: active_mic,
            sys: active_sys,
            requested_microphone_device_id: microphone_device_id.clone(),
            session_id,
            session_clock_origin,
            mic_recovery: RecoveryState::new(),
            sys_recovery: RecoveryState::new(),
        });

        let status = {
            let mut state = self.lock_state()?;
            if microphone_device_id.is_some() {
                state.apply_microphone_status(AudioSourceStatus::Capturing {
                    device_id: microphone_device_id.clone(),
                    activity: Activity::Waiting,
                })?;
            }
            if system_audio_enabled {
                state.apply_system_audio_status(AudioSourceStatus::Capturing {
                    device_id: None,
                    activity: Activity::Waiting,
                })?;
            }
            state.apply_capture_status(CaptureStatus::Listening)?
        };

        if microphone_device_id.is_some() {
            self.emit_audio_status(&app, TranscriptSource::Microphone)?;
        }
        if system_audio_enabled {
            self.emit_audio_status(&app, TranscriptSource::System)?;
        }
        self.emit_capture_status(&app)?;

        Ok(status.capture_status)
    }

    /// Backwards compatibility helper for microphone-only start.
    pub async fn start_microphone(
        self: &Arc<Self>,
        app: AppHandle<R>,
        device_id: Option<String>,
    ) -> Result<CaptureStatus, RuntimeError> {
        self.start_capture(app, device_id, false).await
    }

    /// Spawns a watchdog thread that force-fails the current `Starting`
    /// attempt if it has not signaled completion within
    /// `recovery_policy.starting_watchdog`. Uses `compare_exchange` on
    /// `generation` so it only "wins" (and commits a terminal state) if
    /// nothing else — a real Stop, a new Start, or shutdown — already
    /// moved the session on for its own reason.
    fn arm_starting_watchdog(
        self: &Arc<Self>,
        app: AppHandle<R>,
        generation: u64,
        completed: Arc<AtomicBool>,
    ) {
        let manager = self.clone();
        let duration = self.recovery_policy.starting_watchdog;
        thread::Builder::new()
            .name("mistaken-starting-watchdog".into())
            .spawn(move || {
                thread::sleep(duration);
                if completed.load(Ordering::Acquire) {
                    return;
                }
                if manager
                    .generation
                    .compare_exchange(
                        generation,
                        generation.wrapping_add(1),
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    )
                    .is_err()
                {
                    return;
                }
                manager
                    .counters
                    .record_watchdog_expiry(AudioSource::Microphone);
                let error = RuntimeError::new(
                    RuntimeErrorCode::CaptureStartFailed,
                    "starting did not complete within the watchdog bound",
                    true,
                );
                if let Ok(mut state) = manager.state.lock() {
                    let _ = state.apply_microphone_status(AudioSourceStatus::Error {
                        error: error.clone(),
                    });
                    let _ = state.apply_capture_status(CaptureStatus::Error);
                }
                let _ = manager.emit_audio_status(&app, TranscriptSource::Microphone);
                let _ = manager.emit_capture_status(&app);
                let _ = events::emit_capture_error(&app, &error);
            })
            .expect("failed to spawn starting watchdog thread");
    }

    /// Spawns a watchdog thread that force-commits `Idle` if a `stop_capture`
    /// teardown has not signaled completion within
    /// `recovery_policy.stopping_watchdog`. The underlying blocking
    /// teardown task, if still running, is not force-killed (Rust cannot
    /// safely kill a native OS thread) but remains leak-free: it still
    /// eventually joins and releases every resource on its own.
    fn arm_stopping_watchdog(self: &Arc<Self>, app: AppHandle<R>, completed: Arc<AtomicBool>) {
        let manager = self.clone();
        let duration = self.recovery_policy.stopping_watchdog;
        thread::Builder::new()
            .name("mistaken-stopping-watchdog".into())
            .spawn(move || {
                thread::sleep(duration);
                if completed.load(Ordering::Acquire) {
                    return;
                }
                let error = RuntimeError::new(
                    RuntimeErrorCode::CaptureStopFailed,
                    "stopping did not complete within the watchdog bound",
                    false,
                );
                if let Ok(mut state) = manager.state.lock() {
                    let _ = state.apply_capture_status(CaptureStatus::Idle);
                }
                let _ = manager.emit_capture_status(&app);
                let _ = events::emit_capture_error(&app, &error);
            })
            .expect("failed to spawn stopping watchdog thread");
    }

    async fn fail_starting(
        &self,
        app: &AppHandle<R>,
        generation: u64,
        error: RuntimeError,
    ) -> Result<CaptureStatus, RuntimeError> {
        if self.generation.load(Ordering::SeqCst) == generation {
            let source = error.source;
            {
                let mut state = self.lock_state()?;
                match source {
                    Some(TranscriptSource::System) => {
                        state.apply_system_audio_status(AudioSourceStatus::Error {
                            error: error.clone(),
                        })?;
                        if matches!(
                            state.snapshot().microphone,
                            AudioSourceStatus::Starting { .. }
                        ) {
                            state.apply_microphone_status(AudioSourceStatus::Idle)?;
                        }
                    }
                    _ => {
                        state.apply_microphone_status(AudioSourceStatus::Error {
                            error: error.clone(),
                        })?;
                        if matches!(
                            state.snapshot().system_audio,
                            AudioSourceStatus::Starting { .. }
                        ) {
                            state.apply_system_audio_status(AudioSourceStatus::Idle)?;
                        }
                    }
                }
                state.apply_capture_status(CaptureStatus::Error)?;
            }
            if let Some(src) = source {
                self.emit_audio_status(app, src)?;
            } else {
                self.emit_audio_status(app, TranscriptSource::Microphone)?;
            }
            self.emit_capture_status(app)?;
        }
        let _ = events::emit_capture_error(app, &error);
        Err(error)
    }

    /// Stops dual-source capture.
    pub async fn stop_capture(
        self: &Arc<Self>,
        app: AppHandle<R>,
    ) -> Result<CaptureStatus, RuntimeError> {
        self.generation.fetch_add(1, Ordering::SeqCst);

        if self.lock_state()?.capture_status() == CaptureStatus::Idle {
            return Err(RuntimeError::capture_not_active());
        }

        let existing = self.lock_session()?.take();

        self.lock_state()?
            .apply_capture_status(CaptureStatus::Stopping)?;
        self.emit_capture_status(&app)?;

        if let Some(mut active) = existing {
            let completed = Arc::new(AtomicBool::new(false));
            self.arm_stopping_watchdog(app.clone(), completed.clone());
            tauri::async_runtime::spawn_blocking(move || {
                active.stop();
            })
            .await
            .map_err(|_| RuntimeError::internal("capture stop task panicked"))?;
            completed.store(true, Ordering::Release);
        }

        let status = {
            let mut state = self.lock_state()?;
            let snap = state.snapshot();
            if !matches!(snap.microphone, AudioSourceStatus::Unavailable { .. }) {
                state.apply_microphone_status(AudioSourceStatus::Idle)?;
            }
            if !matches!(snap.system_audio, AudioSourceStatus::Unavailable { .. }) {
                state.apply_system_audio_status(AudioSourceStatus::Idle)?;
            }
            state.apply_capture_status(CaptureStatus::Idle)?
        };
        self.emit_audio_status(&app, TranscriptSource::Microphone)?;
        self.emit_audio_status(&app, TranscriptSource::System)?;
        self.emit_capture_status(&app)?;
        Ok(status.capture_status)
    }

    /// Backwards compatibility helper for microphone-only stop.
    pub async fn stop_microphone(
        self: &Arc<Self>,
        app: AppHandle<R>,
    ) -> Result<CaptureStatus, RuntimeError> {
        self.stop_capture(app).await
    }

    /// Single idempotent native shutdown (Spec 10). Releases both sources
    /// synchronously. Never waits for a React listener, an IPC response,
    /// or a webview state — the window may already be gone. Safe to call
    /// more than once, from any thread, including a signal handler's
    /// dedicated watcher thread. Bumps generation first so any pending
    /// recovery backoff is cancelled immediately.
    pub fn shutdown(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut guard) = self.session.lock() {
            if let Some(mut active) = guard.take() {
                active.stop();
            }
        }
    }

    /// Handles mid-session fault on a single source (called by observers
    /// with only an `AudioErrorKind`), keeping the survivor alive.
    fn handle_source_fault(
        self: &Arc<Self>,
        app: &AppHandle<R>,
        generation: u64,
        source: AudioSource,
        error_kind: AudioErrorKind,
    ) {
        let audio_err = AudioError {
            source,
            kind: error_kind,
        };
        self.handle_source_runtime_error(app, generation, source, map_audio_error(audio_err));
    }

    /// Core Spec 10 classification + recovery/terminal dispatch. Every
    /// mid-session source fault — from a monitor, an ASR worker, or a
    /// failed recovery attempt itself — funnels through here.
    fn handle_source_runtime_error(
        self: &Arc<Self>,
        app: &AppHandle<R>,
        generation: u64,
        source: AudioSource,
        runtime_err: RuntimeError,
    ) {
        if self.generation.load(Ordering::SeqCst) != generation {
            return;
        }

        let decision = classify_recovery(runtime_err.code);

        let mut next_attempt: Option<u8> = None;
        let to_abandon: Option<Box<dyn FnOnce() + Send>> = {
            let Ok(mut guard) = self.session.lock() else {
                return;
            };
            let Some(active) = guard.as_mut() else {
                return;
            };

            let recovery = match source {
                AudioSource::Microphone => &mut active.mic_recovery,
                AudioSource::System => &mut active.sys_recovery,
            };
            if let Some(healthy_since) = recovery.healthy_since.take() {
                if healthy_since.elapsed() >= self.recovery_policy.healthy_reset {
                    recovery.attempts_used = 0;
                }
            }

            let abandon_fn: Option<Box<dyn FnOnce() + Send>> = match source {
                AudioSource::Microphone => active.mic.take().map(|mut m| {
                    Box::new(move || {
                        m.monitor.abandon();
                        m.asr_worker.stop();
                    }) as Box<dyn FnOnce() + Send>
                }),
                AudioSource::System => active.sys.take().map(|mut s| {
                    Box::new(move || {
                        s.monitor.abandon();
                        s.asr_worker.stop();
                    }) as Box<dyn FnOnce() + Send>
                }),
            };

            if decision == RecoveryDecision::Recoverable
                && (match source {
                    AudioSource::Microphone => active.mic_recovery.attempts_used,
                    AudioSource::System => active.sys_recovery.attempts_used,
                }) < self.recovery_policy.max_attempts
            {
                let recovery = match source {
                    AudioSource::Microphone => &mut active.mic_recovery,
                    AudioSource::System => &mut active.sys_recovery,
                };
                recovery.attempts_used += 1;
                next_attempt = Some(recovery.attempts_used);
            }
            abandon_fn
        };

        if let Some(abandon_fn) = to_abandon {
            tauri::async_runtime::spawn_blocking(abandon_fn);
        }

        if let Some(attempt) = next_attempt {
            self.counters.record_attempt(source);
            let max = self.recovery_policy.max_attempts;
            let mut recovering_err = runtime_err;
            recovering_err.message = format!(
                "Reconnecting {}… attempt {} of {}",
                source_label(source),
                attempt,
                max
            );
            recovering_err.recoverable = true;

            let device_id = if source == AudioSource::Microphone {
                self.lock_session().ok().and_then(|guard| {
                    guard
                        .as_ref()
                        .and_then(|active| active.requested_microphone_device_id.clone())
                })
            } else {
                None
            };
            if let Ok(mut state) = self.state.lock() {
                let status = AudioSourceStatus::Starting { device_id };
                let _ = match source {
                    AudioSource::Microphone => state.apply_microphone_status(status),
                    AudioSource::System => state.apply_system_audio_status(status),
                };
            }
            let _ = self.emit_audio_status(app, TranscriptSource::from(source));
            let _ = events::emit_capture_error(app, &recovering_err);

            self.schedule_recovery(app.clone(), generation, source, attempt);
            return;
        }

        let mut terminal_err = runtime_err;
        if decision == RecoveryDecision::Recoverable {
            terminal_err.message =
                format!("{} Automatic reconnection stopped.", terminal_err.message);
        }
        self.commit_terminal(app, source, terminal_err);
    }

    /// Commits a source-terminal state: `Error` for that source, and
    /// `CaptureStatus::Error` only once every requested source has
    /// failed (survivor continuity, Spec 09 AC 17, unchanged by Spec 10).
    fn commit_terminal(&self, app: &AppHandle<R>, source: AudioSource, runtime_err: RuntimeError) {
        let mut all_failed = false;
        if let Ok(mut state) = self.state.lock() {
            match source {
                AudioSource::Microphone => {
                    let _ = state.apply_microphone_status(AudioSourceStatus::Error {
                        error: runtime_err.clone(),
                    });
                }
                AudioSource::System => {
                    let _ = state.apply_system_audio_status(AudioSourceStatus::Error {
                        error: runtime_err.clone(),
                    });
                }
            }

            let has_survivor = if let Ok(guard) = self.session.lock() {
                guard
                    .as_ref()
                    .map(|active| active.mic.is_some() || active.sys.is_some())
                    .unwrap_or(false)
            } else {
                false
            };

            if !has_survivor {
                all_failed = true;
                let _ = state.apply_capture_status(CaptureStatus::Error);
            }
        }

        let _ = self.emit_audio_status(app, TranscriptSource::from(source));
        if all_failed {
            let _ = self.emit_capture_status(app);
        }
        let _ = events::emit_capture_error(app, &runtime_err);
    }

    /// Spawns the interruptible-backoff-then-restart recovery attempt on
    /// its own dedicated, panic-contained thread (never the audio/ASR
    /// threads of the surviving source).
    fn schedule_recovery(
        self: &Arc<Self>,
        app: AppHandle<R>,
        generation: u64,
        source: AudioSource,
        attempt: u8,
    ) {
        let manager = self.clone();
        let name = match source {
            AudioSource::Microphone => "mistaken-supervisor-mic",
            AudioSource::System => "mistaken-supervisor-sys",
        };
        thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let panic_manager = manager.clone();
                let panic_app = app.clone();
                let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
                    manager.run_recovery_attempt(&app, generation, source, attempt);
                }));
                if outcome.is_err() {
                    panic_manager.counters.record_panic(source);
                    panic_manager.commit_terminal(
                        &panic_app,
                        source,
                        RuntimeError::internal("recovery supervisor panicked (contained)")
                            .with_source(TranscriptSource::from(source)),
                    );
                }
            })
            .expect("failed to spawn recovery supervisor thread");
    }

    /// Runs on the dedicated supervisor thread: interruptible backoff,
    /// then a restart attempt at the newly negotiated device/format with
    /// the continued segment index and clamped start offset. Cancellation
    /// (Stop/new Start/shutdown) is detected purely via a generation
    /// mismatch, reusing the same mechanism that already gates every
    /// observer callback.
    fn run_recovery_attempt(
        self: &Arc<Self>,
        app: &AppHandle<R>,
        generation: u64,
        source: AudioSource,
        attempt: u8,
    ) {
        let backoff = self.recovery_policy.backoff_for_attempt(attempt);
        let tick = self.recovery_policy.wait_tick;
        let completed = interruptible_wait(backoff, tick, || {
            self.generation.load(Ordering::SeqCst) != generation
        });
        if !completed || self.generation.load(Ordering::SeqCst) != generation {
            return;
        }

        let context = {
            let Ok(guard) = self.session.lock() else {
                return;
            };
            let Some(active) = guard.as_ref() else {
                return;
            };
            let Ok(model_guard) = self.asr_model.lock() else {
                return;
            };
            let Some(factory) = model_guard.clone() else {
                return;
            };
            let recovery = match source {
                AudioSource::Microphone => &active.mic_recovery,
                AudioSource::System => &active.sys_recovery,
            };
            (
                factory,
                active.session_id,
                active.session_clock_origin,
                active.requested_microphone_device_id.clone(),
                recovery.next_segment_index,
                recovery.last_ended_at_ms,
            )
        };
        let (factory, session_id, clock_origin, device_id, segment_index_start, min_offset_ms) =
            context;

        let result: Result<RestartedSource, RuntimeError> = match source {
            AudioSource::Microphone => {
                let Some(device_id) = device_id.clone() else {
                    return;
                };
                tauri::async_runtime::block_on(self.start_microphone_source(
                    app,
                    generation,
                    &device_id,
                    &factory,
                    session_id,
                    clock_origin,
                    segment_index_start,
                    min_offset_ms,
                ))
                .map(RestartedSource::Mic)
            }
            AudioSource::System => tauri::async_runtime::block_on(self.start_system_source(
                app,
                generation,
                &factory,
                session_id,
                clock_origin,
                segment_index_start,
                min_offset_ms,
            ))
            .map(RestartedSource::Sys),
        };

        if self.generation.load(Ordering::SeqCst) != generation {
            // Cancelled while restarting: whatever `result` created (if
            // anything) is released by its own Drop/RAII when this
            // binding goes out of scope; Stop/shutdown already own the
            // authoritative state.
            return;
        }

        match result {
            Ok(RestartedSource::Mic(active_mic)) => {
                let installed = if let Ok(mut guard) = self.session.lock() {
                    if let Some(active) = guard.as_mut() {
                        active.mic = Some(active_mic);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !installed {
                    return;
                }
                self.counters.record_success(source);
                if let Ok(mut state) = self.state.lock() {
                    let _ = state.apply_microphone_status(AudioSourceStatus::Capturing {
                        device_id,
                        activity: Activity::Waiting,
                    });
                }
                let _ = self.emit_audio_status(app, TranscriptSource::Microphone);
            }
            Ok(RestartedSource::Sys(active_sys)) => {
                let installed = if let Ok(mut guard) = self.session.lock() {
                    if let Some(active) = guard.as_mut() {
                        active.sys = Some(active_sys);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !installed {
                    return;
                }
                self.counters.record_success(source);
                if let Ok(mut state) = self.state.lock() {
                    let _ = state.apply_system_audio_status(AudioSourceStatus::Capturing {
                        device_id: None,
                        activity: Activity::Waiting,
                    });
                }
                let _ = self.emit_audio_status(app, TranscriptSource::System);
            }
            Err(runtime_err) => {
                self.handle_source_runtime_error(app, generation, source, runtime_err);
            }
        }
    }
}

struct RuntimeMicrophoneMonitorObserver<R: Runtime> {
    manager: Arc<RuntimeManager<R>>,
    app: AppHandle<R>,
    generation: u64,
}

impl<R: Runtime> RuntimeMicrophoneMonitorObserver<R> {
    fn is_current(&self) -> bool {
        self.manager.generation.load(Ordering::SeqCst) == self.generation
    }
}

impl<R: Runtime> MicrophoneMonitorObserver for RuntimeMicrophoneMonitorObserver<R> {
    fn on_first_signal(&self) {
        if !self.is_current() {
            return;
        }
        let updated = {
            if let Ok(mut state) = self.manager.state.lock() {
                let current_device_id = match &state.snapshot().microphone {
                    AudioSourceStatus::Capturing { device_id, .. } => device_id.clone(),
                    _ => None,
                };
                state
                    .apply_microphone_status(AudioSourceStatus::Capturing {
                        device_id: current_device_id,
                        activity: Activity::Receiving,
                    })
                    .is_ok()
            } else {
                false
            }
        };
        if let Ok(mut guard) = self.manager.session.lock() {
            if let Some(active) = guard.as_mut() {
                active.mic_recovery.healthy_since = Some(Instant::now());
            }
        }
        if updated {
            let _ = self
                .manager
                .emit_audio_status(&self.app, TranscriptSource::Microphone);
        }
    }

    fn on_overflow(&self) {
        if !self.is_current() {
            return;
        }
        let error = RuntimeError::new(
            RuntimeErrorCode::AudioQueueOverflow,
            "microphone input is delayed; some audio was dropped",
            true,
        )
        .with_source(TranscriptSource::Microphone);
        let _ = events::emit_capture_error(&self.app, &error);
    }

    fn on_fault(&self, error: AudioErrorKind) {
        self.manager.handle_source_fault(
            &self.app,
            self.generation,
            AudioSource::Microphone,
            error,
        );
    }
}

struct RuntimeSystemAudioMonitorObserver<R: Runtime> {
    manager: Arc<RuntimeManager<R>>,
    app: AppHandle<R>,
    generation: u64,
}

impl<R: Runtime> RuntimeSystemAudioMonitorObserver<R> {
    fn is_current(&self) -> bool {
        self.manager.generation.load(Ordering::SeqCst) == self.generation
    }
}

impl<R: Runtime> SystemAudioMonitorObserver for RuntimeSystemAudioMonitorObserver<R> {
    fn on_first_signal(&self) {
        if !self.is_current() {
            return;
        }
        let updated = {
            if let Ok(mut state) = self.manager.state.lock() {
                let current_device_id = match &state.snapshot().system_audio {
                    AudioSourceStatus::Capturing { device_id, .. } => device_id.clone(),
                    _ => None,
                };
                state
                    .apply_system_audio_status(AudioSourceStatus::Capturing {
                        device_id: current_device_id,
                        activity: Activity::Receiving,
                    })
                    .is_ok()
            } else {
                false
            }
        };
        if let Ok(mut guard) = self.manager.session.lock() {
            if let Some(active) = guard.as_mut() {
                active.sys_recovery.healthy_since = Some(Instant::now());
            }
        }
        if updated {
            let _ = self
                .manager
                .emit_audio_status(&self.app, TranscriptSource::System);
        }
    }

    fn on_overflow(&self) {
        if !self.is_current() {
            return;
        }
        let error = RuntimeError::new(
            RuntimeErrorCode::AudioQueueOverflow,
            "system audio input is delayed; some audio was dropped",
            true,
        )
        .with_source(TranscriptSource::System);
        let _ = events::emit_capture_error(&self.app, &error);
    }

    fn on_fault(&self, error: AudioErrorKind) {
        self.manager
            .handle_source_fault(&self.app, self.generation, AudioSource::System, error);
    }
}

struct RuntimeAsrObserver<R: Runtime> {
    manager: Arc<RuntimeManager<R>>,
    app: AppHandle<R>,
    generation: u64,
}

impl<R: Runtime> RuntimeAsrObserver<R> {
    fn is_current(&self) -> bool {
        self.manager.generation.load(Ordering::SeqCst) == self.generation
    }
}

impl<R: Runtime> AsrWorkerObserver for RuntimeAsrObserver<R> {
    fn on_partial(&self, source: AudioSource, segment_id: &str, text: &str, started_at_ms: u64) {
        if !self.is_current() {
            return;
        }
        let segment = TranscriptSegment {
            id: segment_id.to_string(),
            source: TranscriptSource::from(source),
            text: text.to_string(),
            started_at_ms,
            ended_at_ms: None,
            is_final: false,
        };
        let _ = events::emit_transcript_partial(&self.app, &segment);
    }

    fn on_final(
        &self,
        source: AudioSource,
        segment_id: &str,
        text: &str,
        started_at_ms: u64,
        ended_at_ms: u64,
    ) {
        if !self.is_current() {
            return;
        }
        // Recovery-safe identity (Spec 10): keep the per-source segment
        // index and monotonic-offset floor current so a future recovery
        // restart never reuses an id or produces a non-monotonic
        // timestamp for this source.
        if let Ok(mut guard) = self.manager.session.lock() {
            if let Some(active) = guard.as_mut() {
                let recovery = match source {
                    AudioSource::Microphone => &mut active.mic_recovery,
                    AudioSource::System => &mut active.sys_recovery,
                };
                recovery.next_segment_index += 1;
                recovery.last_ended_at_ms = recovery.last_ended_at_ms.max(ended_at_ms);
            }
        }
        let segment = TranscriptSegment {
            id: segment_id.to_string(),
            source: TranscriptSource::from(source),
            text: text.to_string(),
            started_at_ms,
            ended_at_ms: Some(ended_at_ms),
            is_final: true,
        };
        let _ = events::emit_transcript_final(&self.app, &segment);
    }

    fn on_lagging(&self, source: AudioSource) {
        if !self.is_current() {
            return;
        }
        let error = RuntimeError::new(
            RuntimeErrorCode::InferenceLagging,
            "local transcription is falling behind; some audio was dropped",
            true,
        )
        .with_source(TranscriptSource::from(source));
        let _ = events::emit_capture_error(&self.app, &error);
    }

    fn on_lag_changed(&self, source: AudioSource, lagging: bool) {
        if !self.is_current() {
            return;
        }
        if lagging {
            self.manager.counters.record_lag_window(source);
        }
        let message = if lagging {
            format!(
                "{} is transcribing slower than real time. Some audio is being skipped.",
                source_label_capitalized(source)
            )
        } else {
            format!(
                "{} transcription has recovered.",
                source_label_capitalized(source)
            )
        };
        let error = RuntimeError::new(RuntimeErrorCode::InferenceLagging, message, true)
            .with_source(TranscriptSource::from(source));
        let _ = events::emit_capture_error(&self.app, &error);
    }

    fn on_error(&self, source: AudioSource, error: AsrError) {
        self.manager.handle_source_fault(
            &self.app,
            self.generation,
            source,
            match error.kind {
                AsrErrorKind::StreamCreateFailed => AudioErrorKind::StartFailed,
                _ => AudioErrorKind::Internal,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::microphone::NativeMicrophoneDevice;
    use crate::audio::supervisor::fast_test_policy;
    use crate::audio::AudioCaptureSession;
    use parking_lot::Mutex as PlMutex;
    use std::path::Path;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc as std_mpsc;
    use std::time::Duration;

    struct FakeMicrophoneBackend {
        devices: Vec<NativeMicrophoneDevice>,
        start_gate: PlMutex<Option<std_mpsc::Receiver<()>>>,
        next_start_error: PlMutex<Option<AudioError>>,
        /// When set, every `start()` call fails (unlike `next_start_error`,
        /// which is consumed after one call). Used to simulate a
        /// persistently flapping device across every recovery attempt.
        always_fail_kind: PlMutex<Option<AudioErrorKind>>,
        /// Handed to the next constructed `FakeSession`, whose `stop()`
        /// blocks on it. Used to induce a controlled real teardown hang
        /// for the stopping-watchdog regression test.
        stop_gate: PlMutex<Option<std_mpsc::Receiver<()>>>,
        stopped_sessions: Arc<AtomicUsize>,
        start_calls: Arc<AtomicUsize>,
    }

    impl Default for FakeMicrophoneBackend {
        fn default() -> Self {
            Self {
                devices: Vec::new(),
                start_gate: PlMutex::new(None),
                next_start_error: PlMutex::new(None),
                always_fail_kind: PlMutex::new(None),
                stop_gate: PlMutex::new(None),
                stopped_sessions: Arc::new(AtomicUsize::new(0)),
                start_calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    struct FakeSession {
        stopped_sessions: Arc<AtomicUsize>,
        source: AudioSource,
        stop_gate: PlMutex<Option<std_mpsc::Receiver<()>>>,
    }

    impl AudioCaptureSession for FakeSession {
        fn source(&self) -> AudioSource {
            self.source
        }
        fn stop(&mut self) -> Result<(), AudioError> {
            if let Some(gate) = self.stop_gate.lock().take() {
                let _ = gate.recv();
            }
            self.stopped_sessions.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    impl MicrophoneBackend for FakeMicrophoneBackend {
        fn list_devices(&self) -> Result<Vec<NativeMicrophoneDevice>, AudioError> {
            Ok(self.devices.clone())
        }

        fn start(&self, _device_id: Option<&str>) -> Result<MicrophoneCaptureHandle, AudioError> {
            self.start_calls.fetch_add(1, Ordering::SeqCst);

            if let Some(gate) = self.start_gate.lock().take() {
                let _ = gate.recv();
            }

            if let Some(kind) = *self.always_fail_kind.lock() {
                return Err(AudioError {
                    source: AudioSource::Microphone,
                    kind,
                });
            }

            if let Some(error) = self.next_start_error.lock().take() {
                return Err(error);
            }

            let (_producer, consumer) = crate::audio::buffer::build_pool(4);
            Ok(MicrophoneCaptureHandle {
                session: Box::new(FakeSession {
                    stopped_sessions: self.stopped_sessions.clone(),
                    source: AudioSource::Microphone,
                    stop_gate: PlMutex::new(self.stop_gate.lock().take()),
                }),
                consumer,
                fault: Arc::new(crate::audio::microphone::StreamFault::default()),
                format: PcmFormat {
                    sample_rate_hz: std::num::NonZeroU32::new(16_000).unwrap(),
                    channels: std::num::NonZeroU16::new(1).unwrap(),
                },
            })
        }
    }

    struct FakeSystemBackend {
        probe_error: PlMutex<Option<AudioError>>,
        start_error: PlMutex<Option<AudioError>>,
        stopped_sessions: Arc<AtomicUsize>,
        start_calls: Arc<AtomicUsize>,
        sample_rate: u32,
    }

    impl Default for FakeSystemBackend {
        fn default() -> Self {
            Self {
                probe_error: PlMutex::new(None),
                start_error: PlMutex::new(None),
                stopped_sessions: Arc::new(AtomicUsize::new(0)),
                start_calls: Arc::new(AtomicUsize::new(0)),
                sample_rate: 48_000,
            }
        }
    }

    impl SystemAudioBackend for FakeSystemBackend {
        fn probe(&self) -> Result<(), AudioError> {
            if let Some(err) = self.probe_error.lock().take() {
                return Err(err);
            }
            Ok(())
        }

        fn start(
            &self,
            _sink: Box<dyn crate::audio::PcmBlockSink>,
            _on_error: Box<dyn FnMut(AudioError) + Send>,
        ) -> Result<Box<dyn AudioCaptureSession>, AudioError> {
            self.start_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(err) = self.start_error.lock().take() {
                return Err(err);
            }
            Ok(Box::new(FakeSession {
                stopped_sessions: self.stopped_sessions.clone(),
                source: AudioSource::System,
                stop_gate: PlMutex::new(None),
            }))
        }

        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }
    }

    struct FakeStream;
    impl crate::asr::StreamingRecognizer for FakeStream {
        fn accept(&mut self, _samples: &[f32]) -> Result<(), AsrError> {
            Ok(())
        }
        fn poll(&mut self, _out: &mut Vec<crate::asr::RecognizedSegment>) -> Result<(), AsrError> {
            Ok(())
        }
        fn finish(
            &mut self,
            _out: &mut Vec<crate::asr::RecognizedSegment>,
        ) -> Result<(), AsrError> {
            Ok(())
        }
    }

    struct FakeRecognizerFactory;
    impl RecognizerFactory for FakeRecognizerFactory {
        fn open_stream(
            &self,
            _format: PcmFormat,
        ) -> Result<Box<dyn crate::asr::StreamingRecognizer>, AsrError> {
            Ok(Box::new(FakeStream))
        }
    }

    struct FakeModelLoader {
        fail_next: PlMutex<Option<AsrError>>,
        load_calls: Arc<AtomicUsize>,
    }

    impl FakeModelLoader {
        fn success() -> Self {
            Self {
                fail_next: PlMutex::new(None),
                load_calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl AsrModelLoader for FakeModelLoader {
        fn load(&self, _dir: &Path) -> Result<Arc<dyn RecognizerFactory>, AsrError> {
            self.load_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(err) = self.fail_next.lock().take() {
                return Err(err);
            }
            Ok(Arc::new(FakeRecognizerFactory))
        }
    }

    fn test_app() -> AppHandle<tauri::test::MockRuntime> {
        crate::test_support::ensure_model_dir_env();
        tauri::test::mock_app().handle().clone()
    }

    fn fast_manager(
        mic_backend: Arc<FakeMicrophoneBackend>,
        sys_backend: Arc<FakeSystemBackend>,
        loader: Arc<FakeModelLoader>,
    ) -> Arc<RuntimeManager<tauri::test::MockRuntime>> {
        Arc::new(RuntimeManager::new_with_policy(
            mic_backend,
            sys_backend,
            loader,
            fast_test_policy(),
        ))
    }

    #[test]
    fn microphone_only_starts_and_stops_cleanly() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = Arc::new(RuntimeManager::new(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
            ));
            let status = manager
                .start_capture(app.clone(), Some("mic-1".into()), false)
                .await
                .unwrap();
            assert_eq!(status, CaptureStatus::Listening);
            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 1);
            assert_eq!(sys_backend.start_calls.load(Ordering::SeqCst), 0);

            let snap = manager.snapshot().unwrap();
            assert_eq!(snap.capture_status, CaptureStatus::Listening);
            assert!(matches!(
                snap.microphone,
                AudioSourceStatus::Capturing { .. }
            ));
            assert!(matches!(
                snap.system_audio,
                AudioSourceStatus::Unavailable { .. }
            ));

            let stop_status = manager.stop_capture(app).await.unwrap();
            assert_eq!(stop_status, CaptureStatus::Idle);
            assert_eq!(mic_backend.stopped_sessions.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn system_only_starts_and_stops_cleanly() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = Arc::new(RuntimeManager::new(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
            ));
            let status = manager
                .start_capture(app.clone(), None, true)
                .await
                .unwrap();
            assert_eq!(status, CaptureStatus::Listening);
            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 0);
            assert_eq!(sys_backend.start_calls.load(Ordering::SeqCst), 1);

            let snap = manager.snapshot().unwrap();
            assert_eq!(snap.capture_status, CaptureStatus::Listening);
            assert!(matches!(
                snap.system_audio,
                AudioSourceStatus::Capturing { .. }
            ));

            let stop_status = manager.stop_capture(app).await.unwrap();
            assert_eq!(stop_status, CaptureStatus::Idle);
            assert_eq!(sys_backend.stopped_sessions.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn both_sources_start_and_stop_cleanly() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = Arc::new(RuntimeManager::new(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
            ));
            let status = manager
                .start_capture(app.clone(), Some("mic-1".into()), true)
                .await
                .unwrap();
            assert_eq!(status, CaptureStatus::Listening);
            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 1);
            assert_eq!(sys_backend.start_calls.load(Ordering::SeqCst), 1);

            let snap = manager.snapshot().unwrap();
            assert_eq!(snap.capture_status, CaptureStatus::Listening);
            assert!(matches!(
                snap.microphone,
                AudioSourceStatus::Capturing { .. }
            ));
            assert!(matches!(
                snap.system_audio,
                AudioSourceStatus::Capturing { .. }
            ));

            let stop_status = manager.stop_capture(app).await.unwrap();
            assert_eq!(stop_status, CaptureStatus::Idle);
            assert_eq!(mic_backend.stopped_sessions.load(Ordering::SeqCst), 1);
            assert_eq!(sys_backend.stopped_sessions.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn empty_request_rejected_with_invalid_request() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = Arc::new(RuntimeManager::new(mic_backend, sys_backend, loader));
            let err = manager.start_capture(app, None, false).await.unwrap_err();
            assert_eq!(err.code, RuntimeErrorCode::InvalidRequest);
        });
    }

    #[test]
    fn atomic_start_rolls_back_mic_when_system_fails() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            *sys_backend.start_error.lock() = Some(AudioError {
                source: AudioSource::System,
                kind: AudioErrorKind::PermissionDenied,
            });
            let loader = Arc::new(FakeModelLoader::success());

            let manager = Arc::new(RuntimeManager::new(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
            ));
            let err = manager
                .start_capture(app, Some("mic-1".into()), true)
                .await
                .unwrap_err();

            assert_eq!(err.source, Some(TranscriptSource::System));
            // Crucial atomic rollback assertion: microphone was started, then immediately stopped!
            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 1);
            assert_eq!(mic_backend.stopped_sessions.load(Ordering::SeqCst), 1);

            let snap = manager.snapshot().unwrap();
            assert_eq!(snap.capture_status, CaptureStatus::Error);
            assert!(matches!(snap.system_audio, AudioSourceStatus::Error { .. }));
            assert_eq!(snap.microphone, AudioSourceStatus::Idle);
        });
    }

    #[test]
    fn recoverable_fault_enters_reconnecting_state_and_recovers_survivor_untouched() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let policy = RecoveryPolicy {
                backoff: [
                    Duration::from_millis(150),
                    Duration::from_millis(200),
                    Duration::from_millis(250),
                ],
                ..fast_test_policy()
            };
            let manager = Arc::new(RuntimeManager::new_with_policy(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
                policy,
            ));
            manager
                .start_capture(app.clone(), Some("mic-1".into()), true)
                .await
                .unwrap();
            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 1);

            // Simulate a recoverable microphone disconnect mid-session.
            let generation = manager.generation.load(Ordering::SeqCst);
            manager.handle_source_fault(
                &app,
                generation,
                AudioSource::Microphone,
                AudioErrorKind::DeviceDisconnected,
            );

            // Immediately after the fault (before the fast backoff
            // elapses): status is Starting (reconnecting), not Error, and
            // the surviving system-audio source is completely untouched.
            std::thread::sleep(Duration::from_millis(10));
            let mid_snap = manager.snapshot().unwrap();
            assert!(
                matches!(mid_snap.microphone, AudioSourceStatus::Starting { .. }),
                "expected Starting (reconnecting), got {:?}",
                mid_snap.microphone
            );
            assert_eq!(mid_snap.capture_status, CaptureStatus::Listening);
            assert!(matches!(
                mid_snap.system_audio,
                AudioSourceStatus::Capturing { .. }
            ));
            assert_eq!(sys_backend.stopped_sessions.load(Ordering::SeqCst), 0);

            // After the backoff, the source recovers on its own.
            std::thread::sleep(Duration::from_millis(300));
            let recovered_snap = manager.snapshot().unwrap();
            assert!(
                matches!(
                    recovered_snap.microphone,
                    AudioSourceStatus::Capturing { .. }
                ),
                "expected recovered Capturing, got {:?}",
                recovered_snap.microphone
            );
            assert_eq!(recovered_snap.capture_status, CaptureStatus::Listening);
            // A second start call proves the restart really happened.
            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 2);

            let counters = manager.recovery_counters(AudioSource::Microphone);
            assert_eq!(counters.recovery_attempts, 1);
            assert_eq!(counters.recovery_successes, 1);

            manager.stop_capture(app).await.unwrap();
        });
    }

    #[test]
    fn non_recoverable_fault_goes_terminal_without_retry() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = fast_manager(mic_backend.clone(), sys_backend.clone(), loader);
            manager
                .start_capture(app.clone(), Some("mic-1".into()), true)
                .await
                .unwrap();

            let generation = manager.generation.load(Ordering::SeqCst);
            manager.handle_source_fault(
                &app,
                generation,
                AudioSource::Microphone,
                AudioErrorKind::PermissionDenied,
            );
            std::thread::sleep(Duration::from_millis(50));

            let snap = manager.snapshot().unwrap();
            assert!(matches!(snap.microphone, AudioSourceStatus::Error { .. }));
            // Never retried: only the one original start call exists.
            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 1);
            // Survivor continuity unaffected.
            assert_eq!(snap.capture_status, CaptureStatus::Listening);

            manager.stop_capture(app).await.unwrap();
        });
    }

    #[test]
    fn budget_exhaustion_is_terminal_with_reconnection_stopped_message() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = fast_manager(mic_backend.clone(), sys_backend.clone(), loader);
            manager
                .start_capture(app.clone(), Some("mic-1".into()), false)
                .await
                .unwrap();

            // A flapping device: every restart attempt itself fails.
            *mic_backend.always_fail_kind.lock() = Some(AudioErrorKind::StartFailed);

            let generation = manager.generation.load(Ordering::SeqCst);
            manager.handle_source_fault(
                &app,
                generation,
                AudioSource::Microphone,
                AudioErrorKind::DeviceDisconnected,
            );

            // Fast policy backoffs are 2/4/6ms; give ample real time for
            // all 3 attempts (each itself failing immediately) to exhaust.
            std::thread::sleep(Duration::from_millis(300));

            let snap = manager.snapshot().unwrap();
            match &snap.microphone {
                AudioSourceStatus::Error { error } => {
                    assert!(
                        error.message.contains("Automatic reconnection stopped."),
                        "message was: {}",
                        error.message
                    );
                }
                other => panic!("expected terminal Error, got {other:?}"),
            }
            assert_eq!(snap.capture_status, CaptureStatus::Error);

            let counters = manager.recovery_counters(AudioSource::Microphone);
            assert_eq!(counters.recovery_attempts, 3);
            assert_eq!(counters.recovery_successes, 0);
        });
    }

    #[test]
    fn stop_during_backoff_cancels_recovery_without_late_install() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            // Slow (but not glacial) policy so Stop reliably lands during
            // the backoff window rather than racing a fast restart.
            let policy = RecoveryPolicy {
                max_attempts: 3,
                backoff: [
                    Duration::from_millis(200),
                    Duration::from_millis(400),
                    Duration::from_millis(600),
                ],
                healthy_reset: Duration::from_secs(60),
                wait_tick: Duration::from_millis(5),
                starting_watchdog: Duration::from_secs(10),
                stopping_watchdog: Duration::from_secs(3),
            };
            let manager = Arc::new(RuntimeManager::new_with_policy(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
                policy,
            ));
            manager
                .start_capture(app.clone(), Some("mic-1".into()), false)
                .await
                .unwrap();
            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 1);

            let generation = manager.generation.load(Ordering::SeqCst);
            manager.handle_source_fault(
                &app,
                generation,
                AudioSource::Microphone,
                AudioErrorKind::DeviceDisconnected,
            );

            // Land squarely inside the 200ms backoff window, then Stop.
            std::thread::sleep(Duration::from_millis(50));
            let stop_status = manager.stop_capture(app).await.unwrap();
            assert_eq!(stop_status, CaptureStatus::Idle);

            // Give the cancelled backoff thread ample time to have woken
            // up and (incorrectly, if this regresses) tried to install.
            std::thread::sleep(Duration::from_millis(500));

            // No late install: still idle, and no second start call ever
            // happened after cancellation.
            let snap = manager.snapshot().unwrap();
            assert_eq!(snap.capture_status, CaptureStatus::Idle);
            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn recovery_continues_segment_index_and_clamps_offset() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = fast_manager(mic_backend.clone(), sys_backend.clone(), loader);
            manager
                .start_capture(app.clone(), Some("mic-1".into()), false)
                .await
                .unwrap();

            // Simulate two finals already having been emitted for the
            // microphone before it faults, exactly as
            // `RuntimeAsrObserver::on_final` would update it.
            {
                let mut guard = manager.session.lock().unwrap();
                let active = guard.as_mut().unwrap();
                active.mic_recovery.next_segment_index = 2;
                active.mic_recovery.last_ended_at_ms = 5_000;
            }

            let generation = manager.generation.load(Ordering::SeqCst);
            manager.handle_source_fault(
                &app,
                generation,
                AudioSource::Microphone,
                AudioErrorKind::DeviceDisconnected,
            );
            std::thread::sleep(Duration::from_millis(200));

            // The index/offset floor must survive the recovery restart
            // unchanged (a successful restart never resets them; only a
            // brand new session does).
            let guard = manager.session.lock().unwrap();
            let active = guard.as_ref().unwrap();
            assert_eq!(active.mic_recovery.next_segment_index, 2);
            assert_eq!(active.mic_recovery.last_ended_at_ms, 5_000);
        });
    }

    #[test]
    fn partial_failure_survivor_continues_transcribing() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = Arc::new(RuntimeManager::new(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
            ));
            manager
                .start_capture(app.clone(), Some("mic-1".into()), true)
                .await
                .unwrap();

            // Simulate microphone permission denial mid-session: never
            // retried, so this test is not racing a background recovery.
            manager.handle_source_fault(
                &app,
                manager.generation.load(Ordering::SeqCst),
                AudioSource::Microphone,
                AudioErrorKind::PermissionDenied,
            );

            std::thread::sleep(Duration::from_millis(100));

            let snap = manager.snapshot().unwrap();
            // Crucial AC 17 assertion: aggregate status STAYS Listening because system audio survives!
            assert_eq!(snap.capture_status, CaptureStatus::Listening);
            assert!(matches!(snap.microphone, AudioSourceStatus::Error { .. }));
            assert!(matches!(
                snap.system_audio,
                AudioSourceStatus::Capturing { .. }
            ));

            // Now simulate system audio also failing (non-recoverable).
            manager.handle_source_fault(
                &app,
                manager.generation.load(Ordering::SeqCst),
                AudioSource::System,
                AudioErrorKind::Internal,
            );

            std::thread::sleep(Duration::from_millis(100));

            let snap2 = manager.snapshot().unwrap();
            // Now both have failed -> aggregate status transitions to Error!
            assert_eq!(snap2.capture_status, CaptureStatus::Error);
            assert!(matches!(snap2.microphone, AudioSourceStatus::Error { .. }));
            assert!(matches!(
                snap2.system_audio,
                AudioSourceStatus::Error { .. }
            ));
        });
    }

    #[test]
    fn atomic_start_rolls_back_when_probe_fails() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            *sys_backend.probe_error.lock() = Some(AudioError {
                source: AudioSource::System,
                kind: AudioErrorKind::Unavailable,
            });
            let loader = Arc::new(FakeModelLoader::success());

            let manager = Arc::new(RuntimeManager::new(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
            ));
            let err = manager
                .start_capture(app, Some("mic-1".into()), true)
                .await
                .unwrap_err();

            assert_eq!(err.source, Some(TranscriptSource::System));
            assert_eq!(mic_backend.stopped_sessions.load(Ordering::SeqCst), 1);
            assert_eq!(
                manager.snapshot().unwrap().capture_status,
                CaptureStatus::Error
            );
        });
    }

    #[test]
    fn five_dual_source_start_stop_cycles_reset_cleanly() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = Arc::new(RuntimeManager::new(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
            ));

            for _ in 0..5 {
                let status = manager
                    .start_capture(app.clone(), Some("mic-1".into()), true)
                    .await
                    .unwrap();
                assert_eq!(status, CaptureStatus::Listening);

                let stop_status = manager.stop_capture(app.clone()).await.unwrap();
                assert_eq!(stop_status, CaptureStatus::Idle);
            }

            assert_eq!(mic_backend.start_calls.load(Ordering::SeqCst), 5);
            assert_eq!(sys_backend.start_calls.load(Ordering::SeqCst), 5);
            assert_eq!(mic_backend.stopped_sessions.load(Ordering::SeqCst), 5);
            assert_eq!(sys_backend.stopped_sessions.load(Ordering::SeqCst), 5);
        });
    }

    #[test]
    fn shutdown_is_idempotent_and_releases_active_session() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let manager = Arc::new(RuntimeManager::new(
                mic_backend.clone(),
                sys_backend.clone(),
                loader,
            ));
            manager
                .start_capture(app, Some("mic-1".into()), true)
                .await
                .unwrap();

            manager.shutdown();
            assert_eq!(mic_backend.stopped_sessions.load(Ordering::SeqCst), 1);
            assert_eq!(sys_backend.stopped_sessions.load(Ordering::SeqCst), 1);
            assert!(manager.session.lock().unwrap().is_none());

            // Idempotent: a second call touches nothing further.
            manager.shutdown();
            assert_eq!(mic_backend.stopped_sessions.load(Ordering::SeqCst), 1);
            assert_eq!(sys_backend.stopped_sessions.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn starting_watchdog_force_fails_a_stuck_start() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let (_gate_tx, gate_rx) = std_mpsc::channel::<()>();
            *mic_backend.start_gate.lock() = Some(gate_rx); // never signaled: start() blocks forever
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let policy = RecoveryPolicy {
                starting_watchdog: Duration::from_millis(50),
                ..fast_test_policy()
            };
            let manager = Arc::new(RuntimeManager::new_with_policy(
                mic_backend.clone(),
                sys_backend,
                loader,
                policy,
            ));

            // The underlying fake backend call never returns, so a
            // command awaiting `start_capture` directly would hang
            // forever too — that in-flight task is fired-and-forgotten
            // here on purpose. The watchdog's job is to make the *state*
            // terminal on its own within the bound regardless.
            let manager_for_task = manager.clone();
            let app_for_task = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = manager_for_task
                    .start_capture(app_for_task, Some("mic-1".into()), false)
                    .await;
            });

            std::thread::sleep(Duration::from_millis(400));

            let snap = manager.snapshot().unwrap();
            assert!(
                matches!(snap.microphone, AudioSourceStatus::Error { .. }),
                "expected watchdog-forced terminal Error, got {:?}",
                snap.microphone
            );
            assert_eq!(snap.capture_status, CaptureStatus::Error);
            let counters = manager.recovery_counters(AudioSource::Microphone);
            assert_eq!(counters.watchdog_expiries, 1);
        });
    }

    #[test]
    fn stopping_watchdog_force_commits_idle_when_teardown_hangs() {
        tauri::async_runtime::block_on(async {
            let app = test_app();
            let mic_backend = Arc::new(FakeMicrophoneBackend::default());
            let sys_backend = Arc::new(FakeSystemBackend::default());
            let loader = Arc::new(FakeModelLoader::success());

            let policy = RecoveryPolicy {
                stopping_watchdog: Duration::from_millis(50),
                ..fast_test_policy()
            };
            let manager = Arc::new(RuntimeManager::new_with_policy(
                mic_backend.clone(),
                sys_backend,
                loader,
                policy,
            ));

            let (_stop_gate_tx, stop_gate_rx) = std_mpsc::channel::<()>();
            *mic_backend.stop_gate.lock() = Some(stop_gate_rx); // never signaled: session.stop() blocks forever

            manager
                .start_capture(app.clone(), Some("mic-1".into()), false)
                .await
                .unwrap();

            // Fire-and-forget: the underlying `FakeSession::stop()` call
            // never returns, so a command awaiting `stop_capture` directly
            // would hang forever too. The watchdog's job is to make the
            // *state* terminal (Idle) on its own within the bound
            // regardless, while the stuck background task keeps running
            // harmlessly.
            let manager_for_task = manager.clone();
            let app_for_task = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = manager_for_task.stop_capture(app_for_task).await;
            });

            std::thread::sleep(Duration::from_millis(300));

            let snap = manager.snapshot().unwrap();
            assert_eq!(
                snap.capture_status,
                CaptureStatus::Idle,
                "the stopping watchdog must force Idle even though the underlying \
                 teardown call is still hanging in the background"
            );
        });
    }
}
