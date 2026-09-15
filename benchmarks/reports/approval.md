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

## Candidate-set expansion (2026-09-15, product-owner authorized, same session)

Full detail: `benchmarks/reports/2026-09-15-candidate-expansion-research.md`.
After the remediation experiments above confirmed the original five
candidates are exhausted, the product owner explicitly authorized
expanding the candidate set (`docs/context/progress-tracker.md`,
2026-09-15) — an **addition**, not a threshold relaxation: every existing
candidate, its evidence, and this BLOCKED result are preserved unchanged.

One candidate researched and rejected before benchmarking:
`sherpa-onnx-streaming-zipformer-en-2023-02-21` (a same-family, larger
streaming Zipformer) — its int8 export totals 127,772,642 B (≈ 121.85
MiB), 1.85 MiB over the 120 MiB payload ceiling even at its smallest
export, and its upstream TorchScript source
(`Zengwei/icefall-asr-librispeech-pruned-transducer-stateless7-streaming-2022-12-29`)
has no `license:` tag at all — the same provenance gap as the excluded
`2023-06-26` candidates. Two independent, sufficient disqualifiers; not
benchmarked.

One candidate added, license-verified, and evaluated: **`whisper-base-en-q8-ggml`**
(`ggml-base.en-q8_0.bin`, `ggerganov/whisper.cpp` commit
`0b364b566045a405be7225ee1e415a073e04da77`, 81,781,811 B, MIT license,
identical provenance chain to the already-verified `whisper-base-en-ggml`
row). Reuses the existing pinned `whisper.cpp v1.9.4` runtime and adapter
unmodified — zero new runtime/vendoring risk. Full license row in
`benchmarks/licenses/license-record.md`; `mistaken-bench licenses --check`
→ `PASS (6 candidate(s) complete)`.

Focused-subset evaluation (68 of 130 real-corpus clips: `mistake-tense`,
`mistake-minimal-pair`, `fluent-control`, `fast-speech`, `noise-silence`,
`asap` pace, 1 repetition):

| Configuration | MPR | False-corr. | WER | Silence tokens |
|---|---|---|---|---|
| Baseline (no tuning) | 0.5323 | 0.0968 | 0.0923 | 2 |
| + `suppressNst: true, noSpeechThold: 0.3` (adopted) | 0.5323 | 0.0968 | **0.0889** | **0** |
| + `initialPrompt` anti-correction instruction (rejected) | 0.5161 | 0.1129 | 0.1128 | 6 |
| + both combined (rejected) | 0.5161 | 0.1129 | 0.1504 | 28 |

MPR and false-correction are **bit-identical to the already-measured
fp16 `whisper-base-en-ggml` sibling** on every non-silence condition —
Q8_0 quantization fixes the payload gate (81.78 MB vs. 147.96 MB, both
well clear of/over the 120 MB ceiling respectively) without any fidelity
regression, but also without any fidelity improvement. While inspecting
`whisper_full_params`, found that `suppress_nst` (non-speech-token
suppression) defaults to `false` in the vendored `whisper.cpp` library
and was never set by the adapter — every previously frozen Whisper
candidate ran with this suppression off. Enabling it plus lowering
`no_speech_thold` to `0.3` **eliminates silence hallucination on this
subset (2→0 tokens) with no downside** and is adopted into
`whisper-base-en-q8-ggml`'s frozen decoding block. A prompt-engineering
attempt to instruct Whisper not to correct grammar via `initial_prompt`
**failed and made every metric worse** (MPR, false-correction, WER, and
especially silence hallucination when combined with the suppression
fix) — Whisper's `initial_prompt` is a vocabulary/context primer, not an
instruction-following interface, since the underlying model was never
instruction-tuned. Both results are real, reproducible, and disclosed.

**`whisper-base-en-q8-ggml` now clears the payload and silence gates but
remains far short of the fidelity gate** (MPR 0.5323 vs. ≥ 0.90 required,
false-correction 0.0968 vs. ≤ 0.05 required) — the same order of
magnitude shortfall as its fp16 sibling. Per the task's explicit
instruction to reject candidates that clearly cannot approach the frozen
gates, **no full 3-repetition Mac-arm64 benchmark was run** for this
candidate: the focused-subset result already conclusively demonstrates it
cannot pass, and running the full corpus (~10 min `asap` + ~40 min
`realtime`) would not change that conclusion.

**No candidate — the original five or this newly added sixth — clears
every Mac-arm64 gate.** MAC QUALIFIED status has not been reached by any
candidate.

