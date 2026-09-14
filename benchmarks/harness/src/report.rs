//! Markdown report rendering (`mistaken-bench report`) and the license
//! record completeness checker (`mistaken-bench licenses --check`).

use std::collections::BTreeMap;

use crate::scoring::gates::GateResult;

pub struct CandidateReportSection {
    pub candidate_id: String,
    pub host_profile: String,
    pub pace_note: String,
    pub gates: Vec<GateResult>,
    pub blocked_reason: Option<String>,
    pub simulated_streaming: bool,
}

pub fn render_host_report(host_profile: &str, sections: &[CandidateReportSection]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Mistaken benchmark report — host `{host_profile}`\n\n"
    ));
    out.push_str(
        "Units: milliseconds unless noted; RSS in megabytes (MiB); rates are fractions in `[0, 1]` unless noted per row.\n\n",
    );
    for section in sections {
        out.push_str(&format!("## `{}`\n\n", section.candidate_id));
        out.push_str(&format!(
            "Host profile: `{}`. {}\n\n",
            section.host_profile, section.pace_note
        ));
        if section.simulated_streaming {
            out.push_str(
                "**Streaming mode: `simulated-rolling-window`.** Latency numbers below are a \
                 rolling-window re-decode measurement, not native streaming latency, and must \
                 never be compared to a native-streaming candidate's latency as if equivalent.\n\n",
            );
        }
        if let Some(reason) = &section.blocked_reason {
            out.push_str(&format!("**Status: `blocked`.** {reason}\n\n"));
            continue;
        }
        out.push_str("| Gate | Measured | Threshold | Bound | Result |\n");
        out.push_str("|---|---|---|---|---|\n");
        for gate in &section.gates {
            let bound = if gate.is_upper_bound { "<=" } else { ">=" };
            let (measured_str, result) = match gate.pass {
                Some(true) => (format!("{:.4}", gate.measured.unwrap_or(f64::NAN)), "PASS"),
                Some(false) => (format!("{:.4}", gate.measured.unwrap_or(f64::NAN)), "FAIL"),
                None => ("not measured".to_string(), "NOT MEASURED"),
            };
            out.push_str(&format!(
                "| `{}` | {} | {} {:.4} | {} | **{}** |\n",
                gate.name, measured_str, bound, gate.threshold, bound, result
            ));
        }
        out.push('\n');
        let all_pass = section.gates.iter().all(|g| g.pass == Some(true));
        out.push_str(&format!(
            "**Overall on `{}`: {}**\n\n",
            section.host_profile,
            if all_pass { "PASS" } else { "FAIL" }
        ));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseCheckIssue {
    pub candidate_id: String,
    pub rule: String,
}

const REQUIRED_LICENSE_FIELDS: &[&str] = &[
    "Runtime repository",
    "Runtime tag",
    "Runtime license",
    "Linked inference runtime",
    "Model repository",
    "Model revision",
    "Weight license (declared)",
    "Upstream provenance repository",
    "Upstream provenance license",
    "Training corpus",
    "Training corpus terms",
    "Required attribution text",
    "Redistribution verdict",
];

const VALID_VERDICTS: &[&str] = &[
    "permitted-with-attribution",
    "permitted",
    "unclear",
    "prohibited",
];

const PLACEHOLDER_MARKERS: &[&str] = &["TBD", "TODO", "PENDING", "Pending", "pending", "???"];

/// Parse `license-record.md` sections (`## <candidate-id>` headings, `-
/// Field: value` bullets) and confirm every *known candidate* has a
/// section with every required field present, non-placeholder, and a
/// `Redistribution verdict` value from the four allowed enum values (Spec
/// 05 section 6, "Produced — license record"; section 12 AC16).
///
/// `known_candidate_ids` (the real candidate descriptors under
/// `benchmarks/candidates/`) disambiguates a real candidate `##` heading
/// from decorative prose sections such as a closing summary table; a
/// heading outside that set is not validated as a candidate row.
pub fn check_license_record(
    markdown: &str,
    known_candidate_ids: &[String],
) -> Result<Vec<String>, Vec<LicenseCheckIssue>> {
    let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current: Option<String> = None;

    for line in markdown.lines() {
        if let Some(id) = line.strip_prefix("## ") {
            let id = id.trim().trim_matches('`').to_string();
            current = Some(id.clone());
            if known_candidate_ids.iter().any(|k| k == &id) {
                sections.entry(id).or_default();
            } else {
                current = None; // decorative heading: bullets under it are ignored
            }
            continue;
        }
        if let Some(cur) = &current {
            if let Some(rest) = line.trim_start().strip_prefix("- ") {
                if let Some((field, value)) = rest.split_once(':') {
                    sections
                        .get_mut(cur)
                        .unwrap()
                        .insert(field.trim().to_string(), value.trim().to_string());
                }
            }
        }
    }

    let mut issues = Vec::new();
    for known in known_candidate_ids {
        if !sections.contains_key(known) {
            issues.push(LicenseCheckIssue {
                candidate_id: known.clone(),
                rule: "no `## <candidate-id>` section found in license-record.md".to_string(),
            });
        }
    }

    for (candidate_id, fields) in &sections {
        for required in REQUIRED_LICENSE_FIELDS {
            match fields.get(*required) {
                None => issues.push(LicenseCheckIssue {
                    candidate_id: candidate_id.clone(),
                    rule: format!("missing field '{required}'"),
                }),
                Some(value) => {
                    if value.is_empty() || PLACEHOLDER_MARKERS.iter().any(|m| value.contains(m)) {
                        issues.push(LicenseCheckIssue {
                            candidate_id: candidate_id.clone(),
                            rule: format!(
                                "field '{required}' is empty or a placeholder: '{value}'"
                            ),
                        });
                    }
                }
            }
        }
        if let Some(verdict) = fields.get("Redistribution verdict") {
            let normalized = verdict.trim_matches('`').to_string();
            if !VALID_VERDICTS.contains(&normalized.as_str()) {
                issues.push(LicenseCheckIssue {
                    candidate_id: candidate_id.clone(),
                    rule: format!(
                        "Redistribution verdict '{verdict}' is not one of {VALID_VERDICTS:?}"
                    ),
                });
            }
        }
    }

    if issues.is_empty() {
        Ok(sections.keys().cloned().collect())
    } else {
        Err(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::gates::GateResult;

    #[test]
    fn render_marks_simulated_streaming_candidates_inline() {
        let sections = vec![CandidateReportSection {
            candidate_id: "whisper-base-en-ggml".to_string(),
            host_profile: "mac-arm64".to_string(),
            pace_note: "realtime pace, 3 repetitions".to_string(),
            gates: vec![GateResult {
                name: "accuracy.wer_overall",
                measured: Some(0.1),
                threshold: 0.25,
                is_upper_bound: true,
                pass: Some(true),
            }],
            blocked_reason: None,
            simulated_streaming: true,
        }];
        let rendered = render_host_report("mac-arm64", &sections);
        assert!(rendered.contains("simulated-rolling-window"));
        assert!(rendered.contains("PASS"));
    }

    #[test]
    fn render_shows_blocked_status_without_a_gate_table() {
        let sections = vec![CandidateReportSection {
            candidate_id: "sherpa-zipformer-en-2023-06-26-fp32-left64".to_string(),
            host_profile: "win-x64".to_string(),
            pace_note: String::new(),
            gates: vec![],
            blocked_reason: Some("model_load_failed: unsupported opset".to_string()),
            simulated_streaming: false,
        }];
        let rendered = render_host_report("win-x64", &sections);
        assert!(rendered.contains("**Status: `blocked`.**"));
        assert!(rendered.contains("model_load_failed"));
    }

    fn complete_section(id: &str, verdict: &str) -> String {
        format!(
            "## {id}\n\
             - Runtime repository: k2-fsa/sherpa-onnx\n\
             - Runtime tag: v1.13.8\n\
             - Runtime license: Apache-2.0 (source: https://example/LICENSE, retrieved: 2026-09-11)\n\
             - Linked inference runtime: onnxruntime, MIT (source: https://example/LICENSE, retrieved: 2026-09-11)\n\
             - Model repository: example/model\n\
             - Model revision: abc123\n\
             - Weight license (declared): Apache-2.0 (source: https://example, retrieved: 2026-09-11)\n\
             - Upstream provenance repository: example/upstream\n\
             - Upstream provenance license: none declared (source: https://example, retrieved: 2026-09-11)\n\
             - Training corpus: LibriSpeech (OpenSLR SLR12)\n\
             - Training corpus terms: CC BY 4.0, attribution required (source: https://openslr.org/12, retrieved: 2026-09-11)\n\
             - Required attribution text: \"LibriSpeech (c) 2015 by Vassil Panayotov\"\n\
             - Redistribution verdict: {verdict}\n\n"
        )
    }

    #[test]
    fn complete_section_passes_the_check() {
        let md = complete_section("candidate-a", "permitted-with-attribution");
        let ids = vec!["candidate-a".to_string()];
        let result = check_license_record(&md, &ids);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn missing_field_is_reported_by_candidate() {
        let mut md = complete_section("candidate-a", "permitted");
        md = md.replace("- Model revision: abc123\n", "");
        let ids = vec!["candidate-a".to_string()];
        let result = check_license_record(&md, &ids);
        let issues = result.unwrap_err();
        assert!(issues
            .iter()
            .any(|i| i.candidate_id == "candidate-a" && i.rule.contains("Model revision")));
    }

    #[test]
    fn placeholder_value_is_rejected() {
        let md = complete_section("candidate-a", "permitted")
            .replace("- Model revision: abc123\n", "- Model revision: TBD\n");
        let ids = vec!["candidate-a".to_string()];
        let result = check_license_record(&md, &ids);
        let issues = result.unwrap_err();
        assert!(issues.iter().any(|i| i.rule.contains("placeholder")));
    }

    #[test]
    fn invalid_verdict_value_is_rejected() {
        let md = complete_section("candidate-a", "probably-fine");
        let ids = vec!["candidate-a".to_string()];
        let result = check_license_record(&md, &ids);
        let issues = result.unwrap_err();
        assert!(issues
            .iter()
            .any(|i| i.rule.contains("Redistribution verdict")));
    }

    #[test]
    fn unclear_verdict_is_a_valid_enum_value_even_though_it_blocks_approval() {
        // The checker validates *shape*, not whether the candidate is
        // approvable; an honest 'unclear' must not be rejected as malformed.
        let md = complete_section("candidate-a", "unclear");
        let ids = vec!["candidate-a".to_string()];
        let result = check_license_record(&md, &ids);
        assert!(result.is_ok());
    }

    #[test]
    fn a_missing_known_candidate_is_reported() {
        let md = complete_section("candidate-a", "permitted");
        let ids = vec!["candidate-a".to_string(), "candidate-b".to_string()];
        let result = check_license_record(&md, &ids);
        let issues = result.unwrap_err();
        assert!(issues
            .iter()
            .any(|i| i.candidate_id == "candidate-b" && i.rule.contains("no `## ")));
    }

    #[test]
    fn a_decorative_heading_outside_known_candidates_is_not_validated() {
        // A closing "## Summary" table (or similar prose section) must not
        // be misread as an incomplete candidate row.
        let mut md = complete_section("candidate-a", "permitted");
        md.push_str("## Summary (not a candidate row)\n\n| a | b |\n|---|---|\n");
        let ids = vec!["candidate-a".to_string()];
        let result = check_license_record(&md, &ids);
        assert!(result.is_ok(), "{result:?}");
    }
}
