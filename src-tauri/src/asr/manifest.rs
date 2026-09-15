//! The pinned `DevelopmentOnly` model manifest, discovery, and presence /
//! checksum verification.
//!
//! Every field below is copied verbatim from
//! `benchmarks/candidates/sherpa-zipformer-en-20M-2023-02-17-int8.json`
//! (Spec 05, `permitted-with-attribution` license row) plus one digest the
//! descriptor omitted: `tokens.txt`'s SHA-256 was computed directly from
//! the staged model directory (recorded in `build-notes-asr.md`) because
//! the descriptor only pins its byte size. Nothing here may be changed to
//! select a different model/runtime or to claim `ProductionApproved`;
//! that requires a future reviewed Spec 05 approval and manifest
//! replacement.

use std::fs;
use std::path::{Path, PathBuf};

use tauri::{Manager, Runtime};

use super::recognizer::{AsrError, AsrErrorKind};

/// Distinguishes a fully local, license-recorded but quality-unapproved
/// development artifact from a future Spec 05 production approval. Only
/// `DevelopmentOnly` may ever be compiled in while Spec 05 stays blocked;
/// nothing in this crate can promote it at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrMaturity {
    DevelopmentOnly,
    ProductionApproved,
}

/// One declared model file: its path relative to the model directory, its
/// exact byte size, and its expected SHA-256 digest.
#[derive(Debug, Clone, Copy)]
pub struct ModelFileSpec {
    pub relative_path: &'static str,
    pub bytes: u64,
    pub sha256_hex: &'static str,
}

/// The complete pinned model/runtime/decoding identity for one recognizer
/// adapter.
#[derive(Debug, Clone, Copy)]
pub struct AsrModelManifest {
    pub maturity: AsrMaturity,
    pub model_id: &'static str,
    pub runtime_tag: &'static str,
    pub files: &'static [ModelFileSpec],
    pub decoding_method: &'static str,
    pub num_threads: i32,
    pub provider: &'static str,
}

/// The exact temporary development candidate authorized for Spec 06.
/// Copied field-for-field from the Spec 05 candidate descriptor; see the
/// module doc comment for the one added `tokens.txt` digest.
pub const DEVELOPMENT_MANIFEST: AsrModelManifest = AsrModelManifest {
    maturity: AsrMaturity::DevelopmentOnly,
    model_id: "sherpa-zipformer-en-20M-2023-02-17-int8",
    runtime_tag: "v1.13.8",
    files: &[
        ModelFileSpec {
            relative_path: "encoder-epoch-99-avg-1.int8.onnx",
            bytes: 42_845_182,
            sha256_hex: "3810755ce7c3ab26b42a8bcf39d191308fa27fb0f53358823ba46141d03b7eb3",
        },
        ModelFileSpec {
            relative_path: "decoder-epoch-99-avg-1.int8.onnx",
            bytes: 539_499,
            sha256_hex: "21e2a2acd961b3ac72f55be2f10f1a285e1b0b0ba010d7c0b6eab141411b163c",
        },
        ModelFileSpec {
            relative_path: "joiner-epoch-99-avg-1.int8.onnx",
            bytes: 259_572,
            sha256_hex: "e085d73b593cf9b0707f370dbd656d58327d3fe36d80d849202ef81df02cb01e",
        },
        ModelFileSpec {
            relative_path: "tokens.txt",
            bytes: 5_048,
            sha256_hex: "49e3c2646595fd907228b3c6787069658f67b17377c60aeb8619c4551b2316fb",
        },
    ],
    decoding_method: "greedy_search",
    num_threads: 2,
    provider: "cpu",
};

/// Resolves the model directory without touching the filesystem beyond a
/// resource-path lookup. Discovery order:
///
/// 1. `MISTAKEN_MODEL_DIR` when set — development override; must contain
///    the model id directory.
/// 2. The Tauri resource directory entry `resources/models/<model_id>/`.
///
/// There is no third location, no user-configurable path, and no
/// download.
pub fn resolve_model_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, AsrError> {
    if let Ok(dir) = std::env::var("MISTAKEN_MODEL_DIR") {
        return Ok(PathBuf::from(dir).join(DEVELOPMENT_MANIFEST.model_id));
    }
    let resource_dir = app.path().resource_dir().map_err(|_| {
        AsrError::new(
            AsrErrorKind::ModelMissing,
            "could not resolve the app resource directory",
        )
    })?;
    Ok(resource_dir
        .join("resources")
        .join("models")
        .join(DEVELOPMENT_MANIFEST.model_id))
}