## Fundamentally different architecture search (2026-09-15, product-owner authorized, same session)

Full detail: `benchmarks/reports/2026-09-15-fundamentally-different-architecture-research.md`.
Per explicit product-owner direction not to spend further time tuning the
already-exhausted Whisper/Sherpa-transducer configurations, researched
architectures genuinely different from both families: streaming/offline
CTC (icefall zipformer-CTC, NVIDIA NeMo Citrinet/Conformer-CTC) and
Kaldi HMM-DNN + WFST decoding (Vosk). Paper-screened four concrete
options before downloading anything:

| Candidate | Payload | License result | Verdict |
|---|---|---|---|
| Streaming zipformer2-CTC (icefall) | 25 MiB–728 MiB | Apache-2.0 | **No English pretrained model exists** — only Chinese/Russian published; training one is out of scope |
| Offline zipformer-CTC (`sherpa-onnx-zipformer-ctc-en-2023-10-02`) | 67.0 MiB, fits | **Unclear** — upstream `Zengwei/...zipformer-transducer-ctc-2023-06-13` declares no license, same gap as the already-excluded transducer candidates | Rejected before benchmarking |
| Offline NeMo Citrinet-512-CTC (`sherpa-onnx-nemo-ctc-en-citrinet-512`) | 36.3 MiB, fits | **Unclear** — the converter's self-applied `apache-2.0` does not match the upstream NGC `stt_en_citrinet_512` model card's actual "NGC Terms of Use" | Rejected before benchmarking |
| Vosk (`vosk-model-small-en-us-0.15`, new adapter, new runtime) | 67.6 MiB extracted, fits | **Permitted — first-party Apache-2.0**, no upstream provenance gap | **Accepted; benchmarked** |

