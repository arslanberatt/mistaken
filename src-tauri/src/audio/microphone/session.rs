//! The microphone monitor: one supervisor thread per active session.
//!
//! The monitor owns the consumer half of the PCM pool and the live capture
//! session. It drains completed blocks (recycling their buffers), detects
//! the first real signal, rate-limits overflow reporting, watches for a
//! stream fault, and performs bounded teardown. It never touches Tauri,
//! serialization, or transcript state directly; all of that is reported
//! through the small [`super::MicrophoneMonitorObserver`] callback.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::audio::AudioErrorKind;

use super::{MicrophoneCaptureHandle, MicrophoneMonitorObserver};

/// Upper bound on how long the monitor can sleep between checks. Small
/// enough that "first signal" and fault detection stay well under a second;
/// large enough to avoid busy-spinning.
const MONITOR_TICK: Duration = Duration::from_millis(20);
/// Minimum absolute peak sample magnitude that counts as "real signal"
/// rather than silence/noise floor.
const SIGNAL_THRESHOLD: f32 = 0.01;
/// Overflow is reported at most once per this interval while drops continue.
const OVERFLOW_REPORT_INTERVAL: Duration = Duration::from_secs(1);

/// A running monitor thread. Dropping or calling [`MicrophoneMonitor::stop`]
/// both signal the thread to exit and join it, so teardown never leaks the
/// thread, the pool, or the underlying capture session.
pub struct MicrophoneMonitor {
    stop_flag: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl MicrophoneMonitor {
    /// Spawns the monitor loop for one active capture. Takes ownership of
    /// the capture handle (session + pool consumer + fault flag) for its
    /// entire lifetime; nothing else may touch them concurrently.
    pub fn spawn(
        capture: MicrophoneCaptureHandle,
        observer: Arc<dyn MicrophoneMonitorObserver>,
    ) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let thread_stop_flag = stop_flag.clone();

        let join = thread::Builder::new()
            .name("mistaken-mic-monitor".into())
            .spawn(move || run(capture, observer, thread_stop_flag))
            .expect("failed to spawn microphone monitor thread");

        Self {
            stop_flag,
            join: Some(join),
        }
    }

    /// Signals the monitor to stop and blocks until it has torn down the
    /// capture session and pool, then exited. Safe to call more than once
    /// only through `Drop`; this consuming method runs the teardown exactly
    /// once.
    pub fn stop(mut self) {
        self.request_stop_and_join();
    }

