//! Sample-format conversion and channel downmix.
//!
//! Every CPAL sample type is converted to `f32` through `dasp_sample`'s
//! range-correct conversion (the same crate CPAL itself re-exports as
//! [`cpal::FromSample`]), then downmixed to mono by arithmetic mean using an
//! `f64` accumulator, then clamped to `[-1.0, 1.0]`. A non-finite result at
//! any stage rejects the whole frame rather than writing garbage into a
//! reusable buffer.

use cpal::{FromSample, Sample};

/// Downmixes one interleaved frame (one sample per channel) to a single
/// clamped, finite `f32`. Returns `None` if the conversion ever produces a
/// non-finite value; the caller must then discard the in-progress block
/// rather than submit a partially-valid one.
pub fn downmix_frame<T>(frame: &[T]) -> Option<f32>
where
    T: Sample + Copy,
    f32: FromSample<T>,
{
    if frame.is_empty() {
        return Some(0.0);
    }

    let mut sum = 0.0_f64;
    for &sample in frame {
        let as_f32: f32 = f32::from_sample(sample);
        if !as_f32.is_finite() {
            return None;
        }
        sum += f64::from(as_f32);
    }

    let mean = (sum / frame.len() as f64) as f32;
    if !mean.is_finite() {
        return None;
    }
    Some(mean.clamp(-1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_frame_downmixes_by_arithmetic_mean() {
        assert_eq!(downmix_frame::<f32>(&[1.0, -1.0]), Some(0.0));
        assert_eq!(downmix_frame::<f32>(&[0.5, 0.5]), Some(0.5));
    }

    #[test]
    fn mono_frame_passes_through_unchanged() {
        assert_eq!(downmix_frame::<f32>(&[0.25]), Some(0.25));
    }

    #[test]
    fn i16_full_scale_maps_near_plus_minus_one() {
        // dasp_sample's conversion is symmetric around the negative extreme
        // (i16::MIN maps to exactly -1.0), so the positive extreme lands
        // just under +1.0 rather than exactly on it.
        let positive = downmix_frame::<i16>(&[i16::MAX]).unwrap();
        assert!((0.999..=1.0).contains(&positive));
        assert_eq!(downmix_frame::<i16>(&[i16::MIN]), Some(-1.0));
        assert_eq!(downmix_frame::<i16>(&[0]), Some(0.0));
    }

    #[test]
    fn u8_full_scale_maps_near_plus_minus_one() {
        let positive = downmix_frame::<u8>(&[u8::MAX]).unwrap();
        assert!((0.98..=1.0).contains(&positive));
        assert_eq!(downmix_frame::<u8>(&[0]), Some(-1.0));
    }

    #[test]
    fn i32_and_i64_convert_without_panicking() {
        assert_eq!(downmix_frame::<i32>(&[0]), Some(0.0));
        assert!(downmix_frame::<i32>(&[i32::MAX]).unwrap() > 0.99);
        assert_eq!(downmix_frame::<i64>(&[0]), Some(0.0));
        assert!(downmix_frame::<i64>(&[i64::MAX]).unwrap() > 0.99);
    }

    #[test]
    fn u16_u32_u64_convert_without_panicking() {
        assert_eq!(downmix_frame::<u16>(&[1 << 15]), Some(0.0));
        assert_eq!(downmix_frame::<u32>(&[1_u32 << 31]), Some(0.0));
        assert_eq!(downmix_frame::<u64>(&[1_u64 << 63]), Some(0.0));
    }

    #[test]
    fn f64_frame_converts_and_clamps() {
        assert_eq!(downmix_frame::<f64>(&[0.5, -0.5]), Some(0.0));
    }

    #[test]
    fn twenty_four_bit_types_convert_without_panicking() {
        let value = cpal::I24::new(1 << 22).expect("valid I24 magnitude");
        let result = downmix_frame::<cpal::I24>(&[value]).expect("finite conversion");
        assert!(result > 0.0 && result < 1.0);

        let uvalue = cpal::U24::new(1 << 23).expect("valid U24 magnitude");
        let uresult = downmix_frame::<cpal::U24>(&[uvalue]).expect("finite conversion");
        assert!((uresult - 0.0).abs() < 0.01);
    }

    #[test]
    fn stereo_frame_averages_both_channels() {
        assert_eq!(downmix_frame::<f32>(&[1.0, 0.0]), Some(0.5));
    }

    #[test]
    fn output_never_exceeds_valid_range() {
        for &value in &[i16::MIN, 0, i16::MAX] {
            let mixed = downmix_frame::<i16>(&[value]).expect("i16 always converts");
            assert!((-1.0..=1.0).contains(&mixed));
        }
    }
}
