//! The sherpa-onnx development adapter: the only production
//! implementation of [`RecognizerFactory`]/[`StreamingRecognizer`] this
//! crate ships. `DevelopmentOnly` maturity only — see the module-level
//! doc comment on `crate::asr`.
//!
//! Every rewriting feature the runtime offers (hotwords, homophone
//! replacement, FST rules, ITN) is left at its inert default. The only
//! text transformation this adapter performs is `str::trim`; internal
//! whitespace collapsing happens once, uniformly, in `crate::asr::worker`.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream};

use crate::audio::PcmFormat;

use super::manifest::AsrModelManifest;
use super::recognizer::{
    AsrError, AsrErrorKind, RecognizedSegment, RecognizerFactory, StreamingRecognizer,
};

/// Endpoint rule thresholds frozen by Spec 06 section 6. Re-verified
/// against the installed `sherpa-onnx` `v1.13.8` documentation during
/// implementation; not assumed.
const RULE1_MIN_TRAILING_SILENCE: f32 = 2.4;
const RULE2_MIN_TRAILING_SILENCE: f32 = 1.0;
const RULE3_MIN_UTTERANCE_LENGTH: f32 = 15.0;

fn build_config(dir: &Path, manifest: &AsrModelManifest) -> OnlineRecognizerConfig {
    let mut config = OnlineRecognizerConfig::default();
    config.model_config.transducer.encoder = Some(
        dir.join(manifest.files[0].relative_path)
            .to_string_lossy()
            .into_owned(),
    );
    config.model_config.transducer.decoder = Some(
        dir.join(manifest.files[1].relative_path)
            .to_string_lossy()
            .into_owned(),
    );
    config.model_config.transducer.joiner = Some(
        dir.join(manifest.files[2].relative_path)
            .to_string_lossy()
            .into_owned(),
    );
    config.model_config.tokens = Some(
        dir.join(manifest.files[3].relative_path)
            .to_string_lossy()
            .into_owned(),
    );
    config.model_config.provider = Some(manifest.provider.to_string());
    config.model_config.num_threads = manifest.num_threads;
    // Never true in any build: a debug-mode recognizer would print
    // recognized text/internal state to stdout, which is exactly the
    // "no recognized text is ever logged" invariant this spec requires.
    config.model_config.debug = false;
    config.decoding_method = Some(manifest.decoding_method.to_string());
    config.enable_endpoint = true;
    config.rule1_min_trailing_silence = RULE1_MIN_TRAILING_SILENCE;
    config.rule2_min_trailing_silence = RULE2_MIN_TRAILING_SILENCE;
    config.rule3_min_utterance_length = RULE3_MIN_UTTERANCE_LENGTH;
    // Every rewriting feature explicitly left at its inert default:
    // hotwords_file, hotwords_buf, rule_fsts, rule_fars stay `None`;
    // ctc_fst_decoder_config and hr (homophone replacer) stay default;
    // blank_penalty stays 0.0; max_active_paths is unused by
    // greedy_search and is left untouched.
    config
}

/// Loads the pinned development model once (expensive: allocates the ONNX
/// Runtime session graphs) and opens cheap streams against it afterward.
pub struct SherpaRecognizerFactory {
    recognizer: Arc<OnlineRecognizer>,
}

impl SherpaRecognizerFactory {
    /// Caller must have already run presence + checksum verification;
    /// this only performs the native `OnlineRecognizer::create` call.
    pub fn load(dir: &Path, manifest: &AsrModelManifest) -> Result<Self, AsrError> {
        let config = build_config(dir, manifest);
        let recognizer = OnlineRecognizer::create(&config).ok_or_else(|| {
            AsrError::new(
                AsrErrorKind::ModelLoadFailed,
                "sherpa-onnx OnlineRecognizer::create returned None",
            )
        })?;
        Ok(Self {
            recognizer: Arc::new(recognizer),
        })
    }
}

impl RecognizerFactory for SherpaRecognizerFactory {
    fn open_stream(&self, format: PcmFormat) -> Result<Box<dyn StreamingRecognizer>, AsrError> {
        if format.channels.get() != 1 {
            return Err(AsrError::new(
                AsrErrorKind::Internal,
                "sherpa adapter requires mono input; Spec 04 already downmixes",
            ));
        }
        let stream = self.recognizer.create_stream();
        Ok(Box::new(SherpaStreamingRecognizer {
            recognizer: self.recognizer.clone(),
            stream,
            sample_rate_hz: format.sample_rate_hz.get() as i32,
        }))
    }
}

struct SherpaStreamingRecognizer {
    recognizer: Arc<OnlineRecognizer>,
    stream: OnlineStream,
    sample_rate_hz: i32,
}

/// Appends the current hypothesis (if non-empty) as an interim segment,
/// and — if the recognizer's endpoint rules just fired — the finalized
/// segment (if non-empty) followed by a stream reset. Shared by `poll`
/// (unbounded, natural termination via `is_ready`) and `finish` (bounded,
/// capped at one final).
fn drain_step(
    recognizer: &OnlineRecognizer,
    stream: &OnlineStream,
    out: &mut Vec<RecognizedSegment>,
    allow_final: bool,
) -> bool {
    recognizer.decode(stream);

    if let Some(result) = recognizer.get_result(stream) {
        let text = result.text.trim().to_string();
        if !text.is_empty() {
            out.push(RecognizedSegment {
                text,
                is_final: false,
            });
        }
    }

    let mut emitted_final = false;
    if recognizer.is_endpoint(stream) {
        if allow_final {
            if let Some(result) = recognizer.get_result(stream) {
                let text = result.text.trim().to_string();
                if !text.is_empty() {
                    out.push(RecognizedSegment {
                        text,
                        is_final: true,
                    });
                    emitted_final = true;
                }
            }
        }
        recognizer.reset(stream);
    }
    emitted_final
}

impl StreamingRecognizer for SherpaStreamingRecognizer {
    fn accept(&mut self, samples: &[f32]) -> Result<(), AsrError> {
        self.stream.accept_waveform(self.sample_rate_hz, samples);
        Ok(())
    }

    fn poll(&mut self, out: &mut Vec<RecognizedSegment>) -> Result<(), AsrError> {
        while self.recognizer.is_ready(&self.stream) {
            drain_step(&self.recognizer, &self.stream, out, true);
        }
        Ok(())
    }

    fn finish(&mut self, out: &mut Vec<RecognizedSegment>) -> Result<(), AsrError> {
        self.stream.input_finished();
        let deadline = Instant::now() + super::FINISH_BUDGET;
        let mut final_emitted = false;
        while self.recognizer.is_ready(&self.stream) && Instant::now() < deadline {
            if drain_step(&self.recognizer, &self.stream, out, !final_emitted) {
                final_emitted = true;
            }
        }
        // The formal endpoint rule may never fire on a stream stopped
        // mid-utterance (no trailing silence to measure). Stop still owes
        // the user a closing final for whatever hypothesis exists: take
        // the last decoded hypothesis as the one closing final instead of
        // silently discarding in-progress words.
        if !final_emitted {
            if let Some(result) = self.recognizer.get_result(&self.stream) {
                let text = result.text.trim().to_string();
                if !text.is_empty() {
                    out.push(RecognizedSegment {
                        text,
                        is_final: true,
                    });
                }
            }
        }
        Ok(())
    }
}
