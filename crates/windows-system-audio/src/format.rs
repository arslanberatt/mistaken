//! Mix-format validation and sample conversion (spec section 6, "Format
//! validation and conversion contract").
//!
//! Pure Rust: parsing the raw `WAVEFORMATEX`/`WAVEFORMATEXTENSIBLE` pointer is
//! the FFI layer's job (`capture.rs`); this module only sees plain fields, so
//! validation and every conversion branch are exercised directly by unit
//! tests with synthetic values, independent of any live WASAPI stream.

use crate::error::{SystemAudioError, SystemAudioErrorKind};

/// The negotiated session format handed to `SystemAudioSink::on_block`: the
/// endpoint's real mix rate, always 1 channel after downmix, and the source
/// encoding branch that was actually taken (evidence for spec AC 8/12; never
/// used to change behavior downstream of this crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemAudioFormat {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub source_encoding: SystemAudioSampleEncoding,
}

/// The one of exactly four convertible sample encodings this crate accepts,
/// exposed publicly only so a probe or log can report which branch a given
/// endpoint's mix format took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemAudioSampleEncoding {
    Float32,
    Pcm16,
    Pcm24,
    Pcm32,
}

pub(crate) type SampleEncoding = SystemAudioSampleEncoding;

/// The subformat discriminated from `WAVEFORMATEXTENSIBLE.SubFormat` by the
/// Windows-only FFI layer before reaching this pure module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubFormatKind {
    /// Plain `WAVEFORMATEX` (`wFormatTag` is not `WAVE_FORMAT_EXTENSIBLE`); no
    /// subformat applies.
    None,
    Pcm,
    IeeeFloat,
    /// `WAVE_FORMAT_EXTENSIBLE` with a subformat GUID this crate does not
    /// recognize.
    Other,
}

/// The raw fields read from the endpoint's `WAVEFORMATEX`/
/// `WAVEFORMATEXTENSIBLE`, with the subformat already discriminated.
/// Deliberately plain data so validation is testable with synthetic values.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RawMixFormat {
    pub format_tag: u16,
    pub channels: u16,
    pub samples_per_sec: u32,
    pub bits_per_sample: u16,
    pub block_align: u16,
    pub valid_bits_per_sample: u16,
    pub sub_format: SubFormatKind,
}

pub(crate) const WAVE_FORMAT_PCM: u16 = 1;
pub(crate) const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
pub(crate) const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// The endpoint mix format after validation: one fixed rate, encoding, and
/// channel count for the session's lifetime (spec: "The session rate is fixed
/// at `Initialize`").
#[derive(Debug, Clone, Copy)]
pub(crate) struct ValidatedFormat {
    pub sample_rate_hz: u32,
    pub source_channels: u16,
    pub encoding: SampleEncoding,
    pub block_align: u16,
}

impl ValidatedFormat {
    pub fn output_format(&self) -> SystemAudioFormat {
        SystemAudioFormat {
            sample_rate_hz: self.sample_rate_hz,
            channels: 1,
            source_encoding: self.encoding,
        }
    }

    /// Bytes per audio frame (all source channels) — used to convert a
    /// packet's frame count into a byte range.
    pub fn frame_size_bytes(&self) -> usize {
        self.block_align as usize
    }
}

fn unsupported(detail: String) -> SystemAudioError {
    SystemAudioError::new(SystemAudioErrorKind::UnsupportedFormat, detail)
}

fn pcm_encoding_for_bits(bits: u16) -> Result<SampleEncoding, SystemAudioError> {
    match bits {
        16 => Ok(SampleEncoding::Pcm16),
        24 => Ok(SampleEncoding::Pcm24),
        32 => Ok(SampleEncoding::Pcm32),
        other => Err(unsupported(format!(
            "unsupported integer PCM bit depth {other}"
        ))),
    }
}

pub(crate) fn bytes_per_sample(encoding: SampleEncoding) -> usize {
    match encoding {
        SampleEncoding::Float32 => 4,
        SampleEncoding::Pcm16 => 2,
        SampleEncoding::Pcm24 => 3,
        SampleEncoding::Pcm32 => 4,
    }
}

