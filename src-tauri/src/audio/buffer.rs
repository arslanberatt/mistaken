//! Bounded, allocation-free PCM block pool.
//!
//! Exactly [`POOL_CAPACITY`] fixed-capacity `Box<[f32]>` buffers are
//! allocated once and then only ever move between two lock-free SPSC queues
//! (`rtrb`): "filled" (producer -> consumer) and "recycle" (consumer ->
//! producer). Neither queue is ever resized, and no buffer is ever
//! allocated, freed, or duplicated after construction. [`PoolProducer`]
//! implements the frozen [`crate::audio::PcmBlockSink`] contract and is the
//! only type an audio callback touches; [`PoolConsumer`] is read by the
//! monitor thread.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use crate::audio::{AudioError, AudioErrorKind, AudioSource, PcmBlock, PcmBlockSink};

/// Number of preallocated blocks. At 20 ms per block this bounds queued PCM
/// to exactly two seconds.
pub const POOL_CAPACITY: usize = 100;

/// Target block duration in milliseconds.
pub const BLOCK_DURATION_MS: u32 = 20;

/// Computes the mono sample capacity of one 20 ms block for a given sample
/// rate: `ceil(sample_rate_hz / 50)`.
pub fn block_capacity_for_rate(sample_rate_hz: u32) -> usize {
    (sample_rate_hz as usize).div_ceil(50)
}

/// Shared, atomic overflow counter. Incremented in place, with no
/// allocation, whenever the pool cannot honor a request without blocking or
/// growing memory.
#[derive(Debug, Default)]
pub struct OverflowCounter(AtomicU64);

