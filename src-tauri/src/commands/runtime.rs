//! The four typed runtime commands.
//!
//! Every handler validates its own input (if any), delegates the actual
//! decision to `RuntimeState`, releases the lock, and only then emits. No
//! Tauri emission, await, audio operation, or ASR operation happens while
//! the state mutex is held.

use std::sync::{Mutex, MutexGuard};

use tauri::{AppHandle, State};

use crate::events;
use crate::state::runtime::{
    CaptureStatus, MicrophoneDevice, RuntimeError, RuntimeSnapshot, RuntimeState,
    StartCaptureRequest,
};

fn lock<'a>(
    state: &'a State<'_, Mutex<RuntimeState>>,
) -> Result<MutexGuard<'a, RuntimeState>, RuntimeError> {
    state
        .lock()
        .map_err(|_| RuntimeError::internal("runtime state mutex was poisoned"))
}

/// Emits the given operational error to `capture:error` without letting a
/// broken emission channel override the error already being returned to
/// the command's own caller.
fn report_capture_error(app: &AppHandle, error: &RuntimeError) {
    let _ = events::emit_capture_error(app, error);
}

#[tauri::command]
pub fn get_runtime_snapshot(
    state: State<'_, Mutex<RuntimeState>>,
) -> Result<RuntimeSnapshot, RuntimeError> {
    let guard = lock(&state)?;
    Ok(guard.snapshot())
}

#[tauri::command]
pub fn list_microphones(
    state: State<'_, Mutex<RuntimeState>>,
) -> Result<Vec<MicrophoneDevice>, RuntimeError> {
    let guard = lock(&state)?;
    guard.list_microphones()
}

#[tauri::command]
pub fn start_capture(
    request: StartCaptureRequest,
    state: State<'_, Mutex<RuntimeState>>,
    app: AppHandle,
) -> Result<CaptureStatus, RuntimeError> {
    request.validate()?;

    let mut guard = lock(&state)?;
    let result = guard.begin_start_capture(&request);
    drop(guard);

    if let Err(error) = &result {
        report_capture_error(&app, error);
    }

    result
}

#[tauri::command]
pub fn stop_capture(
    state: State<'_, Mutex<RuntimeState>>,
    app: AppHandle,
) -> Result<CaptureStatus, RuntimeError> {
    let mut guard = lock(&state)?;
    let result = guard.begin_stop_capture();
    drop(guard);

    if let Err(error) = &result {
        report_capture_error(&app, error);
    }

    result
}