/// Validates a raw mix format against the contract: exactly 32-bit float or
/// 16/24/32-bit integer PCM (with `WAVE_FORMAT_EXTENSIBLE` subformat
/// discrimination), 1-8 channels, 8 000-192 000 Hz, and a block-align
/// consistent with channels and bit depth. Anything else — compressed tags,
/// 8-bit PCM, mismatched block alignment, zero rate — is `UnsupportedFormat`.
/// No transmute, no reinterpretation, no guess.
pub(crate) fn validate_raw_format(raw: &RawMixFormat) -> Result<ValidatedFormat, SystemAudioError> {
    let encoding = match (raw.format_tag, raw.sub_format) {
        (WAVE_FORMAT_IEEE_FLOAT, _) => SampleEncoding::Float32,
        (WAVE_FORMAT_PCM, _) => pcm_encoding_for_bits(raw.bits_per_sample)?,
        (WAVE_FORMAT_EXTENSIBLE, SubFormatKind::IeeeFloat) => SampleEncoding::Float32,
        (WAVE_FORMAT_EXTENSIBLE, SubFormatKind::Pcm) => {
            let effective_bits = if raw.valid_bits_per_sample > 0 {
                raw.valid_bits_per_sample
            } else {
                raw.bits_per_sample
            };
            pcm_encoding_for_bits(effective_bits)?
        }
        (tag, sub_format) => {
            return Err(unsupported(format!(
                "unsupported wFormatTag 0x{tag:04X} (subformat {sub_format:?})"
            )));
        }
    };

    if !(1..=8).contains(&raw.channels) {
        return Err(unsupported(format!(
            "unsupported channel count {}",
            raw.channels
        )));
    }
    if !(8_000..=192_000).contains(&raw.samples_per_sec) {
        return Err(unsupported(format!(
            "unsupported sample rate {} Hz",
            raw.samples_per_sec
        )));
    }

    let expected_block_align = bytes_per_sample(encoding) as u32 * raw.channels as u32;
    if raw.block_align as u32 != expected_block_align {
        return Err(unsupported(format!(
            "block align {} inconsistent with {} channels at {} bytes/sample (expected {})",
            raw.block_align,
            raw.channels,
            bytes_per_sample(encoding),
            expected_block_align
        )));
    }

    Ok(ValidatedFormat {
        sample_rate_hz: raw.samples_per_sec,
        source_channels: raw.channels,
        encoding,
        block_align: raw.block_align,
    })
}

fn decode_sample(bytes: &[u8], encoding: SampleEncoding) -> f32 {
    match encoding {
        SampleEncoding::Float32 => f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        SampleEncoding::Pcm16 => i16::from_le_bytes([bytes[0], bytes[1]]) as f32 / 32_768.0,
        SampleEncoding::Pcm24 => {
            // Sign-extend the little-endian 24-bit integer into i32: place the
            // three bytes in the low bytes, then use an arithmetic shift pair
            // to propagate the sign bit from bit 23 up through bit 31.
            let buf = [bytes[0], bytes[1], bytes[2], 0];
            let raw = i32::from_le_bytes(buf) << 8 >> 8;
            raw as f32 / 8_388_608.0
        }
        SampleEncoding::Pcm32 => {
            i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32 / 2_147_483_648.0
        }
    }
}

