//! Adapter process orchestration: one spawn per clip run, bounded wall-clock
//! timeout, verified termination, no orphaned children (Spec 05 section 11,
//! "Process lifecycle").

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::adapter::protocol::{parse_ndjson_stream, Event, Job, ProtocolError};

#[derive(Debug)]
pub struct ClipRunResult {
    pub events: Vec<Event>,
    pub protocol_errors: Vec<ProtocolError>,
    pub timed_out: bool,
    pub exit_status: Option<i32>,
    pub stderr: String,
    pub wall_time: Duration,
}

/// `max(30 s, 10 * clip duration)` (Spec 05 section 11).
pub fn clip_timeout(clip_duration_ms: u64) -> Duration {
    Duration::from_millis(clip_duration_ms.saturating_mul(10)).max(Duration::from_secs(30))
}

/// Spawn `adapter_path`, write one job on stdin, read NDJSON from stdout
/// until the process exits or `timeout` elapses. On timeout the child is
/// killed and reaped (verified termination) before returning; the caller
/// records the clip as `timeout` per the error taxonomy and continues the
/// remaining clips rather than aborting the whole run.
pub fn run_adapter(
    adapter_path: &Path,
    job: &Job,
    timeout: Duration,
) -> Result<ClipRunResult, String> {
    let start = Instant::now();
    let mut child: Child = Command::new(adapter_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn adapter {}: {e}", adapter_path.display()))?;

    let job_json =
        serde_json::to_string(job).map_err(|e| format!("failed to serialize job: {e}"))?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "adapter stdin not piped".to_string())?;
        stdin
            .write_all(job_json.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .map_err(|e| format!("failed to write job to adapter stdin: {e}"))?;
    }
    // Close stdin so the adapter sees EOF after its one job.
    drop(child.stdin.take());

    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| "adapter stdout not piped".to_string())?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| "adapter stderr not piped".to_string())?;

    // Stdout is consumed continuously on its own thread so a chatty partial
    // stream cannot fill the OS pipe buffer and deadlock the run (section 11,
    // "Bounded buffering").
    let (stdout_tx, stdout_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout_pipe.read_to_string(&mut buf);
        let _ = stdout_tx.send(buf);
    });

    let (stderr_tx, stderr_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        let _ = stderr_tx.send(buf);
    });

    match stdout_rx.recv_timeout(timeout) {
        Ok(stdout_buf) => {
            let exit_status = child
                .wait()
                .map_err(|e| format!("failed to wait on adapter: {e}"))?;
            let stderr_buf = stderr_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_default();
            let (events, protocol_errors) = parse_ndjson_stream(&stdout_buf);
            Ok(ClipRunResult {
                events,
                protocol_errors,
                timed_out: false,
                exit_status: exit_status.code(),
                stderr: stderr_buf,
                wall_time: start.elapsed(),
            })
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Terminate and verify: kill, then wait to reap so no orphaned
            // process is left behind.
            let _ = child.kill();
            let _ = child.wait();
            let stderr_buf = stderr_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap_or_default();
            Ok(ClipRunResult {
                events: Vec::new(),
                protocol_errors: Vec::new(),
                timed_out: true,
                exit_status: None,
                stderr: stderr_buf,
                wall_time: start.elapsed(),
            })
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = child.kill();
            let _ = child.wait();
            Err("adapter stdout reader thread disconnected unexpectedly".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::protocol::{DecodingParams, Pace};

    fn test_job() -> Job {
        Job {
            clip_id: "probe".to_string(),
            wav_path: "/dev/null".to_string(),
            model: serde_json::json!({}),
            decoding: DecodingParams {
                method: "greedy_search".to_string(),
                num_threads: 1,
            },
            pace: Pace::Asap,
            chunk_ms: 100,
            window_ms: None,
            warmup_silence_ms: None,
            initial_prompt: None,
            no_speech_thold: None,
            suppress_nst: None,
        }
    }

    #[test]
    fn clip_timeout_floors_at_30_seconds() {
        assert_eq!(clip_timeout(500), Duration::from_secs(30));
        assert_eq!(clip_timeout(0), Duration::from_secs(30));
    }

    #[test]
    fn clip_timeout_scales_for_long_clips() {
        // 120 s clip -> 1200 s bound, well above the 30 s floor.
        assert_eq!(clip_timeout(120_000), Duration::from_secs(1200));
    }

    #[test]
    fn a_well_behaved_fake_adapter_emits_events_and_exits_cleanly() {
        // A tiny shell script acts as a stand-in adapter: it reads the job
        // line (discarded) and prints a scripted NDJSON stream, matching the
        // real protocol shape.
        let script = "read _job; printf '%s\\n' '{\"type\":\"ready\",\"atMs\":0}' '{\"type\":\"final\",\"atMs\":10,\"text\":\"hi\",\"segmentIndex\":0}'";

        let mut cmd_path = std::env::temp_dir();
        cmd_path.push("mistaken_bench_fake_adapter_test.sh");
        std::fs::write(&cmd_path, format!("#!/bin/sh\n{script}\n")).unwrap();
        let mut perms = std::fs::metadata(&cmd_path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&cmd_path, perms).unwrap();

        let result = run_adapter(&cmd_path, &test_job(), Duration::from_secs(5)).unwrap();
        std::fs::remove_file(&cmd_path).ok();

        assert!(!result.timed_out);
        assert_eq!(result.exit_status, Some(0));
        assert_eq!(result.events.len(), 2);
        assert!(matches!(result.events[0], Event::Ready { .. }));
        assert!(matches!(result.events[1], Event::Final { .. }));
    }

    #[test]
    fn a_hung_adapter_is_killed_and_reaped_on_timeout() {
        let mut cmd_path = std::env::temp_dir();
        cmd_path.push("mistaken_bench_fake_hung_adapter_test.sh");
        std::fs::write(&cmd_path, "#!/bin/sh\nread _job\nsleep 60\n").unwrap();
        let mut perms = std::fs::metadata(&cmd_path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&cmd_path, perms).unwrap();

        let start = Instant::now();
        let result = run_adapter(&cmd_path, &test_job(), Duration::from_millis(300)).unwrap();
        std::fs::remove_file(&cmd_path).ok();

        assert!(result.timed_out);
        assert_eq!(result.exit_status, None);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "timeout should fire promptly, took {:?}",
            start.elapsed()
        );
    }
}
