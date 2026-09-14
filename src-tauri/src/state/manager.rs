//! `RuntimeManager`: orchestrates the real microphone lifecycle behind the
//! typed runtime commands.
//!
//! This is the Tauri-facing glue between the pure [`RuntimeState`] machine,
//! the platform-agnostic [`MicrophoneBackend`]/[`MicrophoneMonitor`], and
//! native event emission. It owns the one active session (if any) and a
//! monotonic generation counter so a Stop (or a superseding Start) can
//! invalidate an in-flight Start that is still awaiting the macOS
//! permission prompt or native device work.
//!
//! Generic over `R: tauri::Runtime` (defaulting to the production `Wry`
//! runtime) purely so tests can drive it with `tauri::test`'s `MockRuntime`
//! and a real, event-capable `AppHandle` instead of a hand-rolled fake.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Runtime};

use crate::audio::microphone::{
    MicrophoneBackend, MicrophoneCaptureHandle, MicrophoneMonitor, MicrophoneMonitorObserver,
};
use crate::audio::{AudioError, AudioErrorKind};
use crate::events;

use super::runtime::{
    Activity, AudioSourceStatus, CaptureStatus, MicrophoneDevice, RuntimeError, RuntimeErrorCode,
    RuntimeSnapshot, RuntimeState, TranscriptSource,
};

