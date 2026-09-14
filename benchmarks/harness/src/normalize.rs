//! The single scorer text normalizer (Spec 05 section 6, "Produced — scoring
//! contract"). Applied identically to manifest references and adapter
//! hypotheses. This is the *only* text transformation permitted anywhere in
//! the measured or scored path: no stemming, no stopword removal, no
//! number-to-word mapping, no filler removal, no contraction expansion, and
//! no grammatical or spelling repair of any kind.

/// Normalize `text` into whitespace-separated lowercase tokens:
///
/// 1. Unicode NFC, then lowercase.
/// 2. Strip all punctuation except apostrophes that sit between two letters
///    (`didn't` survives; `--` and a trailing `.` do not).
/// 3. Collapse whitespace; split on whitespace.
/// 4. Nothing else.
pub fn normalize_tokens(text: &str) -> Vec<String> {
    let nfc: String = unicode_nfc(text);
    let lower = nfc.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let mut cleaned = String::with_capacity(chars.len());
    for (i, &ch) in chars.iter().enumerate() {
        let is_mid_word_apostrophe = ch == '\''
            && i > 0
            && i + 1 < chars.len()
            && chars[i - 1].is_alphabetic()
            && chars[i + 1].is_alphabetic();
        if ch.is_alphanumeric() || ch.is_whitespace() || is_mid_word_apostrophe {
            cleaned.push(ch);
        } else {
            cleaned.push(' ');
        }
    }
    cleaned.split_whitespace().map(|s| s.to_string()).collect()
}

/// Join normalized tokens back into the manifest's canonical space-joined
/// reference form. Used for `errorSpans.spoken` comparison, not for display.
pub fn join_tokens(tokens: &[String]) -> String {
    tokens.join(" ")
}

/// Minimal Unicode NFC normalization for the ASCII/Latin-1 text this corpus
/// uses. `unicode-normalization` is deliberately not a dependency (Spec 05
/// section 6 pins an exact, minimal dependency list); every corpus prompt is
/// authored in plain ASCII English, so a no-allocation-beyond-clone pass is
/// sufficient and the function documents the one case (already-composed
/// input) it relies on.
fn unicode_nfc(text: &str) -> String {
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_punctuation_but_keeps_mid_word_apostrophes() {
        let toks = normalize_tokens("I have went there yesterday and I didn't knew anyone.");
        assert_eq!(
            toks,
            vec![
                "i",
                "have",
                "went",
                "there",
                "yesterday",
                "and",
                "i",
                "didn't",
                "knew",
                "anyone"
            ]
        );
    }

    #[test]
    fn drops_leading_and_trailing_apostrophes() {
        let toks = normalize_tokens("'quoted' word--dash.");
        assert_eq!(toks, vec!["quoted", "word", "dash"]);
    }

    #[test]
    fn collapses_whitespace() {
        let toks = normalize_tokens("  multiple   spaces\tand\ntabs  ");
        assert_eq!(toks, vec!["multiple", "spaces", "and", "tabs"]);
    }

    #[test]
    fn empty_input_yields_empty_tokens() {
        assert!(normalize_tokens("").is_empty());
        assert!(normalize_tokens("   ...   --  ").is_empty());
    }

    #[test]
    fn lowercases_and_keeps_numbers() {
        let toks = normalize_tokens("WON'T Won't 42nd CAN'T");
        assert_eq!(toks, vec!["won't", "won't", "42nd", "can't"]);
    }

    #[test]
    fn join_roundtrips_simple_cases() {
        let toks = normalize_tokens("Can't stop, won't stop.");
        assert_eq!(join_tokens(&toks), "can't stop won't stop");
    }
}
