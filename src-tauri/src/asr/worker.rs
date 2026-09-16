//! The ASR worker: one dedicated thread per active source that drains the
//! bounded chunk pool, drives a [`StreamingRecognizer`], and emits
//! segment-identified partial/final results through
//! [`AsrWorkerObserver`]. Never touches Tauri, serialization, or
//! transcript state directly.

use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::audio::supervisor::LagWindow;

use super::chunk_pool::AsrChunkConsumer;
use super::recognizer::{AsrErrorKind, RecognizedSegment, StreamingRecognizer};
use super::AsrError;
use super::{
    LAGGING_REPORT_INTERVAL, LAG_THRESHOLD_PERCENT, LAG_WINDOW, PARTIAL_THROTTLE, WORKER_TICK,
};
use crate::audio::AudioSource;

/// Observed directly on the worker thread; implemented by the Tauri-facing
/// runtime layer. Every method must be cheap, non-blocking, and
/// panic-free.
pub trait AsrWorkerObserver: Send + Sync + 'static {
    fn on_partial(&self, source: AudioSource, segment_id: &str, text: &str, started_at_ms: u64);
    fn on_final(
        &self,
        source: AudioSource,
        segment_id: &str,
        text: &str,
        started_at_ms: u64,
        ended_at_ms: u64,
    );
    /// The ASR chunk pool dropped at least one chunk's worth of audio
    /// since the last report; capture and transcription continue.
    fn on_lagging(&self, source: AudioSource);
    /// Sustained-lag state transition over a completed 10 s window
    /// (`true` entering degraded, `false` leaving it). Distinct from
    /// `on_lagging`, which continues to fire on every new drop exactly
    /// as before; this fires only on the state edge.
    fn on_lag_changed(&self, source: AudioSource, lagging: bool);
    /// A recognizer error ended the session; every resource is released
    /// by the caller, the existing transcript is preserved.
    fn on_error(&self, source: AudioSource, error: AsrError);
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
    source: AudioSource,
    session_id: u64,
    segment_index: u64,
    sample_rate_hz: u32,
    source_start_offset_ms: u64,
    fed_samples: u64,
    started_at_ms: Option<u64>,
    last_partial_text: Option<String>,
    last_partial_emit: Option<Instant>,
}

impl SegmentTracker {
    fn new(
        source: AudioSource,
        session_id: u64,
        sample_rate_hz: u32,
        source_start_offset_ms: u64,
        segment_index_start: u64,
    ) -> Self {
        Self {
            source,
            session_id,
            segment_index: segment_index_start,
            sample_rate_hz,
            source_start_offset_ms,
            fed_samples: 0,
            started_at_ms: None,
            last_partial_text: None,
            last_partial_emit: None,
        }
    }

    fn segment_id(&self) -> String {
        match self.source {
            AudioSource::Microphone => format!("mic-{}-{}", self.session_id, self.segment_index),
            AudioSource::System => format!("sys-{}-{}", self.session_id, self.segment_index),
        }
    }

    fn ms_for(&self, samples: u64) -> u64 {
        self.source_start_offset_ms
            + (u128::from(samples) * 1000 / u128::from(self.sample_rate_hz.max(1))) as u64
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
                observer.on_final(self.source, &self.segment_id(), &text, started, ended);
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
        observer.on_partial(
            self.source,
            &self.segment_id(),
            &text,
            self.started_at_ms.unwrap_or(self.source_start_offset_ms),
        );
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
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        source: AudioSource,
        consumer: AsrChunkConsumer,
        stream: Box<dyn StreamingRecognizer>,
        sample_rate_hz: u32,
        session_id: u64,
        source_start_offset_ms: u64,
        segment_index_start: u64,
        observer: Arc<dyn AsrWorkerObserver>,
    ) -> Self {
        let name = match source {
            AudioSource::Microphone => "mistaken-asr-mic-worker",
            AudioSource::System => "mistaken-asr-sys-worker",
        };
        let join = thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let panic_observer = observer.clone();
                let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
                    run(
                        source,
                        consumer,
                        stream,
                        sample_rate_hz,
                        session_id,
                        source_start_offset_ms,
                        segment_index_start,
                        observer,
                    )
                }));
                if outcome.is_err() {
                    // Panic contained: converted to one `internal`
                    // source-terminal error with full resource release
                    // via normal unwind-drop of every local this closure
                    // owned. The payload itself is never logged (it can
                    // contain arbitrary data); only this fixed,
                    // sanitized description crosses the boundary.
                    panic_observer.on_error(
                        source,
                        AsrError::new(AsrErrorKind::Internal, "asr worker panicked (contained)"),
                    );
                }
            })
            .expect("failed to spawn ASR worker thread");
        Self { join: Some(join) }
    }

    pub fn stop(&mut self) {
        if let Some(join) = self.join.take() {
            join_unless_self(join);
        }
    }
}

