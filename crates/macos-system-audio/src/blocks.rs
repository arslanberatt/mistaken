//! Downmix and exact 20 ms mono block assembly.
//!
//! Every function here is allocation-free and safe to call from the
//! adapter's real-time delivery queue: no heap allocation, no locking, no
//! I/O. [`BlockAssembler`] owns one preallocated mono accumulator and never
//! grows it.

/// Downmixes one frame (one sample per channel) to mono by arithmetic mean,
/// clamped to `[-1.0, 1.0]`. Returns `None` for a non-finite frame so the
/// caller can drop it instead of forwarding NaN/Inf audio.
pub fn downmix_frame(channels: &[f32]) -> Option<f32> {
    if channels.is_empty() {
        return None;
    }
    let sum: f32 = channels.iter().copied().sum();
    let mean = sum / channels.len() as f32;
    if mean.is_finite() {
        Some(mean.clamp(-1.0, 1.0))
    } else {
        None
    }
}

/// Assembles a stream of mono samples into exact fixed-size blocks.
///
/// A block is only ever handed to the caller once it is completely filled;
/// a partial tail sits in the accumulator until either more samples arrive
/// or [`BlockAssembler::discard_partial`] is called at stop.
pub struct BlockAssembler {
    accumulator: Box<[f32]>,
    write_pos: usize,
}

impl BlockAssembler {
    /// `block_len` is the exact number of mono samples per 20 ms block
    /// (`sample_rate_hz / 50`).
    pub fn new(block_len: usize) -> Self {
        Self {
            accumulator: vec![0.0_f32; block_len].into_boxed_slice(),
            write_pos: 0,
        }
    }

    pub fn block_len(&self) -> usize {
        self.accumulator.len()
    }

    /// Pushes one mono sample, invoking `emit` with the complete block
    /// whenever the accumulator fills. `emit` observes only the current
    /// block's samples; no allocation occurs here.
    pub fn push_sample(&mut self, sample: f32, emit: &mut dyn FnMut(&[f32])) {
        self.accumulator[self.write_pos] = sample;
        self.write_pos += 1;
        if self.write_pos == self.accumulator.len() {
            emit(&self.accumulator);
            self.write_pos = 0;
        }
    }

    /// Discards any partially filled block. Called at stop so a stale
    /// partial tail is never zero-padded and forwarded as if it were real
    /// audio.
    pub fn discard_partial(&mut self) {
        self.write_pos = 0;
    }
}

/// The layout of one batch of decoded audio frames.
pub enum FrameLayout<'a> {
    /// `channels` interleaved samples per frame, `data.len() == frames * channels`.
    Interleaved(&'a [f32]),
    /// One slice per channel, each of length `frames`.
    NonInterleaved(&'a [&'a [f32]]),
}

/// Maximum channel count this adapter ever downmixes. Matches the format
/// validation bound (1-8 channels).
pub const MAX_CHANNELS: usize = 8;

