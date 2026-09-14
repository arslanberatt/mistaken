//! Native runtime DTOs and the single managed runtime state.
//!
//! Everything in this module is the serializable half of the typed IPC
//! spine: it mirrors `src/lib/tauri/contracts.ts` field-for-field in
//! camelCase JSON. Before a real audio/model backend exists, every command
//! and status reports truthful "unavailable" state; nothing here fabricates
//! a working device, model, or capture session.

use serde::{Deserialize, Serialize};

/// Lifecycle status of native audio capture. Mirrors `src/types/runtime.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureStatus {
    Idle,
    Starting,
    Listening,
    Stopping,
    Error,
}

/// Structural origin of a transcript segment. Mirrors `src/types/transcript.ts`.
///
/// This is a distinct, serializable DTO from `crate::audio::AudioSource`:
/// the audio module's `AudioSource` never crosses IPC, while this type is
/// exactly the small string union the frontend expects on `RuntimeError`
/// and `audio:status` payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptSource {
    Microphone,
    System,
}

impl From<crate::audio::AudioSource> for TranscriptSource {
    fn from(source: crate::audio::AudioSource) -> Self {
        match source {
            crate::audio::AudioSource::Microphone => TranscriptSource::Microphone,
            crate::audio::AudioSource::System => TranscriptSource::System,
        }
    }
}

/// The closed set of structured runtime failure codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeErrorCode {
    RuntimeUnavailable,
    UnsupportedPlatform,
    InvalidRequest,
    ModelMissing,
    ModelLoadFailed,
    ModelUnsupported,
    MicrophonePermissionDenied,
    SystemAudioPermissionDenied,
    MicrophoneUnavailable,
    SystemAudioUnavailable,
    DeviceDisconnected,
    CaptureAlreadyActive,
    CaptureNotActive,
    CaptureStartFailed,
    CaptureStopFailed,
    AudioQueueOverflow,
    InferenceLagging,
    Internal,
}

/// A structured, serializable runtime failure. Never carries transcript
/// text, PCM, environment variables, filesystem paths, secrets, or a raw
/// native error chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeError {
    pub code: RuntimeErrorCode,
    pub message: String,
    pub recoverable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<TranscriptSource>,
}

impl RuntimeError {
    pub fn new(code: RuntimeErrorCode, message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            recoverable,
            source: None,
        }
    }

    /// Attaches the source whose operation produced this error.
    pub fn with_source(mut self, source: TranscriptSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn runtime_unavailable() -> Self {
        Self::new(
            RuntimeErrorCode::RuntimeUnavailable,
            "the audio/model runtime is not configured yet",
            true,
        )
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorCode::InvalidRequest, message, true)
    }

    pub fn capture_not_active() -> Self {
        Self::new(
            RuntimeErrorCode::CaptureNotActive,
            "capture is not active",
            true,
        )
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorCode::Internal, message, false)
    }
}

/// Local ASR model lifecycle status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
#[serde(rename_all_fields = "camelCase")]
pub enum ModelStatus {
    Missing,
    Loading {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model_id: Option<String>,
    },
    Ready {
        model_id: String,
    },
    Failed {
        error: RuntimeError,
    },
    Unsupported {
        error: RuntimeError,
    },
}

/// Activity of a currently capturing audio source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Activity {
    Waiting,
    Receiving,
}

/// Lifecycle/error status of one audio source (microphone or system audio).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
#[serde(rename_all_fields = "camelCase")]
pub enum AudioSourceStatus {
    Unavailable {
        error: RuntimeError,
    },
    Idle,
    Starting {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device_id: Option<String>,
    },
    Capturing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device_id: Option<String>,
        activity: Activity,
    },
    Stopping {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device_id: Option<String>,
    },
    Error {
        error: RuntimeError,
    },
}

impl AudioSourceStatus {
    fn unavailable() -> Self {
        AudioSourceStatus::Unavailable {
            error: RuntimeError::runtime_unavailable(),
        }
    }
}

/// The complete runtime snapshot returned by `get_runtime_snapshot` and
/// carried by every status event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub revision: u64,
    pub capture_status: CaptureStatus,
    pub model_status: ModelStatus,
    pub microphone: AudioSourceStatus,
    pub system_audio: AudioSourceStatus,
}

/// One enumerated microphone device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneDevice {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

