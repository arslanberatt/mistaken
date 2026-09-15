# Spec 05 Approval Decision

## Result: BLOCKED — no candidate approved

This is a genuine, real-evidence blocker on a **real human-recorded
corpus**, not a placeholder and not the prior TTS-corpus blocker. Every
candidate in the frozen set was built from pinned upstream source, ran the
full 130-entry corpus (122 primary clips + the 8-clip 48 kHz duplicate
subset, 29.6 minutes of real recorded speech) offline on real `mac-arm64`
hardware, and was scored with the unmodified deterministic scorer. No
candidate satisfies every gate required for approval, and one structural
blocker (no `win-x64` host) makes approval unreachable this session
regardless of any accuracy number. Both are recorded honestly rather than
worked around.

## What changed since the prior (TTS-corpus) evidence

- The corpus is now **122 real human-recorded primary clips (29.6 minutes)
  + the 8-clip 48 kHz duplicate subset = 130 manifest entries**, replacing
  the previous macOS `say` TTS-synthesized audio. `speakerProfiles` in
  `benchmarks/corpus/manifest.json` record `consentGiven: true` for both
  `sp-01` and `sp-02` with real-recording provenance notes; the prior
  `PENDING`/`false` placeholders are gone. `mistaken-bench validate-corpus`
  passes cleanly: `PASS (122 primary clips, 29.6 minutes, 130 total
  manifest entries)`.
- The `long-turn` condition's accepted duration window was widened from
  60–120 s to **60–180 s** by an explicit product-owner decision recorded
  in `docs/context/progress-tracker.md` (2026-09-15), because the real
  speaker's natural-pace `long-turn` recordings ran 120,656–160,368 ms. No
  fidelity, accuracy, hallucination, latency, payload, memory, or license
  threshold changed; only the corpus-composition duration bound moved, and
  the resource gate's CPU/RSS thresholds remain plain fractions, not values
  pinned to a specific clip length.
- This is the **first real-hardware run against real human speech** for
  every candidate in the frozen set. The corpus-authenticity structural
  blocker from the prior evidence record is fully resolved.

## Remaining structural blocker

### No `win-x64` reference host was available this session (blocks AC8/AC17)

Spec 05 requires both `mac-arm64` and `win-x64` to pass every gate before
any approval ("one host's result never substitutes for the other's"). This
session had access to one real macOS host (Apple M4, 10 cores, 16 GB RAM,
macOS 15.7.5, arm64 — the approved `mac-arm64` reference profile) and no
accessible Windows machine, VM, or cloud instance (checked: no `win-x64`
host is configured or reachable from this workstation). Every measured run
in this record is `mac-arm64` only. AC17 is explicit: "No approval exists
without both hosts' numbers." This blocker is independent of the fidelity
result below: even a candidate that cleared every `mac-arm64` gate could
not be approved without a matching `win-x64` run.

## Real measured evidence (`mac-arm64`, `asap` pace, 3 repetitions, full corpus)

Every candidate was fetched, checksum-verified, run through its adapter
offline, and scored. 390/390 clip-repetitions (130 clips × 3 reps)
contributed for every candidate — no timeouts, no protocol errors, no
adapter crashes, coverage 100% (gate ≥ 98%). Full per-gate tables:
`benchmarks/reports/2026-09-15-mac-arm64.md`.

| Candidate | MPR overall (≥0.90) | False-corr. rate (≤0.05) | WER overall (≤0.25) | WER fluent (≤0.12) | Silence halluc. tokens (=0) | Peak RSS MB (≤700) | Payload MB (≤120) | License verdict |
|---|---|---|---|---|---|---|---|---|
| `sherpa-zipformer-en-20M-2023-02-17-int8` | 0.3241 FAIL | 0.0556 FAIL | 0.3959 FAIL | 0.7286 FAIL | 0 PASS | 164.5 PASS | 41.6 PASS | permitted-with-attribution |
| `sherpa-zipformer-en-2023-06-26-int8-left64` | 0.5417 FAIL | 0.0648 FAIL | 0.2497 PASS | 0.2500 FAIL | 0 PASS | 347.6 PASS | 69.5 PASS | **unclear (license-blocked)** |
| `sherpa-zipformer-en-2023-06-26-fp32-left64` | 0.5231 FAIL | 0.0694 FAIL | 0.2507 FAIL | 0.2429 FAIL | 0 PASS | 932.9 **FAIL** | 253.2 **FAIL** | **unclear (license-blocked)** |
| `whisper-base-en-ggml` | 0.5926 FAIL | 0.1528 FAIL | 0.1097 PASS | 0.0286 PASS | 6 **FAIL** | 505.9 PASS | 141.1 **FAIL** | permitted |
| `whisper-small-en-ggml` | **0.6667 FAIL (closest)** | 0.1157 FAIL | 0.0886 PASS | 0.0286 PASS | 15 **FAIL** | 971.3 **FAIL** | 465.0 **FAIL** | permitted |

