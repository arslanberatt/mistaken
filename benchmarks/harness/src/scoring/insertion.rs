//! Hallucination / insertion-rate scoring for `noise-silence` clips (Spec 05
//! section 6, "Produced — scoring contract" and section 12 AC12).

use crate::scoring::wer::{Alignment, EditOp};

/// `expectPhysicalSilence` clips: any non-empty output is a hallucination by
/// definition. The metric is the absolute non-empty hypothesis token count.
pub fn silence_hallucination_tokens(hypothesis_tokens: &[String]) -> usize {
    hypothesis_tokens.len()
}

/// Remaining `noise-silence` clips (real speech over a noise bed): inserted
/// tokens per second of audio, from the reference/hypothesis alignment.
pub fn inserted_token_count(alignment: &Alignment) -> usize {
    alignment
        .ops
        .iter()
        .filter(|op| matches!(op, EditOp::Insert))
        .count()
}

#[derive(Debug, Default, Clone, Copy)]
pub struct InsertionAccumulator {
    total_inserted_tokens: u64,
    total_audio_ms: u64,
}

impl InsertionAccumulator {
    pub fn add(&mut self, inserted_tokens: usize, audio_ms: u64) {
        self.total_inserted_tokens += inserted_tokens as u64;
        self.total_audio_ms += audio_ms;
    }

    /// Inserted tokens per second of audio, aggregated across every scored
    /// non-silence `noise-silence` clip.
    pub fn tokens_per_second(&self) -> f64 {
        if self.total_audio_ms == 0 {
            0.0
        } else {
            self.total_inserted_tokens as f64 / (self.total_audio_ms as f64 / 1000.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::normalize_tokens;
    use crate::scoring::wer::align;

    #[test]
    fn empty_hypothesis_on_silence_has_zero_hallucination_tokens() {
        assert_eq!(silence_hallucination_tokens(&[]), 0);
    }

    #[test]
    fn any_non_empty_hypothesis_on_silence_is_a_hallucination() {
        let h = normalize_tokens("the weather is nice today");
        assert_eq!(silence_hallucination_tokens(&h), 5);
    }

    #[test]
    fn inserted_tokens_are_counted_from_alignment() {
        let r = normalize_tokens("can you hear me");
        let h = normalize_tokens("can you really hear me now"); // 2 insertions
        let a = align(&r, &h);
        assert_eq!(inserted_token_count(&a), 2);
    }

    #[test]
    fn accumulator_computes_tokens_per_second_across_clips() {
        let mut acc = InsertionAccumulator::default();
        acc.add(2, 10_000); // 2 tokens / 10s
        acc.add(0, 5_000); // 0 tokens / 5s
                           // total 2 tokens / 15s = 0.1333.../s
        assert!((acc.tokens_per_second() - (2.0 / 15.0)).abs() < 1e-9);
    }

    #[test]
    fn zero_audio_duration_does_not_panic_or_divide_by_zero() {
        let acc = InsertionAccumulator::default();
        assert_eq!(acc.tokens_per_second(), 0.0);
    }
}
