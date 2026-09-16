//! The system audio monitor: one supervisor thread per active system audio session.
//!
//! Owns the consumer half of the PCM pool, the live capture session, and
//! the ASR chunk feeder. Drains 20 ms blocks, checks signal magnitude,
//! enforces the source invariant (`AudioSource::System`), feeds ASR, and recycles buffers.

use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::audio::buffer::PoolConsumer;
use crate::audio::{AudioCaptureSession, AudioErrorKind, AudioSource};

const MONITOR_TICK: Duration = Duration::from_millis(20);
const SIGNAL_THRESHOLD: f32 = 0.01;
const OVERFLOW_REPORT_INTERVAL: Duration = Duration::from_secs(1);

pub trait SystemAudioMonitorObserver: Send + Sync + 'static {
    fn on_first_signal(&self);
    fn on_overflow(&self);
    fn on_fault(&self, error: AudioErrorKind);
}

pub struct SystemAudioMonitor {
    thread: Option<JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
    abandon_flag: Arc<AtomicBool>,
}

impl SystemAudioMonitor {
    pub fn spawn(
        session: Box<dyn AudioCaptureSession>,
        consumer: PoolConsumer,
        observer: Arc<dyn SystemAudioMonitorObserver>,
        asr_feeder: crate::asr::chunk_pool::AsrChunkFeeder,
    ) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let abandon_flag = Arc::new(AtomicBool::new(false));
        let thread_stop = stop_flag.clone();
        let thread_abandon = abandon_flag.clone();
        let thread = thread::Builder::new()
            .name("mistaken-system-monitor".into())
            .spawn(move || {
                let panic_observer = observer.clone();
                let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
                    run(
                        session,
                        consumer,
                        observer,
                        thread_stop,
                        thread_abandon,
                        asr_feeder,
                    )
                }));
                if outcome.is_err() {
                    // Panic contained; payload never logged, only this
                    // fixed sanitized description crosses the boundary.
                    panic_observer.on_fault(AudioErrorKind::Internal);
                }
            })
            .expect("failed to spawn system audio monitor thread");

        Self {
            thread: Some(thread),
            stop_flag,
            abandon_flag,
        }
    }

    /// Graceful teardown used by a user-initiated Stop: the in-flight
    /// interim (if any) is finalized normally.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            join_unless_self(thread);
        }
    }

    /// Same teardown as [`Self::stop`], except the ASR feeder is torn
    /// down through `AsrChunkFeeder::abandon` instead of `finish`: no
    /// final is emitted for whatever interim was in flight. Used only by
    /// Spec 10's fault-triggered teardown, never by a user-initiated Stop.
    pub fn abandon(&mut self) {
        self.abandon_flag.store(true, Ordering::Release);
        self.stop_flag.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            join_unless_self(thread);
        }
    }
}

impl Drop for SystemAudioMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

fn join_unless_self(join: JoinHandle<()>) {
    if thread::current().id() == join.thread().id() {
        return;
    }
    let _ = join.join();
}