No candidate passes the fidelity gate (AC10, MPR ≥ 0.90) — the product's
primary quality bar. WER, MPR, and false-correction figures use greedy
decoding, which the scorer's fixture suite (and the spec's own determinism
rule) expects to be bit-identical across repetitions; verified stable
across all 3 `asap` repetitions per candidate (per-clip inserted-token and
MPR-outcome counts were spot-checked identical rep-to-rep).

### Reading these numbers honestly

- **Real human speech is measurably harder for every candidate than the
  prior synthesized-TTS corpus was.** `whisper-small-en-ggml`'s overall MPR
  fell from 0.8611 (TTS) to **0.6667** (real speech) on the same frozen
  candidate and scorer; `whisper-base-en-ggml` fell from 0.8287 to 0.5926.
  This is the expected direction for a real product decision: a clean TTS
  corpus systematically overstated every candidate's ability to preserve
  disfluent, coarticulated real speech, and the prior BLOCKED decision not
  to approve from that corpus is now vindicated by real evidence, not just
  a consent technicality.
- **The `whisper.cpp` candidates confirm the exact defect Spec 05 exists to
  catch.** Both have low WER (8.9%/11.0%) because Whisper's language-model
  prior actively repairs the deliberately incorrect grammar it hears,
  rather than misrecognizing it — concrete, inspected examples from this
  run: reference `"the dogs is barking outside the window right now"` →
  hypothesis `"The dogs are barking outside the window right now."`
  (`mistake-agreement-01`); reference `"we was rushing and i forgot where
  i putted it"` → hypothesis `"We were rushing and I forgot where I put
  it"` (`fast-speech-08`); reference `"she arrived to the airport before
  the flight departed"` → hypothesis `"She arrived at the airport before
  the flight departed."` (`mistake-preposition-02`); reference `"he drived
  off quick before i could catched him"` → hypothesis `"He drives off
  quick before I could catch him."` (`fast-speech-07`). Every one of these
  is scored `false_correction`, not `misrecognized`, because the output is
  the grammatically corrected form of the exact annotated error. This is
  low WER coexisting with silent grammar correction — precisely the
  failure mode section 15 of the spec warns MPR exists to catch, now
  demonstrated on real human speech rather than a hypothetical.
