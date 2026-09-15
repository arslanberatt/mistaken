# Spec 05 remediation — legitimate configuration experiments (2026-09-15)

Goal: determine whether any in-scope, legitimate configuration or runtime
change to the frozen candidate set (`benchmarks/candidates/*.json`) could
plausibly close the gap to the frozen fidelity gate (AC10, MPR ≥ 0.90),
without touching scoring, references, the corpus, or the candidate
identity/license process. All experiments below ran against the real
130-entry human-recorded corpus on `mac-arm64` (Apple M4). None of these
experiments changed `benchmarks/candidates/*.json`, `benchmarks/corpus/**`,
or `benchmarks/harness/src/scoring/**`; every scratch descriptor used for
iteration lived under a removed local scratch path and is not committed.

## 1. Precise technical causes (diagnosis)

Restricting attention to the three **license-clean** candidates (the two
`sherpa-zipformer-en-2023-06-26-*` candidates remain `unclear`/license-
blocked independent of any accuracy result and were not experimented on
further — accuracy evidence cannot unblock a provenance gap):

| Candidate | Dominant cause of MPR failure | Dominant cause of other gate failures |
|---|---|---|
| `sherpa-zipformer-en-20M-2023-02-17-int8` | Systemic early-utterance token loss/corruption: the streaming transducer's causal decode commits tokens before enough left context has accumulated, and — unlike an offline decoder — never revisits an already-emitted token once more audio arrives. Directly observed on real clips (`mistake-tense-01`: hyp `"I HAVE WENT THERE YESTERDAY AND I DIDN'T KNOW ANY"` vs ref `"i have went there yesterday and i didn't knew anyone"` — the *rest* of the utterance is essentially correct; `mistake-tense-02`: hyp `"'S STORE EVERY DAY LAST WEEK"` vs ref `"she go to the store every day last week"` — the first two words are gone entirely). This is a property of this specific 20M chunk-transducer export, not a decoder-configuration bug. | Payload (41.6 MB) and RSS (164.5 MB) already pass; the only failing non-MPR gates are WER (0.396 vs ≤0.25) and false-correction (0.0556 vs ≤0.05, barely over), both downstream of the same low-capacity-model recognition-quality problem. |
| `whisper-base-en-ggml` | Whisper's language-model prior actively repairs the deliberately incorrect grammar it hears rather than misrecognizing it (concrete false-correction examples in `benchmarks/reports/approval.md`, e.g. `"the dogs is barking"` → `"The dogs are barking."`). This is trained-in behavior of the Whisper family, not an adapter decoding-parameter default. | Silence hallucination (6 non-empty tokens on physical-silence clips) is whisper.cpp's well-documented tendency to emit spurious tokens on near-silent input when its internal no-speech gate does not trigger. **Payload (147.96 MB vs ≤120 MB) is architecturally fixed by the frozen model file and cannot be changed by any decoding/runtime configuration** — only a different (unapproved, unlicensed-in-this-record) quantized export could reduce it, which is out of this remediation's scope (see §4). |
| `whisper-small-en-ggml` | Same LM-prior grammar-correction behavior as `whisper-base-en-ggml`, slightly less severe (false-correction 0.1157 vs 0.1528) because the larger model is closer to the true acoustic signal in some cases. | Silence hallucination (15 tokens, worse than `whisper-base`). **Payload (487.6 MB, 4.1× the 120 MB cap) and single-stream RSS (971.3 MB vs ≤700 MB) are both architecturally fixed by the frozen model file** and cannot be changed by configuration. |

