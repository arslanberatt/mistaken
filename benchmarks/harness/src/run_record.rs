//! Per-clip run artifacts and the scoring pipeline that turns a scored run
//! directory into the aggregate metrics `scoring::gates` evaluates against.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::adapter::protocol::{Event, Pace};
use crate::manifest::{Clip, Manifest};
use crate::normalize::normalize_tokens;
use crate::scoring::mpr::{classify_span, known_corrections, MprAccumulator, SpanOutcome};
use crate::scoring::wer::align;

/// One clip's raw adapter run, persisted to `runs/<timestamp>/...` (local
/// only, Git-ignored). Immutable evidence: a re-run creates a new directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipRunRecord {
    pub clip_id: String,
    pub candidate_id: String,
    pub host_profile: String,
    pub pace: Pace,
    pub repeat_index: u32,
    /// Number of adapter processes run concurrently in this record's chunk
    /// (`1` = single-stream; `>1` = the two-stream concurrency measurement,
    /// Spec 05 section 12 AC13 — required only for the leading candidate).
    /// Defaults to `1` when absent, so run records written before this
    /// field existed (every single-stream run in this evidence set) still
    /// deserialize as the single-stream they actually were.
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    /// Which concurrent chunk this clip belonged to within its repetition,
    /// so a two-stream report can pair clips that ran overlapping in wall
    /// time and sum their peak RSS instead of comparing them in isolation.
    #[serde(default)]
    pub chunk_index: usize,
    pub events: Vec<Event>,
    pub timed_out: bool,
    pub exit_status: Option<i32>,
}

fn default_concurrency() -> u32 {
    1
}

impl ClipRunRecord {
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.json", self.clip_id));
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).map_err(|e| e.to_string())
    }
}

/// Concatenated text of every `final` event, in order — the scorable
/// hypothesis for a clip regardless of whether the adapter emitted one
/// final (this crate's sherpa-onnx adapter) or several (the rolling-window
/// whisper.cpp adapter).
pub fn hypothesis_text(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Final { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn first_partial_at_ms(events: &[Event]) -> Option<u64> {
    events.iter().find_map(|e| match e {
        Event::Partial { at_ms, text } if !text.trim().is_empty() => Some(*at_ms),
        _ => None,
    })
}

pub fn last_final_at_ms(events: &[Event]) -> Option<u64> {
    events.iter().rev().find_map(|e| match e {
        Event::Final { at_ms, .. } => Some(*at_ms),
        _ => None,
    })
}

pub fn metrics_event(events: &[Event]) -> Option<(u64, u64, u64, u64, u32, u32)> {
    events.iter().find_map(|e| match e {
        Event::Metrics {
            audio_ms,
            wall_ms,
            cpu_time_ms,
            peak_rss_bytes,
            fed_chunks,
            dropped_chunks,
            ..
        } => Some((
            *audio_ms,
            *wall_ms,
            *cpu_time_ms,
            *peak_rss_bytes,
            *fed_chunks,
            *dropped_chunks,
        )),
        _ => None,
    })
}

pub fn error_event(events: &[Event]) -> Option<(&str, &str)> {
    events.iter().find_map(|e| match e {
        Event::Error { code, message, .. } => Some((code.as_str(), message.as_str())),
        _ => None,
    })
}

/// Per-clip scored result: everything a report/gate evaluation needs,
/// derived once from a `ClipRunRecord` plus its manifest entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipScore {
    pub clip_id: String,
    pub condition: String,
    pub contributed: bool,
    pub failure_reason: Option<String>,
    pub ref_token_count: usize,
    pub edit_distance: u32,
    pub mpr_outcomes: Vec<String>, // "preserved" | "false_correction" | "misrecognized"
    pub silence_hallucination_tokens: usize,
    pub inserted_tokens: usize,
    pub audio_ms: u64,
    pub first_partial_at_ms: Option<u64>,
    pub final_after_endpoint_ms: Option<i64>,
    pub rtf: Option<f64>,
    pub peak_rss_bytes: Option<u64>,
    pub cpu_time_ms: Option<u64>,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default)]
    pub chunk_index: usize,
}

