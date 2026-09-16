//! Local ASR: model manifest/discovery, the recognizer boundary, the
//! bounded inference chunk stage, and the per-source ASR worker.
//!
//! Everything here consumes the pinned **development** adapter
//! (`sherpa-zipformer-en-20M-2023-02-17-int8` on `sherpa-onnx` `v1.13.8`),
//! which is a `DevelopmentOnly` maturity artifact. It has not passed the
//! unchanged Spec 05 production gates (recorded real-human-corpus MPR
//! 0.3241 against the required >= 0.90); every transcript-quality result
//! produced through this module is development/architecture evidence only
//! and confers no production approval. See `docs/context/architecture.md`
//! ("ASR Maturity Contract") and `benchmarks/reports/approval.md`.
//!
//! No module in this crate may promote the compiled-in maturity, add a
//! second recognizer implementation selected by a `cfg`/env var, or run any
//! text-correction/rewriting stage.

pub mod chunk_pool;
pub mod loader;
pub mod manifest;
pub mod recognizer;
pub mod sherpa_adapter;
pub mod worker;

use std::time::Duration;

/// Duration of one bounded ASR inference chunk.
pub const ASR_CHUNK_DURATION_MS: u32 = 100;
/// Number of preallocated ASR chunks. At 100 ms per chunk this bounds
/// queued inference audio to exactly three seconds, independent of Spec
/// 04's separate two-second capture pool.
pub const ASR_POOL_CAPACITY: usize = 30;
/// A partial transcript update is emitted at most once per this interval
/// per source, and only when its trimmed text changed.
pub const PARTIAL_THROTTLE: Duration = Duration::from_millis(150);
/// Bounded budget for the post-`input_finished` decode drain at Stop.
pub const FINISH_BUDGET: Duration = Duration::from_millis(300);
/// `inference_lagging` is reported at most once per this interval while
/// the ASR chunk pool keeps dropping newest samples.
pub const LAGGING_REPORT_INTERVAL: Duration = Duration::from_secs(1);
/// Sustained-lag detection window and threshold (Spec 10): a source
/// enters the degraded state when a completed, non-overlapping window
/// this long sees more than this percentage of its chunks dropped.
pub const LAG_WINDOW: Duration = Duration::from_secs(10);
pub const LAG_THRESHOLD_PERCENT: u8 = 20;
/// Upper bound on how long the ASR worker parks between chunk checks.
pub const WORKER_TICK: Duration = Duration::from_millis(20);

pub use loader::AsrModelLoader;
pub use manifest::{AsrMaturity, AsrModelManifest, ModelFileSpec, DEVELOPMENT_MANIFEST};
pub use recognizer::{
    AsrError, AsrErrorKind, RecognizedSegment, RecognizerFactory, StreamingRecognizer,
};
pub use worker::{AsrWorker, AsrWorkerObserver};
