//! Numbered approval gate thresholds (Spec 05 section 12, AC10-AC15) and
//! their evaluation against measured per-host aggregates. Gate numbers are
//! frozen; changing one requires a recorded product decision, not a quiet
//! edit (section 15, "Gate erosion").

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GateResult {
    pub name: &'static str,
    /// `None` when the underlying metric could not be measured this run
    /// (see `CandidateHostMetrics.rss_growth_after_30s_fraction`); never a
    /// fabricated stand-in value.
    pub measured: Option<f64>,
    pub threshold: f64,
    /// `true` when the threshold is a maximum (`measured <= threshold`
    /// passes); `false` when it is a minimum (`measured >= threshold`
    /// passes).
    pub is_upper_bound: bool,
    /// `None` means "not measured" — distinct from `Some(false)` ("measured
    /// and failed"). A report must render this as `NOT MEASURED`, never as
    /// `PASS` or `FAIL`, and it blocks approval exactly like a failure.
    pub pass: Option<bool>,
}

impl GateResult {
    fn upper_bound(name: &'static str, measured: f64, threshold: f64) -> Self {
        Self {
            name,
            measured: Some(measured),
            threshold,
            is_upper_bound: true,
            pass: Some(measured <= threshold),
        }
    }

    fn lower_bound(name: &'static str, measured: f64, threshold: f64) -> Self {
        Self {
            name,
            measured: Some(measured),
            threshold,
            is_upper_bound: false,
            pass: Some(measured >= threshold),
        }
    }

    fn not_measured(name: &'static str, threshold: f64, is_upper_bound: bool) -> Self {
        Self {
            name,
            measured: None,
            threshold,
            is_upper_bound,
            pass: None,
        }
    }
}

/// Aggregate per-host measurements a gate evaluation is run against. All
/// fields come from real measured runs (`mistaken-bench run`/`score`); this
/// type carries no defaults that could silently manufacture a passing gate.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CandidateHostMetrics {
    pub mpr_overall: f64,
    pub false_correction_rate: f64,
    pub mpr_mistake_tense: f64,
    pub mpr_mistake_minimal_pair: f64,
    pub wer_overall: f64,
    pub wer_fluent_control: f64,
    pub silence_hallucination_tokens: u64,
    pub noise_insertion_tokens_per_second: f64,
    pub median_first_partial_latency_ms: Option<f64>,
    pub p95_final_after_endpoint_latency_ms: Option<f64>,
    pub rtf_single_stream: f64,
    pub rtf_two_stream: Option<f64>,
    pub peak_rss_single_stream_bytes: u64,
    pub peak_rss_two_stream_bytes: Option<u64>,
    pub sustained_cpu_fraction_long_turn: f64,
    pub rss_growth_after_30s_fraction: Option<f64>,
    pub payload_bytes_uncompressed: u64,
    pub contributing_clip_fraction: f64,
}

pub const MAX_MODEL_PAYLOAD_BYTES: u64 = 120 * 1024 * 1024;

/// Evaluate AC10-AC15 for one candidate on one host. AC16 (license) and
/// AC17 (both-hosts approval) are evaluated separately in `report.rs`
/// because they need cross-host and license-record data this function does
/// not have.
pub fn evaluate_gates(m: &CandidateHostMetrics) -> Vec<GateResult> {
    vec![
        GateResult::lower_bound("fidelity.mpr_overall", m.mpr_overall, 0.90),
        GateResult::upper_bound(
            "fidelity.false_correction_rate",
            m.false_correction_rate,
            0.05,
        ),
        GateResult::lower_bound("fidelity.mpr_mistake_tense", m.mpr_mistake_tense, 0.85),
        GateResult::lower_bound(
            "fidelity.mpr_mistake_minimal_pair",
            m.mpr_mistake_minimal_pair,
            0.85,
        ),
        GateResult::upper_bound("accuracy.wer_overall", m.wer_overall, 0.25),
        GateResult::upper_bound("accuracy.wer_fluent_control", m.wer_fluent_control, 0.12),
        GateResult::upper_bound(
            "hallucination.silence_tokens",
            m.silence_hallucination_tokens as f64,
            0.0,
        ),
        GateResult::upper_bound(
            "hallucination.noise_insertion_tokens_per_second",
            m.noise_insertion_tokens_per_second,
            0.02,
        ),
        match m.median_first_partial_latency_ms {
            Some(v) => GateResult::upper_bound("latency.median_first_partial_ms", v, 900.0),
            None => GateResult::not_measured("latency.median_first_partial_ms", 900.0, true),
        },
        match m.p95_final_after_endpoint_latency_ms {
            Some(v) => GateResult::upper_bound("latency.p95_final_after_endpoint_ms", v, 1500.0),
            None => GateResult::not_measured("latency.p95_final_after_endpoint_ms", 1500.0, true),
        },
        GateResult::upper_bound("throughput.rtf_single_stream", m.rtf_single_stream, 0.6),
        match m.rtf_two_stream {
            Some(v) => GateResult::upper_bound("throughput.rtf_two_stream", v, 0.9),
            None => GateResult::not_measured("throughput.rtf_two_stream", 0.9, true),
        },
        GateResult::upper_bound(
            "resource.peak_rss_single_stream_mb",
            m.peak_rss_single_stream_bytes as f64 / (1024.0 * 1024.0),
            700.0,
        ),
        match m.peak_rss_two_stream_bytes {
            Some(v) => GateResult::upper_bound(
                "resource.peak_rss_two_stream_mb",
                v as f64 / (1024.0 * 1024.0),
                1400.0,
            ),
            None => GateResult::not_measured("resource.peak_rss_two_stream_mb", 1400.0, true),
        },
        GateResult::upper_bound(
            "resource.sustained_cpu_fraction",
            m.sustained_cpu_fraction_long_turn,
            0.60,
        ),
        match m.rss_growth_after_30s_fraction {
            Some(v) => GateResult::upper_bound("resource.rss_growth_after_30s_fraction", v, 0.05),
            None => GateResult::not_measured("resource.rss_growth_after_30s_fraction", 0.05, true),
        },
        GateResult::upper_bound(
            "size.payload_bytes",
            m.payload_bytes_uncompressed as f64,
            MAX_MODEL_PAYLOAD_BYTES as f64,
        ),
        GateResult::lower_bound(
            "coverage.contributing_clip_fraction",
            m.contributing_clip_fraction,
            0.98,
        ),
    ]
}

