//! Corpus manifest types and validation
//! (`benchmarks/corpus/manifest.json`, validated against
//! `benchmarks/corpus/manifest.schema.json`). See Spec 05 section 6,
//! "Produced — corpus manifest".

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::normalize::normalize_tokens;

pub const REQUIRED_CONDITION_MINIMUMS: &[(&str, usize)] = &[
    ("mistake-tense", 12),
    ("mistake-agreement", 8),
    ("mistake-article", 6),
    ("mistake-preposition", 8),
    ("mistake-order", 6),
    ("mistake-minimal-pair", 8),
    ("fillers-repetition", 10),
    ("fluent-control", 16),
    ("fast-speech", 12),
    ("system-playback", 16),
    ("noise-silence", 12),
    ("long-turn", 8),
];

pub const MIN_TOTAL_CLIPS: usize = 122;
pub const MIN_TOTAL_DURATION_MS: u64 = 20 * 60 * 1000;
pub const MIN_PHYSICAL_SILENCE_CLIPS: usize = 4;
pub const MIN_PHYSICAL_SILENCE_MS: u64 = 10_000;
pub const LONG_TURN_MIN_MS: u64 = 60_000;
pub const LONG_TURN_MAX_MS: u64 = 120_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleFormat {
    pub codec: String,
    pub channels: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerProfile {
    pub id: String,
    #[serde(rename = "consentGiven")]
    pub consent_given: bool,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSpan {
    #[serde(rename = "startToken")]
    pub start_token: usize,
    #[serde(rename = "endToken")]
    pub end_token: usize,
    pub kind: String,
    pub spoken: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    pub file: String,
    pub condition: String,
    #[serde(rename = "promptFile")]
    pub prompt_file: String,
    #[serde(rename = "sampleRateHz")]
    pub sample_rate_hz: u32,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub sha256: String,
    #[serde(rename = "speakerProfileId")]
    pub speaker_profile_id: String,
    pub reference: String,
    #[serde(rename = "errorSpans")]
    pub error_spans: Vec<ErrorSpan>,
    #[serde(rename = "expectPhysicalSilence")]
    pub expect_physical_silence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "manifestVersion")]
    pub manifest_version: u32,
    #[serde(rename = "sampleFormat")]
    pub sample_format: SampleFormat,
    #[serde(rename = "speakerProfiles")]
    pub speaker_profiles: Vec<SpeakerProfile>,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Empty for manifest-wide errors, otherwise the offending clip id.
    pub clip_id: String,
    pub rule: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.clip_id.is_empty() {
            write!(f, "[manifest] {}", self.rule)
        } else {
            write!(f, "[{}] {}", self.clip_id, self.rule)
        }
    }
}

fn manifest_error(rule: impl Into<String>) -> ValidationError {
    ValidationError {
        clip_id: String::new(),
        rule: rule.into(),
    }
}

