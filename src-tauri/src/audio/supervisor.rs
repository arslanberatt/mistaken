//! Frozen per-source recovery policy: classification, budget, backoff
//! schedule, and watchdog bounds.
//!
//! Everything in this module is pure and deterministic: no I/O, no
//! threads, no locks, no Tauri types. `crate::state::manager` owns the
//! actual supervisor thread, teardown, and restart orchestration built on
//! top of these primitives. See `docs/lifecycle-policy.md` for the policy
//! rationale this module implements exactly.

use std::time::{Duration, Instant};

use crate::state::runtime::RuntimeErrorCode;

/// Whether a source-terminal `RuntimeErrorCode` may be automatically
/// retried by the supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDecision {
    Recoverable,
    Terminal,
}

/// Exhaustive, no-wildcard classification over every `RuntimeErrorCode`.
/// A new code added to that enum fails to compile here until a deliberate
/// recoverable/terminal decision is recorded — see
/// `docs/lifecycle-policy.md` section 1 for the rationale behind each row.
pub fn classify_recovery(code: RuntimeErrorCode) -> RecoveryDecision {
    use RecoveryDecision::{Recoverable, Terminal};
    match code {
        RuntimeErrorCode::DeviceDisconnected => Recoverable,
        RuntimeErrorCode::SystemAudioUnavailable => Recoverable,
        RuntimeErrorCode::MicrophoneUnavailable => Recoverable,
        // Only ever classified while a recovery attempt is already under
        // way: the initial `start_capture` atomic-rollback path never
        // calls this classifier for its own failures.
        RuntimeErrorCode::CaptureStartFailed => Recoverable,

        RuntimeErrorCode::MicrophonePermissionDenied => Terminal,
        RuntimeErrorCode::SystemAudioPermissionDenied => Terminal,
        RuntimeErrorCode::UnsupportedPlatform => Terminal,
        RuntimeErrorCode::ModelMissing => Terminal,
        RuntimeErrorCode::ModelLoadFailed => Terminal,
        RuntimeErrorCode::ModelUnsupported => Terminal,
        RuntimeErrorCode::CaptureStopFailed => Terminal,
        RuntimeErrorCode::Internal => Terminal,
        // Never reached as a source-terminal fault reason in production
        // (validation-only or degradation-only codes); classified
        // Terminal defensively so an unreachable path can never
        // masquerade as an active retry loop.
        RuntimeErrorCode::RuntimeUnavailable => Terminal,
        RuntimeErrorCode::InvalidRequest => Terminal,
        RuntimeErrorCode::CaptureAlreadyActive => Terminal,
        RuntimeErrorCode::CaptureNotActive => Terminal,
        RuntimeErrorCode::AudioQueueOverflow => Terminal,
        RuntimeErrorCode::InferenceLagging => Terminal,
    }
}

/// Frozen recovery budget, backoff schedule, and watchdog bounds. See
/// `docs/lifecycle-policy.md`.
#[derive(Debug, Clone, Copy)]
pub struct RecoveryPolicy {
    pub max_attempts: u8,
    pub backoff: [Duration; 3],
    pub healthy_reset: Duration,
    /// Granularity of the interruptible backoff wait.
    pub wait_tick: Duration,
    pub starting_watchdog: Duration,
    pub stopping_watchdog: Duration,
}

impl RecoveryPolicy {
    /// The frozen production policy: 3 attempts, 500 ms / 2 s / 5 s
    /// backoff, 60 s healthy reset, 10 s/3 s watchdogs.
    pub const fn production() -> Self {
        Self {
            max_attempts: 3,
            backoff: [
                Duration::from_millis(500),
                Duration::from_secs(2),
                Duration::from_secs(5),
            ],
            healthy_reset: Duration::from_secs(60),
            wait_tick: Duration::from_millis(50),
            starting_watchdog: Duration::from_secs(10),
            stopping_watchdog: Duration::from_secs(3),
        }
    }

    /// The backoff duration for a 1-based attempt number, clamped to the
    /// last configured tier if somehow asked for more attempts than
    /// configured.
    pub fn backoff_for_attempt(&self, attempt: u8) -> Duration {
        let index = attempt.saturating_sub(1).min(self.backoff.len() as u8 - 1);
        self.backoff[index as usize]
    }
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        Self::production()
    }
}

/// A policy with millisecond-scale durations, used only by tests so the
/// suite stays fast while exercising the exact same code paths as
/// production. Never selected outside `#[cfg(test)]`.
#[cfg(test)]
pub fn fast_test_policy() -> RecoveryPolicy {
    RecoveryPolicy {
        max_attempts: 3,
        backoff: [
            Duration::from_millis(2),
            Duration::from_millis(4),
            Duration::from_millis(6),
        ],
        healthy_reset: Duration::from_millis(30),
        wait_tick: Duration::from_millis(1),
        starting_watchdog: Duration::from_millis(300),
        stopping_watchdog: Duration::from_millis(300),
    }
}

