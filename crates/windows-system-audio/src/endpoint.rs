//! Default render endpoint resolution and once-per-second change detection
//! (spec section 6, "No `IMMNotificationClient`" decision).
//!
//! Switching the default output device while the old device still exists does
//! **not** invalidate the still-open audio client, so capture would continue
//! on a device nobody hears unless something polls for the change. This
//! module compares `IMMDevice::GetId` against a fresh
//! `GetDefaultAudioEndpoint(eRender, eConsole)` lookup; the caller
//! (`capture.rs`) is responsible for calling it at most once per second from
//! the adapter's own thread.

#![cfg(windows)]

use windows::Win32::Media::Audio::{eConsole, eRender, IMMDevice, IMMDeviceEnumerator};
use windows::Win32::System::Com::CoTaskMemFree;

use crate::error::{SystemAudioError, SystemAudioErrorKind};

/// Resolves the current default render endpoint (`eRender`, `eConsole`: the
/// output the user actually hears). Failure to find one is reported as
/// `NoRenderEndpoint`, never fabricated as a synthetic endpoint.
pub(crate) fn resolve_default_render_endpoint(
    enumerator: &IMMDeviceEnumerator,
) -> Result<IMMDevice, SystemAudioError> {
    unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }.map_err(|err| {
        SystemAudioError::new(
            SystemAudioErrorKind::NoRenderEndpoint,
            format!(
                "GetDefaultAudioEndpoint(eRender, eConsole): HRESULT 0x{:08X} ({})",
                err.code().0 as u32,
                err.message()
            ),
        )
    })
}

/// Reads and owns a copy of the endpoint's string id, freeing the COM
/// task-allocated buffer `IMMDevice::GetId` returns.
pub(crate) fn endpoint_id(device: &IMMDevice) -> Result<String, SystemAudioError> {
    unsafe {
        let pwstr = device.GetId().map_err(|err| {
            SystemAudioError::new(
                SystemAudioErrorKind::Internal,
                format!(
                    "IMMDevice::GetId: HRESULT 0x{:08X} ({})",
                    err.code().0 as u32,
                    err.message()
                ),
            )
        })?;
        let id = pwstr.to_string().unwrap_or_default();
        CoTaskMemFree(Some(pwstr.0 as *const _));
        Ok(id)
    }
}

/// Compares the current default render endpoint id against `current_id`.
/// Returns `Ok(true)` if the default endpoint changed to a different device.
pub(crate) fn has_default_endpoint_changed(
    enumerator: &IMMDeviceEnumerator,
    current_id: &str,
) -> Result<bool, SystemAudioError> {
    let device = resolve_default_render_endpoint(enumerator)?;
    let id = endpoint_id(&device)?;
    Ok(id != current_id)
}
