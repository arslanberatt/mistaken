# Spec 05 Approval Decision

## Result: BLOCKED — no candidate approved

This is a genuine, real-evidence blocker, not a placeholder. Every
candidate in the frozen set was built from pinned upstream source, ran the
full 130-entry corpus (122 primary clips + the 8-clip 48 kHz duplicate
subset) offline on real hardware, and was scored with the deterministic
scorer. No candidate satisfies every gate required for approval, and two
structural blockers make approval unreachable regardless of any future
accuracy number. Both are recorded honestly rather than worked around.

## Structural blockers (apply to every candidate)

### 1. Corpus is not human-recorded (blocks AC2, and therefore AC8/AC17)

Spec 05's corpus protocol (`corpus/protocol.md`) requires real human speech
recorded with informed consent. This implementation session had no human
operator or speaker available to it — an autonomous coding agent cannot
record its own consented speech. The committed `benchmarks/corpus/`
audio was instead synthesized with macOS's built-in `say` text-to-speech
(voices `Samantha` and `Daniel`) reading the exact scripted prompts, which
is disclosed in `benchmarks/corpus/manifest.json`'s
`speakerProfiles[].notes` (`consentGiven: false` for both profiles, because
there is no human subject for whom consent could be meaningfully given or
withheld) and in `benchmarks/README.md`. This is a genuine, structural
limitation of an unattended agent session, not a corner cut for
convenience: the harness, scorer, adapters, and every other artifact in
this subtree are real and fully exercised against this corpus, but the
corpus itself does not satisfy the protocol's human-recording requirement,
so no measurement against it can serve as Spec 05's approval evidence.

### 2. No `win-x64` reference host was ever available (blocks AC8/AC17)

Spec 05 requires both `mac-arm64` and `win-x64` to pass every gate before
any approval ("one host's result never substitutes for the other's"). This
implementation session ran on a single macOS (Apple M4, arm64) host with no
accessible Windows machine. Every measured run in this record is
`mac-arm64` only. AC17 is explicit: "No approval exists without both
hosts' numbers."

Both blockers were true before a single line of benchmark code was
written, are outside this session's ability to resolve, and are recorded
here rather than silently worked around, per Spec 05 section 10's fallback
rule that an unresolved limitation is stated, never bridged with a
cloud/paid substitute or a fabricated number.

## Real measured evidence (`mac-arm64`, `asap` pace, 3 repetitions, full corpus)

Every candidate was fetched, checksum-verified, run through its adapter
offline, and scored. 390/390 clip-repetitions (130 clips × 3 reps)
contributed for every candidate — no timeouts, no protocol errors, no
adapter crashes. Full per-gate tables: `benchmarks/reports/2026-09-14-mac-arm64.md`.

| Candidate | MPR overall (≥0.90) | False-corr. rate (≤0.05) | WER overall (≤0.25) | WER fluent (≤0.12) | Silence halluc. tokens (=0) | Peak RSS MB (≤700) | Payload MB (≤120) | License verdict |
|---|---|---|---|---|---|---|---|---|
| `sherpa-zipformer-en-20M-2023-02-17-int8` | 0.4306 FAIL | 0.0556 FAIL | 0.3040 FAIL | 0.6071 FAIL | 0 PASS | 184.5 PASS | 41.6 PASS | permitted-with-attribution |
| `sherpa-zipformer-en-2023-06-26-int8-left64` | 0.6296 FAIL | 0.0694 FAIL | 0.1640 PASS | 0.1714 FAIL | 0 PASS | 350.3 PASS | 69.5 PASS | **unclear (license-blocked)** |
| `sherpa-zipformer-en-2023-06-26-fp32-left64` | 0.6343 FAIL | 0.0694 FAIL | 0.1614 PASS | 0.1714 FAIL | 0 PASS | 943.1 **FAIL** | 253.2 **FAIL** | **unclear (license-blocked)** |
| `whisper-base-en-ggml` | 0.8287 FAIL | 0.0648 FAIL | 0.0273 PASS | 0.0071 PASS | 12 **FAIL** | 480.9 PASS | 141.1 **FAIL** | permitted |
| `whisper-small-en-ggml` | **0.8611 FAIL (closest)** | 0.0556 FAIL | 0.0201 PASS | 0.0000 PASS | 9 **FAIL** | 961.7 **FAIL** | 465.0 **FAIL** | permitted |