/// Metadata-only presence check: existence and byte size for every
/// declared file. Loads nothing, hashes nothing, allocates no recognizer.
/// Suitable for launch-time status reporting.
pub fn check_presence(dir: &Path, manifest: &AsrModelManifest) -> Result<(), AsrError> {
    for file in manifest.files {
        let path = dir.join(file.relative_path);
        let metadata = fs::metadata(&path).map_err(|_| {
            AsrError::new(
                AsrErrorKind::ModelMissing,
                format!("expected model file at {}", path.display()),
            )
        })?;
        if metadata.len() != file.bytes {
            return Err(AsrError::new(
                AsrErrorKind::ModelMissing,
                format!(
                    "expected model file at {} to be {} bytes, found {}",
                    path.display(),
                    file.bytes,
                    metadata.len()
                ),
            ));
        }
    }
    Ok(())
}

/// Full SHA-256 verification of every declared file. Runs exactly once,
/// at first load, before `OnlineRecognizer::create`. A mismatch is
/// `model_unsupported`: the artifact on disk is not the pinned
/// development artifact and is never loaded.
pub fn verify_checksums(dir: &Path, manifest: &AsrModelManifest) -> Result<(), AsrError> {
    use sha2::{Digest, Sha256};

    for file in manifest.files {
        let path = dir.join(file.relative_path);
        let bytes = fs::read(&path).map_err(|_| {
            AsrError::new(
                AsrErrorKind::ModelMissing,
                format!("expected model file at {}", path.display()),
            )
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let digest = hasher.finalize();
        let actual = hex_encode(&digest);
        if actual != file.sha256_hex {
            return Err(AsrError::new(
                AsrErrorKind::ModelUnsupported,
                format!(
                    "model file {} does not match the pinned development adapter (expected sha256 {}, found {})",
                    file.relative_path, file.sha256_hex, actual
                ),
            ));
        }
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(dir: &Path, name: &str, contents: &[u8]) {
        let mut f = fs::File::create(dir.join(name)).unwrap();
        f.write_all(contents).unwrap();
    }

    fn tiny_manifest() -> (AsrModelManifest, &'static str) {
        (
            AsrModelManifest {
                maturity: AsrMaturity::DevelopmentOnly,
                model_id: "test-model",
                runtime_tag: "vtest",
                files: &[ModelFileSpec {
                    relative_path: "a.bin",
                    bytes: 5,
                    sha256_hex: "",
                }],
                decoding_method: "greedy_search",
                num_threads: 1,
                provider: "cpu",
            },
            "a.bin",
        )
    }

    #[test]
    fn presence_check_fails_when_file_is_absent() {
        let dir = tempfile_dir();
        let (manifest, _) = tiny_manifest();
        let error = check_presence(dir.path(), &manifest).expect_err("no files staged");
        assert_eq!(error.kind, AsrErrorKind::ModelMissing);
    }

    #[test]
    fn presence_check_fails_on_size_mismatch_and_passes_on_match() {
        let dir = tempfile_dir();
        let (manifest, name) = tiny_manifest();
        write_file(dir.path(), name, b"1234"); // 4 bytes, manifest expects 5
        let error = check_presence(dir.path(), &manifest).expect_err("size mismatch");
        assert_eq!(error.kind, AsrErrorKind::ModelMissing);

        write_file(dir.path(), name, b"12345"); // 5 bytes, matches
        check_presence(dir.path(), &manifest).expect("size now matches");
    }

    #[test]
    fn checksum_verification_rejects_corrupted_or_altered_file() {
        let dir = tempfile_dir();
        // Real sha256("12345") digest.
        let manifest = AsrModelManifest {
            maturity: AsrMaturity::DevelopmentOnly,
            model_id: "test-model",
            runtime_tag: "vtest",
            files: &[ModelFileSpec {
                relative_path: "a.bin",
                bytes: 5,
                sha256_hex: "5994471abb01112afcc18159f6cc74b4f511b99806da59b3caf5a9c173cacfc5",
            }],
            decoding_method: "greedy_search",
            num_threads: 1,
            provider: "cpu",
        };
        write_file(dir.path(), "a.bin", b"12345");
        verify_checksums(dir.path(), &manifest).expect("digest matches");

        write_file(dir.path(), "a.bin", b"tampered!");
        let error = verify_checksums(dir.path(), &manifest).expect_err("digest must not match");
        assert_eq!(error.kind, AsrErrorKind::ModelUnsupported);
    }

    #[test]
    fn development_manifest_is_never_production_approved() {
        assert_eq!(DEVELOPMENT_MANIFEST.maturity, AsrMaturity::DevelopmentOnly);
    }

    #[test]
    fn resolve_model_dir_uses_env_override_when_set() {
        crate::test_support::ensure_model_dir_env();
        let app = tauri::test::mock_app();
        let dir = resolve_model_dir(app.handle()).expect("env override resolves");
        assert_eq!(
            dir,
            PathBuf::from(crate::test_support::MODEL_DIR_OVERRIDE)
                .join(DEVELOPMENT_MANIFEST.model_id)
        );
    }

    /// Minimal temp-dir helper so this module needs no extra dev-dependency.
    fn tempfile_dir() -> TempDir {
        let base = std::env::temp_dir().join(format!(
            "mistaken-asr-manifest-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        TempDir(base)
    }

    struct TempDir(PathBuf);
    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