    fn request_stop_and_join(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for MicrophoneMonitor {
    fn drop(&mut self) {
        self.request_stop_and_join();
    }
}

fn run(
    mut capture: MicrophoneCaptureHandle,
    observer: Arc<dyn MicrophoneMonitorObserver>,
    stop_flag: Arc<AtomicBool>,
) {
    let mut signaled = false;
    let mut last_overflow_report: Option<Instant> = None;
    let mut last_overflow_total = 0_u64;

    loop {
        if stop_flag.load(Ordering::Acquire) {
            break;
        }
        if capture.fault.is_set() {
            observer.on_fault(AudioErrorKind::DeviceDisconnected);
            break;
        }

        let mut drained_any = false;
        while let Some(block) = capture.consumer.try_recv() {
            drained_any = true;

            if !signaled {
                let peak = block.samples[..block.valid_samples]
                    .iter()
                    .fold(0.0_f32, |max, sample| max.max(sample.abs()));
                if peak >= SIGNAL_THRESHOLD {
                    signaled = true;
                    observer.on_first_signal();
                }
            }

            // A recycle failure is an internal pool invariant violation:
            // end the session rather than silently shrinking the pool.
            if capture.consumer.recycle(block).is_err() {
                observer.on_fault(AudioErrorKind::Internal);
                let _ = capture.session.stop();
                return;
            }
        }

        let overflow_total = capture.consumer.overflow_total();
        if overflow_total > last_overflow_total {
            let now = Instant::now();
            let should_report = last_overflow_report
                .map(|previous| now.duration_since(previous) >= OVERFLOW_REPORT_INTERVAL)
                .unwrap_or(true);
            if should_report {
                observer.on_overflow();
                last_overflow_report = Some(now);
            }
            last_overflow_total = overflow_total;
        }

        if !drained_any {
            thread::park_timeout(MONITOR_TICK);
        }
    }

    let _ = capture.session.stop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::buffer::build_pool;
    use crate::audio::{AudioCaptureSession, AudioError, AudioSource, PcmBlockSink};
    use parking_lot::Mutex;
    use std::sync::atomic::AtomicUsize;

    struct FakeSession {
        stopped: Arc<AtomicBool>,
    }
    impl AudioCaptureSession for FakeSession {
        fn source(&self) -> AudioSource {
            AudioSource::Microphone
        }
        fn stop(&mut self) -> Result<(), AudioError> {
            self.stopped.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingObserver {
        first_signal: AtomicUsize,
        overflow: AtomicUsize,
        fault: Mutex<Vec<AudioErrorKind>>,
    }
    impl MicrophoneMonitorObserver for RecordingObserver {
        fn on_first_signal(&self) {
            self.first_signal.fetch_add(1, Ordering::SeqCst);
        }
        fn on_overflow(&self) {
            self.overflow.fetch_add(1, Ordering::SeqCst);
        }
        fn on_fault(&self, kind: AudioErrorKind) {
            self.fault.lock().push(kind);
        }
    }

    fn handle_with(
        consumer: crate::audio::buffer::PoolConsumer,
    ) -> (MicrophoneCaptureHandle, Arc<AtomicBool>) {
        let stopped = Arc::new(AtomicBool::new(false));
        let handle = MicrophoneCaptureHandle {
            session: Box::new(FakeSession {
                stopped: stopped.clone(),
            }),
            consumer,
            fault: Arc::new(super::super::StreamFault::default()),
            format: crate::audio::PcmFormat {
                sample_rate_hz: std::num::NonZeroU32::new(16_000).unwrap(),
                channels: std::num::NonZeroU16::new(1).unwrap(),
            },
        };
        (handle, stopped)
    }

    fn submit_test_block(
        producer: &mut crate::audio::buffer::PoolProducer,
        samples: Box<[f32]>,
    ) -> Result<(), ()> {
        let valid_samples = samples.len();
        producer
            .try_submit(crate::audio::PcmBlock {
                source: AudioSource::Microphone,
                sequence: 0,
                format: crate::audio::PcmFormat {
                    sample_rate_hz: std::num::NonZeroU32::new(16_000).unwrap(),
                    channels: std::num::NonZeroU16::new(1).unwrap(),
                },
                valid_samples,
                samples,
            })
            .map_err(|_| ())
    }

    #[test]
    fn reports_first_signal_exactly_once_above_threshold() {
        let (mut producer, consumer) = build_pool(4);
        let (handle, stopped) = handle_with(consumer);
        let observer = Arc::new(RecordingObserver::default());
        let monitor = MicrophoneMonitor::spawn(handle, observer.clone());

        // Below threshold: no signal.
        let mut buf = producer.try_acquire().unwrap();
        buf.copy_from_slice(&[0.0, 0.001, -0.002, 0.0]);
        assert!(submit_test_block(&mut producer, buf).is_ok());

        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(observer.first_signal.load(Ordering::SeqCst), 0);

        // Above threshold: exactly one signal, even after more loud blocks.
        for _ in 0..3 {
            let mut buf = producer.try_acquire().unwrap();
            buf.copy_from_slice(&[0.5, -0.5, 0.5, -0.5]);
            assert!(submit_test_block(&mut producer, buf).is_ok());
        }

        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(observer.first_signal.load(Ordering::SeqCst), 1);

        monitor.stop();
        assert!(stopped.load(Ordering::SeqCst));
    }

    #[test]
    fn stop_joins_the_thread_and_releases_the_session() {
        let (_producer, consumer) = build_pool(4);
        let (handle, stopped) = handle_with(consumer);
        let observer = Arc::new(RecordingObserver::default());
        let monitor = MicrophoneMonitor::spawn(handle, observer);

        assert!(!stopped.load(Ordering::SeqCst));
        monitor.stop();
        assert!(stopped.load(Ordering::SeqCst));
    }

    #[test]
    fn drop_without_explicit_stop_still_releases_the_session() {
        let (_producer, consumer) = build_pool(4);
        let (handle, stopped) = handle_with(consumer);
        let observer = Arc::new(RecordingObserver::default());
        {
            let _monitor = MicrophoneMonitor::spawn(handle, observer);
        }
        assert!(stopped.load(Ordering::SeqCst));
    }

    #[test]
    fn fault_flag_ends_the_session_and_reports_exactly_once() {
        let (_producer, consumer) = build_pool(4);
        let (handle, stopped) = handle_with(consumer);
        let fault = handle.fault.clone();
        let observer = Arc::new(RecordingObserver::default());
        let monitor = MicrophoneMonitor::spawn(handle, observer.clone());

        fault.set();
        std::thread::sleep(Duration::from_millis(80));

        assert!(stopped.load(Ordering::SeqCst));
        assert_eq!(observer.fault.lock().len(), 1);

        // The monitor thread has already exited on its own; stop() still
        // joins cleanly without hanging or double-stopping.
        monitor.stop();
    }
}
