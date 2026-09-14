//! Fixed-size 20 ms mono block assembly through one preallocated accumulator
//! (spec section 6 "Buffering and real-time contract").
//!
//! Pure Rust: exercised directly by unit tests across arbitrary packet sizes,
//! independent of any live WASAPI stream.

/// Accumulates mono samples into fixed-size blocks. Exactly one buffer is
/// preallocated at construction; steady-state `feed` calls never allocate.
pub(crate) struct BlockAssembler {
    accumulator: Vec<f32>,
    block_len: usize,
    write_pos: usize,
}

impl BlockAssembler {
    pub fn new(block_len: usize) -> Self {
        assert!(block_len > 0, "block_len must be positive");
        Self {
            accumulator: vec![0.0; block_len],
            block_len,
            write_pos: 0,
        }
    }

    /// Feeds mono samples into the accumulator, invoking `deliver` once per
    /// completed block with exactly `block_len` samples. A single call may
    /// span zero, one, or many completed blocks depending on `samples.len()`
    /// relative to the remaining space in the accumulator.
    pub fn feed(&mut self, samples: &[f32], mut deliver: impl FnMut(&[f32])) {
        let mut idx = 0;
        while idx < samples.len() {
            let space = self.block_len - self.write_pos;
            let take = space.min(samples.len() - idx);
            self.accumulator[self.write_pos..self.write_pos + take]
                .copy_from_slice(&samples[idx..idx + take]);
            self.write_pos += take;
            idx += take;
            if self.write_pos == self.block_len {
                deliver(&self.accumulator);
                self.write_pos = 0;
            }
        }
    }

    /// Discards a partial tail at stop. A partial block is never zero-padded
    /// and delivered.
    pub fn discard_partial(&mut self) {
        self.write_pos = 0;
    }

    #[cfg(test)]
    pub fn pending_len(&self) -> usize {
        self.write_pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_size_feed_delivers_one_block() {
        let mut assembler = BlockAssembler::new(4);
        let mut delivered: Vec<Vec<f32>> = Vec::new();
        assembler.feed(&[0.1, 0.2, 0.3, 0.4], |block| {
            delivered.push(block.to_vec())
        });
        assert_eq!(delivered, vec![vec![0.1, 0.2, 0.3, 0.4]]);
        assert_eq!(assembler.pending_len(), 0);
    }

    #[test]
    fn undersized_feed_accumulates_without_delivering() {
        let mut assembler = BlockAssembler::new(4);
        let mut delivered: Vec<Vec<f32>> = Vec::new();
        assembler.feed(&[0.1, 0.2], |block| delivered.push(block.to_vec()));
        assert!(delivered.is_empty());
        assert_eq!(assembler.pending_len(), 2);
    }

    #[test]
    fn oversized_feed_delivers_multiple_blocks_and_keeps_remainder() {
        let mut assembler = BlockAssembler::new(3);
        let mut delivered: Vec<Vec<f32>> = Vec::new();
        assembler.feed(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0], |block| {
            delivered.push(block.to_vec())
        });
        assert_eq!(delivered, vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
        assert_eq!(assembler.pending_len(), 1);
    }

    #[test]
    fn arbitrary_packet_sizes_reassemble_into_fixed_blocks() {
        let mut assembler = BlockAssembler::new(5);
        let mut delivered: Vec<Vec<f32>> = Vec::new();
        let packets: [&[f32]; 4] = [
            &[1.0, 2.0],
            &[3.0],
            &[4.0, 5.0, 6.0, 7.0],
            &[8.0, 9.0, 10.0],
        ];
        for packet in packets {
            assembler.feed(packet, |block| delivered.push(block.to_vec()));
        }
        assert_eq!(
            delivered,
            vec![
                vec![1.0, 2.0, 3.0, 4.0, 5.0],
                vec![6.0, 7.0, 8.0, 9.0, 10.0],
            ]
        );
        assert_eq!(assembler.pending_len(), 0);
    }

    #[test]
    fn discard_partial_drops_the_incomplete_tail() {
        let mut assembler = BlockAssembler::new(4);
        let mut delivered: Vec<Vec<f32>> = Vec::new();
        assembler.feed(&[1.0, 2.0, 3.0], |block| delivered.push(block.to_vec()));
        assert_eq!(assembler.pending_len(), 3);
        assembler.discard_partial();
        assert_eq!(assembler.pending_len(), 0);
        assembler.feed(&[9.0], |block| delivered.push(block.to_vec()));
        assert!(delivered.is_empty());
        assert_eq!(assembler.pending_len(), 1);
    }
}
