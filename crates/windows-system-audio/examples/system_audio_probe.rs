//! Crate-local probe: proves real WASAPI loopback capture on real hardware
//! using aggregate statistics only (spec section 7, "Developer verification
//! flow"). Never writes audio to disk and never prints a sample value.
//!
//! Usage: `system_audio_probe [seconds] [options]` (default 10 seconds).
//! Play known audio while it runs, then stop playback to observe the
//! idle-timeline counters.
//!
//! Options:
//!   `--cycles N`          run N consecutive start/stop cycles, reporting the
//!                         start and stop duration of each (spec section 12,
//!                         criterion 18).
//!   `--sink-queue N`      bound the sink at N pending blocks, so a slow
//!                         consumer makes `on_block` return `false` instead of
//!                         queueing without limit (criterion 15).
//!   `--sink-delay-ms M`   the bounded sink's consumer drains one block every
//!                         M milliseconds. Only meaningful with
//!                         `--sink-queue`.
//!
//! Each per-second line reports the window since the previous line and the
//! session totals separately, so a playback phase and an idle phase can be
//! told apart without inferring anything from cumulative values.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows_system_audio::{
    availability, start, SystemAudioConfig, SystemAudioCounters, SystemAudioError,
    SystemAudioFormat, SystemAudioSink,
};

/// Parsed command line. Defaults reproduce the original single-cycle,
/// accept-everything probe run.
struct ProbeArgs {
    seconds: u64,
    cycles: u32,
    sink_queue_blocks: Option<usize>,
    sink_delay_ms: u64,
}

impl ProbeArgs {
    fn parse() -> Self {
        let mut args = ProbeArgs {
            seconds: 10,
            cycles: 1,
            sink_queue_blocks: None,
            sink_delay_ms: 50,
        };
        let mut positional_seen = false;
        let mut argv = std::env::args().skip(1);
        while let Some(arg) = argv.next() {
            match arg.as_str() {
                "--cycles" => {
                    if let Some(value) = argv.next().and_then(|v| v.parse().ok()) {
                        args.cycles = value;
                    }
                }
                "--sink-queue" => {
                    args.sink_queue_blocks = argv.next().and_then(|v| v.parse().ok());
                }
                "--sink-delay-ms" => {
                    if let Some(value) = argv.next().and_then(|v| v.parse().ok()) {
                        args.sink_delay_ms = value;
                    }
                }
                other if !positional_seen => {
                    if let Ok(value) = other.parse() {
                        args.seconds = value;
                        positional_seen = true;
                    }
                }
                _ => {}
            }
        }
        args
    }
}

/// Aggregate statistics only: block and sample counts, energy, and structural
/// checks. No sample value is ever retained or printed.
struct Aggregate {
    format: Option<SystemAudioFormat>,
    blocks: u64,
    samples: u64,
    sum_sq: f64,
    peak: f32,
    window_blocks: u64,
    window_samples: u64,
    window_sum_sq: f64,
    window_peak: f32,
    min_block_len: usize,
    max_block_len: usize,
    wrong_length_blocks: u64,
    out_of_range_samples: u64,
    last_counters: SystemAudioCounters,
}

impl Default for Aggregate {
    fn default() -> Self {
        Self {
            format: None,
            blocks: 0,
            samples: 0,
            sum_sq: 0.0,
            peak: 0.0,
            window_blocks: 0,
            window_samples: 0,
            window_sum_sq: 0.0,
            window_peak: 0.0,
            min_block_len: usize::MAX,
            max_block_len: 0,
            wrong_length_blocks: 0,
            out_of_range_samples: 0,
            last_counters: SystemAudioCounters::default(),
        }
    }
}

impl Aggregate {
    fn reset_window(&mut self) {
        self.window_blocks = 0;
        self.window_samples = 0;
        self.window_sum_sq = 0.0;
        self.window_peak = 0.0;
    }
}

/// Bounded pending queue plus a deliberately slow consumer. Models the real
/// downstream shape: when the consumer falls behind, `on_block` refuses the
/// block rather than growing without limit.
struct Backpressure {
    queue: Mutex<VecDeque<Vec<f32>>>,
    capacity: usize,
}

struct ProbeSink {
    aggregate: Arc<Mutex<Aggregate>>,
    terminal: Arc<AtomicBool>,
    terminal_error: Arc<Mutex<Option<SystemAudioError>>>,
    backpressure: Option<Arc<Backpressure>>,
}

