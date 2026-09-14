//! Adapter configuration (spec section 6, `SystemAudioConfig`).

/// Tunable adapter parameters. Defaults are frozen for reproducible evidence
/// (spec section 6, "Frozen initialization" / "Frozen choices and why").
#[derive(Debug, Clone, Copy)]
pub struct SystemAudioConfig {
    /// Requested endpoint buffer duration in milliseconds. Default 200.
    pub buffer_duration_ms: u32,
    /// Hard cap on synthesized silence per idle gap, in milliseconds. Default
    /// 10 000 (10 seconds).
    pub max_gap_fill_ms: u32,
}

impl Default for SystemAudioConfig {
    fn default() -> Self {
        Self {
            buffer_duration_ms: 200,
            max_gap_fill_ms: 10_000,
        }
    }
}
