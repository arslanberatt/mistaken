//! `mistaken-bench` CLI (Spec 05 section 6, "Produced — harness CLI").

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};

use mistaken_bench::adapter::process::{clip_timeout, run_adapter};
use mistaken_bench::adapter::protocol::{DecodingParams, Job, Pace as ProtocolPace};
use mistaken_bench::candidate::{
    all_verified, verify_model_files, CandidateDescriptor, FetchOutcome,
};
use mistaken_bench::manifest::Manifest;
use mistaken_bench::report::{check_license_record, render_host_report, CandidateReportSection};
use mistaken_bench::run_record::{percentile, score_clip, ClipRunRecord, ScoredAggregate};
use mistaken_bench::scoring::gates::{evaluate_gates, CandidateHostMetrics};

#[derive(Parser)]
#[command(
    name = "mistaken-bench",
    version,
    about = "Spec 05 local ASR benchmark harness"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum, Debug, PartialEq, Eq)]
enum PaceArg {
    Realtime,
    Asap,
}

impl From<PaceArg> for ProtocolPace {
    fn from(p: PaceArg) -> Self {
        match p {
            PaceArg::Realtime => ProtocolPace::Realtime,
            PaceArg::Asap => ProtocolPace::Asap,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Validate the committed corpus manifest against its schema rules and
    /// the local (Git-ignored) clip files.
    ValidateCorpus {
        #[arg(long, default_value = "benchmarks/corpus")]
        corpus_dir: PathBuf,
    },
    /// Recompute SHA-256 and byte size for a candidate's local model files
    /// and refuse to proceed on any mismatch.
    Fetch {
        #[arg(long)]
        candidate: String,
        #[arg(long, default_value = "benchmarks/candidates")]
        candidates_dir: PathBuf,
        #[arg(long, default_value = "benchmarks/models")]
        models_dir: PathBuf,
    },
    /// Run one candidate against the full corpus, one pace, `--repeat`
    /// repetitions, writing immutable per-clip run records.
    Run {
        #[arg(long)]
        candidate: String,
        #[arg(long)]
        host_profile: String,
        #[arg(long, value_enum)]
        pace: PaceArg,
        #[arg(long, default_value_t = 3)]
        repeat: u32,
        #[arg(long, default_value = "benchmarks/corpus")]
        corpus_dir: PathBuf,
        #[arg(long, default_value = "benchmarks/candidates")]
        candidates_dir: PathBuf,
        #[arg(long, default_value = "benchmarks/models")]
        models_dir: PathBuf,
        #[arg(long, default_value = "benchmarks/adapters")]
        adapters_dir: PathBuf,
        #[arg(long, default_value = "benchmarks/runs")]
        runs_dir: PathBuf,
        /// Restrict the run to clips of this condition (used for the
        /// two-stream concurrency measurement and for targeted re-runs).
        #[arg(long)]
        condition: Option<String>,
        /// Run this many adapter processes concurrently (two-stream
        /// concurrency measurement uses 2).
        #[arg(long, default_value_t = 1)]
        concurrency: u32,
    },
    /// Score a run directory's per-clip records against the manifest.
    Score {
        #[arg(long)]
        run: PathBuf,
        #[arg(long, default_value = "benchmarks/corpus")]
        corpus_dir: PathBuf,
    },
    /// Render a host report from one or more scored run directories.
    Report {
        #[arg(long)]
        host_profile: String,
        #[arg(long)]
        out: PathBuf,
        /// Scored run directories to include, in the order they should
        /// appear in the report.
        #[arg(long = "run", required = true)]
        runs: Vec<PathBuf>,
        #[arg(long, default_value = "benchmarks/candidates")]
        candidates_dir: PathBuf,
    },
    /// Check `benchmarks/licenses/license-record.md` for completeness.
    Licenses {
        #[arg(long)]
        check: bool,
        #[arg(long, default_value = "benchmarks/licenses/license-record.md")]
        file: PathBuf,
        #[arg(long, default_value = "benchmarks/candidates")]
        candidates_dir: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::ValidateCorpus { corpus_dir } => cmd_validate_corpus(&corpus_dir),
        Command::Fetch {
            candidate,
            candidates_dir,
            models_dir,
        } => cmd_fetch(&candidate, &candidates_dir, &models_dir),
        Command::Run {
            candidate,
            host_profile,
            pace,
            repeat,
            corpus_dir,
            candidates_dir,
            models_dir,
            adapters_dir,
            runs_dir,
            condition,
            concurrency,
        } => cmd_run(
            &candidate,
            &host_profile,
            pace.into(),
            repeat,
            &corpus_dir,
            &candidates_dir,
            &models_dir,
            &adapters_dir,
            &runs_dir,
            condition.as_deref(),
            concurrency,
        ),
        Command::Score { run, corpus_dir } => cmd_score(&run, &corpus_dir),
        Command::Report {
            host_profile,
            out,
            runs,
            candidates_dir,
        } => cmd_report(&host_profile, &out, &runs, &candidates_dir),
        Command::Licenses {
            check,
            file,
            candidates_dir,
        } => cmd_licenses(check, &file, &candidates_dir),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_validate_corpus(corpus_dir: &Path) -> Result<(), String> {
    let manifest = Manifest::load(&corpus_dir.join("manifest.json"))?;
    let errors = manifest.validate_with_files(corpus_dir);
    if errors.is_empty() {
        let total_clips = manifest
            .clips
            .iter()
            .filter(|c| !c.id.ends_with("-48k"))
            .count();
        let total_ms: u64 = manifest
            .clips
            .iter()
            .filter(|c| !c.id.ends_with("-48k"))
            .map(|c| c.duration_ms)
            .sum();
        println!(
            "validate-corpus: PASS ({total_clips} primary clips, {:.1} minutes, {} total manifest entries)",
            total_ms as f64 / 60000.0,
            manifest.clips.len()
        );
        Ok(())
    } else {
        for e in &errors {
            println!("validate-corpus: FAIL {e}");
        }
        Err(format!("{} validation error(s)", errors.len()))
    }
}

fn cmd_fetch(candidate: &str, candidates_dir: &Path, models_dir: &Path) -> Result<(), String> {
    let descriptor = CandidateDescriptor::load(&candidates_dir.join(format!("{candidate}.json")))?;
    let outcomes = verify_model_files(&descriptor, &models_dir.join(candidate));
    for (file, outcome) in descriptor.model.files.iter().zip(outcomes.iter()) {
        match outcome {
            FetchOutcome::Verified => println!("fetch: VERIFIED {}", file.path),
            FetchOutcome::NoChecksumRecorded { .. } => {
                println!(
                    "fetch: SIZE-ONLY {} (no sha256 declared in descriptor)",
                    file.path
                )
            }
            other => println!("fetch: FAIL {} -> {other:?}", file.path),
        }
    }
    if all_verified(&outcomes) {
        Ok(())
    } else {
        Err(format!(
            "candidate '{candidate}' failed local file verification"
        ))
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_run(
    candidate: &str,
    host_profile: &str,
    pace: ProtocolPace,
    repeat: u32,
    corpus_dir: &Path,
    candidates_dir: &Path,
    models_dir: &Path,
    adapters_dir: &Path,
    runs_dir: &Path,
    condition_filter: Option<&str>,
    concurrency: u32,
) -> Result<(), String> {
    let descriptor = CandidateDescriptor::load(&candidates_dir.join(format!("{candidate}.json")))?;
    let manifest = Manifest::load(&corpus_dir.join("manifest.json"))?;

    let adapter_path = adapters_dir
        .join(&descriptor.adapter)
        .join("build")
        .join(format!("{}-adapter", descriptor.adapter));
    if !adapter_path.is_file() {
        return Err(format!(
            "adapter executable missing: {} (see benchmarks/adapters/{}/build.md)",
            adapter_path.display(),
            descriptor.adapter
        ));
    }

    let candidate_model_dir = models_dir.join(candidate);
    let fetch_outcomes = verify_model_files(&descriptor, &candidate_model_dir);
    if !all_verified(&fetch_outcomes) {
        return Err(format!(
            "candidate '{candidate}' model files are not verified locally; run `mistaken-bench fetch --candidate {candidate}` first ({fetch_outcomes:?})"
        ));
    }

    let pace_dir = match pace {
        ProtocolPace::Realtime => "realtime",
        ProtocolPace::Asap => "asap",
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let run_dir = runs_dir.join(format!("{now}-{candidate}-{host_profile}-{pace_dir}"));
    fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;

    let clips: Vec<_> = manifest
        .clips
        .iter()
        .filter(|c| {
            condition_filter
                .map(|cond| c.condition == cond)
                .unwrap_or(true)
        })
        .collect();

    println!(
        "run: candidate={candidate} host={host_profile} pace={pace_dir} repeat={repeat} clips={} concurrency={concurrency}",
        clips.len()
    );

    for rep in 0..repeat {
        let rep_dir = run_dir.join(format!("rep-{rep}"));
        let clip_chunks: Vec<&[&mistaken_bench::manifest::Clip]> =
            clips.chunks(concurrency.max(1) as usize).collect();
        for (chunk_index, chunk) in clip_chunks.into_iter().enumerate() {
            let mut handles = Vec::new();
            for clip in chunk.iter().copied() {
                let clip_path = corpus_dir.join(&clip.file);
                let job = Job {
                    clip_id: clip.id.clone(),
                    wav_path: clip_path.to_string_lossy().to_string(),
                    model: {
                        let mut model_json =
                            serde_json::to_value(&descriptor.model).unwrap_or_default();
                        if let serde_json::Value::Object(map) = &mut model_json {
                            map.insert(
                                "modelDir".to_string(),
                                serde_json::Value::String(
                                    candidate_model_dir.to_string_lossy().to_string(),
                                ),
                            );
                        }
                        model_json
                    },
                    decoding: DecodingParams {
                        method: descriptor.decoding.method.clone(),
                        num_threads: descriptor.decoding.num_threads,
                    },
                    pace,
                    chunk_ms: 100,
                    window_ms: if descriptor.adapter == "whisper-cpp" {
                        Some(5000)
                    } else {
                        None
                    },
                };
                let timeout = clip_timeout(clip.duration_ms);
                let adapter_path = adapter_path.clone();
                handles.push((clip.clone(), thread_run(adapter_path, job, timeout)));
            }
            for (clip, handle) in handles {
                let outcome = handle
                    .join()
                    .unwrap_or_else(|_| Err("adapter thread panicked".to_string()));
                let record = match outcome {
                    Ok(result) => ClipRunRecord {
                        clip_id: clip.id.clone(),
                        candidate_id: candidate.to_string(),
                        host_profile: host_profile.to_string(),
                        pace,
                        repeat_index: rep,
                        concurrency,
                        chunk_index,
                        events: result.events,
                        timed_out: result.timed_out,
                        exit_status: result.exit_status,
                    },
                    Err(e) => {
                        println!("run: FAIL clip={} rep={rep}: {e}", clip.id);
                        continue;
                    }
                };
                record.save(&rep_dir)?;
            }
        }
        println!("run: repetition {rep} complete -> {}", rep_dir.display());
    }

    println!("run: complete -> {}", run_dir.display());
    Ok(())
}

fn thread_run(
    adapter_path: PathBuf,
    job: Job,
    timeout: Duration,
) -> std::thread::JoinHandle<Result<mistaken_bench::adapter::process::ClipRunResult, String>> {
    std::thread::spawn(move || run_adapter(&adapter_path, &job, timeout))
}

fn cmd_score(run_dir: &Path, corpus_dir: &Path) -> Result<(), String> {
    let manifest = Manifest::load(&corpus_dir.join("manifest.json"))?;
    let mut scored = Vec::new();
    let mut contributed = 0usize;
    let mut attempted = 0usize;

    for rep_entry in fs::read_dir(run_dir).map_err(|e| e.to_string())? {
        let rep_entry = rep_entry.map_err(|e| e.to_string())?;
        if !rep_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        for clip_entry in fs::read_dir(rep_entry.path()).map_err(|e| e.to_string())? {
            let clip_entry = clip_entry.map_err(|e| e.to_string())?;
            let path = clip_entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let record = ClipRunRecord::load(&path)?;
            let clip = manifest
                .clips
                .iter()
                .find(|c| c.id == record.clip_id)
                .ok_or_else(|| {
                    format!("run record references unknown clip '{}'", record.clip_id)
                })?;
            attempted += 1;
            let score = score_clip(clip, &record);
            if score.contributed {
                contributed += 1;
            }
            scored.push(score);
        }
    }

    let out_path = run_dir.join("scored.json");
    fs::write(
        &out_path,
        serde_json::to_string_pretty(&scored).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    println!(
        "score: {contributed}/{attempted} clips contributed -> {}",
        out_path.display()
    );
    if attempted == 0 {
        return Err("no run records found to score".to_string());
    }
    Ok(())
}

/// One run directory's scored aggregate plus the pace it was measured at,
/// keyed by candidate id so `asap` (RTF/WER/MPR/hallucination/resource/
/// size — Spec 05 ties these to `asap` pace explicitly) and `realtime`
/// (first-partial/final-after-endpoint latency — meaningless under `asap`
/// feeding, since audio is fed far faster than real time) contribute to
/// one merged report row per candidate instead of two separate rows.
struct PaceAggregate {
    agg: ScoredAggregate,
    total_edits: u64,
    total_ref: u64,
    total_edits_fluent: u64,
    total_ref_fluent: u64,
}

/// Two-stream concurrency measurement summary for one candidate (Spec 05
/// section 12 AC13, required only for the leading candidate): mean RTF
/// under two-process contention, plus a combined peak-RSS approximation.
/// Each adapter process reports its own `getrusage` high-water mark; the
/// harness has no wall-clock-aligned combined sampler, so the combined
/// figure sums the per-process peaks of clips that ran in the same
/// concurrent chunk (`ClipRunRecord::chunk_index`) and takes the maximum
/// such sum across chunks — a real, slightly conservative upper bound
/// from measured data, never a copy of the single-stream number.
struct TwoStreamAggregate {
    mean_rtf: f64,
    peak_combined_rss_bytes: u64,
}

fn aggregate_two_stream(scores: &[mistaken_bench::run_record::ClipScore]) -> TwoStreamAggregate {
    let rtfs: Vec<f64> = scores.iter().filter_map(|s| s.rtf).collect();
    let mean_rtf = if rtfs.is_empty() {
        0.0
    } else {
        rtfs.iter().sum::<f64>() / rtfs.len() as f64
    };
    let mut by_chunk: BTreeMap<usize, u64> = BTreeMap::new();
    for score in scores {
        if let Some(rss) = score.peak_rss_bytes {
            *by_chunk.entry(score.chunk_index).or_insert(0) += rss;
        }
    }
    let peak_combined_rss_bytes = by_chunk.values().copied().max().unwrap_or(0);
    TwoStreamAggregate {
        mean_rtf,
        peak_combined_rss_bytes,
    }
}

fn aggregate_scores(scores: &[mistaken_bench::run_record::ClipScore]) -> PaceAggregate {
    let mut agg = ScoredAggregate::new();
    let mut total_edits = 0u64;
    let mut total_ref = 0u64;
    let mut total_edits_fluent = 0u64;
    let mut total_ref_fluent = 0u64;
    for score in scores {
        agg.attempted += 1;
        if !score.contributed {
            continue;
        }
        agg.contributed += 1;
        total_edits += score.edit_distance as u64;
        total_ref += score.ref_token_count as u64;
        if score.condition == "fluent-control" {
            total_edits_fluent += score.edit_distance as u64;
            total_ref_fluent += score.ref_token_count as u64;
        }
        for outcome in &score.mpr_outcomes {
            match outcome.as_str() {
                "preserved" => agg.mpr_overall.preserved += 1,
                "false_correction" => agg.mpr_overall.false_corrections += 1,
                _ => agg.mpr_overall.misrecognized += 1,
            }
            if score.condition == "mistake-tense" {
                match outcome.as_str() {
                    "preserved" => agg.mpr_mistake_tense.preserved += 1,
                    "false_correction" => agg.mpr_mistake_tense.false_corrections += 1,
                    _ => agg.mpr_mistake_tense.misrecognized += 1,
                }
            }
            if score.condition == "mistake-minimal-pair" {
                match outcome.as_str() {
                    "preserved" => agg.mpr_mistake_minimal_pair.preserved += 1,
                    "false_correction" => agg.mpr_mistake_minimal_pair.false_corrections += 1,
                    _ => agg.mpr_mistake_minimal_pair.misrecognized += 1,
                }
            }
        }
        if score.condition == "noise-silence" {
            agg.silence_hallucination_tokens += score.silence_hallucination_tokens as u64;
            agg.noise_insertion_tokens += score.inserted_tokens as u64;
            agg.noise_audio_ms += score.audio_ms;
        }
        if let Some(ms) = score.first_partial_at_ms {
            agg.first_partial_latencies_ms.push(ms);
        }
        if let Some(ms) = score.final_after_endpoint_ms {
            agg.final_after_endpoint_latencies_ms.push(ms);
        }
        if let Some(rtf) = score.rtf {
            agg.rtfs.push(rtf);
        }
        if let Some(rss) = score.peak_rss_bytes {
            agg.peak_rss_bytes.push(rss);
        }
    }
    PaceAggregate {
        agg,
        total_edits,
        total_ref,
        total_edits_fluent,
        total_ref_fluent,
    }
}

/// Extract `(candidate_id, pace)` from a run directory name
/// (`<timestamp>-<candidate>-<host>-<pace>`).
fn candidate_and_pace_from_dir(dir: &Path, host_profile: &str) -> Result<(String, String), String> {
    let dir_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid run directory name")?;
    let rest = dir_name
        .split_once('-')
        .map(|(_, rest)| rest)
        .ok_or_else(|| format!("cannot parse run directory name '{dir_name}'"))?;
    let (candidate_id, pace) = rest
        .rsplit_once(&format!("-{host_profile}-"))
        .ok_or_else(|| {
            format!("run directory '{dir_name}' does not match host '{host_profile}'")
        })?;
    Ok((candidate_id.to_string(), pace.to_string()))
}

fn cmd_report(
    host_profile: &str,
    out: &Path,
    run_dirs: &[PathBuf],
    candidates_dir: &Path,
) -> Result<(), String> {
    #[derive(Default)]
    struct CandidateRunData {
        asap: Option<PaceAggregate>,
        realtime: Option<PaceAggregate>,
        two_stream: Option<TwoStreamAggregate>,
    }
    let mut by_candidate: BTreeMap<String, CandidateRunData> = BTreeMap::new();

    for run_dir in run_dirs {
        let scored_path = run_dir.join("scored.json");
        let raw = fs::read_to_string(&scored_path).map_err(|e| {
            format!(
                "read {}: {e} (run `mistaken-bench score` first)",
                scored_path.display()
            )
        })?;
        let scores: Vec<mistaken_bench::run_record::ClipScore> =
            serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let (candidate_id, pace) = candidate_and_pace_from_dir(run_dir, host_profile)?;
        let entry = by_candidate.entry(candidate_id).or_default();
        // A run's own recorded concurrency (not its directory name) decides
        // whether it is the two-stream measurement: `--concurrency 2` can
        // be passed under either pace, and must never silently overwrite
        // that pace's single-stream aggregate.
        let concurrency = scores.first().map(|s| s.concurrency).unwrap_or(1);
        if concurrency > 1 {
            entry.two_stream = Some(aggregate_two_stream(&scores));
            continue;
        }
        let paced = aggregate_scores(&scores);
        match pace.as_str() {
            "asap" => entry.asap = Some(paced),
            "realtime" => entry.realtime = Some(paced),
            other => return Err(format!("unknown pace '{other}' in run directory name")),
        }
    }

    let mut sections = Vec::new();
    for (
        candidate_id,
        CandidateRunData {
            asap,
            realtime,
            two_stream,
        },
    ) in by_candidate
    {
        let descriptor =
            CandidateDescriptor::load(&candidates_dir.join(format!("{candidate_id}.json"))).ok();

        let wer_of = |p: &PaceAggregate| {
            let overall = if p.total_ref == 0 {
                0.0
            } else {
                p.total_edits as f64 / p.total_ref as f64
            };
            let fluent = if p.total_ref_fluent == 0 {
                0.0
            } else {
                p.total_edits_fluent as f64 / p.total_ref_fluent as f64
            };
            (overall, fluent)
        };

        let (
            wer_overall,
            wer_fluent,
            mpr,
            mpr_tense,
            mpr_pair,
            fc_rate,
            silence_tok,
            noise_ips,
            rtf,
            rss,
            contributing,
            attempted_total,
            contributed_total,
        ) = match &asap {
            Some(p) => {
                let (o, f) = wer_of(p);
                (
                    o,
                    f,
                    p.agg.mpr_overall.mpr(),
                    p.agg.mpr_mistake_tense.mpr(),
                    p.agg.mpr_mistake_minimal_pair.mpr(),
                    p.agg.mpr_overall.false_correction_rate(),
                    p.agg.silence_hallucination_tokens,
                    if p.agg.noise_audio_ms == 0 {
                        0.0
                    } else {
                        p.agg.noise_insertion_tokens as f64 / (p.agg.noise_audio_ms as f64 / 1000.0)
                    },
                    p.agg.rtfs.iter().sum::<f64>() / p.agg.rtfs.len().max(1) as f64,
                    p.agg.peak_rss_bytes.iter().copied().max().unwrap_or(0),
                    if p.agg.attempted == 0 {
                        0.0
                    } else {
                        p.agg.contributed as f64 / p.agg.attempted as f64
                    },
                    p.agg.attempted,
                    p.agg.contributed,
                )
            }
            None => (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0.0, 0, 0.0, 0, 0),
        };

        let (median_latency, p95_latency) = match &realtime {
            Some(p) => (
                Some(percentile(
                    p.agg
                        .first_partial_latencies_ms
                        .iter()
                        .map(|v| *v as f64)
                        .collect(),
                    50.0,
                )),
                Some(percentile(
                    p.agg
                        .final_after_endpoint_latencies_ms
                        .iter()
                        .map(|v| (*v).max(0) as f64)
                        .collect(),
                    95.0,
                )),
            ),
            None => (None, None),
        };

        let metrics = CandidateHostMetrics {
            mpr_overall: mpr,
            false_correction_rate: fc_rate,
            mpr_mistake_tense: mpr_tense,
            mpr_mistake_minimal_pair: mpr_pair,
            wer_overall,
            wer_fluent_control: wer_fluent,
            silence_hallucination_tokens: silence_tok,
            noise_insertion_tokens_per_second: noise_ips,
            median_first_partial_latency_ms: median_latency,
            p95_final_after_endpoint_latency_ms: p95_latency,
            rtf_single_stream: rtf,
            rtf_two_stream: two_stream.as_ref().map(|t| t.mean_rtf),
            peak_rss_single_stream_bytes: rss,
            peak_rss_two_stream_bytes: two_stream.as_ref().map(|t| t.peak_combined_rss_bytes),
            sustained_cpu_fraction_long_turn: 0.0,
            rss_growth_after_30s_fraction: None,
            payload_bytes_uncompressed: descriptor.as_ref().map(|d| d.payload_bytes).unwrap_or(0),
            contributing_clip_fraction: contributing,
        };

        let gates = evaluate_gates(&metrics);
        let pace_note = format!(
            "asap: {contributed_total}/{attempted_total} clips contributed{}{}{}",
            if asap.is_some() {
                ""
            } else {
                " (asap pace not run)"
            },
            if realtime.is_some() {
                "; realtime: latency measured".to_string()
            } else {
                "; realtime pace not run (latency not measured)".to_string()
            },
            if two_stream.is_some() {
                "; two-stream concurrency measured".to_string()
            } else {
                "; two-stream concurrency not measured".to_string()
            }
        );
        sections.push(CandidateReportSection {
            candidate_id: candidate_id.clone(),
            host_profile: host_profile.to_string(),
            pace_note,
            gates,
            blocked_reason: None,
            simulated_streaming: descriptor
                .as_ref()
                .map(|d| d.streaming_mode == "simulated-rolling-window")
                .unwrap_or(false),
        });
    }

    let rendered = render_host_report(host_profile, &sections);
    fs::write(out, &rendered).map_err(|e| e.to_string())?;
    println!("report: wrote {}", out.display());
    Ok(())
}

fn cmd_licenses(check: bool, file: &Path, candidates_dir: &Path) -> Result<(), String> {
    if !check {
        return Err("licenses: pass --check".to_string());
    }
    let mut known_ids = Vec::new();
    for entry in fs::read_dir(candidates_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
            if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                known_ids.push(stem.to_string());
            }
        }
    }
    known_ids.sort();

    let markdown = fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
    match check_license_record(&markdown, &known_ids) {
        Ok(candidates) => {
            println!(
                "licenses: PASS ({} candidate(s) complete)",
                candidates.len()
            );
            for c in candidates {
                println!("licenses: PASS {c}");
            }
            Ok(())
        }
        Err(issues) => {
            for issue in &issues {
                println!("licenses: FAIL [{}] {}", issue.candidate_id, issue.rule);
            }
            Err(format!("{} license record issue(s)", issues.len()))
        }
    }
}