pub fn score_clip(clip: &Clip, record: &ClipRunRecord) -> ClipScore {
    if record.timed_out {
        return ClipScore {
            clip_id: clip.id.clone(),
            condition: clip.condition.clone(),
            contributed: false,
            failure_reason: Some("timeout".to_string()),
            ref_token_count: 0,
            edit_distance: 0,
            mpr_outcomes: Vec::new(),
            silence_hallucination_tokens: 0,
            inserted_tokens: 0,
            audio_ms: clip.duration_ms,
            first_partial_at_ms: None,
            final_after_endpoint_ms: None,
            rtf: None,
            peak_rss_bytes: None,
            cpu_time_ms: None,
            concurrency: record.concurrency,
            chunk_index: record.chunk_index,
        };
    }
    if let Some((code, message)) = error_event(&record.events) {
        return ClipScore {
            clip_id: clip.id.clone(),
            condition: clip.condition.clone(),
            contributed: false,
            failure_reason: Some(format!("{code}: {message}")),
            ref_token_count: 0,
            edit_distance: 0,
            mpr_outcomes: Vec::new(),
            silence_hallucination_tokens: 0,
            inserted_tokens: 0,
            audio_ms: clip.duration_ms,
            first_partial_at_ms: None,
            final_after_endpoint_ms: None,
            rtf: None,
            peak_rss_bytes: None,
            cpu_time_ms: None,
            concurrency: record.concurrency,
            chunk_index: record.chunk_index,
        };
    }

    let hyp_text = hypothesis_text(&record.events);
    let hyp_tokens = normalize_tokens(&hyp_text);
    let ref_tokens = normalize_tokens(&clip.reference);
    let alignment = align(&ref_tokens, &hyp_tokens);

    let silence_hallucination_tokens = if clip.expect_physical_silence {
        hyp_tokens.len()
    } else {
        0
    };

    let inserted_tokens = if clip.condition == "noise-silence" && !clip.expect_physical_silence {
        crate::scoring::insertion::inserted_token_count(&alignment)
    } else {
        0
    };

    let corrections = known_corrections();
    let mpr_outcomes: Vec<String> = clip
        .error_spans
        .iter()
        .map(|span| {
            let outcome = classify_span(
                &hyp_tokens,
                &alignment,
                span.start_token,
                span.end_token,
                &span.spoken,
                &corrections,
            );
            match outcome {
                SpanOutcome::Preserved => "preserved",
                SpanOutcome::FalseCorrection => "false_correction",
                SpanOutcome::Misrecognized => "misrecognized",
            }
            .to_string()
        })
        .collect();

    let metrics = metrics_event(&record.events);
    let rtf = metrics.map(|(audio_ms, wall_ms, _, _, _, _)| {
        if audio_ms == 0 {
            0.0
        } else {
            wall_ms as f64 / audio_ms as f64
        }
    });
    let peak_rss_bytes = metrics.map(|(_, _, _, rss, _, _)| rss);
    let cpu_time_ms = metrics.map(|(_, _, cpu, _, _, _)| cpu);

    let first_partial_at_ms = first_partial_at_ms(&record.events);
    let final_after_endpoint_ms =
        last_final_at_ms(&record.events).map(|at| at as i64 - clip.duration_ms as i64);

    ClipScore {
        clip_id: clip.id.clone(),
        condition: clip.condition.clone(),
        contributed: true,
        failure_reason: None,
        ref_token_count: ref_tokens.len(),
        edit_distance: alignment
            .ops
            .iter()
            .filter(|op| !matches!(op, crate::scoring::wer::EditOp::Match))
            .count() as u32,
        mpr_outcomes,
        silence_hallucination_tokens,
        inserted_tokens,
        audio_ms: clip.duration_ms,
        first_partial_at_ms,
        final_after_endpoint_ms,
        rtf,
        peak_rss_bytes,
        cpu_time_ms,
        concurrency: record.concurrency,
        chunk_index: record.chunk_index,
    }
}

/// Aggregate a set of `ClipScore`s (one candidate, one host, one pace) into
/// the summary numbers `scoring::gates::CandidateHostMetrics` needs.
/// `contributing_clip_fraction` is computed separately per pace by the
/// caller because it depends on how many clips were attempted.
pub struct ScoredAggregate {
    pub mpr_overall: MprAccumulator,
    pub mpr_mistake_tense: MprAccumulator,
    pub mpr_mistake_minimal_pair: MprAccumulator,
    pub silence_hallucination_tokens: u64,
    pub noise_insertion_tokens: u64,
    pub noise_audio_ms: u64,
    pub first_partial_latencies_ms: Vec<u64>,
    pub final_after_endpoint_latencies_ms: Vec<i64>,
    pub rtfs: Vec<f64>,
    pub peak_rss_bytes: Vec<u64>,
    pub contributed: usize,
    pub attempted: usize,
}

impl ScoredAggregate {
    pub fn new() -> Self {
        Self {
            mpr_overall: MprAccumulator::default(),
            mpr_mistake_tense: MprAccumulator::default(),
            mpr_mistake_minimal_pair: MprAccumulator::default(),
            silence_hallucination_tokens: 0,
            noise_insertion_tokens: 0,
            noise_audio_ms: 0,
            first_partial_latencies_ms: Vec::new(),
            final_after_endpoint_latencies_ms: Vec::new(),
            rtfs: Vec::new(),
            peak_rss_bytes: Vec::new(),
            contributed: 0,
            attempted: 0,
        }
    }
}

impl Default for ScoredAggregate {
    fn default() -> Self {
        Self::new()
    }
}

pub fn percentile(mut values: Vec<f64>, p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rank = (p / 100.0) * (values.len() as f64 - 1.0);
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        values[lower]
    } else {
        let frac = rank - lower as f64;
        values[lower] * (1.0 - frac) + values[upper] * frac
    }
}

pub fn load_manifest(corpus_dir: &Path) -> Result<Manifest, String> {
    Manifest::load(&corpus_dir.join("manifest.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hypothesis_text_concatenates_final_events_in_order() {
        let events = vec![
            Event::Ready {
                at_ms: 0,
                streaming_mode: None,
            },
            Event::Partial {
                at_ms: 100,
                text: "i have".to_string(),
            },
            Event::Final {
                at_ms: 500,
                text: "i have went there".to_string(),
                segment_index: 0,
            },
        ];
        assert_eq!(hypothesis_text(&events), "i have went there");
    }

    #[test]
    fn first_partial_at_ms_skips_empty_partials() {
        let events = vec![
            Event::Partial {
                at_ms: 50,
                text: "".to_string(),
            },
            Event::Partial {
                at_ms: 150,
                text: "i".to_string(),
            },
        ];
        assert_eq!(first_partial_at_ms(&events), Some(150));
    }

    #[test]
    fn median_percentile_matches_middle_value() {
        let values = vec![100.0, 200.0, 300.0];
        assert_eq!(percentile(values, 50.0), 200.0);
    }

    #[test]
    fn p95_percentile_interpolates() {
        let values: Vec<f64> = (1..=20).map(|i| i as f64 * 10.0).collect();
        let p95 = percentile(values, 95.0);
        assert!((p95 - 191.0).abs() < 1.0);
    }
}
