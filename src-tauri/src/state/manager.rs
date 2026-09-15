//! `RuntimeManager`: orchestrates the real microphone + system-audio + local
//! development ASR lifecycle behind the typed runtime commands.
//!
//! Owns the active dual-source session, atomic start with full rollback,
//! survivor continuity on single-source mid-session failure, and event emission.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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

struct ActiveMicrophoneSession {
    monitor: MicrophoneMonitor,
    asr_worker: AsrWorker,
}

struct ActiveSystemSession {
    monitor: SystemAudioMonitor,
    asr_worker: AsrWorker,
}

struct ActiveSession {
    mic: Option<ActiveMicrophoneSession>,
    sys: Option<ActiveSystemSession>,
}

impl ActiveSession {
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
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> RuntimeManager<R> {
    pub fn new(
        backend: Arc<dyn MicrophoneBackend>,
        system_backend: Arc<dyn SystemAudioBackend>,
        asr_loader: Arc<dyn AsrModelLoader>,
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

        let mut active_mic: Option<ActiveMicrophoneSession> = None;
        if let Some(device_id) = &microphone_device_id {
            let backend = self.backend.clone();
            let device_for_blocking = device_id.clone();
            let start_result = tauri::async_runtime::spawn_blocking(move || {
                backend.start(Some(&device_for_blocking))
            })
            .await;

            let capture = match start_result {
                Ok(Ok(capture)) => capture,
                Ok(Err(audio_error)) => {
                    return self
                        .fail_starting(&app, generation, map_audio_error(audio_error))
                        .await;
                }
                Err(_join_error) => {
                    return self
                        .fail_starting(
                            &app,
                            generation,
                            RuntimeError::internal("microphone start task panicked")
                                .with_source(TranscriptSource::Microphone),
                        )
                        .await;
                }
            };

            if self.generation.load(Ordering::SeqCst) != generation {
                let MicrophoneCaptureHandle { mut session, .. } = capture;
                let _ = session.stop();
                return Err(RuntimeError::capture_not_active());
            }

            let format = capture.format;
            let mic_start_offset_ms = session_clock_origin.elapsed().as_millis() as u64;
            let stream = match factory.open_stream(format) {
                Ok(stream) => stream,
                Err(asr_error) => {
                    let MicrophoneCaptureHandle { mut session, .. } = capture;
                    let _ = session.stop();
                    return self
                        .fail_starting(
                            &app,
                            generation,
                            map_asr_error(&asr_error, Some(TranscriptSource::Microphone)),
                        )
                        .await;
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
                asr_observer,
            );

            let monitor_observer: Arc<dyn MicrophoneMonitorObserver> =
                Arc::new(RuntimeMicrophoneMonitorObserver {
                    manager: self.clone(),
                    app: app.clone(),
                    generation,
                });
            let monitor = MicrophoneMonitor::spawn(capture, monitor_observer, asr_feeder);
            active_mic = Some(ActiveMicrophoneSession {
                monitor,
                asr_worker,
            });
        }

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

            let sys_rate = self.system_backend.sample_rate();
            let sys_format = PcmFormat {
                sample_rate_hz: std::num::NonZeroU32::new(sys_rate)
                    .unwrap_or_else(|| std::num::NonZeroU32::new(48000).unwrap()),
                channels: std::num::NonZeroU16::new(1).unwrap(),
            };

            let sys_stream = match factory.open_stream(sys_format) {
                Ok(stream) => stream,
                Err(asr_error) => {
                    if let Some(mut mic) = active_mic.take() {
                        mic.monitor.stop();
                        mic.asr_worker.stop();
                    }
                    return self
                        .fail_starting(
                            &app,
                            generation,
                            map_asr_error(&asr_error, Some(TranscriptSource::System)),
                        )
                        .await;
                }
            };

            let sys_start_offset_ms = session_clock_origin.elapsed().as_millis() as u64;
            let (sys_pool_producer, sys_pool_consumer) =
                build_pool(block_capacity_for_rate(sys_rate));

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
                    if let Some(mut mic) = active_mic.take() {
                        mic.monitor.stop();
                        mic.asr_worker.stop();
                    }
                    return self
                        .fail_starting(&app, generation, map_audio_error(audio_error))
                        .await;
                }
                Err(_join_error) => {
                    drop(sys_asr_feeder);
                    asr_worker.stop();
                    if let Some(mut mic) = active_mic.take() {
                        mic.monitor.stop();
                        mic.asr_worker.stop();
                    }
                    return self
                        .fail_starting(
                            &app,
                            generation,
                            RuntimeError::internal("system audio start task panicked")
                                .with_source(TranscriptSource::System),
                        )
                        .await;
                }
            };

            if self.generation.load(Ordering::SeqCst) != generation {
                let mut session = sys_session;
                let _ = session.stop();
                drop(sys_asr_feeder);
                asr_worker.stop();
                if let Some(mut mic) = active_mic.take() {
                    mic.monitor.stop();
                    mic.asr_worker.stop();
                }
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
            active_sys = Some(ActiveSystemSession {
                monitor,
                asr_worker,
            });
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
            tauri::async_runtime::spawn_blocking(move || {
                active.stop();
            })
            .await
            .map_err(|_| RuntimeError::internal("capture stop task panicked"))?;
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

    /// Handles mid-session fault on a single source, keeping the survivor alive.
    fn handle_source_fault(
        &self,
        app: &AppHandle<R>,
        generation: u64,
        source: AudioSource,
        error_kind: AudioErrorKind,
    ) {
        if self.generation.load(Ordering::SeqCst) != generation {
            return;
        }

        let audio_err = AudioError {
            source,
            kind: error_kind,
        };
        let runtime_err = map_audio_error(audio_err);

        let to_stop: Option<Box<dyn FnOnce() + Send>> = {
            if let Ok(mut guard) = self.session.lock() {
                if let Some(active) = &mut *guard {
                    match source {
                        AudioSource::Microphone => active.mic.take().map(|mut m| {
                            Box::new(move || {
                                m.monitor.stop();
                                m.asr_worker.stop();
                            }) as Box<dyn FnOnce() + Send>
                        }),
                        AudioSource::System => active.sys.take().map(|mut s| {
                            Box::new(move || {
                                s.monitor.stop();
                                s.asr_worker.stop();
                            }) as Box<dyn FnOnce() + Send>
                        }),
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(stop_fn) = to_stop {
            tauri::async_runtime::spawn_blocking(stop_fn);
        }
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
                if let Some(active) = &*guard {
                    active.mic.is_some() || active.sys.is_some()
                } else {
                    false
                }
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
        stopped_sessions: Arc<AtomicUsize>,
        start_calls: Arc<AtomicUsize>,
    }

    impl Default for FakeMicrophoneBackend {
        fn default() -> Self {
            Self {
                devices: Vec::new(),
                start_gate: PlMutex::new(None),
                next_start_error: PlMutex::new(None),
                stopped_sessions: Arc::new(AtomicUsize::new(0)),
                start_calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    struct FakeSession {
        stopped_sessions: Arc<AtomicUsize>,
        source: AudioSource,
    }

    impl AudioCaptureSession for FakeSession {
        fn source(&self) -> AudioSource {
            self.source
        }
        fn stop(&mut self) -> Result<(), AudioError> {
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

            if let Some(error) = self.next_start_error.lock().take() {
                return Err(error);
            }

            let (_producer, consumer) = crate::audio::buffer::build_pool(4);
            Ok(MicrophoneCaptureHandle {
                session: Box::new(FakeSession {
                    stopped_sessions: self.stopped_sessions.clone(),
                    source: AudioSource::Microphone,
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

            // Simulate microphone disconnecting mid-session
            manager.handle_source_fault(
                &app,
                manager.generation.load(Ordering::SeqCst),
                AudioSource::Microphone,
                AudioErrorKind::DeviceDisconnected,
            );

            // Give background stop task time to run
            std::thread::sleep(Duration::from_millis(100));

            let snap = manager.snapshot().unwrap();
            // Crucial AC 17 assertion: aggregate status STAYS Listening because system audio survives!
            assert_eq!(snap.capture_status, CaptureStatus::Listening);
            assert!(matches!(snap.microphone, AudioSourceStatus::Error { .. }));
            assert!(matches!(
                snap.system_audio,
                AudioSourceStatus::Capturing { .. }
            ));

            // Now simulate system audio also failing
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
}
