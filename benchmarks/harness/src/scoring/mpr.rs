//! Mistake-preservation rate (MPR) and false-correction classification
//! (Spec 05 section 6, "Produced — scoring contract"). MPR is the product's
//! primary fidelity metric: a high-WER candidate that preserves mistakes
//! beats a low-WER candidate that fixes them.

use std::collections::HashMap;

use crate::normalize::normalize_tokens;
use crate::scoring::wer::Alignment;

/// Known grammatically-correct forms of this corpus's `verb-form`,
/// `agreement`, `article`, `preposition`, and `word-order` error spans,
/// keyed by the normalized `spoken` surface text. Populated once from the
/// corpus authoring data (`benchmarks/harness/fixtures/error-corrections.json`)
/// and embedded so the scorer needs no filesystem lookup at run time.
///
/// `minimal-pair` spans are deliberately absent: they are already
/// grammatically correct (`went` vs `won't`, `their` vs `there`, …), so
/// there is no "grammatically corrected form" for them — a wrong output
/// there is acoustic confusion, not grammar auto-correction, and must never
/// be classified as a false correction.
static CORRECTIONS_JSON: &str = include_str!("../../fixtures/error-corrections.json");

#[derive(Debug, serde::Deserialize)]
struct CorrectionsFile {
    corrections: HashMap<String, String>,
}