fn clip_error(clip_id: &str, rule: impl Into<String>) -> ValidationError {
    ValidationError {
        clip_id: clip_id.to_string(),
        rule: rule.into(),
    }
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Manifest, String> {
        let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
    }

    /// Structural + cross-field validation independent of the filesystem
    /// (no checksum/duration re-derivation). Used by fixture tests.
    pub fn validate_structure(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        if self.manifest_version != 1 {
            errors.push(manifest_error(format!(
                "manifestVersion must be 1, got {}",
                self.manifest_version
            )));
        }
        if self.sample_format.codec != "pcm_s16le" || self.sample_format.channels != 1 {
            errors.push(manifest_error(
                "sampleFormat must be pcm_s16le mono".to_string(),
            ));
        }

        let known_profiles: HashSet<&str> = self
            .speaker_profiles
            .iter()
            .map(|p| p.id.as_str())
            .collect();

        let mut seen_ids = HashSet::new();
        for clip in &self.clips {
            if !seen_ids.insert(clip.id.clone()) {
                errors.push(clip_error(&clip.id, "duplicate clip id"));
            }
            if !known_profiles.contains(clip.speaker_profile_id.as_str()) {
                errors.push(clip_error(
                    &clip.id,
                    format!(
                        "speakerProfileId '{}' is not registered in speakerProfiles",
                        clip.speaker_profile_id
                    ),
                ));
            }
            if !REQUIRED_CONDITION_MINIMUMS
                .iter()
                .any(|(c, _)| *c == clip.condition)
            {
                errors.push(clip_error(
                    &clip.id,
                    format!("unknown condition '{}'", clip.condition),
                ));
            }
            if clip.sample_rate_hz != 16000 && clip.sample_rate_hz != 48000 {
                errors.push(clip_error(
                    &clip.id,
                    format!(
                        "sampleRateHz must be 16000 or 48000, got {}",
                        clip.sample_rate_hz
                    ),
                ));
            }
            if clip.sha256.len() != 64 || !clip.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                errors.push(clip_error(
                    &clip.id,
                    "sha256 must be 64 lowercase hex characters",
                ));
            }

            if clip.expect_physical_silence {
                if !clip.reference.is_empty() {
                    errors.push(clip_error(
                        &clip.id,
                        "expectPhysicalSilence clips must have an empty reference",
                    ));
                }
                if !clip.error_spans.is_empty() {
                    errors.push(clip_error(
                        &clip.id,
                        "expectPhysicalSilence clips must have no errorSpans",
                    ));
                }
                if clip.duration_ms < MIN_PHYSICAL_SILENCE_MS {
                    errors.push(clip_error(
                        &clip.id,
                        format!(
                            "physical-silence clip must be >= {} ms, got {} ms",
                            MIN_PHYSICAL_SILENCE_MS, clip.duration_ms
                        ),
                    ));
                }
            }

            if clip.condition == "long-turn"
                && (clip.duration_ms < LONG_TURN_MIN_MS || clip.duration_ms > LONG_TURN_MAX_MS)
            {
                errors.push(clip_error(
                    &clip.id,
                    format!(
                        "long-turn clip must be within {}..={} ms, got {} ms",
                        LONG_TURN_MIN_MS, LONG_TURN_MAX_MS, clip.duration_ms
                    ),
                ));
            }

            // errorSpans must be verbatim, in-order substrings of the
            // normalized reference (AC3: "no reference contains a
            // grammatically corrected form of an annotated error" is a
            // review-level check; this enforces the mechanical half).
            let ref_tokens = normalize_tokens(&clip.reference);
            for span in &clip.error_spans {
                if span.start_token > span.end_token {
                    errors.push(clip_error(
                        &clip.id,
                        format!(
                            "errorSpan startToken {} > endToken {}",
                            span.start_token, span.end_token
                        ),
                    ));
                    continue;
                }
                if span.end_token >= ref_tokens.len() {
                    errors.push(clip_error(
                        &clip.id,
                        format!(
                            "errorSpan endToken {} out of bounds for a {}-token reference",
                            span.end_token,
                            ref_tokens.len()
                        ),
                    ));
                    continue;
                }
                let actual = ref_tokens[span.start_token..=span.end_token].join(" ");
                let expected = normalize_tokens(&span.spoken).join(" ");
                if actual != expected {
                    errors.push(clip_error(
                        &clip.id,
                        format!(
                            "errorSpan.spoken '{}' does not match reference[{}..={}] '{}'",
                            span.spoken, span.start_token, span.end_token, actual
                        ),
                    ));
                }
            }
        }

        // Per-condition minimum counts, counted over the primary (non-48k-duplicate) set.
        for (condition, minimum) in REQUIRED_CONDITION_MINIMUMS {
            let count = self
                .clips
                .iter()
                .filter(|c| c.condition == *condition && !c.id.ends_with("-48k"))
                .count();
            if count < *minimum {
                errors.push(manifest_error(format!(
                    "condition '{condition}' has {count} clips, needs >= {minimum}"
                )));
            }
        }

        let primary_count = self
            .clips
            .iter()
            .filter(|c| !c.id.ends_with("-48k"))
            .count();
        if primary_count < MIN_TOTAL_CLIPS {
            errors.push(manifest_error(format!(
                "manifest has {primary_count} primary clips, needs >= {MIN_TOTAL_CLIPS}"
            )));
        }

        let total_duration_ms: u64 = self
            .clips
            .iter()
            .filter(|c| !c.id.ends_with("-48k"))
            .map(|c| c.duration_ms)
            .sum();
        if total_duration_ms < MIN_TOTAL_DURATION_MS {
            errors.push(manifest_error(format!(
                "primary corpus totals {} ms, needs >= {} ms (20 minutes)",
                total_duration_ms, MIN_TOTAL_DURATION_MS
            )));
        }

        let silence_count = self
            .clips
            .iter()
            .filter(|c| c.expect_physical_silence)
            .count();
        if silence_count < MIN_PHYSICAL_SILENCE_CLIPS {
            errors.push(manifest_error(format!(
                "manifest has {silence_count} physical-silence clips, needs >= {MIN_PHYSICAL_SILENCE_CLIPS}"
            )));
        }

        let dup48_count = self.clips.iter().filter(|c| c.id.ends_with("-48k")).count();
        if dup48_count == 0 {
            errors.push(manifest_error(
                "no 48 kHz duplicate subset found (id suffix '-48k')",
            ));
        }

        errors
    }

    /// Full validation including filesystem checks: clip presence, checksum,
    /// and prompt-file linkage, relative to `corpus_dir`
    /// (`benchmarks/corpus`).
    pub fn validate_with_files(&self, corpus_dir: &Path) -> Vec<ValidationError> {
        let mut errors = self.validate_structure();

        for clip in &self.clips {
            let clip_path: PathBuf = corpus_dir.join(&clip.file);
            if !clip_path.is_file() {
                errors.push(clip_error(
                    &clip.id,
                    format!("clip file missing: {}", clip_path.display()),
                ));
                continue;
            }
            match sha256_file(&clip_path) {
                Ok(actual) if actual == clip.sha256 => {}
                Ok(actual) => errors.push(clip_error(
                    &clip.id,
                    format!(
                        "sha256 mismatch: manifest {} vs recomputed {}",
                        clip.sha256, actual
                    ),
                )),
                Err(e) => errors.push(clip_error(&clip.id, format!("could not hash clip: {e}"))),
            }

            match hound::WavReader::open(&clip_path) {
                Ok(reader) => {
                    let spec = reader.spec();
                    if spec.sample_rate != clip.sample_rate_hz {
                        errors.push(clip_error(
                            &clip.id,
                            format!(
                                "wav sample rate {} does not match manifest {}",
                                spec.sample_rate, clip.sample_rate_hz
                            ),
                        ));
                    }
                    if spec.channels != 1 {
                        errors.push(clip_error(
                            &clip.id,
                            format!("wav has {} channels, expected 1 (mono)", spec.channels),
                        ));
                    }
                    let duration_ms =
                        (reader.duration() as u64 * 1000) / spec.sample_rate.max(1) as u64;
                    let drift = duration_ms.abs_diff(clip.duration_ms);
                    if drift > 50 {
                        errors.push(clip_error(
                            &clip.id,
                            format!(
                                "wav duration {} ms differs from manifest {} ms by {} ms",
                                duration_ms, clip.duration_ms, drift
                            ),
                        ));
                    }
                }
                Err(e) => errors.push(clip_error(&clip.id, format!("could not open wav: {e}"))),
            }

            let prompt_path = corpus_dir.join(&clip.prompt_file);
            if !clip.expect_physical_silence && !prompt_path.is_file() {
                errors.push(clip_error(
                    &clip.id,
                    format!("prompt file missing: {}", prompt_path.display()),
                ));
            }
        }

        errors
    }
}

