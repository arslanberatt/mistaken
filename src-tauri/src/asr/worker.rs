//! The ASR worker: one dedicated thread per active source that drains the
//! bounded chunk pool, drives a [`StreamingRecognizer`], and emits
//! segment-identified partial/final results through
//! [`AsrWorkerObserver`]. Never touches Tauri, serialization, or
//! transcript state directly.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use super::chunk_pool::AsrChunkConsumer;
use super::recognizer::{AsrError, RecognizedSegment, StreamingRecognizer};
use super::{LAGGING_REPORT_INTERVAL, PARTIAL_THROTTLE, WORKER_TICK};

/// Observed directly on the worker thread; implemented by the Tauri-facing
/// runtime layer. Every method must be cheap, non-blocking, and
/// panic-free.
pub trait AsrWorkerObserver: Send + Sync + 'static {
    fn on_partial(&self, segment_id: &str, text: &str, started_at_ms: u64);
    fn on_final(&self, segment_id: &str, text: &str, started_at_ms: u64, ended_at_ms: u64);
    /// The ASR chunk pool dropped at least one chunk's worth of audio
    /// since the last report; capture and transcription continue.
    fn on_lagging(&self);
    /// A recognizer error ended the session; every resource is released
    /// by the caller, the existing transcript is preserved.
    fn on_error(&self, error: AsrError);
}

/// Collapses runs of internal whitespace to one space; trimming already
/// happened at the recognizer boundary. This is the single point where
/// Spec 06's full text-normalization contract (trim + collapse, nothing
/// else) is applied, uniformly, regardless of which recognizer produced
/// the text.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct SegmentTracker {
    session_id: u64,
    segment_index: u64,
    sample_rate_hz: u32,
    fed_samples: u64,
    started_at_ms: Option<u64>,
    last_partial_text: Option<String>,
    last_partial_emit: Option<Instant>,
}

impl SegmentTracker {
    fn new(session_id: u64, sample_rate_hz: u32) -> Self {
        Self {
            session_id,
            segment_index: 0,
            sample_rate_hz,
            fed_samples: 0,
            started_at_ms: None,
            last_partial_text: None,
            last_partial_emit: None,
        }
    }

    fn segment_id(&self) -> String {
        format!("mic-{}-{}", self.session_id, self.segment_index)
    }

    fn ms_for(&self, samples: u64) -> u64 {
        (u128::from(samples) * 1000 / u128::from(self.sample_rate_hz.max(1))) as u64
    }

    fn current_ms(&self) -> u64 {
        self.ms_for(self.fed_samples)
    }

    fn record_fed_samples(&mut self, count: usize) {
        self.fed_samples += count as u64;
    }

    fn handle(&mut self, segment: RecognizedSegment, observer: &dyn AsrWorkerObserver) {
        let text = collapse_whitespace(&segment.text);

        if segment.is_final {
            if !text.is_empty() {
                let started = self.started_at_ms.unwrap_or_else(|| self.current_ms());
                let ended = self.current_ms();
                observer.on_final(&self.segment_id(), &text, started, ended);
                self.segment_index += 1;
            }
            self.started_at_ms = None;
            self.last_partial_text = None;
            self.last_partial_emit = None;
            return;
        }

        if text.is_empty() {
            return;
        }
        if self.started_at_ms.is_none() {
            self.started_at_ms = Some(self.current_ms());
        }
        if self.last_partial_text.as_deref() == Some(text.as_str()) {
            return;
        }
        let now = Instant::now();
        if let Some(previous) = self.last_partial_emit {
            if now.duration_since(previous) < PARTIAL_THROTTLE {
                return;
            }
        }
        self.last_partial_text = Some(text.clone());
        self.last_partial_emit = Some(now);
        observer.on_partial(&self.segment_id(), &text, self.started_at_ms.unwrap_or(0));
    }
}

/// A running ASR worker thread. Dropping or calling [`AsrWorker::stop`]
/// both join the thread; termination is otherwise fully driven by the
/// chunk consumer reaching "no chunk available and finished" (see
/// `AsrChunkConsumer::is_finished`) — there is no separate stop flag.
pub struct AsrWorker {
    join: Option<JoinHandle<()>>,
}

impl AsrWorker {
    pub fn spawn(
        consumer: AsrChunkConsumer,
        stream: Box<dyn StreamingRecognizer>,
        sample_rate_hz: u32,
        session_id: u64,
        observer: Arc<dyn AsrWorkerObserver>,
    ) -> Self {
        let join = thread::Builder::new()
            .name("mistaken-asr-worker".into())
            .spawn(move || run(consumer, stream, sample_rate_hz, session_id, observer))
            .expect("failed to spawn ASR worker thread");
        Self { join: Some(join) }
    }

