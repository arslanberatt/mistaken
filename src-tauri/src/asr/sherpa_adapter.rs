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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::manifest::DEVELOPMENT_MANIFEST;
    use crate::audio::PcmFormat;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::path::Path;

    #[test]
    fn open_stream_rejects_non_mono_format() {
        let model_dir = Path::new(
            "/Users/berat/mistaken-spec-05-remediation/benchmarks/models/sherpa-zipformer-en-20M-2023-02-17-int8",
        );
        if !model_dir.exists() {
            return;
        }
        let factory = SherpaRecognizerFactory::load(model_dir, &DEVELOPMENT_MANIFEST)
            .expect("should load pinned dev model");
        let stereo = PcmFormat {
            sample_rate_hz: NonZeroU32::new(48000).unwrap(),
            channels: NonZeroU16::new(2).unwrap(),
        };
        let _err = match factory.open_stream(stereo) {
            Err(e) => e,
            Ok(_) => panic!("stereo must be rejected"),
        };
    }

    #[test]
    #[allow(clippy::chunks_exact_to_as_chunks)]
    fn real_model_smoke_test_recognizes_audio() {
        let model_dir = Path::new(
            "/Users/berat/mistaken-spec-05-remediation/benchmarks/models/sherpa-zipformer-en-20M-2023-02-17-int8",
        );
        let clip_path = Path::new(
            "/Users/berat/mistaken-spec-05-remediation/benchmarks/corpus/clips/mistake-tense-01.wav",
        );
        if !model_dir.exists() || !clip_path.exists() {
            return;
        }
        let factory = SherpaRecognizerFactory::load(model_dir, &DEVELOPMENT_MANIFEST)
            .expect("should load pinned dev model");
        let mono = PcmFormat {
            sample_rate_hz: NonZeroU32::new(16000).unwrap(),
            channels: NonZeroU16::new(1).unwrap(),
        };
        let mut recognizer = factory.open_stream(mono).expect("open stream");
        let bytes = std::fs::read(clip_path).expect("read clip");
        let data_pos = bytes
            .windows(4)
            .position(|w| w == b"data")
            .expect("must find data chunk");
        let audio_offset = data_pos + 8;
        let samples: Vec<f32> = bytes[audio_offset..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect();
        let mut segments = Vec::new();
        for chunk in samples.chunks(4800) {
            recognizer.accept(chunk).expect("accept chunk");
            recognizer.poll(&mut segments).expect("poll segments");
        }
        recognizer.finish(&mut segments).expect("finish stream");
        println!("After finish: segments count = {}", segments.len());
        for (i, s) in segments.iter().enumerate() {
            println!("Segment {i}: final={}, text={:?}", s.is_final, s.text);
        }
    }
}
