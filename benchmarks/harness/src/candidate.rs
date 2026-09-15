//! Candidate descriptor types (Spec 05 section 6, "Produced — candidate
//! descriptor") and local model-file checksum verification
//! (`mistaken-bench fetch`).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::manifest::sha256_file;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDescriptor {
    pub repo: String,
    pub tag: String,
    #[serde(rename = "buildType")]
    pub build_type: String,
    #[serde(rename = "linkedRuntime")]
    pub linked_runtime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    pub path: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub repo: String,
    pub revision: String,
    pub files: Vec<ModelFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodingDescriptor {
    pub method: String,
    #[serde(rename = "numThreads")]
    pub num_threads: u32,
    pub provider: String,
    #[serde(rename = "enableEndpoint")]
    pub enable_endpoint: bool,
    /// Milliseconds of synthetic zero-valued audio the adapter feeds the
    /// recognizer before the clip's real samples, to let a streaming
    /// model's internal state warm up past its cold-start transient
    /// before the first real word arrives (spec-05-remediation
    /// experiment; sherpa-onnx adapter only). Absent/`None`/`0` means the
    /// original, unchanged behavior — every previously frozen candidate
    /// descriptor omits this field and is unaffected.
    #[serde(
        rename = "warmupSilenceMs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub warmup_silence_ms: Option<u32>,
    /// whisper-cpp adapter only, ignored by sherpa-onnx. Overrides
    /// `whisper_full_params.initial_prompt` (spec-05-remediation
    /// candidate-expansion experiment): prepended decoder context aimed
    /// at reducing Whisper's language-model-driven grammar
    /// auto-correction of deliberately incorrect input. Absent for every
    /// previously frozen candidate descriptor — unchanged behavior
    /// (`nullptr`, whisper.cpp's own default) unless explicitly set.
    #[serde(
        rename = "initialPrompt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub initial_prompt: Option<String>,
    /// whisper-cpp adapter only, ignored by sherpa-onnx. Overrides
    /// `whisper_full_params.no_speech_thold` (library default `0.6`).
    /// Absent means the library default, unchanged for every previously
    /// frozen candidate descriptor.
    #[serde(
        rename = "noSpeechThold",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub no_speech_thold: Option<f32>,
    /// whisper-cpp adapter only, ignored by sherpa-onnx. Overrides
    /// `whisper_full_params.suppress_nst` (non-speech-token suppression;
    /// the whisper.cpp library default is `false` — the adapter never
    /// previously set this field explicitly, so every previously frozen
    /// candidate ran with non-speech-token suppression *off*). Absent
    /// means the library default (`false`), unchanged for every
    /// previously frozen candidate descriptor.
    #[serde(
        rename = "suppressNst",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub suppress_nst: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateDescriptor {
    #[serde(rename = "candidateId")]
    pub candidate_id: String,
    pub adapter: String,
    pub runtime: RuntimeDescriptor,
    #[serde(rename = "streamingMode")]
    pub streaming_mode: String,
    pub model: ModelDescriptor,
    pub decoding: DecodingDescriptor,
    #[serde(rename = "payloadBytes")]
    pub payload_bytes: u64,
}

impl CandidateDescriptor {
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
    }

    /// Sum of every declared model file's `bytes`. Independent from the
    /// hand-recorded `payloadBytes` field so a stale total is detectable.
    pub fn declared_total_bytes(&self) -> u64 {
        self.model.files.iter().map(|f| f.bytes).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    Verified,
    Missing {
        path: String,
    },
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    NoChecksumRecorded {
        path: String,
    },
}

/// Recompute SHA-256 and byte size for every file the descriptor declares,
/// relative to `models_dir` (`benchmarks/models/<candidate-id>/`). Refuses
/// (returns a non-`Verified` outcome) on any mismatch or missing file —
/// `mistaken-bench fetch` never silently proceeds with an unverified file.
pub fn verify_model_files(
    descriptor: &CandidateDescriptor,
    models_dir: &Path,
) -> Vec<FetchOutcome> {
    descriptor
        .model
        .files
        .iter()
        .map(|file| {
            let path = models_dir.join(&file.path);
            if !path.is_file() {
                return FetchOutcome::Missing {
                    path: file.path.clone(),
                };
            }
            let actual_size = match fs::metadata(&path) {
                Ok(meta) => meta.len(),
                Err(_) => {
                    return FetchOutcome::Missing {
                        path: file.path.clone(),
                    }
                }
            };
            if actual_size != file.bytes {
                return FetchOutcome::SizeMismatch {
                    path: file.path.clone(),
                    expected: file.bytes,
                    actual: actual_size,
                };
            }
            match &file.sha256 {
                None => FetchOutcome::NoChecksumRecorded {
                    path: file.path.clone(),
                },
                Some(expected) => match sha256_file(&path) {
                    Ok(actual) if actual == *expected => FetchOutcome::Verified,
                    Ok(actual) => FetchOutcome::ChecksumMismatch {
                        path: file.path.clone(),
                        expected: expected.clone(),
                        actual,
                    },
                    Err(_) => FetchOutcome::Missing {
                        path: file.path.clone(),
                    },
                },
            }
        })
        .collect()
}

/// A file passes when its content checksum is verified, or — matching this
/// spec's own candidate descriptor convention for small non-LFS text files
/// such as `tokens.txt` (section 6 example omits `sha256`) — when its size
/// matches and no checksum was declared to check. `Missing`,
/// `SizeMismatch`, and `ChecksumMismatch` always fail the candidate.
pub fn all_verified(outcomes: &[FetchOutcome]) -> bool {
    outcomes.iter().all(|o| {
        matches!(
            o,
            FetchOutcome::Verified | FetchOutcome::NoChecksumRecorded { .. }
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::Write;

    fn write_temp_file(dir: &Path, name: &str, content: &[u8]) {
        let path = dir.join(name);
        let mut f = fs::File::create(path).unwrap();
        f.write_all(content).unwrap();
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn deserializes_the_documented_candidate_descriptor_shape() {
        let json = r#"{
            "candidateId": "sherpa-zipformer-en-2023-06-26-int8-left64",
            "adapter": "sherpa-onnx",
            "runtime": {"repo": "k2-fsa/sherpa-onnx", "tag": "v1.13.8", "buildType": "Release", "linkedRuntime": "onnxruntime (vendored by tag)"},
            "streamingMode": "native-streaming",
            "model": {
                "repo": "csukuangfj/sherpa-onnx-streaming-zipformer-en-2023-06-26",
                "revision": "672fbf1b30579d6585301139bb363f42a0ad4a24",
                "files": [
                    {"path": "encoder.onnx", "bytes": 71082637, "sha256": "0d072fd4"},
                    {"path": "tokens.txt", "bytes": 5048}
                ]
            },
            "decoding": {"method": "greedy_search", "numThreads": 2, "provider": "cpu", "enableEndpoint": true},
            "payloadBytes": 72899121
        }"#;
        let descriptor: CandidateDescriptor = serde_json::from_str(json).unwrap();
        assert_eq!(
            descriptor.candidate_id,
            "sherpa-zipformer-en-2023-06-26-int8-left64"
        );
        assert_eq!(descriptor.model.files.len(), 2);
        assert_eq!(descriptor.model.files[1].sha256, None);
    }

    #[test]
    fn warmup_silence_ms_defaults_to_none_when_absent() {
        // Every frozen candidate descriptor omits `warmupSilenceMs`; this
        // spec-05-remediation field must not require updating them.
        let json = r#"{"method": "greedy_search", "numThreads": 2, "provider": "cpu", "enableEndpoint": true}"#;
        let decoding: DecodingDescriptor = serde_json::from_str(json).unwrap();
        assert_eq!(decoding.warmup_silence_ms, None);
    }

    #[test]
    fn warmup_silence_ms_is_read_when_present() {
        let json = r#"{"method": "greedy_search", "numThreads": 2, "provider": "cpu", "enableEndpoint": true, "warmupSilenceMs": 300}"#;
        let decoding: DecodingDescriptor = serde_json::from_str(json).unwrap();
        assert_eq!(decoding.warmup_silence_ms, Some(300));
    }

    #[test]
    fn verify_model_files_passes_on_matching_size_and_checksum() {
        let dir = std::env::temp_dir().join(format!("mistaken-bench-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let content = b"fake model bytes";
        write_temp_file(&dir, "model.onnx", content);

        let descriptor = CandidateDescriptor {
            candidate_id: "test".to_string(),
            adapter: "sherpa-onnx".to_string(),
            runtime: RuntimeDescriptor {
                repo: "x".to_string(),
                tag: "v1".to_string(),
                build_type: "Release".to_string(),
                linked_runtime: "onnxruntime".to_string(),
            },
            streaming_mode: "native-streaming".to_string(),
            model: ModelDescriptor {
                repo: "x".to_string(),
                revision: "abc".to_string(),
                files: vec![ModelFile {
                    path: "model.onnx".to_string(),
                    bytes: content.len() as u64,
                    sha256: Some(sha256_hex(content)),
                }],
            },
            decoding: DecodingDescriptor {
                method: "greedy_search".to_string(),
                num_threads: 2,
                provider: "cpu".to_string(),
                enable_endpoint: true,
                warmup_silence_ms: None,
                initial_prompt: None,
                no_speech_thold: None,
                suppress_nst: None,
            },
            payload_bytes: content.len() as u64,
        };

        let outcomes = verify_model_files(&descriptor, &dir);
        assert!(all_verified(&outcomes), "{outcomes:?}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn verify_model_files_refuses_a_deliberately_corrupted_file() {
        let dir = std::env::temp_dir().join(format!(
            "mistaken-bench-test-corrupt-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let original = b"original model bytes";
        write_temp_file(&dir, "model.onnx", original);

        let descriptor = CandidateDescriptor {
            candidate_id: "test".to_string(),
            adapter: "sherpa-onnx".to_string(),
            runtime: RuntimeDescriptor {
                repo: "x".to_string(),
                tag: "v1".to_string(),
                build_type: "Release".to_string(),
                linked_runtime: "onnxruntime".to_string(),
            },
            streaming_mode: "native-streaming".to_string(),
            model: ModelDescriptor {
                repo: "x".to_string(),
                revision: "abc".to_string(),
                files: vec![ModelFile {
                    path: "model.onnx".to_string(),
                    bytes: original.len() as u64,
                    sha256: Some(sha256_hex(original)),
                }],
            },
            decoding: DecodingDescriptor {
                method: "greedy_search".to_string(),
                num_threads: 2,
                provider: "cpu".to_string(),
                enable_endpoint: true,
                warmup_silence_ms: None,
                initial_prompt: None,
                no_speech_thold: None,
                suppress_nst: None,
            },
            payload_bytes: original.len() as u64,
        };

        // Deliberately corrupt the local file after the descriptor was
        // written (AC7's required corruption test).
        write_temp_file(&dir, "model.onnx", b"corrupted!!!!!!!!!!!!");

        let outcomes = verify_model_files(&descriptor, &dir);
        assert!(!all_verified(&outcomes));
        assert!(outcomes.iter().any(|o| matches!(
            o,
            FetchOutcome::SizeMismatch { .. } | FetchOutcome::ChecksumMismatch { .. }
        )));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn verify_model_files_reports_missing_file() {
        let dir = std::env::temp_dir().join(format!(
            "mistaken-bench-test-missing-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();

        let descriptor = CandidateDescriptor {
            candidate_id: "test".to_string(),
            adapter: "sherpa-onnx".to_string(),
            runtime: RuntimeDescriptor {
                repo: "x".to_string(),
                tag: "v1".to_string(),
                build_type: "Release".to_string(),
                linked_runtime: "onnxruntime".to_string(),
            },
            streaming_mode: "native-streaming".to_string(),
            model: ModelDescriptor {
                repo: "x".to_string(),
                revision: "abc".to_string(),
                files: vec![ModelFile {
                    path: "does-not-exist.onnx".to_string(),
                    bytes: 123,
                    sha256: Some("a".repeat(64)),
                }],
            },
            decoding: DecodingDescriptor {
                method: "greedy_search".to_string(),
                num_threads: 2,
                provider: "cpu".to_string(),
                enable_endpoint: true,
                warmup_silence_ms: None,
                initial_prompt: None,
                no_speech_thold: None,
                suppress_nst: None,
            },
            payload_bytes: 123,
        };

        let outcomes = verify_model_files(&descriptor, &dir);
        assert!(matches!(outcomes[0], FetchOutcome::Missing { .. }));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_with_matching_size_and_no_declared_checksum_still_passes_overall() {
        // Mirrors this spec's own descriptor convention for small non-LFS
        // files such as tokens.txt (section 6 example has no `sha256`).
        let dir = std::env::temp_dir().join(format!(
            "mistaken-bench-test-nocheck-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let content = b"1\n2\n3\n";
        write_temp_file(&dir, "tokens.txt", content);

        let descriptor = CandidateDescriptor {
            candidate_id: "test".to_string(),
            adapter: "sherpa-onnx".to_string(),
            runtime: RuntimeDescriptor {
                repo: "x".to_string(),
                tag: "v1".to_string(),
                build_type: "Release".to_string(),
                linked_runtime: "onnxruntime".to_string(),
            },
            streaming_mode: "native-streaming".to_string(),
            model: ModelDescriptor {
                repo: "x".to_string(),
                revision: "abc".to_string(),
                files: vec![ModelFile {
                    path: "tokens.txt".to_string(),
                    bytes: content.len() as u64,
                    sha256: None,
                }],
            },
            decoding: DecodingDescriptor {
                method: "greedy_search".to_string(),
                num_threads: 2,
                provider: "cpu".to_string(),
                enable_endpoint: true,
                warmup_silence_ms: None,
                initial_prompt: None,
                no_speech_thold: None,
                suppress_nst: None,
            },
            payload_bytes: content.len() as u64,
        };

        let outcomes = verify_model_files(&descriptor, &dir);
        assert!(matches!(
            outcomes[0],
            FetchOutcome::NoChecksumRecorded { .. }
        ));
        assert!(all_verified(&outcomes), "{outcomes:?}");
        fs::remove_dir_all(&dir).ok();
    }
}