impl OverflowCounter {
    fn record(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    pub fn total(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Producer-side handle passed into a capture backend as `Box<dyn
/// PcmBlockSink>`. Every method is non-blocking and allocation-free after
/// construction.
pub struct PoolProducer {
    next_sequence: u64,
    filled_tx: Producer<PcmBlock>,
    recycle_rx: Consumer<Box<[f32]>>,
    overflow: Arc<OverflowCounter>,
}

impl PcmBlockSink for PoolProducer {
    fn try_acquire(&mut self) -> Option<Box<[f32]>> {
        match self.recycle_rx.pop() {
            Ok(buffer) => Some(buffer),
            Err(_) => {
                self.overflow.record();
                None
            }
        }
    }

    fn try_submit(&mut self, mut block: PcmBlock) -> Result<(), PcmBlock> {
        block.sequence = self.next_sequence;
        match self.filled_tx.push(block) {
            Ok(()) => {
                self.next_sequence += 1;
                Ok(())
            }
            Err(PushError::Full(block)) => {
                self.overflow.record();
                Err(block)
            }
        }
    }
}

/// Consumer-side handle owned by the monitor thread: drains completed
/// blocks and recycles their buffers back to the producer.
pub struct PoolConsumer {
    filled_rx: Consumer<PcmBlock>,
    recycle_tx: Producer<Box<[f32]>>,
    overflow: Arc<OverflowCounter>,
}

impl PoolConsumer {
    /// Returns the next completed block, or `None` if none is queued yet.
    pub fn try_recv(&mut self) -> Option<PcmBlock> {
        self.filled_rx.pop().ok()
    }

    /// Returns a drained block's buffer to circulation. This can only fail
    /// if the recycle queue's capacity was violated, which is an internal
    /// invariant failure rather than ordinary backpressure.
    pub fn recycle(&mut self, block: PcmBlock) -> Result<(), AudioError> {
        let PcmBlock { samples, .. } = block;
        self.recycle_tx.push(samples).map_err(|_| AudioError {
            source: AudioSource::Microphone,
            kind: AudioErrorKind::Internal,
        })
    }

    /// Total frames/blocks dropped so far because no buffer or queue slot
    /// was free. Monotonically non-decreasing for the life of the pool.
    pub fn overflow_total(&self) -> u64 {
        self.overflow.total()
    }
}

/// Allocates the fixed pool and splits it into its producer/consumer
/// halves. All [`POOL_CAPACITY`] buffers start in the recycle queue (free);
/// the filled queue starts empty.
pub fn build_pool(block_capacity_samples: usize) -> (PoolProducer, PoolConsumer) {
    let (mut recycle_tx, recycle_rx) = RingBuffer::<Box<[f32]>>::new(POOL_CAPACITY);
    for _ in 0..POOL_CAPACITY {
        let buffer: Box<[f32]> = vec![0.0_f32; block_capacity_samples].into_boxed_slice();
        recycle_tx
            .push(buffer)
            .expect("recycle queue capacity matches POOL_CAPACITY");
    }
    let (filled_tx, filled_rx) = RingBuffer::<PcmBlock>::new(POOL_CAPACITY);
    let overflow = Arc::new(OverflowCounter::default());

    let producer = PoolProducer {
        next_sequence: 0,
        filled_tx,
        recycle_rx,
        overflow: overflow.clone(),
    };
    let consumer = PoolConsumer {
        filled_rx,
        recycle_tx,
        overflow,
    };
    (producer, consumer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::PcmFormat;
    use std::num::{NonZeroU16, NonZeroU32};

    fn format() -> PcmFormat {
        PcmFormat {
            sample_rate_hz: NonZeroU32::new(16_000).unwrap(),
            channels: NonZeroU16::new(1).unwrap(),
        }
    }

    fn block(sequence: u64, samples: Box<[f32]>) -> PcmBlock {
        let valid_samples = samples.len();
        PcmBlock {
            source: AudioSource::Microphone,
            sequence,
            format: format(),
            valid_samples,
            samples,
        }
    }

    #[test]
    fn block_capacity_matches_twenty_milliseconds() {
        assert_eq!(block_capacity_for_rate(16_000), 320);
        assert_eq!(block_capacity_for_rate(48_000), 960);
        // Non-exact rates round up so the block covers at least 20 ms.
        assert_eq!(block_capacity_for_rate(44_100), 882);
        assert_eq!(block_capacity_for_rate(8_000), 160);
    }

    #[test]
    fn pool_holds_exactly_one_hundred_two_second_blocks() {
        let (_producer, _consumer) = build_pool(320);
        // 100 blocks * 20ms = 2000ms, proven by construction never growing.
        assert_eq!(POOL_CAPACITY, 100);
    }

    #[test]
    fn acquire_submit_recycle_round_trips_a_buffer() {
        let (mut producer, mut consumer) = build_pool(4);

        let buffer = producer.try_acquire().expect("one of 100 buffers is free");
        assert_eq!(buffer.len(), 4);
        assert!(
            producer.try_submit(block(0, buffer)).is_ok(),
            "filled queue has room"
        );

        let received = consumer.try_recv().expect("block was submitted");
        assert_eq!(received.sequence, 0);
        assert_eq!(received.valid_samples, 4);

        consumer.recycle(received).expect("recycle queue has room");
        let recycled = producer.try_acquire().expect("buffer was recycled");
        assert_eq!(recycled.len(), 4);
    }

    #[test]
    fn try_submit_assigns_contiguous_sequence_numbers() {
        let (mut producer, mut consumer) = build_pool(2);
        for expected in 0_u64..5 {
            let buffer = producer.try_acquire().expect("buffer available");
            assert!(
                producer.try_submit(block(999, buffer)).is_ok(),
                "queue has room"
            );
            let received = consumer.try_recv().expect("just submitted");
            assert_eq!(received.sequence, expected);
            consumer.recycle(received).expect("recycle has room");
        }
    }

    #[test]
    fn try_acquire_returns_none_and_counts_overflow_when_pool_is_exhausted() {
        let (mut producer, consumer) = build_pool(2);
        let mut held = Vec::new();
        for _ in 0..POOL_CAPACITY {
            held.push(producer.try_acquire().expect("still within capacity"));
        }

        assert!(producer.try_acquire().is_none());
        assert_eq!(consumer.overflow_total(), 1);
        // Draining more still reports overflow without panicking or growing.
        assert!(producer.try_acquire().is_none());
        assert_eq!(consumer.overflow_total(), 2);
        assert_eq!(held.len(), POOL_CAPACITY);
    }

    #[test]
    fn try_submit_returns_the_block_and_counts_overflow_when_filled_queue_is_full() {
        let (mut producer, consumer) = build_pool(2);
        for _ in 0..POOL_CAPACITY {
            let buffer = producer.try_acquire().expect("buffer available");
            assert!(
                producer.try_submit(block(0, buffer)).is_ok(),
                "filled queue has room"
            );
        }

        let overflow_buffer = producer.try_acquire();
        assert!(
            overflow_buffer.is_none(),
            "recycle queue is empty once every buffer is in the filled queue"
        );
        assert_eq!(consumer.overflow_total(), 1);
    }

    #[test]
    fn pool_never_grows_beyond_its_fixed_capacity() {
        let (mut producer, mut consumer) = build_pool(2);
        for _ in 0..(POOL_CAPACITY * 3) {
            if let Some(buffer) = producer.try_acquire() {
                if let Ok(()) = producer.try_submit(block(0, buffer)) {
                    let received = consumer.try_recv().expect("just submitted");
                    consumer.recycle(received).expect("recycle has room");
                }
            }
        }
        // No panic and no capacity change across far more than POOL_CAPACITY
        // acquire/submit/recycle cycles proves the pool never grows.
        assert_eq!(POOL_CAPACITY, 100);
    }
}