pub fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_manifest() -> Manifest {
        Manifest {
            manifest_version: 1,
            sample_format: SampleFormat {
                codec: "pcm_s16le".to_string(),
                channels: 1,
            },
            speaker_profiles: vec![SpeakerProfile {
                id: "sp-01".to_string(),
                consent_given: true,
                notes: None,
            }],
            clips: vec![Clip {
                id: "mistake-tense-01".to_string(),
                file: "clips/mistake-tense-01.wav".to_string(),
                condition: "mistake-tense".to_string(),
                prompt_file: "prompts/mistake-tense-01.md".to_string(),
                sample_rate_hz: 16000,
                duration_ms: 3000,
                sha256: "a".repeat(64),
                speaker_profile_id: "sp-01".to_string(),
                reference: "i have went there".to_string(),
                error_spans: vec![ErrorSpan {
                    start_token: 1,
                    end_token: 2,
                    kind: "verb-form".to_string(),
                    spoken: "have went".to_string(),
                }],
                expect_physical_silence: false,
            }],
        }
    }

    #[test]
    fn valid_error_span_passes() {
        let m = minimal_manifest();
        let errors: Vec<_> = m
            .validate_structure()
            .into_iter()
            .filter(|e| e.clip_id == "mistake-tense-01")
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn mismatched_error_span_is_rejected() {
        let mut m = minimal_manifest();
        m.clips[0].error_spans[0].spoken = "have gone".to_string();
        let errors = m.validate_structure();
        assert!(errors
            .iter()
            .any(|e| e.clip_id == "mistake-tense-01" && e.rule.contains("does not match")));
    }

    #[test]
    fn out_of_bounds_span_is_rejected() {
        let mut m = minimal_manifest();
        m.clips[0].error_spans[0].end_token = 99;
        let errors = m.validate_structure();
        assert!(errors
            .iter()
            .any(|e| e.clip_id == "mistake-tense-01" && e.rule.contains("out of bounds")));
    }

    #[test]
    fn silence_clip_must_have_empty_reference_and_no_spans() {
        let mut m = minimal_manifest();
        m.clips[0].expect_physical_silence = true;
        m.clips[0].duration_ms = 10500;
        let errors = m.validate_structure();
        assert!(errors.iter().any(|e| e.rule.contains("empty reference")));
        assert!(errors.iter().any(|e| e.rule.contains("no errorSpans")));
    }

    #[test]
    fn short_physical_silence_is_rejected() {
        let mut m = minimal_manifest();
        m.clips[0].expect_physical_silence = true;
        m.clips[0].reference = String::new();
        m.clips[0].error_spans.clear();
        m.clips[0].duration_ms = 4000;
        let errors = m.validate_structure();
        assert!(errors.iter().any(|e| e.rule.contains(">= 10000 ms")));
    }

    #[test]
    fn long_turn_out_of_bounds_duration_is_rejected() {
        let mut m = minimal_manifest();
        m.clips[0].condition = "long-turn".to_string();
        m.clips[0].duration_ms = 5000;
        let errors = m.validate_structure();
        assert!(errors
            .iter()
            .any(|e| e.rule.contains("long-turn clip must be within")));
    }

    #[test]
    fn unknown_speaker_profile_is_rejected() {
        let mut m = minimal_manifest();
        m.clips[0].speaker_profile_id = "sp-99".to_string();
        let errors = m.validate_structure();
        assert!(errors.iter().any(|e| e.rule.contains("not registered")));
    }

    #[test]
    fn duplicate_clip_id_is_rejected() {
        let mut m = minimal_manifest();
        let dup = m.clips[0].clone();
        m.clips.push(dup);
        let errors = m.validate_structure();
        assert!(errors.iter().any(|e| e.rule.contains("duplicate clip id")));
    }

    #[test]
    fn per_condition_and_total_minimums_are_enforced_on_undersized_manifest() {
        let m = minimal_manifest();
        let errors = m.validate_structure();
        assert!(errors.iter().any(|e| e.rule.contains("needs >= 12")));
        assert!(errors.iter().any(|e| e.rule.contains("needs >= 122")));
        assert!(errors.iter().any(|e| e.rule.contains("20 minutes")));
        assert!(errors
            .iter()
            .any(|e| e.rule.contains("physical-silence clips")));
        assert!(errors.iter().any(|e| e.rule.contains("48 kHz duplicate")));
    }
}