/// Runs one interruptible backoff wait for the given duration. Sleeps in
/// `policy.wait_tick`-sized slices, checking `cancelled` between each, so
/// a concurrent cancellation (Stop, new Start, shutdown — all expressed by
/// the caller as a generation mismatch) is observed within one tick
/// instead of the full backoff. Returns `false` if cancelled at any point
/// during the wait, `true` if the wait completed without cancellation.
pub fn interruptible_wait(
    duration: Duration,
    tick: Duration,
    cancelled: impl Fn() -> bool,
) -> bool {
    if cancelled() {
        return false;
    }
    let deadline = Instant::now() + duration;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return !cancelled();
        }
        let remaining = deadline - now;
        std::thread::park_timeout(remaining.min(tick));
        if cancelled() {
            return false;
        }
    }
}

/// Per-source sliding-window lag state, tracked over discrete
/// non-overlapping windows of dropped-vs-drained inference chunks. See
/// `docs/lifecycle-policy.md` section 4.
#[derive(Debug, Clone, Copy)]
pub struct LagWindow {
    window: Duration,
    threshold_percent: u8,
    window_start: Instant,
    drained_in_window: u64,
    dropped_in_window: u64,
    lagging: bool,
}

impl LagWindow {
    pub fn new(window: Duration, threshold_percent: u8) -> Self {
        Self {
            window,
            threshold_percent,
            window_start: Instant::now(),
            drained_in_window: 0,
            dropped_in_window: 0,
            lagging: false,
        }
    }

    pub fn is_lagging(&self) -> bool {
        self.lagging
    }

    /// Records one drained chunk and `dropped` newly observed dropped
    /// chunks (an overflow-counter delta) since the last call. Evaluates
    /// the window once `window` has elapsed since it started, resetting
    /// counters for the next window. Returns `Some(lagging)` exactly on a
    /// state transition, `None` otherwise.
    pub fn record(&mut self, drained: u64, dropped: u64) -> Option<bool> {
        self.record_at(Instant::now(), drained, dropped)
    }

