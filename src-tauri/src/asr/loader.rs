//! The model-loading boundary: given an already-resolved model directory,
//! produce a cacheable [`RecognizerFactory`]. Decoupled from `AppHandle`
//! so it stays `dyn`-safe and injectable — production wires
//! [`SherpaModelLoader`]; tests inject a deterministic fake that never
//! touches sherpa-onnx or the filesystem.

use std::path::Path;
use std::sync::Arc;

use super::manifest::{check_presence, verify_checksums, AsrModelManifest};
use super::recognizer::{AsrError, RecognizerFactory};
use super::sherpa_adapter::SherpaRecognizerFactory;

/// Resolves, verifies, and loads the pinned development model exactly
/// once per successful call. Callers (the runtime manager) are
/// responsible for caching the returned factory for the process lifetime
/// so a second Start never re-verifies or re-loads.
pub trait AsrModelLoader: Send + Sync {
    fn load(&self, dir: &Path) -> Result<Arc<dyn RecognizerFactory>, AsrError>;
}

/// Production loader: presence check, full SHA-256 verification, then
/// `OnlineRecognizer::create` through the sherpa-onnx adapter.
pub struct SherpaModelLoader {
    manifest: &'static AsrModelManifest,
}

impl SherpaModelLoader {
    pub fn new(manifest: &'static AsrModelManifest) -> Self {
        Self { manifest }
    }
}

impl AsrModelLoader for SherpaModelLoader {
    fn load(&self, dir: &Path) -> Result<Arc<dyn RecognizerFactory>, AsrError> {
        check_presence(dir, self.manifest)?;
        verify_checksums(dir, self.manifest)?;
        let factory = SherpaRecognizerFactory::load(dir, self.manifest)?;
        Ok(Arc::new(factory))
    }
}
