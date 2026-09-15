//! The four typed runtime commands.
//!
//! Every handler validates its own input (if any) and delegates to
//! [`RuntimeManager`], which owns the actual state machine, native
//! audio lifecycle, and event emission. No command performs audio or
//! ASR work directly.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::state::runtime::{
    CaptureStatus, MicrophoneDevice, RuntimeError, RuntimeSnapshot, StartCaptureRequest,
};
use crate::state::RuntimeManager;

#[tauri::command]
pub fn get_runtime_snapshot(
    manager: State<'_, Arc<RuntimeManager>>,
) -> Result<RuntimeSnapshot, RuntimeError> {
    manager.snapshot()
}

#[tauri::command]
pub fn list_microphones(
    manager: State<'_, Arc<RuntimeManager>>,
    app: AppHandle,
) -> Result<Vec<MicrophoneDevice>, RuntimeError> {
    manager.list_microphones(&app)
}

#[tauri::command]
pub async fn start_capture(
    request: StartCaptureRequest,
    manager: State<'_, Arc<RuntimeManager>>,
    app: AppHandle,
) -> Result<CaptureStatus, RuntimeError> {
    request.validate()?;

    let manager = manager.inner().clone();
    manager
        .start_capture(
            app,
            request.microphone_device_id,
            request.system_audio_enabled,
        )
        .await
}

#[tauri::command]
pub async fn stop_capture(
    manager: State<'_, Arc<RuntimeManager>>,
    app: AppHandle,
) -> Result<CaptureStatus, RuntimeError> {
    let manager = manager.inner().clone();
    manager.stop_capture(app).await
}