    /// Same as [`Self::record`] but with an injectable clock reading, so
    /// tests can exercise window rollover deterministically without a
    /// real 10-second sleep.
    pub fn record_at(&mut self, now: Instant, drained: u64, dropped: u64) -> Option<bool> {
        self.drained_in_window += drained;
        self.dropped_in_window += dropped;

        if now.duration_since(self.window_start) < self.window {
            return None;
        }

        let total = self.drained_in_window + self.dropped_in_window;
        let was_lagging = self.lagging;
        if let Some(drop_percent) = (self.dropped_in_window * 100).checked_div(total) {
            if drop_percent > u64::from(self.threshold_percent) {
                self.lagging = true;
            } else if self.dropped_in_window == 0 {
                self.lagging = false;
            }
            // A completed window with drops present but under threshold
            // leaves the current state unchanged, matching "left when a
            // completed window has a 0% drop rate" exactly.
        }

        self.window_start = now;
        self.drained_in_window = 0;
        self.dropped_in_window = 0;

        if was_lagging == self.lagging {
            None
        } else {
            Some(self.lagging)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_covers_every_runtime_error_code_exhaustively() {
        // Compiling this match with no wildcard arm is itself the
        // exhaustiveness proof; this test additionally pins each row to
        // its documented decision so a future edit that flips one
        // silently is caught.
        let recoverable = [
            RuntimeErrorCode::DeviceDisconnected,
            RuntimeErrorCode::SystemAudioUnavailable,
            RuntimeErrorCode::MicrophoneUnavailable,
            RuntimeErrorCode::CaptureStartFailed,
        ];
        let terminal = [
            RuntimeErrorCode::MicrophonePermissionDenied,
            RuntimeErrorCode::SystemAudioPermissionDenied,
            RuntimeErrorCode::UnsupportedPlatform,
            RuntimeErrorCode::ModelMissing,
            RuntimeErrorCode::ModelLoadFailed,
            RuntimeErrorCode::ModelUnsupported,
            RuntimeErrorCode::CaptureStopFailed,
            RuntimeErrorCode::Internal,
            RuntimeErrorCode::RuntimeUnavailable,
            RuntimeErrorCode::InvalidRequest,
            RuntimeErrorCode::CaptureAlreadyActive,
            RuntimeErrorCode::CaptureNotActive,
            RuntimeErrorCode::AudioQueueOverflow,
            RuntimeErrorCode::InferenceLagging,
        ];
        for code in recoverable {
            assert_eq!(
                classify_recovery(code),
                RecoveryDecision::Recoverable,
                "{code:?} must be recoverable"
            );
        }
        for code in terminal {
            assert_eq!(
                classify_recovery(code),
                RecoveryDecision::Terminal,
                "{code:?} must be terminal"
            );
        }
        // 18 total variants: 4 recoverable + 14 terminal accounts for
        // every RuntimeErrorCode variant with no gap.
        assert_eq!(recoverable.len() + terminal.len(), 18);
    }

    #[test]
    fn backoff_schedule_matches_frozen_500ms_2s_5s() {
        let policy = RecoveryPolicy::production();
        assert_eq!(policy.backoff_for_attempt(1), Duration::from_millis(500));
        assert_eq!(policy.backoff_for_attempt(2), Duration::from_secs(2));
        assert_eq!(policy.backoff_for_attempt(3), Duration::from_secs(5));
        // Clamped, never panics, for a hypothetical out-of-range attempt.
        assert_eq!(policy.backoff_for_attempt(4), Duration::from_secs(5));
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.healthy_reset, Duration::from_secs(60));
        assert_eq!(policy.starting_watchdog, Duration::from_secs(10));
        assert_eq!(policy.stopping_watchdog, Duration::from_secs(3));
    }

    #[test]
    fn interruptible_wait_completes_without_cancellation() {
        let start = Instant::now();
        let completed =
            interruptible_wait(Duration::from_millis(20), Duration::from_millis(2), || {
                false
            });
        assert!(completed);
        assert!(start.elapsed() >= Duration::from_millis(20));
    }

    #[test]
    fn interruptible_wait_returns_early_on_cancellation() {
        let start = Instant::now();
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag_writer = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            flag_writer.store(true, std::sync::atomic::Ordering::Release);
        });
        let completed =
            interruptible_wait(Duration::from_secs(5), Duration::from_millis(2), || {
                flag.load(std::sync::atomic::Ordering::Acquire)
            });
        assert!(!completed);
        // Cancelled well before the full 5s duration.
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn interruptible_wait_never_sleeps_when_already_cancelled() {
        let start = Instant::now();
        let completed =
            interruptible_wait(Duration::from_secs(5), Duration::from_millis(50), || true);
        assert!(!completed);
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn lag_window_enters_above_20_percent_drop_rate() {
        let mut window = LagWindow::new(Duration::from_millis(10), 20);
        let t0 = Instant::now();
        // Within the window: 70 drained, 30 dropped -> 30% drop rate.
        assert_eq!(window.record_at(t0, 70, 30), None);
        // Window elapses; evaluated on this call.
        let transition = window.record_at(t0 + Duration::from_millis(11), 0, 0);
        assert_eq!(transition, Some(true));
        assert!(window.is_lagging());
    }

    #[test]
    fn lag_window_stays_off_at_or_below_20_percent() {
        let mut window = LagWindow::new(Duration::from_millis(10), 20);
        let t0 = Instant::now();
        // 80 drained, 20 dropped -> exactly 20%, not "exceeds" 20%.
        window.record_at(t0, 80, 20);
        let transition = window.record_at(t0 + Duration::from_millis(11), 0, 0);
        assert_eq!(transition, None);
        assert!(!window.is_lagging());
    }

    #[test]
    fn lag_window_leaves_only_on_a_fully_clean_window() {
        let mut window = LagWindow::new(Duration::from_millis(10), 20);
        let mut t = Instant::now();
        window.record_at(t, 50, 50); // 100% drop -> lagging after eval
        t += Duration::from_millis(11);
        assert_eq!(window.record_at(t, 0, 0), Some(true));

        // A window with a nonzero but sub-threshold drop rate does NOT
        // clear lagging; only a fully 0%-drop window does.
        t += Duration::from_millis(1);
        window.record_at(t, 95, 5);
        t += Duration::from_millis(11);
        assert_eq!(window.record_at(t, 0, 0), None);
        assert!(window.is_lagging());

        t += Duration::from_millis(1);
        window.record_at(t, 100, 0);
        t += Duration::from_millis(11);
        assert_eq!(window.record_at(t, 0, 0), Some(false));
        assert!(!window.is_lagging());
    }

    #[test]
    fn lag_window_does_not_evaluate_before_the_window_elapses() {
        let mut window = LagWindow::new(Duration::from_secs(10), 20);
        let t0 = Instant::now();
        assert_eq!(window.record_at(t0, 0, 100), None);
        assert!(!window.is_lagging());
        assert_eq!(window.record_at(t0 + Duration::from_secs(1), 0, 100), None);
        assert!(!window.is_lagging());
    }
}
