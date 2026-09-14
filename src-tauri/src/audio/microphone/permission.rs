//! macOS microphone authorization via `AVCaptureDevice`.
//!
//! Queried/requested only from an explicit user action (Test microphone),
//! never at launch and never during device enumeration. `request_access`
//! blocks the calling thread until the user responds (or immediately, if
//! the OS answers without a prompt), so callers must run it off the async
//! runtime — see `MicrophoneBackend::start`, which is always invoked via
//! `tauri::async_runtime::spawn_blocking`.

use std::sync::mpsc;

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

/// The three terminal authorization outcomes. `NotDetermined` is
/// deliberately not a variant here: callers observe it as `None` and must
/// call [`request_access`] to resolve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophoneAuthorization {
    Authorized,
    Denied,
    Restricted,
}

fn media_type_audio() -> &'static objc2_av_foundation::AVMediaType {
    // SAFETY: this is a well-known Apple framework constant that is always
    // present once AVFoundation is linked; only a broken system installation
    // would make it null, which is not a condition this app can recover
    // from.
    unsafe { AVMediaTypeAudio }.expect("AVMediaTypeAudio constant is unavailable")
}

/// Returns the current authorization status without prompting. `None`
/// means "not determined yet": the caller decides whether to request access.
pub fn authorization_status() -> Option<MicrophoneAuthorization> {
    // SAFETY: `authorizationStatusForMediaType:` is a read-only query with
    // no side effects; passing the audio media type constant matches
    // Apple's documented contract.
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type_audio()) };
    match status {
        AVAuthorizationStatus::Authorized => Some(MicrophoneAuthorization::Authorized),
        AVAuthorizationStatus::Denied => Some(MicrophoneAuthorization::Denied),
        AVAuthorizationStatus::Restricted => Some(MicrophoneAuthorization::Restricted),
        _ => None,
    }
}

/// Requests access, showing the OS permission dialog if the status is not
/// determined. Blocks the calling thread until the completion handler runs
/// on Apple's arbitrary dispatch queue.
pub fn request_access() -> MicrophoneAuthorization {
    let (tx, rx) = mpsc::channel::<bool>();
    let handler = RcBlock::new(move |granted: Bool| {
        let _ = tx.send(granted.as_bool());
    });

    // SAFETY: the handler matches the required `Fn(Bool)` block signature,
    // and the media type constant matches Apple's documented contract.
    unsafe {
        AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type_audio(), &handler);
    }

    match rx.recv() {
        Ok(true) => MicrophoneAuthorization::Authorized,
        _ => MicrophoneAuthorization::Denied,
    }
}