impl Drop for AsrWorker {
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

#[allow(clippy::too_many_arguments)]
fn run(
    source: AudioSource,
    mut consumer: AsrChunkConsumer,
    mut stream: Box<dyn StreamingRecognizer>,
    sample_rate_hz: u32,
    session_id: u64,
    source_start_offset_ms: u64,
    segment_index_start: u64,
    observer: Arc<dyn AsrWorkerObserver>,
) {
    let mut tracker = SegmentTracker::new(
        source,
        session_id,
        sample_rate_hz,
        source_start_offset_ms,
        segment_index_start,
    );
    let mut segments: Vec<RecognizedSegment> = Vec::new();
    let mut last_overflow_report: Option<Instant> = None;
    let mut last_overflow_total = 0_u64;
    let mut lag_window_last_overflow_total = 0_u64;
    let mut lag_window = LagWindow::new(LAG_WINDOW, LAG_THRESHOLD_PERCENT);

    'outer: loop {
        let mut drained_any = false;
        let mut drained_this_tick = 0_u64;
        while let Some(chunk) = consumer.try_recv() {
            drained_any = true;
            drained_this_tick += 1;
            let valid = &chunk.samples[..chunk.valid_samples];

            if let Err(error) = stream.accept(valid) {
                observer.on_error(source, error);
                break 'outer;
            }
            tracker.record_fed_samples(chunk.valid_samples);

            segments.clear();
            if let Err(error) = stream.poll(&mut segments) {
                observer.on_error(source, error);
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
                observer.on_lagging(source);
                last_overflow_report = Some(now);
            }
            last_overflow_total = overflow_total;
        }

        let dropped_this_tick = overflow_total - lag_window_last_overflow_total;
        lag_window_last_overflow_total = overflow_total;
        if let Some(lagging) = lag_window.record(drained_this_tick, dropped_this_tick) {
            observer.on_lag_changed(source, lagging);
        }

        if !drained_any {
            if consumer.is_finished() {
                break;
            }
            thread::park_timeout(WORKER_TICK);
        }
    }

    // Abandoned (Spec 10 fault-triggered teardown): the in-flight interim
    // is intentionally left unfinalized. Never call `finish()` here —
    // doing so could emit a final for audio the recognizer already
    // buffered even though no new chunk was submitted.
    if consumer.is_abandoned() {
        return;
    }