- **The `sherpa-onnx` streaming candidates preserve mistakes better
  (lower false-correction rate, 0.056–0.069 vs. Whisper's 0.116–0.153) but
  recognize substantially less of each utterance correctly**, especially
  on `mistake-tense` (MPR as low as 0.00–0.39 by candidate) — inspection of
  `mistake-tense-01` (reference `"i have went there yesterday and i didn't
  knew anyone"`) shows the 20M candidate producing `"I HAVE WENT THERE
  YESTERDAY AND I DIDN'T KNOW ANY"`: it preserves `"have went"` correctly
  but drops `"anyone"` and misrecognizes `"knew"` as `"know"`, losing the
  second annotated error span to an ordinary misrecognition rather than a
  correction. This matches the chunk-context/short-utterance finding from
  the prior TTS-corpus evidence and persists on real speech.
- **The two `sherpa-zipformer-en-2023-06-26-*` candidates remain
  license-blocked independent of any accuracy number.** Their upstream
  TorchScript source (`Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17`)
  still declares no license (unchanged since the license record was last
  verified live, 2026-09-11; not re-verified this session because nothing
  about the corpus change affects license status). Per Spec 05 section 10,
  an `unclear` verdict can never be approved regardless of accuracy — these
  two rows would be blocked even if their MPR had passed.

## `realtime`-pace latency evidence

`realtime` pace (real wall-clock pacing, required for a meaningful
first-partial/final-after-endpoint latency measurement — `asap` pace feeds
audio far faster than real time and so cannot be used for latency) was run
for all 5 candidates, full corpus, but only **1 repetition per candidate**
instead of the specified 3, for wall-clock time reasons: each full-corpus
`realtime`-pace repetition is bounded below by the corpus's own ~29.6-minute
duration (the pacing itself sleeps in real time), so 3 repetitions × 5
candidates would have cost roughly 7–8 real hours in a single session (the
full 5-candidate batch actually observed took 3h28m wall time for 1
repetition each). This reduction is disclosed, not hidden, and does not
change the approval outcome: no candidate's approval is blocked *only* on
latency — every candidate already fails the fidelity gate (AC10) on
`asap`-pace evidence, which is pace-independent (WER/MPR come from the
recognized text, not from feed timing). 130/130 clips contributed for
every candidate (zero timeouts). Per-candidate latency figures:

| Candidate | Median first-partial ms (≤900) | p95 final-after-endpoint ms (≤1500) |
|---|---|---|
| `sherpa-zipformer-en-2023-06-26-int8-left64` | 1898 **FAIL** | 16298 **FAIL** |
| `sherpa-zipformer-en-2023-06-26-fp32-left64` | 1922 **FAIL** | 15135 **FAIL** |
| `sherpa-zipformer-en-20M-2023-02-17-int8` | 3091 **FAIL** | 9726 **FAIL** |
| `whisper-base-en-ggml` | 1446 **FAIL** | 43136 **FAIL** |
| `whisper-small-en-ggml` | 2145 **FAIL** | 127235 **FAIL** |

The two `whisper-cpp` candidates' `simulated-rolling-window` adapter is the
dominant driver of their worse p95 final-after-endpoint latency, most
visibly on the now-longer (up to 180 s) `long-turn` clips: `whisper-small`'s
single realtime-pace repetition took roughly 63 minutes of wall time
against the ~29.6-minute corpus, meaning decode fell behind real-time
pacing and had to catch up — a genuine measured finding about this
candidate/adapter combination at this quantization and corpus length, not
a harness artifact. No candidate's approval is blocked *only* by these
latency numbers, since every candidate already fails the fidelity gate
independently of pace.

## Two-stream concurrency measurement

Run for `whisper-small-en-ggml` (the strongest fidelity candidate despite
failing overall) per Spec 05 section 12 AC13's "leading candidate" scoping:
`mistaken-bench run --candidate whisper-small-en-ggml --host-profile
mac-arm64 --pace asap --repeat 1 --concurrency 2`. Full result in
`benchmarks/reports/2026-09-15-mac-arm64.md`; the other 4 candidates
correctly show the two-stream gates as **NOT MEASURED** rather than a
fabricated copy of their single-stream numbers (Spec 05 scopes the
two-stream measurement to the leading candidate only, AC13).

| Gate | Measured | Threshold | Result |
|---|---|---|---|
| `throughput.rtf_two_stream` | 0.3133 | ≤ 0.9000 | **PASS** |
| `resource.peak_rss_two_stream_mb` | 1879.67 | ≤ 1400.0000 | **FAIL** |

`peak_rss_two_stream_mb` is computed by pairing clips that ran concurrently
(same chunk, real wall-time overlap) and summing their individually
measured `getrusage` peak RSS, then taking the maximum such pair-sum across
the run — each process's own peak is real measured data, and the harness
has no wall-clock-aligned combined memory sampler, so this is a real,
slightly-conservative (upper-bound) approximation of combined system
memory, not a fabricated number. `whisper-small-en-ggml` was already
failing its single-stream RSS gate (971.3 MB > 700 MB); two concurrent
streams predictably approach double that and fail the 1400 MB two-stream
ceiling too.

## `resource.rss_growth_after_30s_fraction` — not measured (pre-existing harness gap)

The harness's `run`/`score` pipeline does not currently compute a rolling
RSS-over-time curve from the `long-turn` clips; `rss_growth_after_30s_fraction`
is hard-coded to "not measured" in `benchmarks/harness/src/main.rs`. This is
a pre-existing gap in the original Spec 05 implementation, not something
introduced or masked by this remediation session, and it is disclosed here
rather than silently left out of the gate table (the rendered report
correctly shows `NOT MEASURED`, which blocks approval exactly like a
failure per the gate contract). It does not change the approval outcome:
every candidate already fails the fidelity gate independent of this metric.
Computing it would require adding a periodic-sampling instrument to the
adapter/harness metrics protocol — real engineering work, out of this
remediation's scope (re-running the frozen benchmark against the real
corpus), and is recorded here as a legitimate follow-up rather than
patched in ad hoc mid-run.

