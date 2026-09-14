//! Real microphone enumeration: stable IDs, safe labels, and deterministic
//! ordering. Never prompts for permission and never opens a stream.

use cpal::traits::{DeviceTrait, HostTrait};

use crate::audio::{AudioError, AudioErrorKind, AudioSource};

use super::NativeMicrophoneDevice;

fn fail(kind: AudioErrorKind) -> AudioError {
    AudioError {
        source: AudioSource::Microphone,
        kind,
    }
}

/// Strips control characters and falls back to a safe placeholder for a
/// blank name. Never trims meaningful whitespace out of a real label.
fn sanitize_label(raw: &str) -> String {
    let cleaned: String = raw.chars().filter(|c| !c.is_control()).collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Microphone".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Lists usable input devices: every returned device has an obtainable ID
/// and at least one supported input configuration. Sorted default-first,
/// then case-insensitive label, then ID, so Refresh is deterministic.
pub fn list_devices() -> Result<Vec<NativeMicrophoneDevice>, AudioError> {
    let host = cpal::default_host();
    let default_id = host
        .default_input_device()
        .and_then(|device| device.id().ok());

    let devices = host
        .input_devices()
        .map_err(|_| fail(AudioErrorKind::Unavailable))?;

    let mut result = Vec::new();
    for device in devices {
        let Ok(id) = device.id() else {
            continue;
        };
        if device.default_input_config().is_err() {
            continue;
        }
        let label = device
            .description()
            .map(|description| sanitize_label(description.name()))
            .unwrap_or_else(|_| "Microphone".to_string());
        let is_default = default_id.as_ref() == Some(&id);
        result.push(NativeMicrophoneDevice {
            id,
            label,
            is_default,
        });
    }

    result.sort_by(|a, b| {
        (!a.is_default, a.label.to_lowercase(), a.id.to_string()).cmp(&(
            !b.is_default,
            b.label.to_lowercase(),
            b.id.to_string(),
        ))
    });

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_label_strips_control_characters() {
        assert_eq!(sanitize_label("USB\u{0007} Mic"), "USB Mic");
    }

    #[test]
    fn sanitize_label_falls_back_for_blank_names() {
        assert_eq!(sanitize_label(""), "Microphone");
        assert_eq!(sanitize_label("   "), "Microphone");
        assert_eq!(sanitize_label("\u{0000}\u{0001}"), "Microphone");
    }

    #[test]
    fn sanitize_label_preserves_meaningful_whitespace_free_names() {
        assert_eq!(
            sanitize_label("MacBook Pro Microphone"),
            "MacBook Pro Microphone"
        );
    }

    /// Real enumeration is exercised on physical hardware (see the spec's
    /// developer verification flow); this smoke test only proves the call
    /// does not panic and returns a well-formed (possibly empty) list on
    /// whatever host actually runs the test suite.
    #[test]
    fn list_devices_never_panics_and_returns_unique_non_empty_ids() {
        if let Ok(devices) = list_devices() {
            let mut ids: Vec<String> = devices.iter().map(|d| d.id.to_string()).collect();
            let unique_count = {
                ids.sort();
                ids.dedup();
                ids.len()
            };
            assert_eq!(unique_count, devices.len());
            assert!(devices.iter().all(|d| !d.id.to_string().is_empty()));
            assert!(devices.iter().filter(|d| d.is_default).count() <= 1);
        }
    }
}