/// Request payload for `start_capture`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartCaptureRequest {
    pub microphone_device_id: Option<String>,
    pub system_audio_enabled: bool,
}

impl StartCaptureRequest {
    /// Validates the request shape. A non-null microphone id must contain
    /// at least one non-whitespace character; requesting neither source is
    /// invalid.
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if let Some(device_id) = &self.microphone_device_id {
            if device_id.trim().is_empty() {
                return Err(RuntimeError::invalid_request(
                    "microphoneDeviceId must contain at least one non-whitespace character",
                ));
            }
        }

        if self.microphone_device_id.is_none() && !self.system_audio_enabled {
            return Err(RuntimeError::invalid_request(
                "start_capture requires a microphone device or system audio to be enabled",
            ));
        }

        Ok(())
    }
}

/// Computes the next monotonic revision, refusing to wrap silently.
fn next_revision(current: u64) -> Result<u64, RuntimeError> {
    current
        .checked_add(1)
        .ok_or_else(|| RuntimeError::internal("runtime revision counter overflowed"))
}

/// The single process-managed runtime state.
///
/// Holds only the revision and small statuses described by this spec: no
/// webview handles, listeners, devices, streams, buffers, workers, model
/// sessions, transcript history, or secrets.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeState {
    revision: u64,
    capture_status: CaptureStatus,
    model_status: ModelStatus,
    microphone: AudioSourceStatus,
    system_audio: AudioSourceStatus,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeState {
    /// The truthful pre-backend state: idle capture, missing model, and
    /// both sources reported unavailable.
    pub fn new() -> Self {
        Self {
            revision: 0,
            capture_status: CaptureStatus::Idle,
            model_status: ModelStatus::Missing,
            microphone: AudioSourceStatus::unavailable(),
            system_audio: AudioSourceStatus::unavailable(),
        }
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            revision: self.revision,
            capture_status: self.capture_status,
            model_status: self.model_status.clone(),
            microphone: self.microphone.clone(),
            system_audio: self.system_audio.clone(),
        }
    }

    /// The current overall capture lifecycle status.
    pub fn capture_status(&self) -> CaptureStatus {
        self.capture_status
    }
    /// Applies a new capture status. This is a successor contract used by
    /// later specs to drive real transitions; nothing in Spec 03 calls it
    /// in production. Every accepted mutation increments the revision
    /// exactly once.
    pub fn apply_capture_status(
        &mut self,
        status: CaptureStatus,
    ) -> Result<RuntimeSnapshot, RuntimeError> {
        let revision = next_revision(self.revision)?;
        self.capture_status = status;
        self.revision = revision;
        Ok(self.snapshot())
    }

    pub fn apply_model_status(
        &mut self,
        status: ModelStatus,
    ) -> Result<RuntimeSnapshot, RuntimeError> {
        let revision = next_revision(self.revision)?;
        self.model_status = status;
        self.revision = revision;
        Ok(self.snapshot())
    }

    pub fn apply_microphone_status(
        &mut self,
        status: AudioSourceStatus,
    ) -> Result<RuntimeSnapshot, RuntimeError> {
        let revision = next_revision(self.revision)?;
        self.microphone = status;
        self.revision = revision;
        Ok(self.snapshot())
    }

    pub fn apply_system_audio_status(
        &mut self,
        status: AudioSourceStatus,
    ) -> Result<RuntimeSnapshot, RuntimeError> {
        let revision = next_revision(self.revision)?;
        self.system_audio = status;
        self.revision = revision;
        Ok(self.snapshot())
    }

    #[cfg(test)]
    fn with_revision(revision: u64) -> Self {
        let mut state = Self::new();
        state.revision = revision;
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_truthfully_unavailable_at_revision_zero() {
        let state = RuntimeState::new();
        let snapshot = state.snapshot();

        assert_eq!(snapshot.revision, 0);
        assert_eq!(snapshot.capture_status, CaptureStatus::Idle);
        assert_eq!(snapshot.model_status, ModelStatus::Missing);
        assert!(matches!(
            snapshot.microphone,
            AudioSourceStatus::Unavailable { .. }
        ));
        assert!(matches!(
            snapshot.system_audio,
            AudioSourceStatus::Unavailable { .. }
        ));
    }

    #[test]
    fn invalid_start_capture_request_rejects_without_touching_state() {
        let blank_device = StartCaptureRequest {
            microphone_device_id: Some("   ".to_string()),
            system_audio_enabled: false,
        };
        let error = blank_device.validate().unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::InvalidRequest);

        let no_source = StartCaptureRequest {
            microphone_device_id: None,
            system_audio_enabled: false,
        };
        let error = no_source.validate().unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::InvalidRequest);
    }

    #[test]
    fn apply_capture_status_increments_revision_exactly_once() {
        let mut state = RuntimeState::new();
        let snapshot = state
            .apply_capture_status(CaptureStatus::Starting)
            .expect("revision has not overflowed");

        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.capture_status, CaptureStatus::Starting);
        assert_eq!(state.snapshot().revision, 1);
    }

    #[test]
    fn each_status_field_bumps_the_shared_revision_counter() {
        let mut state = RuntimeState::new();
        state
            .apply_capture_status(CaptureStatus::Listening)
            .unwrap();
        state
            .apply_model_status(ModelStatus::Ready {
                model_id: "test-model".to_string(),
            })
            .unwrap();
        state
            .apply_microphone_status(AudioSourceStatus::Idle)
            .unwrap();
        let snapshot = state
            .apply_system_audio_status(AudioSourceStatus::Idle)
            .unwrap();

        assert_eq!(snapshot.revision, 4);
    }

    #[test]
    fn revision_overflow_is_a_structured_internal_error_not_a_panic() {
        let mut state = RuntimeState::with_revision(u64::MAX);
        let result = state.apply_capture_status(CaptureStatus::Error);

        let error = result.unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::Internal);
        assert!(!error.recoverable);
        // The revision must not have silently wrapped.
        assert_eq!(state.snapshot().revision, u64::MAX);
    }

    #[test]
    fn runtime_error_camel_case_round_trips_with_optional_source() {
        let error = RuntimeError::runtime_unavailable().with_source(TranscriptSource::System);
        let json = serde_json::to_value(&error).unwrap();

        assert_eq!(json["code"], "runtime_unavailable");
        assert_eq!(json["recoverable"], true);
        assert_eq!(json["source"], "system");

        let round_tripped: RuntimeError = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, error);
    }

    #[test]
    fn runtime_error_without_source_omits_the_field() {
        let error = RuntimeError::capture_not_active();
        let json = serde_json::to_value(&error).unwrap();

        assert!(json.get("source").is_none());
    }

    #[test]
    fn model_status_tagged_variants_use_camel_case_field_names() {
        let loading = ModelStatus::Loading {
            model_id: Some("candidate".to_string()),
        };
        let json = serde_json::to_value(&loading).unwrap();
        assert_eq!(json["status"], "loading");
        assert_eq!(json["modelId"], "candidate");

        let missing_json = serde_json::to_value(&ModelStatus::Missing).unwrap();
        assert_eq!(missing_json["status"], "missing");
        assert!(missing_json.get("modelId").is_none());
    }

    #[test]
    fn audio_source_status_capturing_uses_camel_case_device_id() {
        let capturing = AudioSourceStatus::Capturing {
            device_id: Some("mic-1".to_string()),
            activity: Activity::Receiving,
        };
        let json = serde_json::to_value(&capturing).unwrap();

        assert_eq!(json["status"], "capturing");
        assert_eq!(json["deviceId"], "mic-1");
        assert_eq!(json["activity"], "receiving");
    }

    #[test]
    fn runtime_snapshot_serializes_every_field_in_camel_case() {
        let state = RuntimeState::new();
        let json = serde_json::to_value(state.snapshot()).unwrap();

        for key in [
            "revision",
            "captureStatus",
            "modelStatus",
            "microphone",
            "systemAudio",
        ] {
            assert!(json.get(key).is_some(), "missing key {key}");
        }
    }

    #[test]
    fn transcript_source_serializes_as_lowercase_strings() {
        assert_eq!(
            serde_json::to_value(TranscriptSource::Microphone).unwrap(),
            "microphone"
        );
        assert_eq!(
            serde_json::to_value(TranscriptSource::System).unwrap(),
            "system"
        );
    }

    #[test]
    fn audio_source_maps_onto_transcript_source() {
        assert_eq!(
            TranscriptSource::from(crate::audio::AudioSource::Microphone),
            TranscriptSource::Microphone
        );
        assert_eq!(
            TranscriptSource::from(crate::audio::AudioSource::System),
            TranscriptSource::System
        );
    }
}