/// Converts one packet's interleaved bytes to downmixed mono `f32` samples in
/// `[-1.0, 1.0]`, appending one sample per frame to `out`. `frame_count` is
/// trusted from the caller (the packet's reported frame count); `bytes` must
/// contain at least `frame_count * format.frame_size_bytes()` bytes.
///
/// Downmix is the arithmetic mean per frame, clamped to `[-1.0, 1.0]`.
/// Non-finite per-frame results — which a genuinely valid conversion never
/// produces, but a corrupted buffer could — are never forwarded: that frame
/// contributes silence (`0.0`) to `out` instead, so every delivered sample
/// stays finite and the block never shrinks.
pub(crate) fn convert_and_downmix(
    bytes: &[u8],
    frame_count: usize,
    format: &ValidatedFormat,
    out: &mut Vec<f32>,
) {
    let channels = format.source_channels as usize;
    let sample_bytes = bytes_per_sample(format.encoding);
    let frame_bytes = sample_bytes * channels;
    out.reserve(frame_count);
    for frame in 0..frame_count {
        let frame_start = frame * frame_bytes;
        let mut sum = 0.0f64;
        for ch in 0..channels {
            let sample_start = frame_start + ch * sample_bytes;
            let sample = &bytes[sample_start..sample_start + sample_bytes];
            sum += decode_sample(sample, format.encoding) as f64;
        }
        let mean = (sum / channels as f64) as f32;
        let clamped = if mean.is_finite() {
            mean.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        out.push(clamped);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(
        format_tag: u16,
        channels: u16,
        samples_per_sec: u32,
        bits_per_sample: u16,
        sub_format: SubFormatKind,
    ) -> RawMixFormat {
        let block_align = channels * (bits_per_sample / 8);
        RawMixFormat {
            format_tag,
            channels,
            samples_per_sec,
            bits_per_sample,
            block_align,
            valid_bits_per_sample: bits_per_sample,
            sub_format,
        }
    }

    #[test]
    fn plain_float32_format_validates() {
        let format = raw(WAVE_FORMAT_IEEE_FLOAT, 2, 48_000, 32, SubFormatKind::None);
        let validated = validate_raw_format(&format).unwrap();
        assert_eq!(validated.encoding, SampleEncoding::Float32);
        assert_eq!(validated.sample_rate_hz, 48_000);
        assert_eq!(validated.output_format().channels, 1);
    }

    #[test]
    fn extensible_pcm_16_validates() {
        let format = raw(WAVE_FORMAT_EXTENSIBLE, 2, 44_100, 16, SubFormatKind::Pcm);
        let validated = validate_raw_format(&format).unwrap();
        assert_eq!(validated.encoding, SampleEncoding::Pcm16);
    }

    #[test]
    fn extensible_pcm_24_validates() {
        let format = raw(WAVE_FORMAT_EXTENSIBLE, 2, 48_000, 24, SubFormatKind::Pcm);
        let validated = validate_raw_format(&format).unwrap();
        assert_eq!(validated.encoding, SampleEncoding::Pcm24);
    }

    #[test]
    fn extensible_pcm_32_validates() {
        let format = raw(WAVE_FORMAT_EXTENSIBLE, 2, 48_000, 32, SubFormatKind::Pcm);
        let validated = validate_raw_format(&format).unwrap();
        assert_eq!(validated.encoding, SampleEncoding::Pcm32);
    }

    #[test]
    fn extensible_ieee_float_validates() {
        let format = raw(
            WAVE_FORMAT_EXTENSIBLE,
            6,
            48_000,
            32,
            SubFormatKind::IeeeFloat,
        );
        let validated = validate_raw_format(&format).unwrap();
        assert_eq!(validated.encoding, SampleEncoding::Float32);
    }

    #[test]
    fn plain_pcm_16_validates() {
        let format = raw(WAVE_FORMAT_PCM, 1, 16_000, 16, SubFormatKind::None);
        let validated = validate_raw_format(&format).unwrap();
        assert_eq!(validated.encoding, SampleEncoding::Pcm16);
    }

    #[test]
    fn unsupported_bit_depth_is_rejected() {
        let format = raw(WAVE_FORMAT_PCM, 2, 48_000, 8, SubFormatKind::None);
        let err = validate_raw_format(&format).unwrap_err();
        assert_eq!(err.kind, SystemAudioErrorKind::UnsupportedFormat);
    }

    #[test]
    fn compressed_tag_is_rejected() {
        let format = raw(0x0002, 2, 48_000, 16, SubFormatKind::None); // WAVE_FORMAT_ADPCM
        let err = validate_raw_format(&format).unwrap_err();
        assert_eq!(err.kind, SystemAudioErrorKind::UnsupportedFormat);
    }

    #[test]
    fn extensible_with_unrecognized_subformat_is_rejected() {
        let format = raw(WAVE_FORMAT_EXTENSIBLE, 2, 48_000, 16, SubFormatKind::Other);
        let err = validate_raw_format(&format).unwrap_err();
        assert_eq!(err.kind, SystemAudioErrorKind::UnsupportedFormat);
    }

    #[test]
    fn out_of_range_channel_count_is_rejected() {
        let format = raw(WAVE_FORMAT_PCM, 0, 48_000, 16, SubFormatKind::None);
        assert!(validate_raw_format(&format).is_err());
        let format = raw(WAVE_FORMAT_PCM, 9, 48_000, 16, SubFormatKind::None);
        assert!(validate_raw_format(&format).is_err());
    }

    #[test]
    fn out_of_range_sample_rate_is_rejected() {
        let format = raw(WAVE_FORMAT_PCM, 2, 4_000, 16, SubFormatKind::None);
        assert!(validate_raw_format(&format).is_err());
        let format = raw(WAVE_FORMAT_PCM, 2, 200_000, 16, SubFormatKind::None);
        assert!(validate_raw_format(&format).is_err());
    }

    #[test]
    fn zero_sample_rate_is_rejected() {
        let format = raw(WAVE_FORMAT_PCM, 2, 0, 16, SubFormatKind::None);
        assert!(validate_raw_format(&format).is_err());
    }

    #[test]
    fn mismatched_block_align_is_rejected() {
        let mut format = raw(WAVE_FORMAT_PCM, 2, 48_000, 16, SubFormatKind::None);
        format.block_align = 3; // inconsistent with 2 channels * 2 bytes
        let err = validate_raw_format(&format).unwrap_err();
        assert_eq!(err.kind, SystemAudioErrorKind::UnsupportedFormat);
    }

    #[test]
    fn float32_mono_round_trips_without_downmix() {
        let format = validate_raw_format(&raw(
            WAVE_FORMAT_IEEE_FLOAT,
            1,
            48_000,
            32,
            SubFormatKind::None,
        ))
        .unwrap();
        let bytes = 0.5f32.to_le_bytes();
        let mut out = Vec::new();
        convert_and_downmix(&bytes, 1, &format, &mut out);
        assert_eq!(out, vec![0.5]);
    }

    #[test]
    fn float32_stereo_downmixes_to_mean() {
        let format = validate_raw_format(&raw(
            WAVE_FORMAT_IEEE_FLOAT,
            2,
            48_000,
            32,
            SubFormatKind::None,
        ))
        .unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0.2f32.to_le_bytes());
        bytes.extend_from_slice(&0.6f32.to_le_bytes());
        let mut out = Vec::new();
        convert_and_downmix(&bytes, 1, &format, &mut out);
        assert_eq!(out.len(), 1);
        assert!((out[0] - 0.4).abs() < 1e-6);
    }

    #[test]
    fn pcm16_full_scale_values_convert_and_clamp() {
        let format =
            validate_raw_format(&raw(WAVE_FORMAT_PCM, 1, 48_000, 16, SubFormatKind::None)).unwrap();
        let mut out = Vec::new();
        convert_and_downmix(&i16::MAX.to_le_bytes(), 1, &format, &mut out);
        assert!(out[0] > 0.99 && out[0] <= 1.0);
        let mut out = Vec::new();
        convert_and_downmix(&i16::MIN.to_le_bytes(), 1, &format, &mut out);
        assert!((out[0] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn pcm24_sign_extends_negative_values() {
        let format =
            validate_raw_format(&raw(WAVE_FORMAT_PCM, 1, 48_000, 24, SubFormatKind::None)).unwrap();
        // -1 as a 24-bit little-endian signed integer: 0xFFFFFF.
        let bytes = [0xFF, 0xFF, 0xFF];
        let mut out = Vec::new();
        convert_and_downmix(&bytes, 1, &format, &mut out);
        assert!((out[0] - (-1.0 / 8_388_608.0)).abs() < 1e-9);
    }

    #[test]
    fn pcm24_max_positive_value_is_near_one() {
        let format =
            validate_raw_format(&raw(WAVE_FORMAT_PCM, 1, 48_000, 24, SubFormatKind::None)).unwrap();
        let bytes = [0xFF, 0xFF, 0x7F]; // 8_388_607
        let mut out = Vec::new();
        convert_and_downmix(&bytes, 1, &format, &mut out);
        assert!(out[0] > 0.99999 && out[0] <= 1.0);
    }

    #[test]
    fn pcm32_full_scale_values_convert_and_clamp() {
        let format =
            validate_raw_format(&raw(WAVE_FORMAT_PCM, 1, 48_000, 32, SubFormatKind::None)).unwrap();
        let mut out = Vec::new();
        convert_and_downmix(&i32::MIN.to_le_bytes(), 1, &format, &mut out);
        assert!((out[0] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn every_delivered_sample_is_finite_and_in_range() {
        let format = validate_raw_format(&raw(
            WAVE_FORMAT_IEEE_FLOAT,
            1,
            48_000,
            32,
            SubFormatKind::None,
        ))
        .unwrap();
        for raw_bits in [
            f32::NAN.to_le_bytes(),
            f32::INFINITY.to_le_bytes(),
            f32::NEG_INFINITY.to_le_bytes(),
        ] {
            let mut out = Vec::new();
            convert_and_downmix(&raw_bits, 1, &format, &mut out);
            assert_eq!(
                out.len(),
                1,
                "a non-finite frame must still produce exactly one sample"
            );
            assert!(out[0].is_finite());
            assert!((-1.0..=1.0).contains(&out[0]));
        }
    }

    #[test]
    fn multi_frame_conversion_produces_one_sample_per_frame() {
        let format = validate_raw_format(&raw(
            WAVE_FORMAT_IEEE_FLOAT,
            1,
            48_000,
            32,
            SubFormatKind::None,
        ))
        .unwrap();
        let mut bytes = Vec::new();
        for value in [0.1f32, -0.2, 0.3, -0.4] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let mut out = Vec::new();
        convert_and_downmix(&bytes, 4, &format, &mut out);
        assert_eq!(out.len(), 4);
    }
}