    segments.clear();
    if let Err(error) = stream.finish(&mut segments) {
        observer.on_error(source, error);
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
            source: AudioSource,
            id: String,
            text: String,
            started_at_ms: u64,
        },
        Final {
            source: AudioSource,
            id: String,
            text: String,
            started_at_ms: u64,
            ended_at_ms: u64,
        },
        Lagging(AudioSource),
        LagChanged(AudioSource, bool),
        Error(AudioSource),
    }

    #[derive(Default)]
    struct RecordingObserver {
        events: PlMutex<Vec<Event>>,
    }
    impl AsrWorkerObserver for RecordingObserver {
        fn on_partial(
            &self,
            source: AudioSource,
            segment_id: &str,
            text: &str,
            started_at_ms: u64,
        ) {
            self.events.lock().push(Event::Partial {
                source,
                id: segment_id.to_string(),
                text: text.to_string(),
                started_at_ms,
            });
        }
        fn on_final(
            &self,
            source: AudioSource,
            segment_id: &str,
            text: &str,
            started_at_ms: u64,
            ended_at_ms: u64,
        ) {
            self.events.lock().push(Event::Final {
                source,
                id: segment_id.to_string(),
                text: text.to_string(),
                started_at_ms,
                ended_at_ms,
            });
        }
        fn on_lagging(&self, source: AudioSource) {
            self.events.lock().push(Event::Lagging(source));
        }
        fn on_lag_changed(&self, source: AudioSource, lagging: bool) {
            self.events.lock().push(Event::LagChanged(source, lagging));
        }
        fn on_error(&self, source: AudioSource, _error: AsrError) {
            self.events.lock().push(Event::Error(source));
        }
    }

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
        source: AudioSource,
        script: Vec<RecognizedSegment>,
        finish_script: Vec<RecognizedSegment>,
        offset_ms: u64,
    ) -> (AsrChunkFeeder, AsrWorker, Arc<RecordingObserver>) {
        let (producer, consumer) = build_asr_pool(4);
        let feeder = AsrChunkFeeder::new(producer, 4);
        let recognizer: Box<dyn StreamingRecognizer> = Box::new(ScriptedRecognizer {
            accepted_samples: StdAtomicU64::new(0),
            script: PlMutex::new(script),
            finish_script: PlMutex::new(finish_script),
        });
        let observer = Arc::new(RecordingObserver::default());
        let worker = AsrWorker::spawn(
            source,
            consumer,
            recognizer,
            16_000,
            7,
            offset_ms,
            0,
            observer.clone(),
        );
        (feeder, worker, observer)
    }

    #[test]
    fn emits_verbatim_text_with_only_whitespace_collapsed() {
        let (mut feeder, mut worker, observer) = spawn_worker_with_script(
            AudioSource::Microphone,
            vec![RecognizedSegment {
                text: "I  actually   have went".to_string(),
                is_final: false,
            }],
            Vec::new(),
            0,
        );
        feeder.accept(&[0.0; 4]);
        std::thread::sleep(Duration::from_millis(80));
        drop(feeder);
        worker.stop();

        let events = observer.events.lock().clone();
        assert_eq!(
            events,
            vec![Event::Partial {
                source: AudioSource::Microphone,
                id: "mic-7-0".to_string(),
                text: "I actually have went".to_string(),
                started_at_ms: 0,
            }]
        );
    }

    #[test]
    fn final_segment_increments_segment_index_and_resets_partial_state() {
        let (mut feeder, mut worker, observer) = spawn_worker_with_script(
            AudioSource::Microphone,
            vec![
                RecognizedSegment {
                    text: "first sentence".to_string(),
                    is_final: true,
                },
                RecognizedSegment {
                    text: "second".to_string(),
                    is_final: false,
                },
            ],
            Vec::new(),
            0,
        );
        feeder.accept(&[0.0; 4]);
        std::thread::sleep(Duration::from_millis(80));
        drop(feeder);
        worker.stop();

        let events = observer.events.lock().clone();
        assert_eq!(
            events,
            vec![
                Event::Final {
                    source: AudioSource::Microphone,
                    id: "mic-7-0".to_string(),
                    text: "first sentence".to_string(),
                    started_at_ms: 0,
                    ended_at_ms: 0,
                },
                Event::Partial {
                    source: AudioSource::Microphone,
                    id: "mic-7-1".to_string(),
                    text: "second".to_string(),
                    started_at_ms: 0,
                },
            ]
        );
    }

    #[test]
    fn empty_text_segments_are_never_emitted() {
        let (mut feeder, mut worker, observer) = spawn_worker_with_script(
            AudioSource::Microphone,
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
            0,
        );
        feeder.accept(&[0.0; 4]);
        std::thread::sleep(Duration::from_millis(80));
        drop(feeder);
        worker.stop();

        let events = observer.events.lock().clone();
        assert!(events.is_empty());
    }

    #[test]
    fn system_source_generates_sys_ids_and_offsets() {
        let (mut feeder, mut worker, observer) = spawn_worker_with_script(
            AudioSource::System,
            vec![
                RecognizedSegment {
                    text: "Hello from system".to_string(),
                    is_final: false,
                },
                RecognizedSegment {
                    text: "Hello from system".to_string(),
                    is_final: true,
                },
            ],
            Vec::new(),
            150, // 150 ms offset
        );
        feeder.accept(&[0.0; 16]);
        std::thread::sleep(Duration::from_millis(50));
        drop(feeder);
        worker.stop();

        let events = observer.events.lock().clone();
        assert_eq!(events.len(), 2);
        match &events[0] {
            Event::Partial {
                source,
                id,
                text,
                started_at_ms,
            } => {
                assert_eq!(*source, AudioSource::System);
                assert_eq!(id, "sys-7-0");
                assert_eq!(text, "Hello from system");
                assert_eq!(*started_at_ms, 150);
            }
            other => panic!("expected partial, got {:?}", other),
        }
        match &events[1] {
            Event::Final {
                source,
                id,
                text,
                started_at_ms,
                ended_at_ms,
            } => {
                assert_eq!(*source, AudioSource::System);
                assert_eq!(id, "sys-7-0");
                assert_eq!(text, "Hello from system");
                assert_eq!(*started_at_ms, 150);
                assert_eq!(*ended_at_ms, 150);
                // Spec 09 AC 10: native text must NEVER contain leading "- "!
                assert!(!text.starts_with("- "));
            }
            other => panic!("expected final, got {:?}", other),
        }
    }

    #[test]
    fn worker_exits_promptly_once_feeder_finishes_with_no_pending_chunks() {
        let (producer, consumer) = build_asr_pool(4);
        let feeder = AsrChunkFeeder::new(producer, 4);
        let recognizer: Box<dyn StreamingRecognizer> = Box::new(ScriptedRecognizer {
            accepted_samples: StdAtomicU64::new(0),
            script: PlMutex::new(Vec::new()),
            finish_script: PlMutex::new(Vec::new()),
        });
        let observer = Arc::new(RecordingObserver::default());
        let mut worker = AsrWorker::spawn(
            AudioSource::Microphone,
            consumer,
            recognizer,
            16_000,
            1,
            0,
            0,
            observer,
        );

        drop(feeder);
        let start = Instant::now();
        worker.stop();
        assert!(start.elapsed() < Duration::from_millis(300));
    }

    #[test]
    fn finish_drain_emits_the_closing_final_after_teardown() {
        let (producer, consumer) = build_asr_pool(4);
        let feeder = AsrChunkFeeder::new(producer, 4);
        let recognizer: Box<dyn StreamingRecognizer> = Box::new(ScriptedRecognizer {
            accepted_samples: StdAtomicU64::new(0),
            script: PlMutex::new(Vec::new()),
            finish_script: PlMutex::new(vec![RecognizedSegment {
                text: "closing utterance".to_string(),
                is_final: true,
            }]),
        });
        let observer = Arc::new(RecordingObserver::default());
        let mut worker = AsrWorker::spawn(
            AudioSource::Microphone,
            consumer,
            recognizer,
            16_000,
            42,
            0,
            0,
            observer.clone(),
        );

        drop(feeder);
        worker.stop();

        let events = observer.events.lock().clone();
        assert_eq!(
            events,
            vec![Event::Final {
                source: AudioSource::Microphone,
                id: "mic-42-0".to_string(),
                text: "closing utterance".to_string(),
                started_at_ms: 0,
                ended_at_ms: 0,
            }]
        );
    }

    struct PanickingRecognizer;
    impl StreamingRecognizer for PanickingRecognizer {
        fn accept(&mut self, _samples: &[f32]) -> Result<(), AsrError> {
            panic!("injected panic for Spec 10 containment test");
        }
        fn poll(&mut self, _out: &mut Vec<RecognizedSegment>) -> Result<(), AsrError> {
            Ok(())
        }
        fn finish(&mut self, _out: &mut Vec<RecognizedSegment>) -> Result<(), AsrError> {
            Ok(())
        }
    }

    #[test]
    fn injected_panic_in_stream_accept_is_contained_and_reported_as_internal() {
        let (producer, consumer) = build_asr_pool(4);
        let mut feeder = AsrChunkFeeder::new(producer, 4);
        let recognizer: Box<dyn StreamingRecognizer> = Box::new(PanickingRecognizer);
        let observer = Arc::new(RecordingObserver::default());
        let mut worker = AsrWorker::spawn(
            AudioSource::Microphone,
            consumer,
            recognizer,
            16_000,
            1,
            0,
            0,
            observer.clone(),
        );

        // A full chunk reaches `stream.accept()`, which panics.
        feeder.accept(&[0.1, 0.2, 0.3, 0.4]);
        // `stop()` joins the worker thread; the panic was already caught
        // inside the spawned closure, so the thread exits normally rather
        // than propagating a panic to this join — a process abort here
        // would fail this test outright.
        worker.stop();

        let events = observer.events.lock().clone();
        assert_eq!(events, vec![Event::Error(AudioSource::Microphone)]);
    }

    #[test]
    fn abandon_never_emits_a_final_for_the_in_flight_interim() {
        let (producer, consumer) = build_asr_pool(4);
        let mut feeder = AsrChunkFeeder::new(producer, 4);
        // `finish_script` carries a final that would be emitted by a
        // *graceful* stop; abandon must never reach it.
        let recognizer: Box<dyn StreamingRecognizer> = Box::new(ScriptedRecognizer {
            accepted_samples: StdAtomicU64::new(0),
            script: PlMutex::new(vec![RecognizedSegment {
                text: "still speaking".to_string(),
                is_final: false,
            }]),
            finish_script: PlMutex::new(vec![RecognizedSegment {
                text: "would-be final".to_string(),
                is_final: true,
            }]),
        });
        let observer = Arc::new(RecordingObserver::default());
        let mut worker = AsrWorker::spawn(
            AudioSource::Microphone,
            consumer,
            recognizer,
            16_000,
            9,
            0,
            0,
            observer.clone(),
        );

        feeder.accept(&[0.1, 0.2, 0.3, 0.4]);
        std::thread::sleep(Duration::from_millis(80));
        // Fault-triggered teardown (Spec 10): abandon, not finish.
        feeder.abandon();
        worker.stop();

        let events = observer.events.lock().clone();
        assert_eq!(
            events,
            vec![Event::Partial {
                source: AudioSource::Microphone,
                id: "mic-9-0".to_string(),
                text: "still speaking".to_string(),
                started_at_ms: 0,
            }],
            "abandon must never drain finish_script into a final"
        );
    }
}