fn run(
    mut session: Box<dyn AudioCaptureSession>,
    mut consumer: PoolConsumer,
    observer: Arc<dyn SystemAudioMonitorObserver>,
    stop_flag: Arc<AtomicBool>,
    abandon_flag: Arc<AtomicBool>,
    mut asr_feeder: crate::asr::chunk_pool::AsrChunkFeeder,
) {
    let mut signaled = false;
    let mut last_overflow_report: Option<Instant> = None;
    let mut last_overflow_total = 0_u64;

    loop {
        if stop_flag.load(Ordering::Acquire) {
            break;
        }

        let mut drained_any = false;
        while let Some(block) = consumer.try_recv() {
            drained_any = true;

            // Spec 09 AC 9: Source mismatch is a fatal internal error!
            if block.source != AudioSource::System {
                observer.on_fault(AudioErrorKind::Internal);
                let _ = session.stop();
                // Internal-invariant violation: state is untrusted, so
                // this exit always abandons regardless of `abandon_flag`.
                asr_feeder.abandon();
                return;
            }

            let valid = &block.samples[..block.valid_samples];
            if !signaled {
                let peak = valid
                    .iter()
                    .fold(0.0_f32, |max, sample| max.max(sample.abs()));
                if peak >= SIGNAL_THRESHOLD {
                    signaled = true;
                    observer.on_first_signal();
                }
            }

            // Copy into current ASR chunk before recycling
            asr_feeder.accept(valid);

            if consumer.recycle(block).is_err() {
                observer.on_fault(AudioErrorKind::Internal);
                let _ = session.stop();
                asr_feeder.abandon();
                return;
            }
        }

        let overflow_total = consumer.overflow_total();
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

    let _ = session.stop();
    if abandon_flag.load(Ordering::Acquire) {
        asr_feeder.abandon();
    } else {
        asr_feeder.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::chunk_pool::build_asr_pool;
    use crate::audio::buffer::build_pool;
    use crate::audio::{AudioError, AudioSource, PcmBlock, PcmBlockSink, PcmFormat};
    use parking_lot::Mutex;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::sync::atomic::AtomicUsize;

    struct MockCaptureSession {
        stopped: Arc<AtomicBool>,
    }

    impl AudioCaptureSession for MockCaptureSession {
        fn source(&self) -> AudioSource {
            AudioSource::System
        }

        fn stop(&mut self) -> Result<(), AudioError> {
            self.stopped.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    struct TestObserver {
        first_signal: Arc<AtomicBool>,
        overflow: Arc<AtomicUsize>,
        fault: Arc<Mutex<Option<AudioErrorKind>>>,
    }

    impl SystemAudioMonitorObserver for TestObserver {
        fn on_first_signal(&self) {
            self.first_signal.store(true, Ordering::SeqCst);
        }
        fn on_overflow(&self) {
            self.overflow.fetch_add(1, Ordering::SeqCst);
        }
        fn on_fault(&self, error: AudioErrorKind) {
            *self.fault.lock() = Some(error);
        }
    }

    #[test]
    fn system_monitor_processes_blocks_and_signals_threshold() {
        let stopped = Arc::new(AtomicBool::new(false));
        let session = Box::new(MockCaptureSession {
            stopped: stopped.clone(),
        });
        let (mut producer, consumer) = build_pool(960);
        let (asr_producer, _) = build_asr_pool(4800);
        let asr_feeder = crate::asr::chunk_pool::AsrChunkFeeder::new(asr_producer, 4800);

        let observer = Arc::new(TestObserver {
            first_signal: Arc::new(AtomicBool::new(false)),
            overflow: Arc::new(AtomicUsize::new(0)),
            fault: Arc::new(Mutex::new(None)),
        });

        let mut monitor =
            SystemAudioMonitor::spawn(session, consumer, observer.clone(), asr_feeder);

        // Submit quiet block: peak < 0.01
        let mut buf = producer.try_acquire().unwrap();
        buf[0..960].fill(0.005);
        let block = PcmBlock {
            source: AudioSource::System,
            sequence: 0,
            format: PcmFormat {
                sample_rate_hz: NonZeroU32::new(48000).unwrap(),
                channels: NonZeroU16::new(1).unwrap(),
            },
            valid_samples: 960,
            samples: buf,
        };
        assert!(producer.try_submit(block).is_ok());

        std::thread::sleep(Duration::from_millis(50));
        assert!(!observer.first_signal.load(Ordering::SeqCst));

        // Submit loud block: peak >= 0.01
        let mut buf2 = producer.try_acquire().unwrap();
        buf2[0..960].fill(0.05);
        let block2 = PcmBlock {
            source: AudioSource::System,
            sequence: 1,
            format: PcmFormat {
                sample_rate_hz: NonZeroU32::new(48000).unwrap(),
                channels: NonZeroU16::new(1).unwrap(),
            },
            valid_samples: 960,
            samples: buf2,
        };
        assert!(producer.try_submit(block2).is_ok());

        std::thread::sleep(Duration::from_millis(50));
        assert!(observer.first_signal.load(Ordering::SeqCst));

        monitor.stop();
        assert!(stopped.load(Ordering::SeqCst));
    }

    #[test]
    fn system_monitor_rejects_source_mismatch_as_internal_fault() {
        let stopped = Arc::new(AtomicBool::new(false));
        let session = Box::new(MockCaptureSession {
            stopped: stopped.clone(),
        });
        let (mut producer, consumer) = build_pool(960);
        let (asr_producer, _) = build_asr_pool(4800);
        let asr_feeder = crate::asr::chunk_pool::AsrChunkFeeder::new(asr_producer, 4800);

        let fault = Arc::new(Mutex::new(None));
        let observer = Arc::new(TestObserver {
            first_signal: Arc::new(AtomicBool::new(false)),
            overflow: Arc::new(AtomicUsize::new(0)),
            fault: fault.clone(),
        });

        let mut monitor = SystemAudioMonitor::spawn(session, consumer, observer, asr_feeder);

        // Submit block with source = Microphone instead of System!
        let mut buf = producer.try_acquire().unwrap();
        buf[0..960].fill(0.05);
        let block = PcmBlock {
            source: AudioSource::Microphone, // WRONG SOURCE
            sequence: 0,
            format: PcmFormat {
                sample_rate_hz: NonZeroU32::new(48000).unwrap(),
                channels: NonZeroU16::new(1).unwrap(),
            },
            valid_samples: 960,
            samples: buf,
        };
        assert!(producer.try_submit(block).is_ok());

        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(*fault.lock(), Some(AudioErrorKind::Internal));
        assert!(stopped.load(Ordering::SeqCst));

        monitor.stop();
    }

    struct DropReleasedSession {
        released: Arc<AtomicBool>,
    }
    impl AudioCaptureSession for DropReleasedSession {
        fn source(&self) -> AudioSource {
            AudioSource::System
        }
        fn stop(&mut self) -> Result<(), AudioError> {
            Ok(())
        }
    }
    impl Drop for DropReleasedSession {
        fn drop(&mut self) {
            self.released.store(true, Ordering::SeqCst);
        }
    }

    struct PanicOnFirstSignalObserver {
        fault: Arc<Mutex<Option<AudioErrorKind>>>,
    }
    impl SystemAudioMonitorObserver for PanicOnFirstSignalObserver {
        fn on_first_signal(&self) {
            panic!("injected panic for Spec 10 containment test");
        }
        fn on_overflow(&self) {}
        fn on_fault(&self, error: AudioErrorKind) {
            *self.fault.lock() = Some(error);
        }
    }

    #[test]
    fn panic_in_observer_callback_is_contained_reported_and_still_releases_the_session() {
        let released = Arc::new(AtomicBool::new(false));
        let session = Box::new(DropReleasedSession {
            released: released.clone(),
        });
        let (mut producer, consumer) = build_pool(960);
        let (asr_producer, _) = build_asr_pool(4800);
        let asr_feeder = crate::asr::chunk_pool::AsrChunkFeeder::new(asr_producer, 4800);

        let fault = Arc::new(Mutex::new(None));
        let observer = Arc::new(PanicOnFirstSignalObserver {
            fault: fault.clone(),
        });
        let mut monitor = SystemAudioMonitor::spawn(session, consumer, observer, asr_feeder);

        let mut buf = producer.try_acquire().unwrap();
        buf[0..960].fill(0.5);
        let block = PcmBlock {
            source: AudioSource::System,
            sequence: 0,
            format: PcmFormat {
                sample_rate_hz: NonZeroU32::new(48000).unwrap(),
                channels: NonZeroU16::new(1).unwrap(),
            },
            valid_samples: 960,
            samples: buf,
        };
        assert!(producer.try_submit(block).is_ok());

        let deadline = Instant::now() + Duration::from_secs(5);
        while fault.lock().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(
            *fault.lock(),
            Some(AudioErrorKind::Internal),
            "the panic must be reported as exactly one internal fault, never propagated"
        );
        assert!(
            released.load(Ordering::SeqCst),
            "the capture session must still be released even though the panic \
             happened before the normal tail teardown ran"
        );

        monitor.stop();
    }
}