fn map_audio_error(error: AudioError) -> RuntimeError {
    let source = TranscriptSource::from(error.source);
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

struct ActiveMicrophoneSession {
    monitor: MicrophoneMonitor,
}

/// The single process-managed runtime handle. `Arc`-wrapped by Tauri's
/// managed state so background work (the async command's blocking task and
/// the monitor's observer) can hold a cheap clone.
pub struct RuntimeManager<R: Runtime = tauri::Wry> {
    state: Mutex<RuntimeState>,
    backend: Arc<dyn MicrophoneBackend>,
    session: Mutex<Option<ActiveMicrophoneSession>>,
    generation: AtomicU64,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> RuntimeManager<R> {
    pub fn new(backend: Arc<dyn MicrophoneBackend>) -> Self {
        Self {
            state: Mutex::new(RuntimeState::new()),
            backend,
            session: Mutex::new(None),
            generation: AtomicU64::new(0),
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
    ) -> Result<std::sync::MutexGuard<'_, Option<ActiveMicrophoneSession>>, RuntimeError> {
        self.session
            .lock()
            .map_err(|_| RuntimeError::internal("microphone session mutex was poisoned"))
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

    /// Lists real devices and reconciles microphone availability. Emits
    /// `audio:status` only when availability actually changed.
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

    /// Starts (or restarts) microphone capture. Rejects concurrent starts;
    /// otherwise transitions `idle`/`error` -> `starting` synchronously,
    /// performs native work (which may block on the macOS permission
    /// prompt) off the caller's async task, and only then commits
    /// `listening` — unless a concurrent Stop invalidated this attempt in
    /// the meantime, in which case any native resource it built is released
    /// without touching state further.
    pub async fn start_microphone(
        self: &Arc<Self>,
        app: AppHandle<R>,
        device_id: Option<String>,
    ) -> Result<CaptureStatus, RuntimeError> {
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
            state.apply_microphone_status(AudioSourceStatus::Starting {
                device_id: device_id.clone(),
            })?;
            generation
        };
        self.emit_capture_status(&app)?;
        self.emit_audio_status(&app, TranscriptSource::Microphone)?;

        let backend = self.backend.clone();
        let device_for_blocking = device_id.clone();
        let start_result = tauri::async_runtime::spawn_blocking(move || {
            backend.start(device_for_blocking.as_deref())
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
                        RuntimeError::internal("microphone start task panicked"),
                    )
                    .await;
            }
        };

        if self.generation.load(Ordering::SeqCst) != generation {
            // A Stop (or a newer Start) ran while we awaited native work.
            // Release whatever was just built and leave state exactly as
            // the winner left it.
            let MicrophoneCaptureHandle { mut session, .. } = capture;
            let _ = session.stop();
            return Err(RuntimeError::capture_not_active());
        }

        let observer: Arc<dyn MicrophoneMonitorObserver> = Arc::new(RuntimeMonitorObserver {
            manager: self.clone(),
            app: app.clone(),
            generation,
        });
        let monitor = MicrophoneMonitor::spawn(capture, observer);
        *self.lock_session()? = Some(ActiveMicrophoneSession { monitor });

        let status = {
            let mut state = self.lock_state()?;
            state.apply_microphone_status(AudioSourceStatus::Capturing {
                device_id,
                activity: Activity::Waiting,
            })?;
            state.apply_capture_status(CaptureStatus::Listening)?
        };
        self.emit_audio_status(&app, TranscriptSource::Microphone)?;
        self.emit_capture_status(&app)?;
        Ok(status.capture_status)
    }

    async fn fail_starting(
        &self,
        app: &AppHandle<R>,
        generation: u64,
        error: RuntimeError,
    ) -> Result<CaptureStatus, RuntimeError> {
        if self.generation.load(Ordering::SeqCst) == generation {
            {
                let mut state = self.lock_state()?;
                state.apply_microphone_status(AudioSourceStatus::Error {
                    error: error.clone(),
                })?;
                state.apply_capture_status(CaptureStatus::Error)?;
            }
            self.emit_audio_status(app, TranscriptSource::Microphone)?;
            self.emit_capture_status(app)?;
        }
        let _ = events::emit_capture_error(app, &error);
        Err(error)
    }

    /// Stops capture. Idle rejects with `capture_not_active`; every other
    /// state (starting/listening/stopping/error) releases any active
    /// session and settles into idle.
    pub async fn stop_microphone(
        self: &Arc<Self>,
        app: AppHandle<R>,
    ) -> Result<CaptureStatus, RuntimeError> {
        // Bump first so a Start awaiting native work in another task
        // notices it has been superseded as soon as it wakes up.
        self.generation.fetch_add(1, Ordering::SeqCst);

        if self.lock_state()?.capture_status() == CaptureStatus::Idle {
            return Err(RuntimeError::capture_not_active());
        }

        let existing = self.lock_session()?.take();

        self.lock_state()?
            .apply_capture_status(CaptureStatus::Stopping)?;
        self.emit_capture_status(&app)?;

        if let Some(active) = existing {
            tauri::async_runtime::spawn_blocking(move || active.monitor.stop())
                .await
                .map_err(|_| RuntimeError::internal("microphone stop task panicked"))?;
        }

        let status = {
            let mut state = self.lock_state()?;
            state.apply_microphone_status(AudioSourceStatus::Idle)?;
            state.apply_capture_status(CaptureStatus::Idle)?
        };
        self.emit_audio_status(&app, TranscriptSource::Microphone)?;
        self.emit_capture_status(&app)?;
        Ok(status.capture_status)
    }
}

/// Bridges the platform-agnostic monitor thread to Tauri state/events. Every
/// method is called directly on the monitor thread, so it must stay cheap,
/// non-blocking, and panic-free.
struct RuntimeMonitorObserver<R: Runtime> {
    manager: Arc<RuntimeManager<R>>,
    app: AppHandle<R>,
    generation: u64,
}

impl<R: Runtime> RuntimeMonitorObserver<R> {
    fn is_current(&self) -> bool {
        self.manager.generation.load(Ordering::SeqCst) == self.generation
    }
}

impl<R: Runtime> MicrophoneMonitorObserver for RuntimeMonitorObserver<R> {
    fn on_first_signal(&self) {
        if !self.is_current() {
            return;
        }
        let device_id = {
            let Ok(state) = self.manager.state.lock() else {
                return;
            };
            match state.snapshot().microphone {
                AudioSourceStatus::Capturing { device_id, .. } => device_id,
                _ => return,
            }
        };
        if let Ok(mut state) = self.manager.state.lock() {
            let _ = state.apply_microphone_status(AudioSourceStatus::Capturing {
                device_id,
                activity: Activity::Receiving,
            });
        }
        let _ = self
            .manager
            .emit_audio_status(&self.app, TranscriptSource::Microphone);
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

    fn on_fault(&self, _kind: AudioErrorKind) {
        if !self.is_current() {
            return;
        }
        // Drop our own session entry so a subsequent Stop does not try to
        // join a monitor thread that has already exited on its own.
        if let Ok(mut session) = self.manager.session.lock() {
            session.take();
        }

        let error = RuntimeError::new(
            RuntimeErrorCode::DeviceDisconnected,
            "the selected microphone was disconnected",
            true,
        )
        .with_source(TranscriptSource::Microphone);

        if let Ok(mut state) = self.manager.state.lock() {
            let _ = state.apply_microphone_status(AudioSourceStatus::Error {
                error: error.clone(),
            });
            let _ = state.apply_capture_status(CaptureStatus::Error);
        }
        let _ = self
            .manager
            .emit_audio_status(&self.app, TranscriptSource::Microphone);
        let _ = self.manager.emit_capture_status(&self.app);
        let _ = events::emit_capture_error(&self.app, &error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::microphone::NativeMicrophoneDevice;
    use parking_lot::Mutex as PlMutex;
    use std::sync::mpsc as std_mpsc;
    use std::time::Duration;

    /// A deterministic backend for tests: never touches a real device.
    /// Configurable start behavior lets tests exercise success, failure,
    /// and slow/blocking starts (to race against a concurrent Stop).
    struct FakeMicrophoneBackend {
        devices: Vec<NativeMicrophoneDevice>,
        start_gate: PlMutex<Option<std_mpsc::Receiver<()>>>,
        next_start_error: PlMutex<Option<AudioError>>,
        stopped_sessions: Arc<std::sync::atomic::AtomicUsize>,
        start_calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Default for FakeMicrophoneBackend {
        fn default() -> Self {
            Self {
                devices: Vec::new(),
                start_gate: PlMutex::new(None),
                next_start_error: PlMutex::new(None),
                stopped_sessions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                start_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }
        }
    }

    struct FakeSession {
        stopped_sessions: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl crate::audio::AudioCaptureSession for FakeSession {
        fn source(&self) -> crate::audio::AudioSource {
            crate::audio::AudioSource::Microphone
        }
        fn stop(&mut self) -> Result<(), AudioError> {
            self.stopped_sessions
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    impl MicrophoneBackend for FakeMicrophoneBackend {
        fn list_devices(&self) -> Result<Vec<NativeMicrophoneDevice>, AudioError> {
            Ok(self.devices.clone())
        }

        fn start(&self, _device_id: Option<&str>) -> Result<MicrophoneCaptureHandle, AudioError> {
            self.start_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            // Block until the test releases the gate, simulating a slow
            // macOS permission prompt or device open.
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
                }),
                consumer,
                fault: Arc::new(crate::audio::microphone::StreamFault::default()),
                format: crate::audio::PcmFormat {
                    sample_rate_hz: std::num::NonZeroU32::new(16_000).unwrap(),
                    channels: std::num::NonZeroU16::new(1).unwrap(),
                },
            })
        }
    }

    fn manager_with(
        backend: FakeMicrophoneBackend,
    ) -> (
        Arc<RuntimeManager<tauri::test::MockRuntime>>,
        tauri::App<tauri::test::MockRuntime>,
    ) {
        let app = tauri::test::mock_app();
        let manager = Arc::new(RuntimeManager::<tauri::test::MockRuntime>::new(Arc::new(
            backend,
        )));
        (manager, app)
    }

    #[test]
    fn start_transitions_idle_to_listening_and_stop_releases_the_session() {
        tauri::async_runtime::block_on(async {
            let (manager, app) = manager_with(FakeMicrophoneBackend::default());
            let handle = app.handle().clone();

            let status = manager
                .start_microphone(handle.clone(), Some("mic-1".to_string()))
                .await
                .expect("start succeeds against a working fake backend");
            assert_eq!(status, CaptureStatus::Listening);
            assert!(matches!(
                manager.snapshot().unwrap().microphone,
                AudioSourceStatus::Capturing { .. }
            ));

            let status = manager
                .stop_microphone(handle)
                .await
                .expect("stop succeeds while listening");
            assert_eq!(status, CaptureStatus::Idle);
            assert_eq!(
                manager.snapshot().unwrap().microphone,
                AudioSourceStatus::Idle
            );
        });
    }

    #[test]
    fn start_stop_start_cycle_releases_and_reacquires_resources() {
        tauri::async_runtime::block_on(async {
            let backend = FakeMicrophoneBackend::default();
            let stopped = backend.stopped_sessions.clone();
            let starts = backend.start_calls.clone();
            let (manager, app) = manager_with(backend);
            let handle = app.handle().clone();

            for _ in 0..2 {
                manager
                    .start_microphone(handle.clone(), None)
                    .await
                    .expect("start succeeds");
                manager
                    .stop_microphone(handle.clone())
                    .await
                    .expect("stop succeeds");
            }

            assert_eq!(starts.load(std::sync::atomic::Ordering::SeqCst), 2);
            assert_eq!(stopped.load(std::sync::atomic::Ordering::SeqCst), 2);
        });
    }

    #[test]
    fn duplicate_start_is_rejected_without_touching_the_active_session() {
        tauri::async_runtime::block_on(async {
            let (manager, app) = manager_with(FakeMicrophoneBackend::default());
            let handle = app.handle().clone();

            manager
                .start_microphone(handle.clone(), None)
                .await
                .expect("first start succeeds");

            let error = manager
                .start_microphone(handle.clone(), None)
                .await
                .expect_err("second start while listening must be rejected");
            assert_eq!(error.code, RuntimeErrorCode::CaptureAlreadyActive);
            assert_eq!(
                manager.snapshot().unwrap().capture_status,
                CaptureStatus::Listening
            );
        });
    }

    #[test]
    fn stop_while_idle_is_rejected() {
        tauri::async_runtime::block_on(async {
            let (manager, app) = manager_with(FakeMicrophoneBackend::default());
            let error = manager
                .stop_microphone(app.handle().clone())
                .await
                .expect_err("stop while idle must be rejected");
            assert_eq!(error.code, RuntimeErrorCode::CaptureNotActive);
        });
    }

    #[test]
    fn start_failure_enters_error_state_and_a_later_start_can_recover() {
        tauri::async_runtime::block_on(async {
            let backend = FakeMicrophoneBackend::default();
            *backend.next_start_error.lock() = Some(AudioError {
                source: crate::audio::AudioSource::Microphone,
                kind: AudioErrorKind::PermissionDenied,
            });
            let (manager, app) = manager_with(backend);
            let handle = app.handle().clone();

            let error = manager
                .start_microphone(handle.clone(), None)
                .await
                .expect_err("permission-denied start fails");
            assert_eq!(error.code, RuntimeErrorCode::MicrophonePermissionDenied);
            assert_eq!(
                manager.snapshot().unwrap().capture_status,
                CaptureStatus::Error
            );

            // error -> starting is allowed: a later Start can recover.
            let status = manager
                .start_microphone(handle, None)
                .await
                .expect("retry after the transient error succeeds");
            assert_eq!(status, CaptureStatus::Listening);
        });
    }

    #[test]
    fn stop_during_a_slow_start_cancels_it_and_releases_its_resources() {
        tauri::async_runtime::block_on(async {
            let (start_gate_tx, start_gate_rx) = std_mpsc::channel::<()>();
            let backend = FakeMicrophoneBackend {
                start_gate: PlMutex::new(Some(start_gate_rx)),
                ..FakeMicrophoneBackend::default()
            };
            let stopped = backend.stopped_sessions.clone();
            let (manager, app) = manager_with(backend);
            let handle = app.handle().clone();

            let start_manager = manager.clone();
            let start_handle = handle.clone();
            let start_task = tauri::async_runtime::spawn(async move {
                start_manager.start_microphone(start_handle, None).await
            });

            // Give the blocking task time to actually enter `backend.start`
            // and park on the gate before we stop.
            std::thread::sleep(Duration::from_millis(50));
            assert_eq!(
                manager.snapshot().unwrap().capture_status,
                CaptureStatus::Starting
            );

            // Nothing is installed yet (native work is still in flight), so
            // this Stop takes the "no active session" path but still bumps
            // the generation, invalidating the in-flight Start.
            let stop_result = manager.stop_microphone(handle.clone()).await;
            assert!(stop_result.is_ok());

            // Release the gate: the stale Start now completes and must
            // discard its own capture handle instead of installing it.
            let _ = start_gate_tx.send(());
            let start_result = start_task.await.expect("start task did not panic");
            assert_eq!(
                start_result
                    .expect_err("a superseded start must not succeed")
                    .code,
                RuntimeErrorCode::CaptureNotActive
            );

            assert_eq!(stopped.load(std::sync::atomic::Ordering::SeqCst), 1);
            assert_eq!(
                manager.snapshot().unwrap().capture_status,
                CaptureStatus::Idle
            );
        });
    }

    #[test]
    fn list_microphones_reconciles_availability_from_empty_to_present() {
        let backend = FakeMicrophoneBackend {
            devices: vec![NativeMicrophoneDevice {
                id: cpal::DeviceId::new(cpal::default_host().id(), "test-device"),
                label: "Test Mic".to_string(),
                is_default: true,
            }],
            ..FakeMicrophoneBackend::default()
        };
        let (manager, app) = manager_with(backend);
        assert!(matches!(
            manager.snapshot().unwrap().microphone,
            AudioSourceStatus::Unavailable { .. }
        ));

        let devices = manager
            .list_microphones(app.handle())
            .expect("listing succeeds");
        assert_eq!(devices.len(), 1);
        assert_eq!(
            manager.snapshot().unwrap().microphone,
            AudioSourceStatus::Idle
        );
    }
}