## Offline-networking verification

Source inspection: neither adapter (`benchmarks/adapters/sherpa-onnx/src/main.cc`,
`benchmarks/adapters/whisper-cpp/src/main.cc`) nor the harness
(`benchmarks/harness/src/**`) imports any socket, HTTP, or DNS API — grep
for `socket|connect(|getaddrinfo|CFNetwork|NSURLSession|curl_easy|URLSession`
across every adapter/harness source file returns zero code hits. This host
was not taken fully offline at the OS level during the measured runs (doing
so would have disrupted this same session's own tooling), so "networking
disabled" is verified by source inspection plus the observed fact that
every measured run completed in bounded, expected wall-clock time with zero
adapter `error` events and zero `timeout`-coded clips across 1,950
`asap`-pace clip-repetitions plus 650 `realtime`-pace clip-runs plus 260
two-stream clip-repetitions — a stalled DNS/socket call inside either
adapter would have manifested as a hung or `timeout`-coded clip, and none
occurred.

## Remediation experiments performed (2026-09-15, same session)

Full detail, exact configurations, and raw before/after numbers:
`reports/2026-09-15-remediation-experiments.md`. Summary: focused,
representative-subset experiments (`mistake-tense`, `fluent-control`,
full-corpus single-repetition confirmation) tested two legitimate,
in-scope decoder/runtime configuration changes against
`sherpa-zipformer-en-20M-2023-02-17-int8` — the only license-clean
candidate small enough to ever satisfy the payload/RSS gates —
**`modified_beam_search` decoding** (real WER improvement of 5–39%
relative depending on condition, MPR completely unchanged at ~0.31–0.32
overall, plus a new silence-hallucination regression) and **a synthetic
silence cold-start warm-up before the real audio** (no measurable
improvement, occasionally worse). Neither is a credible path to the
MPR ≥ 0.90 gate, so **no full 3-repetition re-run was performed** and
**no configuration was changed on any frozen candidate**; both
experiments are recorded as real, reproducible negative results.
`whisper-*` candidates were not experimented on further this session
because both are unfixably over the payload gate (147.96 MB / 487.6 MB
vs ≤ 120 MB) regardless of any decoding-parameter change.

## Recommended next action

1. **Obtain a `win-x64` host** meeting the minimum profile (≥ 8 cores, ≥ 16
   GB RAM, AC power) and repeat every measured step there. This is now the
   only structural (non-accuracy) blocker; the corpus-authenticity blocker
   from the prior evidence record is resolved.
2. **No candidate should be approved even after a `win-x64` run** without a
   legitimate accuracy change: every candidate's `mac-arm64` MPR (0.32–0.67)
   is far below the 0.90 gate, and this gap is architectural, not
   measurement noise — `win-x64` numbers are expected to land in the same
   range (CPU-only greedy decoding is deterministic across host
   architecture; Spec 05 does not claim OS-level behavioral differences).
3. **Legitimate engineering changes already benchmarked this session on
   `sherpa-zipformer-en-20M-2023-02-17-int8`** (full detail in
   `reports/2026-09-15-remediation-experiments.md`): `modified_beam_search`
   decoding (real WER improvement, zero MPR improvement, plus a new
   silence-hallucination regression — not adopted) and a synthetic
   silence cold-start warm-up (no measurable improvement — not adopted).
   Neither closes the fidelity gap; the early-utterance token loss this
   candidate exhibits is a property of its specific streaming-transducer
   export, not a fixable decoder/runtime configuration. **Not yet
   benchmarked** (deprioritized because both `whisper-*` candidates are
   unfixably over the payload gate regardless): Whisper
   `initial_prompt`/`no_speech_thold`/`suppress_nst` tuning against a
   *smaller* license-clean Whisper export once one is added to the
   candidate set through the normal process — this remains the most
   promising lever for the grammar-auto-correction and
   silence-hallucination failure modes specifically.
4. **The two `2023-06-26` zipformer candidates should not be re-benchmarked**
   until their upstream provenance license question is resolved (either a
   license appears on
   `Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17`, or a
   written rights clarification is obtained directly) — accuracy evidence
   cannot unblock an `unclear` provenance verdict.
5. **None of the above authorizes starting Spec 06.** Spec 06's
   precondition is an approved Spec 05 candidate with both-host evidence;
   neither exists yet.
