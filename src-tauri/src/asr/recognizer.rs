//! The recognizer boundary: the small trait pair every ASR adapter
//! (development or, in the future, production-approved) must implement.
//!
//! Nothing here knows about sherpa-onnx, Tauri, or transcript formatting.
//! `poll`/`finish` never block the caller's audio thread, allocate per
//! sample, or emit any event; they return data through `out`.

/// One hypothesis or endpoint-finalized result from a recognizer stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecognizedSegment {
    /// Verbatim recognizer output, whitespace-trimmed only. Internal
    /// whitespace collapsing (the other half of Spec 06's text contract)
    /// happens once, uniformly, at the point this is turned into a
    /// transcript event — never inside an adapter.
    pub text: String,
    pub is_final: bool,
}

/// Reasons a native ASR operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrErrorKind {
    ModelMissing,
    ModelUnsupported,
    ModelLoadFailed,
    StreamCreateFailed,
    DecodeFailed,
    Internal,
}

/// A native ASR failure. Never carries transcript text, PCM, or a raw
/// native error chain; `detail` is a short, sanitized description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrError {
    pub kind: AsrErrorKind,
    pub detail: String,
}

impl AsrError {
    pub fn new(kind: AsrErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

/// One open recognizer stream for one active source. Exactly one constant
/// sample rate is fed to a given instance for its entire life; a device
/// rate change requires a new stream, never a rate change on this one.
pub trait StreamingRecognizer: Send {
    /// Feed exactly `format.sample_rate_hz`-rate mono samples.
    fn accept(&mut self, samples: &[f32]) -> Result<(), AsrError>;

    /// Decode all currently available frames and return the latest
    /// hypothesis plus any endpoint-finalized segment, in emission order.
    fn poll(&mut self, out: &mut Vec<RecognizedSegment>) -> Result<(), AsrError>;

    /// Signal end of input and drain within this call's own bounded
    /// budget ([`super::FINISH_BUDGET`]). Emits at most one final segment.
    fn finish(&mut self, out: &mut Vec<RecognizedSegment>) -> Result<(), AsrError>;
}

/// Opens recognizer streams against one already-loaded model. The sherpa
/// development adapter is the only production implementation of this
/// trait; a deterministic fake exists only in tests.
pub trait RecognizerFactory: Send + Sync {
    fn open_stream(
        &self,
        format: crate::audio::PcmFormat,
    ) -> Result<Box<dyn StreamingRecognizer>, AsrError>;
}
