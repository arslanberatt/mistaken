//! Crate-local probe: proves real WASAPI loopback capture on real hardware
//! using aggregate statistics only (spec section 7, "Developer verification
//! flow"). Never writes audio to disk and never prints a sample value.
//!
//! Usage: `system_audio_probe [seconds]` (default 10). Play known audio while
//! it runs, then stop playback to observe the idle-timeline counters.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows_system_audio::{
    availability, start, SystemAudioConfig, SystemAudioCounters, SystemAudioError,
    SystemAudioFormat, SystemAudioSink,
};

#[derive(Default)]
struct Aggregate {
    format: Option<SystemAudioFormat>,
    blocks: u64,
    samples: u64,
    sum_sq: f64,
    peak: f32,
    last_counters: SystemAudioCounters,
}

struct ProbeSink {
    aggregate: Arc<Mutex<Aggregate>>,
    terminal: Arc<AtomicBool>,
    terminal_error: Arc<Mutex<Option<SystemAudioError>>>,
}

impl SystemAudioSink for ProbeSink {
    fn on_block(&mut self, samples: &[f32], format: SystemAudioFormat) -> bool {
        let Ok(mut agg) = self.aggregate.lock() else {
            return false;
        };
        agg.format = Some(format);
        agg.blocks += 1;
        agg.samples += samples.len() as u64;
        for &sample in samples {
            agg.sum_sq += (sample as f64) * (sample as f64);
            if sample.abs() > agg.peak {
                agg.peak = sample.abs();
            }
        }
        true
    }

    fn on_error(&mut self, error: SystemAudioError) {
        self.terminal.store(true, Ordering::SeqCst);
        if let Ok(mut slot) = self.terminal_error.lock() {
            *slot = Some(error);
        }
    }
}

fn print_report(aggregate: &Aggregate, elapsed: Duration) {
    let (rate, channels, encoding) = match aggregate.format {
        Some(format) => (
            format.sample_rate_hz,
            format.channels,
            format!("{:?}", format.source_encoding),
        ),
        None => (0, 0, "unknown".to_string()),
    };
    let rms = if aggregate.samples > 0 {
        (aggregate.sum_sq / aggregate.samples as f64).sqrt()
    } else {
        0.0
    };
    println!(
        "elapsed_s={:.1} rate_hz={rate} channels={channels} encoding={encoding} \
         blocks={} samples={} windowed_peak={:.4} windowed_rms={:.6}",
        elapsed.as_secs_f64(),
        aggregate.blocks,
        aggregate.samples,
        aggregate.peak,
        rms
    );
    let counters = aggregate.last_counters;
    println!(
        "  counters: captured_frames={} silent_flag_frames={} synthesized_frames={} \
         discontinuity_events={} truncated_gap_events={} dropped_blocks={}",
        counters.captured_frames,
        counters.silent_flag_frames,
        counters.synthesized_frames,
        counters.discontinuity_events,
        counters.truncated_gap_events,
        counters.dropped_blocks
    );
}

fn main() {
    println!(
        "windows-system-audio probe: aggregate statistics only. \
         No audio sample or file is ever written or printed."
    );

    match availability() {
        Ok(()) => println!("availability(): Ok"),
        Err(err) => {
            println!("availability(): {err}");
            std::process::exit(1);
        }
    }

    let run_seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(10);

    let aggregate = Arc::new(Mutex::new(Aggregate::default()));
    let terminal = Arc::new(AtomicBool::new(false));
    let terminal_error: Arc<Mutex<Option<SystemAudioError>>> = Arc::new(Mutex::new(None));

    let sink = ProbeSink {
        aggregate: Arc::clone(&aggregate),
        terminal: Arc::clone(&terminal),
        terminal_error: Arc::clone(&terminal_error),
    };

    let mut session = match start(SystemAudioConfig::default(), Box::new(sink)) {
        Ok(session) => session,
        Err(err) => {
            println!("start() failed: {err}");
            std::process::exit(1);
        }
    };

    println!("capturing for {run_seconds}s (play known audio now, then optionally stop it)...");
    let start_instant = Instant::now();
    while start_instant.elapsed() < Duration::from_secs(run_seconds) {
        std::thread::sleep(Duration::from_secs(1));
        if terminal.load(Ordering::SeqCst) {
            if let Ok(guard) = terminal_error.lock() {
                if let Some(err) = guard.as_ref() {
                    println!("terminal error reported: {err}");
                }
            }
            break;
        }
        if let Ok(mut guard) = aggregate.lock() {
            guard.last_counters = session.counters();
            print_report(&guard, start_instant.elapsed());
        }
    }

    match session.stop() {
        Ok(()) => println!("stop(): Ok"),
        Err(err) => println!("stop(): {err}"),
    }

    if let Ok(mut guard) = aggregate.lock() {
        guard.last_counters = session.counters();
        print_report(&guard, start_instant.elapsed());
    }
    println!("done.");
}