**Structural conclusion before any experiment ran:** among the three
license-clean candidates, the only one small enough to ever satisfy the
payload (≤120 MB) and RSS (≤700 MB) gates is `sherpa-zipformer-en-20M-2023-02-17-int8`
(41.6 MB / 164.5 MB), and it is also the candidate furthest from the
fidelity gate (MPR 0.3241, `mistake-tense` sub-gate 0.0435). Both
`whisper-*` candidates are closer on fidelity but are unfixably over
budget on payload/RSS without quantization work this spec explicitly
excludes (section 4, "Model fine-tuning, training, quantization
authoring... out of scope"). This tension — the small-enough candidate is
too weak, the accurate-enough candidates are too big — is the real reason
no candidate can pass every gate today, and is why the experiments below
focus on `sherpa-zipformer-en-20M-2023-02-17-int8`: it is the only
candidate for which a fidelity improvement could actually complete a
full-gate pass.

## 2. Experiments performed (representative subsets first)

All commands used `--candidates-dir`/`--runs-dir` pointing at a local
scratch path outside `benchmarks/candidates/**`; the frozen candidate
files were never edited. Subsets: `mistake-tense` (20 clips: 12 primary +
8×48kHz duplicates — the single-worst-scoring sub-gate) and
`fluent-control` (16 clips — bounds WER-fluent/false-correction).

### 2.1 Decoder search width: `greedy_search` vs `modified_beam_search`

Legitimate per-candidate config already exposed by the adapter and
descriptor (`decoding.method`); zero adapter code change required.
`max_active_paths = 4` was already hardcoded identically for both methods.

| Metric | `mistake-tense` greedy (baseline) | `mistake-tense` beam | `fluent-control` greedy (baseline) | `fluent-control` beam |
|---|---|---|---|---|
| WER | 0.6608 | **0.5439** (−18% rel.) | 0.7286 | **0.4429** (−39% rel.) |
| MPR | 0.0435 | 0.0435 (unchanged) | n/a | n/a |

Beam search gives a real, repeatable WER improvement (worth adopting for
any future candidate that ships with `modified_beam_search` available) but
**does not move MPR at all**: the specific tokens inside annotated error
spans are lost the same way regardless of search width, because the
failure is early-context loss, not search-width-limited suboptimality.

Confirmed at full-corpus scale (130 clips, 1 repetition, `asap` pace):

| Metric | Greedy baseline (3 reps) | Beam (1 rep) | Verdict |
|---|---|---|---|
| `fidelity.mpr_overall` | 0.3241 | 0.3102 | **No credible improvement** (within noise, still 0.58 below the 0.90 gate) |
| `fidelity.false_correction_rate` | 0.0556 | 0.0556 | **Unchanged**, still fails ≤0.05 |
| `accuracy.wer_overall` | 0.3959 | 0.3758 | Real but small improvement (−5% rel.), still fails ≤0.25 |
| `hallucination.silence_tokens` | 0 (PASS) | 1 | **New regression** — would flip this gate to FAIL |

**Verdict: not a credible path to passing.** Beam search was not adopted
for any frozen candidate.

### 2.2 Cold-start mitigation: synthetic silence warm-up before real audio

Implemented as a real, tested, backward-compatible adapter feature
(`benchmarks/adapters/sherpa-onnx/src/main.cc`, `warmupSilenceMs` job
field wired from an optional `DecodingDescriptor.warmup_silence_ms`,
`benchmarks/harness/src/candidate.rs` / `adapter/protocol.rs` /
`main.rs`): feed N ms of zero-valued synthetic audio through the
recognizer before the clip's real samples, so the streaming model's
internal state has time to leave its cold-start transient before the
first real word arrives. Absent/`None` for every frozen candidate
descriptor — zero behavior change unless explicitly opted in.

Tested at 300 ms on the `mistake-tense` subset:

| Metric | Baseline (no warm-up) | 300 ms warm-up |
|---|---|---|
| MPR | 0.0435 | 0.0435 (unchanged) |
| WER | 0.6608 | 0.6082 (modest, within clip-to-clip noise) |

Inspecting individual hypotheses showed the warm-up did **not** fix the
early-word-loss pattern (`mistake-tense-02`: still `"'S STORE EVERY DAY
LAST WEEK"`, missing `"she go to the"`) and in one case made a clip
*worse* (`mistake-tense-01-48k` went from a partially-correct hypothesis
to a completely empty final). This confirms the root cause is not a
literal "not enough audio buffered yet" warm-up problem — feeding digital
silence provides no useful acoustic content for the encoder to condition
on, so it does not help. **Verdict: not a credible path to passing.**

This capability is kept in the adapter/harness (fully opt-in, zero effect
on any frozen candidate, real regression test coverage added in
`candidate.rs`) as a documented, reproducible negative result and a
building block if a future candidate's actual failure mode is genuinely a
buffering issue — per `code-standards.md`, "Benchmark changes to
model/runtime configuration against the Mistaken-specific test corpus" and
"Record benchmark configuration so results are reproducible."

### 2.3 Diagnostic: 48 kHz resampling path

While reviewing `mistake-tense` hypotheses, the 8×48kHz duplicate clips
scored consistently worse than their 16kHz originals:

| Sample rate | MPR (`mistake-tense`, 3 reps) | WER |
|---|---|---|
| 16000 Hz (native, 12 clips × 3 reps = 36 records) | 0.0714 | 0.6117 |
| 48000 Hz (duplicate, 8 clips × 3 reps = 24 records) | 0.0000 | 0.7353 |

Real and measurable, but **not the dominant cause**: even the native 16
kHz clips score 0.0714 MPR on this sub-gate, nowhere close to the required
0.85. Fixing sherpa-onnx's internal 48→16 kHz resampling quality (out of
this remediation's scope — it lives inside the vendored, pinned
`k2-fsa/sherpa-onnx` runtime, not the adapter) would at best nudge the
`mistake-tense` sub-gate from ~0.043 toward ~0.07, still an order of
magnitude short of 0.85. Recorded as a finding, not pursued further.

