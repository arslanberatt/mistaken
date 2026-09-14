//! Word error rate: Levenshtein distance over normalized tokens.

/// Token-level edit operations produced by the alignment, used by MPR/false-
/// correction scoring to locate where a reference span landed in the
/// hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOp {
    Match,
    Substitute,
    Delete, // reference token has no hypothesis counterpart
    Insert, // hypothesis token has no reference counterpart
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alignment {
    pub ops: Vec<EditOp>,
    /// For each op, the reference token index it consumes (`None` for `Insert`).
    pub ref_index: Vec<Option<usize>>,
    /// For each op, the hypothesis token index it consumes (`None` for `Delete`).
    pub hyp_index: Vec<Option<usize>>,
}

/// Classic Levenshtein DP with traceback, unit cost per operation. `O(n*m)`
/// time/memory, which is fine here: corpus references are short (longest
/// `long-turn` script is on the order of 300 tokens) and this runs once per
/// clip per candidate repetition, not per audio frame.
pub fn align(reference: &[String], hypothesis: &[String]) -> Alignment {
    let n = reference.len();
    let m = hypothesis.len();
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i as u32;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j as u32;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub_cost = if reference[i - 1] == hypothesis[j - 1] {
                0
            } else {
                1
            };
            let sub = dp[i - 1][j - 1] + sub_cost;
            let del = dp[i - 1][j] + 1;
            let ins = dp[i][j - 1] + 1;
            dp[i][j] = sub.min(del).min(ins);
        }
    }

    // Traceback, preferring match/substitute over indel on ties so aligned
    // spans stay as tight as possible.
    let mut ops = Vec::new();
    let mut ref_index = Vec::new();
    let mut hyp_index = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 {
            let sub_cost = if reference[i - 1] == hypothesis[j - 1] {
                0
            } else {
                1
            };
            if dp[i][j] == dp[i - 1][j - 1] + sub_cost {
                ops.push(if sub_cost == 0 {
                    EditOp::Match
                } else {
                    EditOp::Substitute
                });
                ref_index.push(Some(i - 1));
                hyp_index.push(Some(j - 1));
                i -= 1;
                j -= 1;
                continue;
            }
        }
        if i > 0 && dp[i][j] == dp[i - 1][j] + 1 {
            ops.push(EditOp::Delete);
            ref_index.push(Some(i - 1));
            hyp_index.push(None);
            i -= 1;
            continue;
        }
        ops.push(EditOp::Insert);
        ref_index.push(None);
        hyp_index.push(Some(j - 1));
        j -= 1;
    }
    ops.reverse();
    ref_index.reverse();
    hyp_index.reverse();
    Alignment {
        ops,
        ref_index,
        hyp_index,
    }
}

pub fn edit_distance(reference: &[String], hypothesis: &[String]) -> u32 {
    align(reference, hypothesis)
        .ops
        .iter()
        .filter(|op| !matches!(op, EditOp::Match))
        .count() as u32
}

/// Aggregate WER = total edits / total reference tokens, computed over a set
/// of (reference, hypothesis) token-sequence pairs, matching Spec 05 section
/// 6: "computed per clip and aggregated as total-edits over
/// total-reference-tokens".
pub struct WerAccumulator {
    total_edits: u64,
    total_ref_tokens: u64,
}

impl WerAccumulator {
    pub fn new() -> Self {
        Self {
            total_edits: 0,
            total_ref_tokens: 0,
        }
    }

    pub fn add(&mut self, reference: &[String], hypothesis: &[String]) {
        self.total_edits += edit_distance(reference, hypothesis) as u64;
        self.total_ref_tokens += reference.len() as u64;
    }

    pub fn wer(&self) -> f64 {
        if self.total_ref_tokens == 0 {
            0.0
        } else {
            self.total_edits as f64 / self.total_ref_tokens as f64
        }
    }
}

impl Default for WerAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<String> {
        s.split_whitespace().map(|t| t.to_string()).collect()
    }

    #[test]
    fn identical_sequences_have_zero_distance() {
        let t = toks("i have went there");
        assert_eq!(edit_distance(&t, &t), 0);
    }

    #[test]
    fn single_substitution_costs_one() {
        let r = toks("i have went there");
        let h = toks("i have gone there");
        assert_eq!(edit_distance(&r, &h), 1);
    }

    #[test]
    fn single_deletion_costs_one() {
        let r = toks("i have went there");
        let h = toks("i went there");
        assert_eq!(edit_distance(&r, &h), 1);
    }

    #[test]
    fn single_insertion_costs_one() {
        let r = toks("i went there");
        let h = toks("i have went there");
        assert_eq!(edit_distance(&r, &h), 1);
    }

    #[test]
    fn wer_accumulates_across_clips() {
        let mut acc = WerAccumulator::new();
        acc.add(&toks("i have went there"), &toks("i have gone there")); // 1 edit / 4 ref
        acc.add(&toks("she can't do it"), &toks("she can't do it")); // 0 edit / 4 ref
        assert!((acc.wer() - 0.125).abs() < 1e-9);
    }

    #[test]
    fn empty_reference_with_empty_hypothesis_has_zero_wer() {
        let mut acc = WerAccumulator::new();
        acc.add(&[], &[]);
        assert_eq!(acc.wer(), 0.0);
    }

    #[test]
    fn empty_reference_with_hallucinated_hypothesis_has_only_insertions() {
        let r: Vec<String> = vec![];
        let h = toks("hello there");
        assert_eq!(edit_distance(&r, &h), 2);
    }
}
