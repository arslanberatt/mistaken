# Mistaken Benchmark Subtree (Spec 05)

This subtree is the Spec 05 deliverable: a reproducible Mistaken-specific
incorrect-English benchmark corpus, a Rust harness (`mistaken-bench`) that
validates the corpus, runs pinned local ASR candidates against it through a
standalone adapter-process protocol, scores the results, renders reports,
and checks a license record. See
`docs/specs/spec-05-asr-benchmark-license-gate.md` for the full spec.

## Layout

```text
benchmarks/
  corpus/
    protocol.md            Recording protocol, taxonomy, consent rule
    manifest.schema.json   JSON Schema for the corpus manifest
    manifest.json          Committed corpus manifest (checksums, durations,
                            error spans) — no audio
    prompts/                122 prompt files, one per primary clip
    clips/                  Git-ignored local audio (never committed)
  harness/                  mistaken-bench Rust crate (own Cargo workspace)
  candidates/                5 frozen candidate descriptor JSON files
  adapters/
    sherpa-onnx/            sherpa-onnx adapter (C++, sherpa-onnx C API)
    whisper-cpp/             whisper.cpp adapter (C++, rolling-window)
  licenses/
    license-record.md        Runtime/weight/provenance license rows
  reports/
    <date>-mac-arm64.md      Per-host gate report
    approval.md               Approval decision or explicit blocker
  models/                   Git-ignored local model weights (never committed)
  runs/                      Git-ignored local run/score artifacts (never committed)
  .vendor/                   Git-ignored pinned upstream runtime source checkouts
```

## Reproducing a run from scratch

### 1. Read the protocol and record (or reuse) the corpus

**Remediation branch note (`spec/05-remediation`):** the corpus audio
committed by the original Spec 05 implementation was synthesized with
macOS `say` TTS, not recorded from a real human speaker, so it never
satisfied `corpus/protocol.md`'s informed-consent recording requirement —
see `benchmarks/reports/approval.md` for the full disclosed blocker. On
this branch, `benchmarks/corpus/manifest.json`'s `speakerProfiles[].notes`
have been reset to `PENDING real human recording`; the prior TTS
`sha256`/`durationMs` values are still present per clip but are stale
placeholders that `validate-corpus` will reject once you try to run
against them — do not treat a currently-clean `validate-corpus` as
possible without real audio. Follow `corpus/protocol.md` (summarized as an
operator checklist in `corpus/RECORDING_CHECKLIST.md`) to record a real
corpus, then run:

```bash
python3 benchmarks/corpus/scripts/register_clips.py
```

to recompute and write the real `sha256`/`durationMs` for every clip file
found under `benchmarks/corpus/clips/` (Git-ignored, never committed), and
manually update `reference`/`errorSpans` for any clip where what was
actually said differs from the prompt, and `speakerProfiles[].consentGiven`
once real informed consent has been given. See `corpus/RECORDING_CHECKLIST.md`
step 1 for the consent rule.

Corpus totals (target, unchanged from the original spec): 122 primary
clips across all 12 conditions, ≥ 20 minutes total, plus an 8-clip 48 kHz
duplicate subset of `mistake-tense-01`..`08` (130 manifest entries total).
Validate with:

```bash
cd benchmarks/harness && cargo build --release
cd ../..
./benchmarks/harness/target/release/mistaken-bench validate-corpus
```

### 2. Build the two adapters (one-time, online staging step)

```bash
# sherpa-onnx
cat benchmarks/adapters/sherpa-onnx/build.md   # follow steps 1-3

# whisper.cpp
cat benchmarks/adapters/whisper-cpp/build.md   # follow steps 1-3
```

Both vendor pinned upstream source into `benchmarks/.vendor/` (Git-ignored,
never modified) and produce
`benchmarks/adapters/<name>/build/<name>-adapter`.

### 3. Fetch and verify model weights (one-time, online staging step)

Download each candidate's declared files under
`benchmarks/models/<candidate-id>/` (see `benchmarks/candidates/*.json` for
exact repository/revision/file list), then:

```bash
for c in sherpa-zipformer-en-2023-06-26-int8-left64 \
         sherpa-zipformer-en-2023-06-26-fp32-left64 \
         sherpa-zipformer-en-20M-2023-02-17-int8 \
         whisper-base-en-ggml \
         whisper-small-en-ggml; do
  ./benchmarks/harness/target/release/mistaken-bench fetch --candidate "$c"
done
```

`fetch` recomputes SHA-256 and byte size for every declared file and
refuses to proceed on any mismatch.

### 4. Run, score, and report (offline)

```bash
BIN=./benchmarks/harness/target/release/mistaken-bench
for c in <candidate-id>...; do
  $BIN run --candidate "$c" --host-profile mac-arm64 --pace asap --repeat 3
  $BIN run --candidate "$c" --host-profile mac-arm64 --pace realtime --repeat 3
done
$BIN score --run benchmarks/runs/<run-dir>          # once per run directory
$BIN report --host-profile mac-arm64 --out benchmarks/reports/<date>-mac-arm64.md \
  --run benchmarks/runs/<run-dir-1> --run benchmarks/runs/<run-dir-2> ...
$BIN licenses --check
```

Disable networking before the measured `run` invocations (Spec 05 section
10). This implementation session could not disable networking system-wide
without disrupting concurrent sibling Wave 2 worktrees sharing the same
host; see `benchmarks/reports/approval.md` for the exact offline-safety
verification method actually used (source inspection plus live
process/file-descriptor checks).

### Two-stream concurrency measurement

```bash
$BIN run --candidate <leading-candidate> --host-profile mac-arm64 \
  --pace asap --repeat 1 --concurrency 2
```

`--concurrency 2` runs two adapter processes concurrently against the same
candidate, matching Spec 05's two-stream resource gate.

## Developer verification (fixtures, no real corpus/model required)

```bash
cd benchmarks/harness
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Known limitations (see `reports/approval.md` for the full record)

1. **Corpus is synthesized, not human-recorded.** The committed manifest's
   audio was produced with macOS `say` TTS as a disclosed substitute for an
   unavailable human operator/speaker. This does not satisfy
   `corpus/protocol.md`'s human-informed-consent requirement.
2. **`win-x64` reference host was never available.** Every measured run in
   this implementation session ran only on `mac-arm64`. Spec 05 requires
   both hosts to pass every gate before any approval.
3. **`realtime` pace ran 1 repetition, not 3**, for time-budget reasons
   (each full-corpus realtime-pace repetition is wall-clock-bound at
   roughly the corpus's own ~21-minute duration). `asap` pace ran the full
   3 repetitions as specified.

Because of (1) and (2), **no candidate is approved**. See
`reports/approval.md` for the complete, honest gate-by-gate record.
