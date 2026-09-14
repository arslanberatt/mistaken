//! Frozen stream configuration and macOS-version gate.

use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_core_media::CMTime;
use objc2_foundation::{NSOperatingSystemVersion, NSProcessInfo};
use objc2_screen_capture_kit::{SCContentFilter, SCDisplay, SCStreamConfiguration};

use crate::error::{SystemAudioError, SystemAudioErrorKind};

/// Minimum macOS version this adapter supports. Every `SCStreamConfiguration`
/// audio property (`capturesAudio`, `sampleRate`, `channelCount`,
/// `excludesCurrentProcessAudio`) is macOS 13.0+; below that the audio half
/// of ScreenCaptureKit cannot work at all, so this crate refuses to start
/// rather than silently producing a stream with no audio.
pub const MINIMUM_MACOS_VERSION: NSOperatingSystemVersion = NSOperatingSystemVersion {
    majorVersion: 13,
    minorVersion: 0,
    patchVersion: 0,
};

/// The sample rates ScreenCaptureKit documents as supported for audio
/// capture. Any other value falls back to 48 kHz inside the framework.
pub const SUPPORTED_SAMPLE_RATES: [u32; 4] = [8_000, 16_000, 24_000, 48_000];

/// Adapter start configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemAudioConfig {
    /// One of 8000, 16000, 24000, 48000. Default 48000.
    pub sample_rate_hz: u32,
    /// Always true in Mistaken. Exposed so the value is explicit, not
    /// implied by a hardcoded constant deep in the stream module.
    pub exclude_current_process_audio: bool,
}

impl Default for SystemAudioConfig {
    fn default() -> Self {
        Self {
            sample_rate_hz: 48_000,
            exclude_current_process_audio: true,
        }
    }
}

/// Requested capture channel count. Frozen at stereo so the downstream
/// downmix stage always sees the same negotiated shape; the delivered
/// format is still validated at runtime rather than assumed.
pub const REQUESTED_CHANNEL_COUNT: u16 = 2;

/// Returns `Ok(())` when the running macOS version is at least
/// [`MINIMUM_MACOS_VERSION`], otherwise a [`SystemAudioErrorKind::Unsupported`]
/// error naming the running and required versions.
pub fn check_macos_version() -> Result<(), SystemAudioError> {
    let process_info = NSProcessInfo::processInfo();
    if process_info.isOperatingSystemAtLeastVersion(MINIMUM_MACOS_VERSION) {
        Ok(())
    } else {
        let running = process_info.operatingSystemVersion();
        Err(SystemAudioError::new(
            SystemAudioErrorKind::Unsupported,
            format!(
                "running macOS {}.{}.{} is older than the required {}.{}.{}",
                running.majorVersion,
                running.minorVersion,
                running.patchVersion,
                MINIMUM_MACOS_VERSION.majorVersion,
                MINIMUM_MACOS_VERSION.minorVersion,
                MINIMUM_MACOS_VERSION.patchVersion,
            ),
        ))
    }
}

/// Builds the frozen `SCStreamConfiguration` for audio-only capture. The
/// nominal video fields (`width`/`height`/`minimumFrameInterval`/`queueDepth`)
/// exist only because `SCStream` requires a configuration object; no video
/// output is ever registered, so they are never consumed.
pub fn build_stream_configuration(config: &SystemAudioConfig) -> Retained<SCStreamConfiguration> {
    // SAFETY: `new` is a plain `NSObject`-style initializer with no extra
    // preconditions.
    let stream_config = unsafe { SCStreamConfiguration::new() };
    // SAFETY: `stream_config` is freshly allocated and owned exclusively
    // here; these setters have no documented preconditions beyond a live
    // receiver.
    unsafe {
        stream_config.setCapturesAudio(true);
        stream_config.setSampleRate(config.sample_rate_hz as isize);
        stream_config.setChannelCount(REQUESTED_CHANNEL_COUNT as isize);
        stream_config.setExcludesCurrentProcessAudio(config.exclude_current_process_audio);
        stream_config.setWidth(2);
        stream_config.setHeight(2);
        stream_config.setMinimumFrameInterval(CMTime::new(1, 1));
        stream_config.setQueueDepth(3);
    }
    stream_config
}

/// Builds a content filter over the primary display excluding no windows.
/// This filter is never used to read pixels; it exists only because
/// `SCStream` requires one to be constructed.
pub fn build_content_filter(display: &SCDisplay) -> Retained<SCContentFilter> {
    let excluded = objc2_foundation::NSArray::new();
    // SAFETY: `SCContentFilter::alloc()` is a fresh, uninitialized
    // allocation consumed exactly once by this initializer; `display` and
    // `excluded` are valid live references for the duration of the call.
    unsafe {
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            display,
            &excluded,
        )
    }
}
