//! Bounded, allocation-free ASR inference chunk pool and the feeder that
//! accumulates Spec 04's 20 ms mic blocks into 100 ms ASR chunks.
//!
//! This mirrors `crate::audio::buffer`'s pool shape (preallocated fixed
//! buffers moving between two lock-free SPSC `rtrb` queues) but is a
//! second, independent stage: it never grows, and its 30 × 100 ms
//! capacity bounds ASR-queued audio to exactly three seconds, separate
//! from Spec 04's own two-second capture pool.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use rtrb::{Consumer, Producer, PushError, RingBuffer};

/// Number of preallocated ASR chunks. At 100 ms per chunk this bounds
/// queued inference audio to exactly three seconds.
pub const ASR_POOL_CAPACITY: usize = super::ASR_POOL_CAPACITY;

/// Computes the mono sample capacity of one 100 ms chunk for a given
/// sample rate: `ceil(sample_rate_hz / 10)`.
pub fn asr_chunk_capacity_for_rate(sample_rate_hz: u32) -> usize {
    (sample_rate_hz as usize).div_ceil(10)
}

/// One bounded, reusable ASR chunk. Only `samples[..valid_samples]` holds
/// current audio; a final partial chunk at Stop may have
/// `valid_samples < samples.len()` and is never zero-padded.
pub struct AsrChunk {
    pub samples: Box<[f32]>,
    pub valid_samples: usize,
}