A brand-new adapter (`benchmarks/adapters/vosk/**`) was built and vendored
(prebuilt `libvosk` from the official Alpha Cephei PyPI wheel — Vosk ships
no source-buildable release, mirroring the existing precedent of the
sherpa-onnx adapter's own prebuilt-ONNX-Runtime dependency) and a new
candidate `vosk-small-en-us` was added, license-verified, and
checksum-fetched (14 files, `mistaken-bench fetch` → `VERIFIED` for all).

Focused-subset evaluation (90 of 130 real-corpus clips, `asap` pace, 1
repetition):

| Metric | `vosk-small-en-us` | Gate | Result |
|---|---|---|---|
| MPR (combined) | 0.5349 | ≥ 0.90 | **FAIL — not close** |
| MPR (`mistake-tense`) | 0.4348 | ≥ 0.85 | **FAIL** |
| MPR (`mistake-minimal-pair`) | 0.6250 | ≥ 0.85 | **FAIL** |
| False-correction rate | **0.0349** | ≤ 0.05 | **PASS** |
| WER (combined) | 0.2144 | ≤ 0.25 | **PASS** |
| Silence hallucination | **0 tokens** (12/12 clips incl. 4 physical-silence) | = 0 | **PASS** |

**This is the first candidate in the entire remediation series whose
false-correction rate, WER, and silence hallucination all individually
pass on a focused subset.** Concrete confirmation the architecture
genuinely avoids grammar correction:
`mistake-tense-02-48k`, reference `"she go to the store every day last
week"` → hypothesis `"she go to the store every day last week"` — the
deliberate agreement error survives verbatim. But the primary MPR gate is
not close, driven by ordinary small-model misrecognition rather than
correction (e.g. `mistake-tense-03`: `"we was gone..."` → `"he was
gone..."`, an acoustic confusion of a short function word, not a
grammatical repair). No larger Vosk model fits the payload gate
(`vosk-model-en-us-0.22-lgraph` is 128 MiB, 8 MiB over the ceiling) to
try for better raw accuracy.

**Per the task's explicit instruction to reject early when focused MPR is
nowhere near 0.90, no full 3-repetition Mac-arm64 benchmark was run.**
STOP condition reached: three fundamentally different architecture
families (attention-based streaming transducer, autoregressive
encoder-decoder, and now Kaldi HMM-DNN+WFST) have each been evaluated with
real measured evidence against the real human corpus, and none clears the
fidelity gate within the 120 MiB payload ceiling.

**No candidate — the original five, `whisper-base-en-q8-ggml`, or
`vosk-small-en-us` — clears every Mac-arm64 gate. MAC QUALIFIED status has
not been reached by any candidate.**

## Payload-policy revision and larger-model test (2026-09-15, product-owner decision, this session)

Full detail: `benchmarks/reports/2026-09-15-larger-model-payload-policy-research.md`.
**Product-owner decision, recorded verbatim:** preserving deliberate
grammatical mistakes remains a hard requirement — MPR ≥ 0.90,
false-correction ≤ 0.05, WER thresholds, the silence requirement, and
every latency/resource gate are **unchanged**. The ≤ 120 MiB *bundled
installer* payload ceiling is replaced with a two-part packaging rule:
(1) initial application/package size remains separately controlled
(Specs 13–14's concern), and (2) local ASR model resources may exceed
120 MiB when delivered as a separately downloadable, checksum-pinned
local resource, subject to remaining fully local/offline, no paid
fallback, clear license/provenance, checksum/version pinning, integrity
verification, a truthful download lifecycle, and separately documented
installer/model sizes. **No specific new numeric ceiling is adopted** —
one is deferred until a concrete candidate is approved and its real
payload justifies it; `benchmarks/harness/src/scoring/gates.rs`'s
`MAX_MODEL_PAYLOAD_BYTES` constant is deliberately left unchanged in code
this session (not silently removed), and every candidate below was
evaluated on its real payload number rather than auto-rejected at 120 MiB.

Under this policy, `vosk-en-us-0.22-lgraph` (204.0 MB extracted, 130.6 MB
compressed — the same Kaldi HMM-DNN+WFST architecture as `vosk-small-en-us`
at roughly 3× the acoustic-model and decoding-graph capacity, same
first-party Apache-2.0 license) was added, license-verified, and
checksum-fetched (17 files, `mistaken-bench fetch` → `VERIFIED` for all).

Focused-subset evaluation (64 of 130 real-corpus clips, `asap` pace, 1
repetition):

| Metric | `vosk-small-en-us` (68 MiB) | `vosk-en-us-0.22-lgraph` (204 MiB) | Gate | Result |
|---|---|---|---|---|
| MPR (`mistake-tense`) | 0.4348 | **0.4348 — identical** | ≥ 0.85 | **FAIL, unmoved** |
| MPR (`mistake-minimal-pair`) | 0.6250 | 0.5625 — worse | ≥ 0.85 | **FAIL** |
| MPR (combined, comparable conditions) | 0.5349 | 0.5532 | ≥ 0.90 | **FAIL — not close** |
| False-correction rate | 0.0349 | **0.0000** | ≤ 0.05 | PASS (both) |
| WER (combined, comparable conditions) | ~0.21 | 0.1922 | ≤ 0.25 | PASS (both) |
| Silence hallucination | 0 tokens | **6 tokens** (all 4 physical-silence clips) | = 0 | PASS → **new FAIL** |

**The `mistake-tense` sub-gate — the single most informative measurement
for this corpus — is bit-identical between a 68 MiB and a 204 MiB model
of the same architecture.** `mistake-minimal-pair` measured worse at the
larger size, and the larger acoustic model introduced a new
silence-hallucination regression (a more sensitive model picks up
room-noise-floor signal the smaller model correctly recognized as
non-speech). WER and false-correction both improved, but neither reaches
the fidelity bar, and the unmoved `mistake-tense` result gives no
evidence basis to expect a further size increase would change this
qualitatively. **Per the task's stop-condition instruction, a much larger
Vosk model (the ~1.8 GB non-`lgraph` release) was deliberately not
tested** — the identical result across an already-3× jump does not
justify the cost of a further ~9× jump — and **no full 3-repetition
Mac-arm64 benchmark was run** for `vosk-en-us-0.22-lgraph` (focused
result already conclusive).

**Conclusion: model size is not the actual limiting factor.** Real
evidence at two sizes within the best-behaved architecture family found
in this remediation effort, combined with the earlier findings that
Whisper's grammar correction is trained-in (not fixed by quantization or
prompting) and the sherpa-onnx transducer's context loss is
export-specific (not fixed by beam search or warm-up), means **six
candidates across three architecturally distinct families have now been
evaluated with real measured evidence, and none clears the fidelity gate
— independent of payload, which this session explicitly relaxed and
still could not use to produce a passing result.**

**No candidate becomes MAC QUALIFIED.**

## Product-constraint statement (for the product owner; option 1 below has now been exercised and closed out)

Real, measured evidence across three architecturally distinct local ASR
families — including a deliberate test of a 3× larger model within the
best-behaved family after the payload ceiling was explicitly relaxed —
indicates that **the combination of (a) MPR ≥ 0.90, (b) false-correction
rate ≤ 0.05, and (c) the current latency/resource gates appears
technically incompatible with currently available, license-clean,
practical local English ASR models, at any of the payload sizes tested**,
on real disfluent human speech containing deliberate grammatical errors:

- Every model small enough to have been considered "size-blocked" under
  the old ≤ 120 MiB rule (sherpa 20M transducer, Vosk small) lacks the
  raw acoustic/language capacity to recognize enough of each utterance
  correctly, regardless of decoding configuration.
- **Relaxing the payload ceiling and testing a materially larger model
  in the best-behaved family (Vosk, 68 MiB → 204 MiB) did not move the
  primary fidelity sub-gate at all** (`mistake-tense` MPR identical at
  0.4348) and made one other sub-gate and the silence gate worse —
  direct evidence that size was not the limiting factor for this family.
- Every model with enough capacity to approach the fidelity bar (Whisper
  base/small, in any tested quantization) exhibits trained-in grammar
  auto-correction that no prompting, decoding-parameter change, or
  payload relaxation removes.
- No English streaming CTC model — the architecture class most likely to
  combine native low latency with literal, LM-light decoding — is
  currently published for evaluation.

This is not a request or a recommendation to relax a gate; no
fidelity/latency/resource threshold was changed, and the one constraint
that *was* revised (the bundled-payload ceiling) has now been tested and
shown insufficient on its own. The decision this finding requires from
the product owner is one of the following, made explicitly rather than
worked around:

1. **Commission or fund a purpose-built anti-normalization ASR model** —
   explicitly trained or fine-tuned to preserve grammatical errors rather
   than correct them. This remains the only path identified in this
   entire remediation effort that directly targets the false-correction
   failure mode without also inheriting either Whisper's language-model
   bias or a small-model's raw accuracy ceiling. Out of Spec 05's scope
   (section 4 excludes training/fine-tuning); would require a new spec
   or an explicit scope exception.
2. **Revisit the MPR/false-correction thresholds themselves** as a
   deliberate, recorded product decision (not a quiet erosion) if the
   product can tolerate a lower fidelity bar than originally specified.
3. **Continue blocked** and treat "no currently available local ASR
   model satisfies Mistaken's fidelity promise, at any size, on any of
   the architectures evaluated" as the honest, evidence-backed current
   answer, revisiting as the local-ASR model landscape (new open English
   streaming-CTC releases, purpose-built anti-correction checkpoints)
   evolves.

None of these three options was selected by this session; they are
presented for an explicit product-owner decision.

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
3. **Legitimate engineering changes already benchmarked this session**:
   on `sherpa-zipformer-en-20M-2023-02-17-int8`, `modified_beam_search`
   decoding and a synthetic silence cold-start warm-up (neither adopted,
   neither closed the fidelity gap); on `whisper-base-en-q8-ggml`,
   `suppressNst`/`noSpeechThold` tuning (adopted, fixed silence only) and
   `initialPrompt` anti-correction prompting (made every metric worse,
   not adopted); on the Vosk family, scaling the model 3× larger (real
   WER/false-correction improvement, the primary `mistake-tense` sub-gate
   completely unmoved, a new silence regression). **Conclusion: neither
   decoder/runtime configuration nor model-size scaling closes the
   fidelity gap in any family evaluated** — sherpa's early-utterance
   token loss is export-specific, Whisper's grammar auto-correction is
   trained-in, and Vosk's small-model misrecognition is not resolved by a
   3× capacity increase.
4. **The architecture and payload-policy search is complete for this
   session** (see the sections above and the three dated research
   reports referenced throughout this document). Two CTC candidates were
   paper-screened and rejected on the same upstream-provenance-license
   pattern already seen three times in this series, without spending
   compute measuring their accuracy. The next step is not further model
   search inside the currently available landscape or further payload
   relaxation — it is the product-owner decision named in the
   "Product-constraint statement" section above (commission a
   purpose-built model, revise the fidelity thresholds, or stay blocked).
5. **The two `2023-06-26` zipformer candidates should not be re-benchmarked**
   until their upstream provenance license question is resolved (either a
   license appears on
   `Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17`, or a
   written rights clarification is obtained directly) — accuracy evidence
   cannot unblock an `unclear` provenance verdict.
6. **None of the above authorizes starting Spec 06.** Spec 06's
   precondition is an approved Spec 05 candidate with both-host evidence;
   neither exists yet.
