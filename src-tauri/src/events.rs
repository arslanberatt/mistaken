//! Native event names, payload shapes, and the main-webview emit helper.
//!
//! Rust emits only to the window labeled `main` via [`tauri::Emitter::emit_to`].
//! Nothing here ever broadcasts globally or grants the frontend an emit
//! permission; see `src-tauri/capabilities/main.json`.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::state::runtime::{RuntimeError, RuntimeSnapshot, TranscriptSource};

/// The exact six native event names. These must match
/// `src/lib/tauri/contracts.ts`'s `NATIVE_EVENTS` byte-for-byte.
pub const CAPTURE_STATUS_EVENT: &str = "capture:status";
pub const AUDIO_STATUS_EVENT: &str = "audio:status";
pub const MODEL_STATUS_EVENT: &str = "asr:model-status";
pub const TRANSCRIPT_PARTIAL_EVENT: &str = "transcript:partial";
pub const TRANSCRIPT_FINAL_EVENT: &str = "transcript:final";
pub const CAPTURE_ERROR_EVENT: &str = "capture:error";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatusEvent {
    pub snapshot: RuntimeSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatusEvent {
    pub source: TranscriptSource,
    pub snapshot: RuntimeSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatusEvent {
    pub snapshot: RuntimeSnapshot,
}

/// Emits a `capture:error` payload to the `main` window only.
///
/// The state mutex must already be released before calling this: no Tauri
/// emission happens while the runtime lock is held. A failed emission maps
/// to a structured internal error instead of panicking; callers decide
/// whether that failure should replace or accompany the error already being
/// reported to the command's own caller.
pub fn emit_capture_error<R: tauri::Runtime>(
    app: &AppHandle<R>,
    error: &RuntimeError,
) -> Result<(), RuntimeError> {
    app.emit_to("main", CAPTURE_ERROR_EVENT, error)
        .map_err(|_| RuntimeError::internal("failed to emit capture:error to the main window"))
}

/// Emits a `capture:status` full-snapshot payload to the `main` window.
/// Callers apply the state transition, release the state lock, and only
/// then call this — no Tauri emission happens while the runtime lock is
/// held.
pub fn emit_capture_status<R: tauri::Runtime>(
    app: &AppHandle<R>,
    snapshot: RuntimeSnapshot,
) -> Result<(), RuntimeError> {
    app.emit_to(
        "main",
        CAPTURE_STATUS_EVENT,
        CaptureStatusEvent { snapshot },
    )
    .map_err(|_| RuntimeError::internal("failed to emit capture:status to the main window"))
}

/// Emits an `audio:status` full-snapshot payload, tagged with the source
/// whose status changed, to the `main` window.
pub fn emit_audio_status<R: tauri::Runtime>(
    app: &AppHandle<R>,
    source: TranscriptSource,
    snapshot: RuntimeSnapshot,
) -> Result<(), RuntimeError> {
    app.emit_to(
        "main",
        AUDIO_STATUS_EVENT,
        AudioStatusEvent { source, snapshot },
    )
    .map_err(|_| RuntimeError::internal("failed to emit audio:status to the main window"))
}

/// Emits an `asr:model-status` full-snapshot payload to the `main` window.
pub fn emit_model_status<R: tauri::Runtime>(
    app: &AppHandle<R>,
    snapshot: RuntimeSnapshot,
) -> Result<(), RuntimeError> {
    app.emit_to("main", MODEL_STATUS_EVENT, ModelStatusEvent { snapshot })
        .map_err(|_| RuntimeError::internal("failed to emit asr:model-status to the main window"))
}

/// Emits a `transcript:partial` payload — the segment itself, not wrapped
/// — to the `main` window.
pub fn emit_transcript_partial<R: tauri::Runtime>(
    app: &AppHandle<R>,
    segment: &crate::state::runtime::TranscriptSegment,
) -> Result<(), RuntimeError> {
    app.emit_to("main", TRANSCRIPT_PARTIAL_EVENT, segment)
        .map_err(|_| RuntimeError::internal("failed to emit transcript:partial to the main window"))
}

/// Emits a `transcript:final` payload — the segment itself, not wrapped —
/// to the `main` window.
pub fn emit_transcript_final<R: tauri::Runtime>(
    app: &AppHandle<R>,
    segment: &crate::state::runtime::TranscriptSegment,
) -> Result<(), RuntimeError> {
    app.emit_to("main", TRANSCRIPT_FINAL_EVENT, segment)
        .map_err(|_| RuntimeError::internal("failed to emit transcript:final to the main window"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names_match_the_frozen_contract() {
        assert_eq!(CAPTURE_STATUS_EVENT, "capture:status");
        assert_eq!(AUDIO_STATUS_EVENT, "audio:status");
        assert_eq!(MODEL_STATUS_EVENT, "asr:model-status");
        assert_eq!(TRANSCRIPT_PARTIAL_EVENT, "transcript:partial");
        assert_eq!(TRANSCRIPT_FINAL_EVENT, "transcript:final");
        assert_eq!(CAPTURE_ERROR_EVENT, "capture:error");
    }

    #[test]
    fn audio_status_event_serializes_source_and_snapshot_in_camel_case() {
        let event = AudioStatusEvent {
            source: TranscriptSource::System,
            snapshot: crate::state::RuntimeState::new().snapshot(),
        };
        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["source"], "system");
        assert!(json.get("snapshot").is_some());
    }
}
