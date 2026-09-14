//! Real-hardware developer probe for the macOS system-audio adapter.
//!
//! Proves capture on a real Mac using aggregate signal statistics only:
//! block counts, negotiated rate, peak/RMS per one-second window, and
//! drop/rejection counters. It never reads, writes, or prints raw audio
//! samples, and it never touches a frame/pixel/image API.
//!
//! Usage: `cargo run --example system_audio_probe [seconds_per_window]`
//! (default 8 seconds). Play known local audio during the first window to
//! observe non-silent peak/RMS, then let it continue into silence to
//! confirm blocks keep arriving at a flat rate with the peak falling to
//! the silence floor.

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use macos_system_audio::{
    permission_status, start, SystemAudioConfig, SystemAudioError, SystemAudioFormat,
    SystemAudioSink,
};

#[derive(Default, Clone, Copy)]
struct WindowStats {
    block_count: u64,
    peak: f32,
    sum_sq: f64,
    sample_count: u64,
}

struct ProbeSink {
    format: Arc<Mutex<Option<SystemAudioFormat>>>,
    window: Arc<Mutex<WindowStats>>,
    error_count: Arc<Mutex<u64>>,
    last_error: Arc<Mutex<Option<SystemAudioError>>>,
}

impl SystemAudioSink for ProbeSink {
    fn on_block(&mut self, samples: &[f32], format: SystemAudioFormat) -> bool {
        *self.format.lock() = Some(format);
        let mut w = self.window.lock();
        w.block_count += 1;
        for &s in samples {
            let abs = s.abs();
            if abs > w.peak {
                w.peak = abs;
            }
            w.sum_sq += f64::from(s) * f64::from(s);
            w.sample_count += 1;
        }
        true
    }

    fn on_error(&mut self, error: SystemAudioError) {
        eprintln!("[probe] on_error: {error}");
        *self.error_count.lock() += 1;
        *self.last_error.lock() = Some(error);
    }
}

/// Resident set size of this process in kilobytes, read via `ps` so the
/// probe needs no extra dependency for a diagnostic-only measurement.
fn resident_kb() -> Option<u64> {
    let pid = std::process::id();
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

/// Thread count of this process, read via `ps -M` (one row per thread plus
/// a header row on macOS).
fn thread_count() -> Option<u64> {
    let pid = std::process::id();
    let output = Command::new("ps")
        .args(["-M", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let rows = text.lines().count();
    (rows as u64).checked_sub(1)
}

fn run_capture_window(label: &str, duration: Duration) -> Result<(), SystemAudioError> {
    let format_slot: Arc<Mutex<Option<SystemAudioFormat>>> = Arc::new(Mutex::new(None));
    let window = Arc::new(Mutex::new(WindowStats::default()));
    let error_count = Arc::new(Mutex::new(0u64));
    let last_error = Arc::new(Mutex::new(None));

    let sink = ProbeSink {
        format: format_slot.clone(),
        window: window.clone(),
        error_count: error_count.clone(),
        last_error: last_error.clone(),
    };

    let mut session = start(SystemAudioConfig::default(), Box::new(sink))?;
    println!(
        "[probe] {label}: capture started (rss={:?} kB, threads={:?})",
        resident_kb(),
        thread_count()
    );

    let started = Instant::now();
    let mut last_report = Instant::now();
    while started.elapsed() < duration {
        std::thread::sleep(Duration::from_millis(200));
        if last_report.elapsed() >= Duration::from_secs(1) {
            last_report = Instant::now();
            let snapshot = {
                let mut w = window.lock();
                let snap = *w;
                *w = WindowStats::default();
                snap
            };
            let rms = if snapshot.sample_count > 0 {
                (snapshot.sum_sq / snapshot.sample_count as f64).sqrt()
            } else {
                0.0
            };
            let fmt = *format_slot.lock();
            println!(
                "[probe] {label}: blocks={:>4} rate_hz={:?} peak={:.5} rms={:.5} dropped_total={} non_audio_total={} format_rejections_total={}",
                snapshot.block_count,
                fmt.map(|f| f.sample_rate_hz),
                snapshot.peak,
                rms,
                session.dropped_blocks(),
                session.non_audio_deliveries(),
                session.format_rejections(),
            );
        }
    }

    session.stop()?;
    println!(
        "[probe] {label}: capture stopped (rss={:?} kB, threads={:?}); totals: dropped_blocks={} non_audio_deliveries={} format_rejections={} on_error_calls={}",
        resident_kb(),
        thread_count(),
        session.dropped_blocks(),
        session.non_audio_deliveries(),
        session.format_rejections(),
        *error_count.lock(),
    );
    if let Some(e) = session.last_stream_stopped_error() {
        println!("[probe] {label}: SCStreamDelegate reported stream stopped: {e}");
    }
    if let Some(e) = last_error.lock().take() {
        println!("[probe] {label}: last on_error observed: {e}");
    }
    Ok(())
}

fn main() {
    println!("=== Mistaken macOS system-audio probe ===");
    println!(
        "[probe] permission_status() (non-prompting): {:?}",
        permission_status()
    );
    println!("[probe] initial rss={:?} kB", resident_kb());

    let duration_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);

    println!("\n[probe] --- cycle 1/5: play known local audio now ---");
    if let Err(e) = run_capture_window("cycle-1", Duration::from_secs(duration_secs)) {
        eprintln!(
            "[probe] start failed: kind={:?} detail={}",
            e.kind, e.detail
        );
        std::process::exit(1);
    }

    for cycle in 2..=5 {
        println!("\n[probe] --- cycle {cycle}/5: exercising Start -> Stop -> Start ---");
        let label = format!("cycle-{cycle}");
        if let Err(e) = run_capture_window(&label, Duration::from_secs(2)) {
            eprintln!(
                "[probe] cycle {cycle} failed: kind={:?} detail={}",
                e.kind, e.detail
            );
            std::process::exit(1);
        }
    }

    println!("\n[probe] final rss={:?} kB", resident_kb());
    println!("[probe] done: 5/5 Start -> Stop cycles completed.");
}