pub fn all_pass(results: &[GateResult]) -> bool {
    results.iter().all(|r| r.pass == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passing_metrics() -> CandidateHostMetrics {
        CandidateHostMetrics {
            mpr_overall: 0.95,
            false_correction_rate: 0.02,
            mpr_mistake_tense: 0.90,
            mpr_mistake_minimal_pair: 0.88,
            wer_overall: 0.18,
            wer_fluent_control: 0.08,
            silence_hallucination_tokens: 0,
            noise_insertion_tokens_per_second: 0.01,
            median_first_partial_latency_ms: Some(700.0),
            p95_final_after_endpoint_latency_ms: Some(1200.0),
            rtf_single_stream: 0.4,
            rtf_two_stream: Some(0.7),
            peak_rss_single_stream_bytes: 500 * 1024 * 1024,
            peak_rss_two_stream_bytes: Some(1200 * 1024 * 1024),
            sustained_cpu_fraction_long_turn: 0.5,
            rss_growth_after_30s_fraction: Some(0.02),
            payload_bytes_uncompressed: 72_899_121,
            contributing_clip_fraction: 1.0,
        }
    }

    #[test]
    fn fully_passing_metrics_pass_every_gate() {
        let results = evaluate_gates(&passing_metrics());
        assert!(all_pass(&results), "{results:?}");
        assert_eq!(results.len(), 18);
    }

    #[test]
    fn mpr_below_threshold_fails_only_that_gate() {
        let mut m = passing_metrics();
        m.mpr_overall = 0.80;
        let results = evaluate_gates(&m);
        assert!(!all_pass(&results));
        let mpr_gate = results
            .iter()
            .find(|r| r.name == "fidelity.mpr_overall")
            .unwrap();
        assert_eq!(mpr_gate.pass, Some(false));
        let other_gates_still_pass = results
            .iter()
            .filter(|r| r.name != "fidelity.mpr_overall")
            .all(|r| r.pass == Some(true));
        assert!(other_gates_still_pass);
    }

    #[test]
    fn a_low_wer_candidate_that_fails_mpr_still_fails_overall() {
        // The product invariant this whole spec exists to protect: fidelity
        // outranks WER. A candidate with excellent WER but poor mistake
        // preservation must never read as passing.
        let mut m = passing_metrics();
        m.wer_overall = 0.05; // excellent WER
        m.mpr_overall = 0.50; // silently corrects half the mistakes
        let results = evaluate_gates(&m);
        assert!(!all_pass(&results));
    }

    #[test]
    fn any_silence_hallucination_fails_the_gate() {
        let mut m = passing_metrics();
        m.silence_hallucination_tokens = 1;
        let results = evaluate_gates(&m);
        let gate = results
            .iter()
            .find(|r| r.name == "hallucination.silence_tokens")
            .unwrap();
        assert_eq!(gate.pass, Some(false));
    }

    #[test]
    fn payload_exactly_at_the_size_ceiling_passes() {
        let mut m = passing_metrics();
        m.payload_bytes_uncompressed = MAX_MODEL_PAYLOAD_BYTES;
        let results = evaluate_gates(&m);
        let gate = results
            .iter()
            .find(|r| r.name == "size.payload_bytes")
            .unwrap();
        assert_eq!(gate.pass, Some(true));
    }

    #[test]
    fn payload_one_byte_over_the_ceiling_fails() {
        let mut m = passing_metrics();
        m.payload_bytes_uncompressed = MAX_MODEL_PAYLOAD_BYTES + 1;
        let results = evaluate_gates(&m);
        let gate = results
            .iter()
            .find(|r| r.name == "size.payload_bytes")
            .unwrap();
        assert_eq!(gate.pass, Some(false));
    }

    #[test]
    fn unmeasured_rss_growth_is_distinct_from_a_failure_and_still_blocks_approval() {
        let mut m = passing_metrics();
        m.rss_growth_after_30s_fraction = None;
        let results = evaluate_gates(&m);
        let gate = results
            .iter()
            .find(|r| r.name == "resource.rss_growth_after_30s_fraction")
            .unwrap();
        assert_eq!(gate.pass, None);
        assert_eq!(gate.measured, None);
        // Not-measured must never read as an overall pass.
        assert!(!all_pass(&results));
    }
}