### 2.4 Avenues considered and not implemented (with reasoning)

- **Whisper `initial_prompt` / `no_speech_thold` tuning:** a real,
  implementable lever (whisper.cpp's `whisper_full_params` exposes both),
  and plausibly the correct fix for both the grammar-auto-correction and
  silence-hallucination failure modes. Not implemented this session
  because both `whisper-base-en-ggml` and `whisper-small-en-ggml` are
  **unfixably over the payload gate** (147.96 MB and 487.6 MB vs a 120 MB
  ceiling) regardless of any decoding-parameter change — engineering
  effort here cannot produce a candidate that clears every gate, so it was
  deprioritized in favor of the one candidate (`sherpa-zipformer-en-20M-2023-02-17-int8`)
  for which a fidelity fix could actually complete a full pass. This
  remains the correct next diagnostic step if a future, smaller Whisper
  export enters the candidate set through the normal license/provenance
  process (see §4).
- **Sherpa `chunk_ms` feed-granularity sweep:** `chunk_ms` is currently
  hardcoded to 100 in `cmd_run` rather than sourced from the candidate
  descriptor. Not pursued: the warm-up experiment (§2.2) already showed
  that more or differently-shaped leading audio does not fix the
  early-token-loss pattern, and the online recognizer's internal
  "is-ready" gating is governed by the model's fixed encoder chunk size,
  not by how finely the adapter subdivides its `AcceptWaveform` calls, so
  a `chunk_ms` sweep would not be expected to behave differently from the
  warm-up experiment's negative result.
- **Endpoint rule (`rule1`/`rule2`/`rule3`) retuning:** these are already
  pinned to Mistaken's frozen production values; retuning them changes
  segmentation timing, not per-token recognition accuracy, so it cannot
  address the dominant MPR failure mode.

## 3. Conclusion

**No candidate now clears all `mac-arm64` gates. STOP per the task's
explicit instruction — no full 3-repetition re-run was performed, because
no experiment showed credible improvement toward the fidelity gate.**

The two real, working, adopted-in-code-but-not-in-any-frozen-candidate
changes are: (1) `modified_beam_search` as an available decoding option
(real WER improvement, zero MPR improvement, a new silence-hallucination
regression — net not beneficial for this candidate) and (2) an optional
`warmupSilenceMs` cold-start mitigation (no measurable improvement). Both
are backward-compatible, tested, and unused by every frozen candidate
descriptor; `benchmarks/candidates/*.json` and
`benchmarks/corpus/manifest.json` are byte-for-byte unchanged from the
2026-09-15 mac-arm64 evidence run.

## 4. Required next legitimate model-selection action

Configuration remediation inside the existing frozen candidate family is
exhausted. The next legitimate action is **model selection**, not
configuration, and requires the full Spec 05 candidate-addition process
(license/provenance research, checksum registration, both-host
benchmarking) rather than a quiet substitution:

1. A **smaller, license-clean Whisper (or comparable non-streaming)
   export** — ideally ≤ 120 MB uncompressed — that preserves
   `whisper-small-en-ggml`'s relatively better MPR/WER while fitting the
   payload/RSS budget. This is the most promising direction: `whisper-small`
   already has the best MPR (0.6667) and WER (0.0886) of the frozen set:
   closing the size gap, not the fidelity gap, is now the binding
   constraint for that architecture family.
2. In parallel, **explicit anti-correction prompt/threshold tuning**
   (`initial_prompt`, `no_speech_thold`, `suppress_nst`) should be
   benchmarked against whatever smaller Whisper export is selected in (1),
   since it was not exercised this session but remains the most plausible
   lever for the false-correction and silence-hallucination gates
   specifically.
3. The `sherpa-zipformer-en-2023-06-26-*` license question (upstream
   `Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17`
   declaring no license) remains unresolved and continues to block those
   two candidates regardless of any accuracy work.
4. A `win-x64` reference host remains unobtained this session; per
   AC17 no approval is possible on `mac-arm64` evidence alone even once a
   candidate clears every fidelity/accuracy/hallucination/latency/
   resource/size gate.

None of the above authorizes starting Spec 06.
