//! Continuous-timeline reconstruction: gap measurement, capped silence
//! synthesis, and the counter set that keeps captured and synthesized frames
//! separate (spec section 6, "Timeline contract — the honest part").
//!
//! Pure Rust: exercised directly by unit tests with synthetic positions and
//! frame counts, independent of any live WASAPI stream.

/// Aggregate counters for one capture session. Every field is a running total
/// for the session's lifetime. `captured_frames`, `silent_flag_frames`, and
/// `synthesized_frames` are always reported separately: no report may present
/// synthesized silence as captured audio.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemAudioCounters {
    pub captured_frames: u64,
    pub silent_flag_frames: u64,
    pub synthesized_frames: u64,
    pub discontinuity_events: u64,
    pub truncated_gap_events: u64,
    pub dropped_blocks: u64,
}

/// The outcome of observing one packet or one idle timeout: how many silent
/// frames the caller must emit through the normal block path before the
/// packet's own frames (if any), and whether that fill was capped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GapFill {
    pub silent_frames_to_emit: u64,
    pub truncated: bool,
}

/// Reconstructs a continuous frame timeline from WASAPI's non-continuous
/// packet delivery. Owns no I/O; every input is already-read frame counts and
/// device positions.
pub(crate) struct Timeline {
    expected_position: u64,
    max_gap_fill_frames: u64,
    counters: SystemAudioCounters,
}

impl Timeline {
    pub fn new(max_gap_fill_frames: u64) -> Self {
        Self {
            expected_position: 0,
            max_gap_fill_frames,
            counters: SystemAudioCounters::default(),
        }
    }

    /// Observes one delivered packet. `device_position` is the packet's first
    /// frame position as returned by `GetBuffer`; `silent`/`discontinuity`
    /// mirror the corresponding `AUDCLNT_BUFFERFLAGS_*` bits.
    ///
    /// A discontinuity is never treated as a fillable gap: the driver has
    /// already told us data was lost, so synthesizing silence on top of that
    /// would misrepresent an unknown loss as measured idle time. The position
    /// is still resynchronized so the next packet's gap math starts clean.
    pub fn observe_packet(
        &mut self,
        device_position: u64,
        frame_count: u64,
        silent: bool,
        discontinuity: bool,
    ) -> GapFill {
        let fill = if discontinuity {
            self.counters.discontinuity_events += 1;
            GapFill {
                silent_frames_to_emit: 0,
                truncated: false,
            }
        } else {
            let gap = device_position.saturating_sub(self.expected_position);
            self.cap_gap(gap)
        };

        if fill.silent_frames_to_emit > 0 {
            self.counters.synthesized_frames += fill.silent_frames_to_emit;
        }
        if silent {
            self.counters.silent_flag_frames += frame_count;
        } else {
            self.counters.captured_frames += frame_count;
        }
        self.expected_position = device_position + frame_count;
        fill
    }

    /// Observes the capture thread's bounded wait timing out with no packet
    /// delivered at all. `elapsed_frames` is the idle duration converted to
    /// frames at the session rate. Advances the expected position
    /// optimistically by the emitted fill; the next real packet reconciles
    /// any remaining drift through `observe_packet`'s own gap math.
    pub fn note_idle_wait(&mut self, elapsed_frames: u64) -> GapFill {
        let fill = self.cap_gap(elapsed_frames);
        if fill.silent_frames_to_emit > 0 {
            self.counters.synthesized_frames += fill.silent_frames_to_emit;
            self.expected_position += fill.silent_frames_to_emit;
        }
        fill
    }

    pub fn note_dropped_block(&mut self) {
        self.counters.dropped_blocks += 1;
    }

    pub fn snapshot(&self) -> SystemAudioCounters {
        self.counters
    }