impl SystemAudioSink for ProbeSink {
    fn on_block(&mut self, samples: &[f32], format: SystemAudioFormat) -> bool {
        let Ok(mut agg) = self.aggregate.lock() else {
            return false;
        };
        agg.format = Some(format);
        agg.blocks += 1;
        agg.window_blocks += 1;
        agg.samples += samples.len() as u64;
        agg.window_samples += samples.len() as u64;
        agg.min_block_len = agg.min_block_len.min(samples.len());
        agg.max_block_len = agg.max_block_len.max(samples.len());
        if samples.len() != (format.sample_rate_hz / 50) as usize {
            agg.wrong_length_blocks += 1;
        }
        for &sample in samples {
            if !sample.is_finite() || !(-1.0..=1.0).contains(&sample) {
                agg.out_of_range_samples += 1;
            }
            agg.sum_sq += (sample as f64) * (sample as f64);
            agg.window_sum_sq += (sample as f64) * (sample as f64);
            let magnitude = sample.abs();
            if magnitude > agg.peak {
                agg.peak = magnitude;
            }
            if magnitude > agg.window_peak {
                agg.window_peak = magnitude;
            }
        }
        drop(agg);

        // The bounded-sink mode's refusal path: full queue means the block is
        // dropped and counted by the adapter, never retried or buffered here.
        if let Some(backpressure) = self.backpressure.as_ref() {
            let Ok(mut queue) = backpressure.queue.lock() else {
                return false;
            };
            if queue.len() >= backpressure.capacity {
                return false;
            }
            queue.push_back(samples.to_vec());
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

fn rms(sum_sq: f64, samples: u64) -> f64 {
    if samples > 0 {
        (sum_sq / samples as f64).sqrt()
    } else {
        0.0
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
    let min_block_len = if aggregate.min_block_len == usize::MAX {
        0
    } else {
        aggregate.min_block_len
    };
    println!(
        "elapsed_s={:.1} rate_hz={rate} channels={channels} encoding={encoding} \
         window_blocks={} window_peak={:.4} window_rms={:.6} | \
         blocks={} samples={} peak={:.4} rms={:.6} \
         block_len_min={min_block_len} block_len_max={} wrong_length_blocks={} \
         out_of_range_samples={}",
        elapsed.as_secs_f64(),
        aggregate.window_blocks,
        aggregate.window_peak,
        rms(aggregate.window_sum_sq, aggregate.window_samples),
        aggregate.blocks,
        aggregate.samples,
        aggregate.peak,
        rms(aggregate.sum_sq, aggregate.samples),
        aggregate.max_block_len,
        aggregate.wrong_length_blocks,
        aggregate.out_of_range_samples
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

/// Runs one complete start → capture → stop cycle. Returns `false` if the
/// session reported a terminal error, so a caller running multiple cycles can
/// stop rather than report meaningless later cycles.
fn run_cycle(cycle: u32, args: &ProbeArgs) -> bool {
    let aggregate = Arc::new(Mutex::new(Aggregate::default()));
    let terminal = Arc::new(AtomicBool::new(false));
    let terminal_error: Arc<Mutex<Option<SystemAudioError>>> = Arc::new(Mutex::new(None));

    let backpressure = args.sink_queue_blocks.map(|capacity| {
        Arc::new(Backpressure {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        })
    });

    // The deliberately slow consumer for the bounded-sink mode.
    let consumer_running = Arc::new(AtomicBool::new(true));
    let consumer = backpressure.as_ref().map(|backpressure| {
        let backpressure = Arc::clone(backpressure);
        let running = Arc::clone(&consumer_running);
        let delay = Duration::from_millis(args.sink_delay_ms);
        std::thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                std::thread::sleep(delay);
                if let Ok(mut queue) = backpressure.queue.lock() {
                    queue.pop_front();
                }
            }
        })
    });

    let sink = ProbeSink {
        aggregate: Arc::clone(&aggregate),
        terminal: Arc::clone(&terminal),
        terminal_error: Arc::clone(&terminal_error),
        backpressure: backpressure.clone(),
    };

    let start_began = Instant::now();
    let mut session = match start(SystemAudioConfig::default(), Box::new(sink)) {
        Ok(session) => session,
        Err(err) => {
            println!("cycle={cycle} start() failed: {err}");
            consumer_running.store(false, Ordering::SeqCst);
            if let Some(handle) = consumer {
                let _ = handle.join();
            }
            return false;
        }
    };
    let start_ms = start_began.elapsed().as_secs_f64() * 1000.0;
    println!(
        "cycle={cycle} start(): Ok in {start_ms:.1} ms — capturing for {}s",
        args.seconds
    );

    let mut clean = true;
    let start_instant = Instant::now();
    while start_instant.elapsed() < Duration::from_secs(args.seconds) {
        std::thread::sleep(Duration::from_secs(1));
        if terminal.load(Ordering::SeqCst) {
            if let Ok(guard) = terminal_error.lock() {
                if let Some(err) = guard.as_ref() {
                    println!("cycle={cycle} terminal error reported: {err}");
                }
            }
            clean = false;
            break;
        }
        if let Ok(mut guard) = aggregate.lock() {
            guard.last_counters = session.counters();
            print_report(&guard, start_instant.elapsed());
            guard.reset_window();
        }
    }

    let stop_began = Instant::now();
    let stop_result = session.stop();
    let stop_ms = stop_began.elapsed().as_secs_f64() * 1000.0;
    match stop_result {
        Ok(()) => println!("cycle={cycle} stop(): Ok in {stop_ms:.1} ms"),
        Err(err) => {
            println!("cycle={cycle} stop(): {err} (after {stop_ms:.1} ms)");
            clean = false;
        }
    }

    // A second stop() must be a no-op rather than an error or a second
    // teardown (spec section 11, idempotent teardown).
    match session.stop() {
        Ok(()) => println!("cycle={cycle} stop() again: Ok (idempotent)"),
        Err(err) => {
            println!("cycle={cycle} stop() again: {err}");
            clean = false;
        }
    }

    if let Ok(mut guard) = aggregate.lock() {
        guard.last_counters = session.counters();
        print_report(&guard, start_instant.elapsed());
    }

    consumer_running.store(false, Ordering::SeqCst);
    if let Some(handle) = consumer {
        let _ = handle.join();
    }
    clean
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

    let args = ProbeArgs::parse();
    if let Some(capacity) = args.sink_queue_blocks {
        println!(
            "bounded sink: capacity={capacity} blocks, consumer drains one every {} ms",
            args.sink_delay_ms
        );
    }

    for cycle in 1..=args.cycles {
        if !run_cycle(cycle, &args) {
            println!("stopping after cycle {cycle}: the session did not end cleanly");
            std::process::exit(1);
        }
        if cycle < args.cycles {
            std::thread::sleep(Duration::from_millis(300));
        }
    }
    println!("done.");
}