    /// Blocks until the worker has drained remaining chunks, run the
    /// bounded finish drain, and exited. Safe to call at most once;
    /// `Drop` is the fallback for callers that skip it.
    pub fn stop(mut self) {
        if let Some(join) = self.join.take() {
            join_unless_self(join);
        }
    }
}

impl Drop for AsrWorker {
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            join_unless_self(join);
        }
    }
}

/// Mirrors `MicrophoneMonitor`'s self-join guard: an observer callback
/// that drops the worker from inside the worker thread itself must not
/// join that thread (`pthread_join`/`WaitForSingleObject` on one's own
/// thread deadlocks or hangs forever).
fn join_unless_self(join: JoinHandle<()>) {
    if join.thread().id() == thread::current().id() {
        return;
    }
    let _ = join.join();
}

fn run(
    mut consumer: AsrChunkConsumer,
    mut stream: Box<dyn StreamingRecognizer>,
    sample_rate_hz: u32,
    session_id: u64,
    observer: Arc<dyn AsrWorkerObserver>,
) {
    let mut tracker = SegmentTracker::new(session_id, sample_rate_hz);
    let mut segments: Vec<RecognizedSegment> = Vec::new();
    let mut last_overflow_report: Option<Instant> = None;
    let mut last_overflow_total = 0_u64;

    'outer: loop {
        let mut drained_any = false;
        while let Some(chunk) = consumer.try_recv() {
            drained_any = true;
            let valid = &chunk.samples[..chunk.valid_samples];

            if let Err(error) = stream.accept(valid) {
                observer.on_error(error);
                break 'outer;
            }
            tracker.record_fed_samples(chunk.valid_samples);

            segments.clear();
            if let Err(error) = stream.poll(&mut segments) {
                observer.on_error(error);
                break 'outer;
            }
            for segment in segments.drain(..) {
                tracker.handle(segment, observer.as_ref());
            }

            let _ = consumer.recycle(chunk.samples);
        }

        let overflow_total = consumer.overflow_total();
        if overflow_total > last_overflow_total {
            let now = Instant::now();
            let should_report = last_overflow_report
                .map(|previous| now.duration_since(previous) >= LAGGING_REPORT_INTERVAL)
                .unwrap_or(true);
            if should_report {
                observer.on_lagging();
                last_overflow_report = Some(now);
            }
            last_overflow_total = overflow_total;
        }

        if !drained_any {
            if consumer.is_finished() {
                break;
            }
            thread::park_timeout(WORKER_TICK);
        }
    }

    segments.clear();
    if let Err(error) = stream.finish(&mut segments) {
        observer.on_error(error);
        return;
    }
    for segment in segments.drain(..) {
        tracker.handle(segment, observer.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::chunk_pool::{build_asr_pool, AsrChunkFeeder};
    use parking_lot::Mutex as PlMutex;
    use std::sync::atomic::{AtomicU64 as StdAtomicU64, Ordering};
    use std::time::Duration;

    #[derive(Debug, Clone, PartialEq)]
    enum Event {
        Partial {
            id: String,
            text: String,
            started_at_ms: u64,
        },
        Final {
            id: String,
            text: String,
            started_at_ms: u64,
            ended_at_ms: u64,
        },
        Lagging,
        Error,
    }

    #[derive(Default)]
    struct RecordingObserver {
        events: PlMutex<Vec<Event>>,
    }
    impl AsrWorkerObserver for RecordingObserver {
        fn on_partial(&self, segment_id: &str, text: &str, started_at_ms: u64) {
            self.events.lock().push(Event::Partial {
                id: segment_id.to_string(),
                text: text.to_string(),
                started_at_ms,
            });
        }
        fn on_final(&self, segment_id: &str, text: &str, started_at_ms: u64, ended_at_ms: u64) {
            self.events.lock().push(Event::Final {
                id: segment_id.to_string(),
                text: text.to_string(),
                started_at_ms,
                ended_at_ms,
            });
        }
        fn on_lagging(&self) {
            self.events.lock().push(Event::Lagging);
        }
        fn on_error(&self, _error: AsrError) {
            self.events.lock().push(Event::Error);
        }
    }

    /// A fake recognizer that echoes scripted `RecognizedSegment`s back
    /// verbatim on the next `poll()`/`finish()` call. Proves the worker
    /// applies no transformation beyond whitespace collapsing — there is
    /// no dictionary, no correction, no dependency on sherpa-onnx.
    struct ScriptedRecognizer {
        accepted_samples: StdAtomicU64,
        script: PlMutex<Vec<RecognizedSegment>>,
        finish_script: PlMutex<Vec<RecognizedSegment>>,
    }
    impl StreamingRecognizer for ScriptedRecognizer {
        fn accept(&mut self, samples: &[f32]) -> Result<(), AsrError> {
            self.accepted_samples
                .fetch_add(samples.len() as u64, Ordering::SeqCst);
            Ok(())
        }
        fn poll(&mut self, out: &mut Vec<RecognizedSegment>) -> Result<(), AsrError> {
            out.extend(self.script.lock().drain(..));
            Ok(())
        }
        fn finish(&mut self, out: &mut Vec<RecognizedSegment>) -> Result<(), AsrError> {
            out.extend(self.finish_script.lock().drain(..));
            Ok(())
        }
    }

    fn spawn_worker_with_script(
        script: Vec<RecognizedSegment>,
        finish_script: Vec<RecognizedSegment>,
    ) -> (AsrChunkFeeder, AsrWorker, Arc<RecordingObserver>) {
        let (producer, consumer) = build_asr_pool(4);
        let feeder = AsrChunkFeeder::new(producer, 4);
        let recognizer: Box<dyn StreamingRecognizer> = Box::new(ScriptedRecognizer {
            accepted_samples: StdAtomicU64::new(0),
            script: PlMutex::new(script),
            finish_script: PlMutex::new(finish_script),
        });
        let observer = Arc::new(RecordingObserver::default());
        let worker = AsrWorker::spawn(consumer, recognizer, 16_000, 7, observer.clone());
        (feeder, worker, observer)
    }

    #[test]
    fn emits_verbatim_text_with_only_whitespace_collapsed() {
        let (mut feeder, worker, observer) = spawn_worker_with_script(
            vec![RecognizedSegment {
                text: "I  actually   have went".to_string(),
                is_final: false,
            }],
            Vec::new(),
        );
        feeder.accept(&[0.0; 4]);
        std::thread::sleep(Duration::from_millis(80));
        drop(feeder);
        worker.stop();

        let events = observer.events.lock().clone();
        assert_eq!(
            events,
            vec![Event::Partial {
                id: "mic-7-0".to_string(),
                text: "I actually have went".to_string(),
                started_at_ms: 0,
            }]
        );
    }

    #[test]
    fn final_segment_increments_segment_index_and_resets_partial_state() {
        let (mut feeder, worker, observer) = spawn_worker_with_script(
            vec![
                RecognizedSegment {
                    text: "hello".to_string(),
                    is_final: false,
                },
                RecognizedSegment {
                    text: "hello there".to_string(),
                    is_final: true,
                },
            ],
            Vec::new(),
        );
        feeder.accept(&[0.0; 4]);
        std::thread::sleep(Duration::from_millis(80));
        drop(feeder);
        worker.stop();

        let events = observer.events.lock().clone();
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], Event::Partial { id, .. } if id == "mic-7-0"));
        assert!(matches!(&events[1], Event::Final { id, .. } if id == "mic-7-0"));
    }

    #[test]
    fn empty_text_segments_are_never_emitted() {
        let (mut feeder, worker, observer) = spawn_worker_with_script(
            vec![
                RecognizedSegment {
                    text: "   ".to_string(),
                    is_final: false,
                },
                RecognizedSegment {
                    text: "".to_string(),
                    is_final: true,
                },
            ],
            Vec::new(),
        );
        feeder.accept(&[0.0; 4]);
        std::thread::sleep(Duration::from_millis(80));
        drop(feeder);
        worker.stop();

        assert!(observer.events.lock().is_empty());
    }

    #[test]
    fn finish_drain_emits_the_closing_final_after_teardown() {
        let (mut feeder, worker, observer) = spawn_worker_with_script(
            Vec::new(),
            vec![RecognizedSegment {
                text: "closing line".to_string(),
                is_final: true,
            }],
        );
        feeder.accept(&[0.0; 2]); // partial chunk, flushed by feeder's finish()
        drop(feeder); // signals finished; worker proceeds to bounded finish drain
        worker.stop();

        let events = observer.events.lock().clone();
        assert_eq!(
            events,
            vec![Event::Final {
                id: "mic-7-0".to_string(),
                text: "closing line".to_string(),
                started_at_ms: 0,
                ended_at_ms: 0,
            }]
        );
    }

    #[test]
    fn worker_exits_promptly_once_feeder_finishes_with_no_pending_chunks() {
        let (feeder, worker, _observer) = spawn_worker_with_script(Vec::new(), Vec::new());
        drop(feeder);
        // `stop()` blocks until the thread exits; a hang here fails the test.
        worker.stop();
    }
}