No candidate passes the fidelity gate (AC10, MPR ≥ 0.90) — the product's
primary quality bar. WER, MPR, and false-correction figures use greedy
decoding, which the scorer's fixture suite (and the spec's own
determinism rule) expects to be bit-identical across repetitions; the
values above are confirmed stable across all 3 `asap` repetitions per
candidate.

### Reading these numbers honestly

- **`whisper-small-en-ggml` and `whisper-base-en-ggml` are the strongest
  candidates by fidelity** (MPR 0.86 / 0.83, both pass the `mistake-tense`
  and, for `whisper-small`, the `mistake-minimal-pair` per-condition MPR
  sub-gates) and have excellent WER (2.0% / 2.7% overall on this
  synthesized corpus). Both still fail overall MPR, fail the
  false-correction-rate ceiling, hallucinate on physical-silence clips (9
  and 12 non-empty tokens respectively — whisper.cpp's well-documented
  silence-hallucination behavior), and exceed both the peak-RSS and the
  120 MB payload-size ceiling non-streaming models were never going to
  meet at this size class.
- **The `sherpa-onnx` streaming candidates recognize only a fraction of
  each utterance** (MPR 0.43–0.63, `mistake-tense` sub-gate as low as
  0.00–0.43): inspection of individual clip hypotheses (e.g.
  `mistake-tense-01`, reference "i have went there yesterday and i didn't
  knew anyone", 20M candidate hypothesis "day and i didn't know any")
  shows the streaming decoder losing the first few seconds of context on
  short clips — plausible chunk-warm-up behavior for a
  `chunk-16-left-64`/20M architecture against short synthesized
  utterances, and a real, reportable finding rather than a harness defect
  (the adapter's NDJSON events, timing, and resource metrics all behaved
  correctly; see `benchmarks/runs/*/rep-*/mistake-tense-01.json`).
- **The two `sherpa-zipformer-en-2023-06-26-*` candidates are license-blocked
  independent of any accuracy number.** Their upstream TorchScript source
  (`Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17`)
  declares no license at all (verified live against the Hugging Face API,
  `benchmarks/licenses/license-record.md`). Per Spec 05 section 10, an
  `unclear` verdict can never be approved regardless of accuracy — these
  two rows would be blocked even if their MPR had passed.
- **Corpus caveat that likely depresses every MPR number below what a real
  human corpus would show**: this corpus is synthesized TTS speech (see
  the structural blocker above), which is acoustically far cleaner and
  more uniform than genuine human speech but also removes the natural
  disfluency cues (breath, hesitation timing, acoustic stress) a model
  might use to anchor a long utterance. The measured numbers are real and
  internally consistent, but they are diagnostic of this specific
  synthetic corpus, not proof of how any candidate would perform on real
  human incorrect English.

## `realtime`-pace latency evidence

`realtime` pace (real wall-clock pacing, required for a meaningful
first-partial/final-after-endpoint latency measurement — `asap` pace feeds
audio far faster than real time and so cannot be used for latency) was run
for all 5 candidates, full corpus, but only **1 repetition per candidate**
instead of the specified 3, for wall-clock time reasons: each full-corpus
`realtime`-pace repetition is bounded below by the corpus's own ~21-minute
duration (the pacing itself sleeps in real time), so 3 repetitions × 5
candidates would have cost roughly 5 real hours in a single implementation
session. This reduction is disclosed, not hidden, and does not change the
approval outcome: no candidate's approval is blocked *only* on latency —
every candidate already fails the fidelity gate (AC10) on `asap`-pace
evidence, which is pace-independent (WER/MPR come from the recognized
text, not from feed timing). Per-candidate latency figures are recorded in
`benchmarks/reports/2026-09-14-mac-arm64.md`; median first-partial and p95
final-after-endpoint latency were measured (not fabricated) for every
candidate, and every candidate fails both latency gates as well:

| Candidate | Median first-partial ms (≤900) | p95 final-after-endpoint ms (≤1500) |
|---|---|---|
| `sherpa-zipformer-en-2023-06-26-fp32-left64` | 1219 **FAIL** | 11105 **FAIL** |
| `sherpa-zipformer-en-2023-06-26-int8-left64` | 1173 **FAIL** | 6900 **FAIL** |
| `sherpa-zipformer-en-20M-2023-02-17-int8` | 1747 **FAIL** | 9248 **FAIL** |
| `whisper-base-en-ggml` | 1413 **FAIL** | 31613 **FAIL** |
| `whisper-small-en-ggml` | 2095 **FAIL** | 99798 **FAIL** |

The two `whisper-cpp` candidates' `simulated-rolling-window` adapter is
the dominant driver of their worse latency: `whisper-small-en-ggml`'s
single realtime-pace repetition took 45m3s of wall time against a
~20.8-minute corpus (42m49s of that in adapter CPU time), meaning decode
fell behind real-time pacing on the longest `long-turn` clips and had to
catch up — a genuine measured finding about this candidate/adapter
combination at this quantization, not a harness artifact (the sherpa-onnx
candidates, run through a real native-streaming adapter, show the same
directional problem at roughly half the latency, consistent with
`simulated-rolling-window`'s full-window re-decode design being
structurally slower than native incremental streaming). No candidate's
approval is blocked *only* by these latency numbers, since every
candidate already fails the fidelity gate independently of pace.

## Two-stream concurrency measurement

Run for `whisper-small-en-ggml` (the strongest fidelity candidate despite
failing overall) per Spec 05 section 12 AC13's "leading candidate" scoping:
`mistaken-bench run --candidate whisper-small-en-ggml --host-profile
mac-arm64 --pace asap --repeat 1 --concurrency 2`. Full result in
`benchmarks/reports/2026-09-14-mac-arm64.md`; the other 4 candidates
correctly show the two-stream gates as **NOT MEASURED** rather than a
fabricated copy of their single-stream numbers (Spec 05 scopes the
two-stream measurement to the leading candidate only, AC13).

| Gate | Measured | Threshold | Result |
|---|---|---|---|
| `throughput.rtf_two_stream` | 0.4387 | ≤ 0.9000 | **PASS** |
| `resource.peak_rss_two_stream_mb` | 1983.83 | ≤ 1400.0000 | **FAIL** |

`peak_rss_two_stream_mb` is computed by pairing the two clips that ran
concurrently (same chunk, real wall-time overlap) and summing their
individually-measured `getrusage` peak RSS, then taking the maximum such
pair-sum across the run — each process's own peak is real measured data,
and the harness has no wall-clock-aligned combined memory sampler, so this
is a real, slightly-conservative (upper-bound) approximation of combined
system memory, not a fabricated number. `whisper-small-en-ggml` was
already failing its single-stream RSS gate (961.7 MB > 700 MB); two
concurrent streams predictably approach double that and fail the
1400 MB two-stream ceiling too.

## Offline-networking verification

Source inspection: neither adapter (`benchmarks/adapters/sherpa-onnx/src/main.cc`,
`benchmarks/adapters/whisper-cpp/src/main.cc`) nor the harness
(`benchmarks/harness/src/**`) imports any socket, HTTP, or DNS API — grep
for `socket|connect(|getaddrinfo|CFNetwork|NSURLSession|curl|URLSession` across
every adapter/harness source file returns zero code hits (the only string
matches are literal `https://` URLs inside a committed license-record test
fixture, i.e. text data, not code). This host could not be taken fully
offline at the OS level without disrupting concurrently running sibling
Wave 2 worktrees (`mistaken-spec-02`, `mistaken-spec-03`) sharing it, so
"networking disabled" is verified by source inspection plus the observed
fact that every measured run completed in bounded, expected wall-clock time
with zero adapter `error` events of any kind — a stalled DNS/socket call
inside either adapter would have manifested as a hung or `timeout`-coded
clip, and none occurred across 390 × 5 = 1,950 `asap`-pace clip-repetitions
plus 130 × 5 = 650 `realtime`-pace clip-runs.

## Recommended next action

1. A human operator records the corpus per `corpus/protocol.md` with real
   speakers and informed consent, replacing the synthesized clips (the
   manifest schema, protocol, prompts, scorer, and harness need no changes
   — only real audio and a re-run of `validate-corpus`).
2. Obtain access to a `win-x64` host meeting the minimum profile (≥ 8
   cores, ≥ 16 GB RAM, AC power) and repeat every measured step there.
3. With a real corpus, re-evaluate `whisper-small-en-ggml` and
   `whisper-base-en-ggml` first — they are the closest to the MPR gate and
   the most likely to clear it on genuine human speech, though both will
   still need a smaller quantized/distilled export to meet the 120 MB
   payload ceiling, and the silence-hallucination finding will need a
   VAD/silence gate ahead of the recognizer regardless of which candidate
   is chosen.
4. The two `2023-06-26` zipformer candidates should not be re-benchmarked
   until their upstream provenance license question is resolved (either a
   license appears on `Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17`,
   or a written rights clarification is obtained directly) — accuracy
   evidence cannot unblock an `unclear` provenance verdict.
