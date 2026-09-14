//! Screen-recording (TCC) permission inspection and request.
//!
//! `CGPreflightScreenCaptureAccess` never prompts and cannot itself
//! distinguish "denied" from "never asked", so [`permission_status`] only
//! ever reports [`ScreenRecordingPermission::Granted`] or
//! [`ScreenRecordingPermission::Undetermined`]. `CGRequestScreenCaptureAccess`
//! prompts only when the state is genuinely undetermined and otherwise
//! returns the already-determined result immediately, so
//! [`request_permission`] is the only place [`ScreenRecordingPermission::Denied`]
//! is produced. This asymmetry is intentional and documented in
//! `platform-notes-macos.md`; it is not a missing feature.

use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

/// Screen Recording TCC permission state relevant to audio-only capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRecordingPermission {
    Granted,
    Denied,
    Undetermined,
}

/// Non-prompting inspection. Safe to call at any time, including launch.
pub fn permission_status() -> ScreenRecordingPermission {
    // These two Core Graphics calls are plain C functions (no ObjC
    // messaging), and objc2-core-graphics exposes them as safe `fn`s.
    if CGPreflightScreenCaptureAccess() {
        ScreenRecordingPermission::Granted
    } else {
        ScreenRecordingPermission::Undetermined
    }
}

/// Prompting request. Must only be called from an explicit user-initiated
/// start, never at launch or in the background.
pub fn request_permission() -> ScreenRecordingPermission {
    // See the safety note on `permission_status` above.
    if CGRequestScreenCaptureAccess() {
        ScreenRecordingPermission::Granted
    } else {
        ScreenRecordingPermission::Denied
    }
}