/// Feeds `frames` frames of `channels`-wide audio through downmix and block
/// assembly. Uses a fixed-size stack scratch frame (bounded by
/// [`MAX_CHANNELS`]) so non-interleaved gather never allocates.
pub fn feed_frames(
    layout: FrameLayout<'_>,
    frames: usize,
    channels: usize,
    assembler: &mut BlockAssembler,
    emit: &mut dyn FnMut(&[f32]),
) {
    debug_assert!((1..=MAX_CHANNELS).contains(&channels));
    match layout {
        FrameLayout::Interleaved(data) => {
            for frame in 0..frames {
                let start = frame * channels;
                let end = start + channels;
                if let Some(mono) = downmix_frame(&data[start..end]) {
                    assembler.push_sample(mono, emit);
                }
            }
        }
        FrameLayout::NonInterleaved(chans) => {
            let mut scratch = [0.0_f32; MAX_CHANNELS];
            for frame in 0..frames {
                for (c, chan) in chans.iter().enumerate().take(channels) {
                    scratch[c] = chan[frame];
                }
                if let Some(mono) = downmix_frame(&scratch[..channels]) {
                    assembler.push_sample(mono, emit);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_channels_and_clamps() {
        assert_eq!(downmix_frame(&[1.0, 1.0]), Some(1.0));
        assert_eq!(downmix_frame(&[0.5, -0.5]), Some(0.0));
        assert_eq!(downmix_frame(&[2.0, 2.0]), Some(1.0)); // clamped
        assert_eq!(downmix_frame(&[-2.0, -2.0]), Some(-1.0)); // clamped
    }

    #[test]
    fn downmix_drops_non_finite_frames() {
        assert_eq!(downmix_frame(&[f32::NAN, 0.0]), None);
        assert_eq!(downmix_frame(&[f32::INFINITY, 0.0]), None);
        assert_eq!(downmix_frame(&[]), None);
    }

    #[test]
    fn block_len_matches_20ms_at_the_requested_rate() {
        for &rate in &[8_000_u32, 16_000, 24_000, 48_000] {
            let assembler = BlockAssembler::new(rate as usize / 50);
            assert_eq!(assembler.block_len(), rate as usize / 50);
        }
    }

    #[test]
    fn block_assembler_emits_only_on_exact_fill() {
        let mut assembler = BlockAssembler::new(4);
        let mut emitted: Vec<Vec<f32>> = Vec::new();
        for s in [1.0, 2.0, 3.0] {
            assembler.push_sample(s, &mut |block| emitted.push(block.to_vec()));
        }
        assert!(emitted.is_empty(), "must not emit a partial block");
        assembler.push_sample(4.0, &mut |block| emitted.push(block.to_vec()));
        assert_eq!(emitted, vec![vec![1.0, 2.0, 3.0, 4.0]]);
    }

    #[test]
    fn block_assembler_resets_after_emit_and_continues() {
        let mut assembler = BlockAssembler::new(2);
        let mut emitted: Vec<Vec<f32>> = Vec::new();
        for s in [1.0, 2.0, 3.0, 4.0, 5.0] {
            assembler.push_sample(s, &mut |block| emitted.push(block.to_vec()));
        }
        assert_eq!(emitted, vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        assert_eq!(assembler.write_pos, 1); // "5.0" is a pending partial sample
    }

    #[test]
    fn discard_partial_drops_pending_tail_without_emitting() {
        let mut assembler = BlockAssembler::new(4);
        let mut emitted: Vec<Vec<f32>> = Vec::new();
        for s in [1.0, 2.0] {
            assembler.push_sample(s, &mut |block| emitted.push(block.to_vec()));
        }
        assembler.discard_partial();
        assert!(emitted.is_empty());
        assembler.push_sample(9.0, &mut |block| emitted.push(block.to_vec()));
        assert!(
            emitted.is_empty(),
            "accumulator must have been reset, not merged with the discarded tail"
        );
    }

    #[test]
    fn feed_frames_interleaved_matches_non_interleaved() {
        let interleaved = [0.1, 0.3, 0.2, 0.4]; // frame0: L=0.1 R=0.3, frame1: L=0.2 R=0.4
        let mut a = BlockAssembler::new(2);
        let mut emitted_a: Vec<Vec<f32>> = Vec::new();
        feed_frames(
            FrameLayout::Interleaved(&interleaved),
            2,
            2,
            &mut a,
            &mut |b| emitted_a.push(b.to_vec()),
        );

        let left = [0.1_f32, 0.2];
        let right = [0.3_f32, 0.4];
        let chans: [&[f32]; 2] = [&left, &right];
        let mut b = BlockAssembler::new(2);
        let mut emitted_b: Vec<Vec<f32>> = Vec::new();
        feed_frames(
            FrameLayout::NonInterleaved(&chans),
            2,
            2,
            &mut b,
            &mut |blk| emitted_b.push(blk.to_vec()),
        );

        assert_eq!(emitted_a, emitted_b);
        let block = &emitted_a[0];
        assert!(
            (block[0] - 0.2).abs() < 1e-6,
            "mean(0.1,0.3)=0.2, got {}",
            block[0]
        );
        assert!(
            (block[1] - 0.3).abs() < 1e-6,
            "mean(0.2,0.4)=0.3, got {}",
            block[1]
        );
    }

    #[test]
    fn feed_frames_skips_non_finite_frame_without_shifting_block_boundary() {
        let interleaved = [0.1, 0.1, f32::NAN, f32::NAN, 0.3, 0.3];
        let mut assembler = BlockAssembler::new(2);
        let mut emitted: Vec<Vec<f32>> = Vec::new();
        feed_frames(
            FrameLayout::Interleaved(&interleaved),
            3,
            2,
            &mut assembler,
            &mut |b| emitted.push(b.to_vec()),
        );
        // Only two finite frames were fed (mono 0.1 and 0.3); the NaN frame
        // contributed nothing, so they land in the same block together.
        assert_eq!(emitted.len(), 1);
        assert!((emitted[0][0] - 0.1).abs() < 1e-6);
        assert!((emitted[0][1] - 0.3).abs() < 1e-6);
    }
}