#[derive(Debug, Default)]
struct AsrOverflowCounter(AtomicU64);
impl AsrOverflowCounter {
    fn record(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
    fn total(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Producer-side handle, held only by [`AsrChunkFeeder`]. Every method is
/// non-blocking and allocation-free after construction.
pub struct AsrChunkProducer {
    filled_tx: Producer<AsrChunk>,
    recycle_rx: Consumer<Box<[f32]>>,
    overflow: Arc<AsrOverflowCounter>,
    finished: Arc<AtomicBool>,
    abandoned: Arc<AtomicBool>,
}

impl AsrChunkProducer {
    fn try_acquire(&mut self) -> Option<Box<[f32]>> {
        match self.recycle_rx.pop() {
            Ok(buffer) => Some(buffer),
            Err(_) => {
                self.overflow.record();
                None
            }
        }
    }

    fn try_submit(&mut self, chunk: AsrChunk) -> Result<(), Box<[f32]>> {
        match self.filled_tx.push(chunk) {
            Ok(()) => Ok(()),
            Err(PushError::Full(chunk)) => {
                self.overflow.record();
                Err(chunk.samples)
            }
        }
    }

    /// Signals that no further chunks will ever be submitted. Idempotent;
    /// also called by [`AsrChunkFeeder`]'s `Drop` so every exit path
    /// (normal stop, fault, early return) reaches it exactly once.
    fn mark_finished(&self) {
        self.finished.store(true, Ordering::Release);
    }

    /// Signals that the stream is being abandoned rather than gracefully
    /// finished: no pending partial chunk was submitted, and the worker
    /// must skip its normal `StreamingRecognizer::finish()` drain
    /// entirely so no final is emitted for the abandoned interim. Implies
    /// `mark_finished` so the worker's "no chunk available and finished"
    /// exit condition is also satisfied.
    fn mark_abandoned(&self) {
        self.abandoned.store(true, Ordering::Release);
        self.finished.store(true, Ordering::Release);
    }
}

/// Consumer-side handle, owned by the ASR worker thread.
pub struct AsrChunkConsumer {
    filled_rx: Consumer<AsrChunk>,
    recycle_tx: Producer<Box<[f32]>>,
    overflow: Arc<AsrOverflowCounter>,
    finished: Arc<AtomicBool>,
    abandoned: Arc<AtomicBool>,
}

impl AsrChunkConsumer {
    pub fn try_recv(&mut self) -> Option<AsrChunk> {
        self.filled_rx.pop().ok()
    }

    /// Returns a drained chunk's buffer to circulation. Returns `false`
    /// only on an internal pool-invariant violation (the feeder already
    /// exited and dropped its recycle receiver); harmless to ignore
    /// during teardown.
    pub fn recycle(&mut self, buffer: Box<[f32]>) -> bool {
        self.recycle_tx.push(buffer).is_ok()
    }

    pub fn overflow_total(&self) -> u64 {
        self.overflow.total()
    }

    /// `true` once the feeder has flushed its final partial chunk (if
    /// any) and will never submit another. The worker treats
    /// "no chunk available and finished" as end of input.
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    /// `true` if the feeder's teardown was an `abandon()` rather than a
    /// graceful `finish()`: the worker must skip `StreamingRecognizer::
    /// finish()` entirely in that case, emitting no final for whatever
    /// interim was in flight. Always implies [`Self::is_finished`].
    pub fn is_abandoned(&self) -> bool {
        self.abandoned.load(Ordering::Acquire)
    }
}

/// Allocates the fixed ASR chunk pool and splits it into its
/// producer/consumer halves.
pub fn build_asr_pool(chunk_capacity_samples: usize) -> (AsrChunkProducer, AsrChunkConsumer) {
    let (mut recycle_tx, recycle_rx) = RingBuffer::<Box<[f32]>>::new(ASR_POOL_CAPACITY);
    for _ in 0..ASR_POOL_CAPACITY {
        let buffer: Box<[f32]> = vec![0.0_f32; chunk_capacity_samples].into_boxed_slice();
        recycle_tx
            .push(buffer)
            .expect("recycle queue capacity matches ASR_POOL_CAPACITY");
    }
    let (filled_tx, filled_rx) = RingBuffer::<AsrChunk>::new(ASR_POOL_CAPACITY);
    let overflow = Arc::new(AsrOverflowCounter::default());
    let finished = Arc::new(AtomicBool::new(false));
    let abandoned = Arc::new(AtomicBool::new(false));

    let producer = AsrChunkProducer {
        filled_tx,
        recycle_rx,
        overflow: overflow.clone(),
        finished: finished.clone(),
        abandoned: abandoned.clone(),
    };
    let consumer = AsrChunkConsumer {
        filled_rx,
        recycle_tx,
        overflow,
        finished,
        abandoned,
    };
    (producer, consumer)
}

/// Accumulates Spec 04's 20 ms mic blocks into 100 ms ASR chunks and
/// submits full chunks through an [`AsrChunkProducer`]. Owned by the
/// microphone monitor thread; `accept` is allocation-free, non-blocking,
/// and takes no lock.
pub struct AsrChunkFeeder {
    producer: AsrChunkProducer,
    current: Option<Box<[f32]>>,
    cursor: usize,
    capacity: usize,
    finished_called: bool,
}

impl AsrChunkFeeder {
    pub fn new(producer: AsrChunkProducer, capacity: usize) -> Self {
        Self {
            producer,
            current: None,
            cursor: 0,
            capacity,
            finished_called: false,
        }
    }

    /// Copies `samples` into the current partially filled chunk, this
    /// call's own caller is expected to have already copied the samples
    /// out of a still-live buffer it owns (Spec 04's monitor recycles its
    /// own block immediately after this call returns).
    pub fn accept(&mut self, mut samples: &[f32]) {
        while !samples.is_empty() {
            if self.current.is_none() {
                match self.producer.try_acquire() {
                    Some(buffer) => {
                        self.current = Some(buffer);
                        self.cursor = 0;
                    }
                    None => return, // Overflow already recorded by try_acquire.
                }
            }
            let buffer = self
                .current
                .as_deref_mut()
                .expect("current was just ensured to be Some");
            let space = self.capacity - self.cursor;
            let n = space.min(samples.len());
            buffer[self.cursor..self.cursor + n].copy_from_slice(&samples[..n]);
            self.cursor += n;
            samples = &samples[n..];

            if self.cursor == self.capacity {
                let filled = self.current.take().expect("cursor reached capacity");
                self.cursor = 0;
                let chunk = AsrChunk {
                    samples: filled,
                    valid_samples: self.capacity,
                };
                if let Err(returned) = self.producer.try_submit(chunk) {
                    // Filled queue is full: this chunk's content is
                    // dropped (already counted as overflow), but its
                    // buffer stays in circulation instead of leaking.
                    self.current = Some(returned);
                    self.cursor = 0;
                }
            }
        }
    }

    /// Submits any partially filled chunk (best effort; a full queue just
    /// drops it, counted as overflow) and marks the stream finished.
    /// Idempotent — safe to call explicitly and again from `Drop`.
    pub fn finish(&mut self) {
        if self.finished_called {
            return;
        }
        self.finished_called = true;
        if let Some(buffer) = self.current.take() {
            if self.cursor > 0 {
                let chunk = AsrChunk {
                    samples: buffer,
                    valid_samples: self.cursor,
                };
                let _ = self.producer.try_submit(chunk);
            }
            self.cursor = 0;
        }
        self.producer.mark_finished();
    }

    /// Discards any partially filled chunk without submitting it and
    /// marks the stream abandoned (never gracefully finished). Consuming
    /// `self` here, rather than taking `&mut self`, guarantees this can
    /// only ever run once per feeder and that the subsequent `Drop`
    /// cannot re-run `finish()`'s flush-and-submit behavior afterward.
    pub fn abandon(mut self) {
        self.finished_called = true;
        self.current = None;
        self.cursor = 0;
        self.producer.mark_abandoned();
    }
}

impl Drop for AsrChunkFeeder {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_chunks_are_submitted_and_partial_tail_is_not() {
        let (producer, mut consumer) = build_asr_pool(4);
        let mut feeder = AsrChunkFeeder::new(producer, 4);

        feeder.accept(&[1.0, 2.0, 3.0]); // partial, not yet submitted
        assert!(consumer.try_recv().is_none());

        feeder.accept(&[4.0, 5.0, 6.0]); // completes chunk 1, starts chunk 2
        let chunk = consumer.try_recv().expect("first chunk submitted");
        assert_eq!(chunk.valid_samples, 4);
        assert_eq!(&chunk.samples[..4], &[1.0, 2.0, 3.0, 4.0]);
        assert!(consumer.try_recv().is_none());
    }

    #[test]
    fn finish_flushes_partial_tail_without_zero_padding() {
        let (producer, mut consumer) = build_asr_pool(4);
        let mut feeder = AsrChunkFeeder::new(producer, 4);
        feeder.accept(&[9.0, 8.0]);
        feeder.finish();

        let chunk = consumer.try_recv().expect("partial tail flushed at finish");
        assert_eq!(chunk.valid_samples, 2);
        assert_eq!(&chunk.samples[..2], &[9.0, 8.0]);
        assert!(consumer.is_finished());
    }

    #[test]
    fn finish_with_no_partial_data_still_marks_finished_and_submits_nothing() {
        let (producer, mut consumer) = build_asr_pool(4);
        let mut feeder = AsrChunkFeeder::new(producer, 4);
        feeder.finish();
        assert!(consumer.try_recv().is_none());
        assert!(consumer.is_finished());
    }

    #[test]
    fn abandon_discards_partial_tail_and_never_finishes_gracefully() {
        let (producer, mut consumer) = build_asr_pool(4);
        let mut feeder = AsrChunkFeeder::new(producer, 4);
        feeder.accept(&[1.0, 2.0]); // partial, would normally flush at finish()
        feeder.abandon();

        // The partial tail was discarded, never submitted as a chunk.
        assert!(consumer.try_recv().is_none());
        assert!(consumer.is_finished());
        assert!(consumer.is_abandoned());
    }

    #[test]
    fn dropping_the_feeder_marks_finished_exactly_once() {
        let (producer, consumer) = build_asr_pool(4);
        {
            let _feeder = AsrChunkFeeder::new(producer, 4);
        }
        assert!(consumer.is_finished());
    }

    #[test]
    fn no_recycled_buffer_drops_samples_and_counts_overflow_without_growing() {
        let (producer, mut consumer) = build_asr_pool(4);
        let mut feeder = AsrChunkFeeder::new(producer, 4);
        // Drain the whole recycle pool by submitting ASR_POOL_CAPACITY
        // full chunks without the consumer recycling any of them back.
        for i in 0..ASR_POOL_CAPACITY {
            let sample = i as f32;
            feeder.accept(&[sample, sample, sample, sample]);
        }
        // Pool holds at most ASR_POOL_CAPACITY chunks; further input must
        // be dropped rather than growing memory.
        feeder.accept(&[1.0, 2.0, 3.0, 4.0]);
        let mut received = 0;
        while consumer.try_recv().is_some() {
            received += 1;
        }
        assert!(received <= ASR_POOL_CAPACITY);
        assert!(consumer.overflow_total() > 0);
    }
}