pub fn known_corrections() -> HashMap<String, String> {
    let parsed: CorrectionsFile =
        serde_json::from_str(CORRECTIONS_JSON).expect("embedded corrections fixture is valid JSON");
    parsed.corrections
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanOutcome {
    Preserved,
    FalseCorrection,
    Misrecognized,
}

/// The contiguous hypothesis token window aligned to reference `[start,
/// end]`, or `None` if every reference token in the span was deleted
/// (no aligned hypothesis tokens at all).
pub fn aligned_hypothesis_window(
    alignment: &Alignment,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    let mut hyp_positions = Vec::new();
    for (op_idx, ref_idx) in alignment.ref_index.iter().enumerate() {
        if let Some(r) = ref_idx {
            if *r >= start && *r <= end {
                if let Some(h) = alignment.hyp_index[op_idx] {
                    hyp_positions.push(h);
                }
            }
        }
    }
    if hyp_positions.is_empty() {
        return None;
    }
    let min = *hyp_positions.iter().min().unwrap();
    let max = *hyp_positions.iter().max().unwrap();
    Some((min, max))
}

/// Classify one error span given the full reference/hypothesis token
/// sequences, the precomputed alignment between them, and the span's
/// `[start, end]` reference token range plus its verbatim `spoken` form.
pub fn classify_span(
    hypothesis: &[String],
    alignment: &Alignment,
    start: usize,
    end: usize,
    spoken: &str,
    corrections: &HashMap<String, String>,
) -> SpanOutcome {
    let spoken_norm = normalize_tokens(spoken).join(" ");
    let window = aligned_hypothesis_window(alignment, start, end);
    let extracted = match window {
        Some((min, max)) if max < hypothesis.len() => hypothesis[min..=max].join(" "),
        _ => return SpanOutcome::Misrecognized,
    };

    if extracted == spoken_norm {
        return SpanOutcome::Preserved;
    }
    if let Some(corrected) = corrections.get(&spoken_norm) {
        if extracted == *corrected {
            return SpanOutcome::FalseCorrection;
        }
    }
    SpanOutcome::Misrecognized
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MprAccumulator {
    pub preserved: u64,
    pub false_corrections: u64,
    pub misrecognized: u64,
}

impl MprAccumulator {
    pub fn add(&mut self, outcome: SpanOutcome) {
        match outcome {
            SpanOutcome::Preserved => self.preserved += 1,
            SpanOutcome::FalseCorrection => self.false_corrections += 1,
            SpanOutcome::Misrecognized => self.misrecognized += 1,
        }
    }

    pub fn total(&self) -> u64 {
        self.preserved + self.false_corrections + self.misrecognized
    }

    /// Mistake-preservation rate: preserved / total annotated spans.
    pub fn mpr(&self) -> f64 {
        if self.total() == 0 {
            0.0
        } else {
            self.preserved as f64 / self.total() as f64
        }
    }

    /// False-correction rate: false corrections / total annotated spans.
    pub fn false_correction_rate(&self) -> f64 {
        if self.total() == 0 {
            0.0
        } else {
            self.false_corrections as f64 / self.total() as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::wer::align;

    fn toks(s: &str) -> Vec<String> {
        normalize_tokens(s)
    }

    #[test]
    fn exact_preservation_is_detected() {
        let r = toks("i have went there yesterday");
        let h = toks("i have went there yesterday");
        let a = align(&r, &h);
        let corrections = known_corrections();
        let outcome = classify_span(&h, &a, 1, 2, "have went", &corrections);
        assert_eq!(outcome, SpanOutcome::Preserved);
    }

    #[test]
    fn grammatical_correction_is_classified_separately_from_ordinary_misrecognition() {
        // This is the AC5 fixture: a hypothesis that silently fixes the
        // annotated error must NOT be scored as a preserved match.
        let r = toks("i have went there yesterday");
        let h = toks("i have gone there yesterday");
        let a = align(&r, &h);
        let corrections = known_corrections();
        let outcome = classify_span(&h, &a, 1, 2, "have went", &corrections);
        assert_eq!(outcome, SpanOutcome::FalseCorrection);
    }

    #[test]
    fn ordinary_misrecognition_is_not_a_false_correction() {
        let r = toks("i have went there yesterday");
        let h = toks("i have wet there yesterday");
        let a = align(&r, &h);
        let corrections = known_corrections();
        let outcome = classify_span(&h, &a, 1, 2, "have went", &corrections);
        assert_eq!(outcome, SpanOutcome::Misrecognized);
    }

    #[test]
    fn minimal_pair_span_can_never_be_a_false_correction() {
        // "went"/"won't" is acoustic confusion, not a grammar fix: there is
        // no entry in `corrections` for it, so a substitution must land as
        // an ordinary misrecognition even though it "sounds like a fix".
        let r = toks("i said i won't go there");
        let h = toks("i said i went there"); // recognizer swapped won't -> went
        let a = align(&r, &h);
        let corrections = known_corrections();
        assert!(!corrections.contains_key("won't go"));
        let outcome = classify_span(&h, &a, 3, 4, "won't go", &corrections);
        assert_eq!(outcome, SpanOutcome::Misrecognized);
    }

    #[test]
    fn deleted_span_with_no_aligned_hypothesis_tokens_is_misrecognized() {
        let r = toks("i have went there yesterday");
        let h = toks("i there yesterday"); // "have went" entirely dropped
        let a = align(&r, &h);
        let corrections = known_corrections();
        let outcome = classify_span(&h, &a, 1, 2, "have went", &corrections);
        assert_eq!(outcome, SpanOutcome::Misrecognized);
    }

    #[test]
    fn accumulator_computes_mpr_and_false_correction_rate() {
        let mut acc = MprAccumulator::default();
        acc.add(SpanOutcome::Preserved);
        acc.add(SpanOutcome::Preserved);
        acc.add(SpanOutcome::FalseCorrection);
        acc.add(SpanOutcome::Misrecognized);
        assert_eq!(acc.total(), 4);
        assert!((acc.mpr() - 0.5).abs() < 1e-9);
        assert!((acc.false_correction_rate() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn every_corpus_correction_pair_is_actually_different_from_its_spoken_form() {
        // A corrections entry that equals its own key would let a
        // preserved match be misclassified as a false correction.
        for (spoken, corrected) in known_corrections() {
            assert_ne!(spoken, corrected, "no-op correction entry for '{spoken}'");
        }
    }
}