    /// Caps `gap` at `max_gap_fill_frames`, counting a truncation event when
    /// the cap binds. A zero gap is not a truncation and emits nothing.
    fn cap_gap(&mut self, gap: u64) -> GapFill {
        if gap == 0 {
            return GapFill {
                silent_frames_to_emit: 0,
                truncated: false,
            };
        }
        if gap > self.max_gap_fill_frames {
            self.counters.truncated_gap_events += 1;
            GapFill {
                silent_frames_to_emit: self.max_gap_fill_frames,
                truncated: true,
            }
        } else {
            GapFill {
                silent_frames_to_emit: gap,
                truncated: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_packet_with_zero_position_has_no_gap() {
        let mut timeline = Timeline::new(1_000);
        let fill = timeline.observe_packet(0, 100, false, false);
        assert_eq!(fill.silent_frames_to_emit, 0);
        assert!(!fill.truncated);
        let counters = timeline.snapshot();
        assert_eq!(counters.captured_frames, 100);
        assert_eq!(counters.synthesized_frames, 0);
    }

    #[test]
    fn idle_gap_between_packets_is_synthesized_and_counted() {
        let mut timeline = Timeline::new(1_000);
        timeline.observe_packet(0, 100, false, false);
        let fill = timeline.observe_packet(300, 50, false, false);
        assert_eq!(fill.silent_frames_to_emit, 200);
        assert!(!fill.truncated);
        let counters = timeline.snapshot();
        assert_eq!(counters.captured_frames, 150);
        assert_eq!(counters.synthesized_frames, 200);
        assert_eq!(counters.truncated_gap_events, 0);
    }

    #[test]
    fn gap_longer_than_cap_is_truncated_and_resynchronized() {
        let mut timeline = Timeline::new(500);
        timeline.observe_packet(0, 100, false, false);
        let fill = timeline.observe_packet(2_100, 50, false, false);
        assert_eq!(fill.silent_frames_to_emit, 500);
        assert!(fill.truncated);
        let counters = timeline.snapshot();
        assert_eq!(counters.truncated_gap_events, 1);
        assert_eq!(counters.synthesized_frames, 500);
        // The timeline resynchronizes to the packet's actual position plus
        // its own frames, never accumulating unbounded fabricated audio.
        let fill2 = timeline.observe_packet(2_150, 10, false, false);
        assert_eq!(fill2.silent_frames_to_emit, 0);
    }

    #[test]
    fn silent_flagged_packets_count_separately_from_captured() {
        let mut timeline = Timeline::new(1_000);
        let fill = timeline.observe_packet(0, 480, true, false);
        assert_eq!(fill.silent_frames_to_emit, 0);
        let counters = timeline.snapshot();
        assert_eq!(counters.silent_flag_frames, 480);
        assert_eq!(counters.captured_frames, 0);
        assert_eq!(counters.synthesized_frames, 0);
    }

    #[test]
    fn discontinuity_is_counted_and_skips_gap_synthesis() {
        let mut timeline = Timeline::new(1_000);
        timeline.observe_packet(0, 100, false, false);
        // A huge apparent jump accompanied by the discontinuity flag must not
        // be interpreted as idle time to fill with silence.
        let fill = timeline.observe_packet(50_000, 100, false, true);
        assert_eq!(fill.silent_frames_to_emit, 0);
        let counters = timeline.snapshot();
        assert_eq!(counters.discontinuity_events, 1);
        assert_eq!(counters.synthesized_frames, 0);
        assert_eq!(counters.captured_frames, 200);
    }

    #[test]
    fn idle_wait_timeout_synthesizes_and_advances_expected_position() {
        let mut timeline = Timeline::new(1_000);
        let fill = timeline.note_idle_wait(160);
        assert_eq!(fill.silent_frames_to_emit, 160);
        let counters = timeline.snapshot();
        assert_eq!(counters.synthesized_frames, 160);
        // The next packet at exactly the advanced position has no further gap.
        let fill2 = timeline.observe_packet(160, 20, false, false);
        assert_eq!(fill2.silent_frames_to_emit, 0);
    }

    #[test]
    fn idle_wait_timeout_respects_the_cap() {
        let mut timeline = Timeline::new(100);
        let fill = timeline.note_idle_wait(500);
        assert_eq!(fill.silent_frames_to_emit, 100);
        assert!(fill.truncated);
        assert_eq!(timeline.snapshot().truncated_gap_events, 1);
    }

    #[test]
    fn dropped_block_increments_its_own_counter_only() {
        let mut timeline = Timeline::new(1_000);
        timeline.observe_packet(0, 100, false, false);
        timeline.note_dropped_block();
        timeline.note_dropped_block();
        let counters = timeline.snapshot();
        assert_eq!(counters.dropped_blocks, 2);
        assert_eq!(counters.captured_frames, 100);
    }

    #[test]
    fn counters_never_merge_captured_and_synthesized() {
        let mut timeline = Timeline::new(1_000);
        timeline.observe_packet(0, 100, false, false);
        timeline.observe_packet(300, 50, true, false);
        let counters = timeline.snapshot();
        assert_eq!(counters.captured_frames, 100);
        assert_eq!(counters.silent_flag_frames, 50);
        assert_eq!(counters.synthesized_frames, 200);
        assert_eq!(
            counters.captured_frames + counters.silent_flag_frames + counters.synthesized_frames,
            350
        );
    }
}
