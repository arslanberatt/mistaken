//! Runtime validation of the audio format actually delivered by
//! ScreenCaptureKit, read from each sample buffer's format description
//! rather than assumed from the requested configuration.

use objc2_core_audio_types::{
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved, kAudioFormatFlagIsPacked,
    kAudioFormatLinearPCM,
};
use objc2_core_media::{
    kCMMediaType_Audio, CMAudioFormatDescriptionGetStreamBasicDescription, CMFormatDescription,
};

use crate::config::SUPPORTED_SAMPLE_RATES;
use crate::error::{SystemAudioError, SystemAudioErrorKind};

/// The generic shape of one delivered audio sample buffer, extracted from
/// its format description and validated against the constraints this
/// adapter can actually process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedFormat {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub non_interleaved: bool,
}

fn unsupported(detail: impl Into<String>) -> SystemAudioError {
    SystemAudioError::new(SystemAudioErrorKind::UnsupportedFormat, detail)
}

/// Validates a `CMFormatDescription` against the adapter's audio contract:
/// linear PCM, 32-bit float, packed, a rate from the framework's documented
/// supported set, and 1-8 channels. Any other shape is rejected outright;
/// nothing is transmuted or guessed.
pub fn validate_format_description(
    desc: &CMFormatDescription,
) -> Result<ValidatedFormat, SystemAudioError> {
    // SAFETY: `desc` is a valid, live `CMFormatDescription` reference for
    // the duration of this call.
    let media_type = unsafe { desc.media_type() };
    if media_type != kCMMediaType_Audio {
        return Err(unsupported("format description is not audio media"));
    }

    // SAFETY: `desc` is a valid, live `CMFormatDescription` for the
    // lifetime of this call, matching the media-type check above.
    let asbd_ptr = unsafe { CMAudioFormatDescriptionGetStreamBasicDescription(desc) };
    // SAFETY: a null return from `CMAudioFormatDescriptionGetStreamBasicDescription`
    // is handled by the `Option` below without dereferencing; a non-null
    // return is documented to point at data owned by `desc`, which is
    // still alive here.
    let Some(asbd) = (unsafe { asbd_ptr.as_ref() }) else {
        return Err(unsupported(
            "audio format description has no stream basic description",
        ));
    };

    if asbd.mFormatID != kAudioFormatLinearPCM {
        return Err(unsupported(format!(
            "unsupported mFormatID {:#010x}",
            asbd.mFormatID
        )));
    }
    if asbd.mFormatFlags & kAudioFormatFlagIsFloat == 0 {
        return Err(unsupported("audio is not float32 PCM"));
    }
    if asbd.mFormatFlags & kAudioFormatFlagIsPacked == 0 {
        return Err(unsupported("audio is not packed"));
    }
    if asbd.mBitsPerChannel != 32 {
        return Err(unsupported(format!(
            "unsupported mBitsPerChannel {}",
            asbd.mBitsPerChannel
        )));
    }

    let sample_rate_hz = asbd.mSampleRate.round() as u32;
    if sample_rate_hz == 0 || !SUPPORTED_SAMPLE_RATES.contains(&sample_rate_hz) {
        return Err(unsupported(format!(
            "unsupported sample rate {sample_rate_hz}"
        )));
    }

    let channels = asbd.mChannelsPerFrame;
    if !(1..=8).contains(&channels) {
        return Err(unsupported(format!("unsupported channel count {channels}")));
    }

    let non_interleaved = asbd.mFormatFlags & kAudioFormatFlagIsNonInterleaved != 0;

    Ok(ValidatedFormat {
        sample_rate_hz,
        channels: channels as u16,
        non_interleaved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr::NonNull;
    use objc2_core_audio_types::{kAudioFormatFlagIsSignedInteger, AudioStreamBasicDescription};
    use objc2_core_foundation::CFRetained;
    use objc2_core_media::CMAudioFormatDescriptionCreate;

    /// Builds a real `CMFormatDescription` from a synthetic ASBD so
    /// `validate_format_description` is exercised against the exact same
    /// C struct ScreenCaptureKit would deliver, without needing a live
    /// stream or a real `CMSampleBuffer`.
    fn make_format_description(
        asbd: AudioStreamBasicDescription,
    ) -> CFRetained<CMFormatDescription> {
        let mut out: *const CMFormatDescription = core::ptr::null();
        // SAFETY: `asbd` is a valid live local; `format_description_out`
        // is a valid pointer to a local; `layout`/`magic_cookie` are
        // null, which the function documents as accepted.
        let status = unsafe {
            CMAudioFormatDescriptionCreate(
                None,
                NonNull::from(&asbd),
                0,
                core::ptr::null(),
                0,
                core::ptr::null(),
                None,
                NonNull::from(&mut out),
            )
        };
        assert_eq!(
            status, 0,
            "CMAudioFormatDescriptionCreate failed with OSStatus {status}"
        );
        let non_null = NonNull::new(out as *mut CMFormatDescription).expect("non-null on success");
        // SAFETY: `CMAudioFormatDescriptionCreate` returned success (0)
        // with a `+1` retained format description per its documented
        // ownership contract.
        unsafe { CFRetained::from_raw(non_null) }
    }

    fn base_asbd() -> AudioStreamBasicDescription {
        AudioStreamBasicDescription {
            mSampleRate: 48_000.0,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
            mBytesPerPacket: 8,
            mFramesPerPacket: 1,
            mBytesPerFrame: 8,
            mChannelsPerFrame: 2,
            mBitsPerChannel: 32,
            mReserved: 0,
        }
    }

    #[test]
    fn accepts_valid_stereo_float32_48k() {
        let desc = make_format_description(base_asbd());
        let validated = validate_format_description(&desc).expect("valid format must validate");
        assert_eq!(validated.sample_rate_hz, 48_000);
        assert_eq!(validated.channels, 2);
        assert!(!validated.non_interleaved);
    }

    #[test]
    fn accepts_every_documented_sample_rate() {
        for &rate in &SUPPORTED_SAMPLE_RATES {
            let mut asbd = base_asbd();
            asbd.mSampleRate = rate as f64;
            let desc = make_format_description(asbd);
            let validated =
                validate_format_description(&desc).expect("documented rate must validate");
            assert_eq!(validated.sample_rate_hz, rate);
        }
    }

    #[test]
    fn rejects_undocumented_sample_rate() {
        let mut asbd = base_asbd();
        asbd.mSampleRate = 44_100.0;
        let desc = make_format_description(asbd);
        let err = validate_format_description(&desc).unwrap_err();
        assert_eq!(err.kind, SystemAudioErrorKind::UnsupportedFormat);
    }

    #[test]
    fn rejects_integer_pcm() {
        let mut asbd = base_asbd();
        asbd.mFormatFlags = kAudioFormatFlagIsPacked | kAudioFormatFlagIsSignedInteger;
        asbd.mBitsPerChannel = 16;
        asbd.mBytesPerFrame = 4;
        asbd.mBytesPerPacket = 4;
        let desc = make_format_description(asbd);
        let err = validate_format_description(&desc).unwrap_err();
        assert_eq!(err.kind, SystemAudioErrorKind::UnsupportedFormat);
    }

    #[test]
    fn detects_non_interleaved_layout() {
        let mut asbd = base_asbd();
        asbd.mFormatFlags |= kAudioFormatFlagIsNonInterleaved;
        asbd.mBytesPerFrame = 4; // per-channel stride once non-interleaved
        asbd.mBytesPerPacket = 4;
        let desc = make_format_description(asbd);
        let validated = validate_format_description(&desc)
            .expect("non-interleaved float32 must still validate");
        assert!(validated.non_interleaved);
    }

    #[test]
    fn rejects_channel_count_out_of_range() {
        let mut asbd = base_asbd();
        asbd.mChannelsPerFrame = 0;
        asbd.mBytesPerFrame = 0;
        asbd.mBytesPerPacket = 0;
        let desc = make_format_description(asbd);
        assert_eq!(
            validate_format_description(&desc).unwrap_err().kind,
            SystemAudioErrorKind::UnsupportedFormat
        );
    }
}
