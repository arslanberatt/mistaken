//! Adapter process protocol (Spec 05 section 6, "Produced — adapter process
//! protocol"): one JSON job on stdin, newline-delimited JSON events on
//! stdout, stderr diagnostic-only.

use serde::{Deserialize, Serialize};

/// NDJSON lines longer than this are a protocol error for that clip, not an
/// unbounded allocation (section 11, "Bounded buffering").
pub const MAX_LINE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pace {
    Realtime,
    Asap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodingParams {
    pub method: String,
    #[serde(rename = "numThreads")]
    pub num_threads: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    #[serde(rename = "clipId")]
    pub clip_id: String,
    #[serde(rename = "wavPath")]
    pub wav_path: String,
    pub model: serde_json::Value,
    pub decoding: DecodingParams,
    pub pace: Pace,
    #[serde(rename = "chunkMs")]
    pub chunk_ms: u32,
    #[serde(rename = "windowMs", skip_serializing_if = "Option::is_none")]
    pub window_ms: Option<u32>,
    /// See `candidate::DecodingDescriptor::warmup_silence_ms`. Absent for
    /// every previously frozen candidate; sherpa-onnx adapter only.
    #[serde(
        rename = "warmupSilenceMs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub warmup_silence_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Event {
    Ready {
        #[serde(rename = "atMs")]
        at_ms: u64,
        #[serde(rename = "streamingMode", skip_serializing_if = "Option::is_none")]
        streaming_mode: Option<String>,
    },
    Partial {
        #[serde(rename = "atMs")]
        at_ms: u64,
        text: String,
    },
    Final {
        #[serde(rename = "atMs")]
        at_ms: u64,
        text: String,
        #[serde(rename = "segmentIndex")]
        segment_index: u32,
    },
    Metrics {
        #[serde(rename = "atMs")]
        at_ms: u64,
        #[serde(rename = "audioMs")]
        audio_ms: u64,
        #[serde(rename = "wallMs")]
        wall_ms: u64,
        #[serde(rename = "cpuTimeMs")]
        cpu_time_ms: u64,
        #[serde(rename = "peakRssBytes")]
        peak_rss_bytes: u64,
        #[serde(rename = "fedChunks")]
        fed_chunks: u32,
        #[serde(rename = "droppedChunks")]
        dropped_chunks: u32,
    },
    Error {
        #[serde(rename = "atMs")]
        at_ms: u64,
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    LineTooLong { bytes: usize },
    InvalidJson { line: String, reason: String },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::LineTooLong { bytes } => {
                write!(
                    f,
                    "NDJSON line exceeds {MAX_LINE_BYTES} bytes (got {bytes})"
                )
            }
            ProtocolError::InvalidJson { line, reason } => {
                write!(f, "invalid NDJSON line '{line}': {reason}")
            }
        }
    }
}

/// Parse one NDJSON line into an `Event`, enforcing the 64 KiB size cap
/// before attempting to parse.
pub fn parse_event_line(line: &str) -> Result<Event, ProtocolError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(ProtocolError::LineTooLong { bytes: line.len() });
    }
    serde_json::from_str::<Event>(line).map_err(|e| ProtocolError::InvalidJson {
        line: line.to_string(),
        reason: e.to_string(),
    })
}

/// Parse a full NDJSON stream captured from an adapter's stdout, returning
/// every successfully parsed event and every protocol error encountered
/// (a malformed line ends that clip's run per section 11's error taxonomy,
/// but parsing keeps going so the caller can see the full stream for
/// diagnostics).
pub fn parse_ndjson_stream(stdout: &str) -> (Vec<Event>, Vec<ProtocolError>) {
    let mut events = Vec::new();
    let mut errors = Vec::new();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_event_line(line) {
            Ok(event) => events.push(event),
            Err(e) => errors.push(e),
        }
    }
    (events, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ready_event() {
        let line = r#"{"type":"ready","atMs":0}"#;
        let event = parse_event_line(line).unwrap();
        assert_eq!(
            event,
            Event::Ready {
                at_ms: 0,
                streaming_mode: None
            }
        );
    }

    #[test]
    fn parses_ready_event_with_streaming_mode() {
        let line = r#"{"type":"ready","atMs":0,"streamingMode":"simulated-rolling-window"}"#;
        let event = parse_event_line(line).unwrap();
        assert_eq!(
            event,
            Event::Ready {
                at_ms: 0,
                streaming_mode: Some("simulated-rolling-window".to_string())
            }
        );
    }

    #[test]
    fn parses_partial_final_metrics_error_events() {
        let partial =
            parse_event_line(r#"{"type":"partial","atMs":640,"text":"i actually have"}"#).unwrap();
        assert!(matches!(partial, Event::Partial { at_ms: 640, .. }));

        let fin = parse_event_line(
            r#"{"type":"final","atMs":7710,"text":"i actually have went","segmentIndex":0}"#,
        )
        .unwrap();
        assert!(matches!(
            fin,
            Event::Final {
                at_ms: 7710,
                segment_index: 0,
                ..
            }
        ));

        let metrics = parse_event_line(
            r#"{"type":"metrics","atMs":7980,"audioMs":7480,"wallMs":7980,"cpuTimeMs":2140,"peakRssBytes":384102400,"fedChunks":75,"droppedChunks":0}"#,
        )
        .unwrap();
        assert!(matches!(
            metrics,
            Event::Metrics {
                fed_chunks: 75,
                dropped_chunks: 0,
                ..
            }
        ));

        let error = parse_event_line(
            r#"{"type":"error","atMs":120,"code":"model_load_failed","message":"boom"}"#,
        )
        .unwrap();
        assert!(matches!(error, Event::Error { code, .. } if code == "model_load_failed"));
    }

    #[test]
    fn oversized_line_is_a_protocol_error_not_a_panic() {
        let huge = "x".repeat(MAX_LINE_BYTES + 1);
        let line = format!(r#"{{"type":"partial","atMs":0,"text":"{huge}"}}"#);
        let result = parse_event_line(&line);
        assert_eq!(
            result,
            Err(ProtocolError::LineTooLong { bytes: line.len() })
        );
    }

    #[test]
    fn malformed_json_is_a_protocol_error() {
        let result = parse_event_line("not json");
        assert!(matches!(result, Err(ProtocolError::InvalidJson { .. })));
    }

    #[test]
    fn stream_parsing_keeps_going_after_one_bad_line() {
        let stream = "{\"type\":\"ready\",\"atMs\":0}\nnot json\n{\"type\":\"final\",\"atMs\":10,\"text\":\"hi\",\"segmentIndex\":0}\n";
        let (events, errors) = parse_ndjson_stream(stream);
        assert_eq!(events.len(), 2);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn job_serializes_with_documented_field_names() {
        let job = Job {
            clip_id: "mistake-tense-03".to_string(),
            wav_path: "/abs/path/clips/mistake-tense-03.wav".to_string(),
            model: serde_json::json!({"repo": "x"}),
            decoding: DecodingParams {
                method: "greedy_search".to_string(),
                num_threads: 2,
            },
            pace: Pace::Realtime,
            chunk_ms: 100,
            window_ms: None,
            warmup_silence_ms: None,
        };
        let value = serde_json::to_value(&job).unwrap();
        assert_eq!(value["clipId"], "mistake-tense-03");
        assert_eq!(value["wavPath"], "/abs/path/clips/mistake-tense-03.wav");
        assert_eq!(value["chunkMs"], 100);
        assert_eq!(value["pace"], "realtime");
        assert!(value.get("windowMs").is_none());
    }
}
