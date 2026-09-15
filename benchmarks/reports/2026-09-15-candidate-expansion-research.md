# Spec 05 candidate-set expansion research (2026-09-15)

Authorized by the explicit product-owner decision recorded in
`docs/context/progress-tracker.md` (2026-09-15, "candidate set expanded, no
gate relaxed"), after `benchmarks/reports/2026-09-15-remediation-experiments.md`
established that no legitimate configuration change to the original five
frozen candidates can plausibly close the fidelity gap. This document
records every candidate investigated for possible addition, with license,
provenance, and payload verification performed the same way the original
Spec 05 candidates were verified (primary-source retrieval, not copied
claims). All facts below were retrieved live on 2026-09-15.

## Constraint recap (unchanged from Spec 05, section 12)

Any new candidate must still pass every existing numbered gate to be
approved: MPR ≥ 0.90 overall (≥ 0.85 on `mistake-tense`/`mistake-minimal-pair`),
false-correction rate ≤ 0.05, WER ≤ 0.25 overall / ≤ 0.12 `fluent-control`,
zero non-empty finals on physical-silence clips, ≤ 0.02 insertion
tokens/sec on noise clips, ≤ 900 ms median first-partial / ≤ 1500 ms p95
final-after-endpoint (`realtime` pace), RTF ≤ 0.6 single-stream, peak RSS
≤ 700 MB single-stream, payload ≤ 120 MB (125,829,120 bytes) uncompressed,
and a `permitted`/`permitted-with-attribution` license verdict. A model
whose provenance is `unclear` cannot be approved regardless of accuracy
(Spec 05 section 10).

## Candidate 1 — REJECTED before benchmarking: `sherpa-onnx-streaming-zipformer-en-2023-02-21` (mid-size, int8)

A same-family, higher-capacity streaming Zipformer than the already-tested
20M model, hosted at
`csukuangfj/sherpa-onnx-streaming-zipformer-en-2023-02-21` (retrieved via
the Hugging Face model + tree API, 2026-09-15).

- **Model/version:** `pruned_transducer_stateless7_streaming`, epoch 30
  avg 9, exported to ONNX by `csukuangfj`, revision (repo HEAD)
  `bfbb11950302dad9955f8ede01f0e768595e4fba`.
- **Runtime:** would reuse the already-vendored, already-pinned
  `k2-fsa/sherpa-onnx v1.13.8` and the existing `sherpa-onnx` adapter —
  zero new runtime code.
- **Declared license:** `apache-2.0` (Hugging Face `cardData.license`).
- **Upstream provenance:** the model card states the TorchScript source is
  `Zengwei/icefall-asr-librispeech-pruned-transducer-stateless7-streaming-2022-12-29`.
  That upstream repository's Hugging Face `tags` array is
  `["tensorboard", "region:us"]` — **no `license:` tag at all**, the exact
  same provenance gap already found and disclosed for the excluded
  `sherpa-zipformer-en-2023-06-26-*` candidates. Verdict would be
  **`unclear`**, which Spec 05 section 10 states can never be approved
  regardless of accuracy.
- **Payload (the actual disqualifying reason, checked first):**
  `encoder-epoch-99-avg-1.int8.onnx` 126,967,571 B +
  `decoder-epoch-99-avg-1.int8.onnx` 540,643 B +
  `joiner-epoch-99-avg-1.int8.onnx` 259,380 B + `tokens.txt` 5,048 B =
  **127,772,642 B ≈ 121.85 MiB — 1.85 MiB over the 120 MiB (125,829,120 B)
  ceiling even in its most compressed (int8) export.** This repository
  has no `bpe.model`/other smaller-vocabulary file to trim, and no fp32
  export is smaller. There is no quantization work in scope that shrinks
  an already-int8 ONNX encoder further without re-authoring it (out of
  scope per section 4).
- **Decision: rejected, not benchmarked.** Two independent, sufficient
  disqualifiers (payload over budget, upstream license unclear) — either
  alone blocks approval regardless of accuracy, so no compute was spent
  measuring MPR/WER for this candidate. Recorded here to close the
  question rather than leave it unresearched, per the task's instruction
  not to hide considered-and-rejected options.

## Candidate 2 — ACCEPTED for evaluation: `whisper-base-en-q8_0-ggml`

The already-verified, already-benchmarked `whisper-base-en-ggml` candidate
(MPR 0.5926, WER 0.1097, the second-strongest fidelity candidate in the
frozen set) quantized to `Q8_0` by the same upstream maintainer
(`ggerganov`/`ggml-org`), in the same repository, under the same license,
using a published artifact rather than any quantization performed by this
session (Spec 05 section 4 permits evaluating "only published artifacts").

- **Exact model/version:** `ggml-base.en-q8_0.bin`, from
  `ggerganov/whisper.cpp` at commit `0b364b566045a405be7225ee1e415a073e04da77`
  ("Add Q8_0 models", 2024-10-29T17:49:50Z — the commit that added this
  exact file; verified unchanged between that commit and the current
  `main` HEAD `5359861c739e955e79d9a303bcbc70fb988958b1`).
- **Runtime/version:** reuses the already-vendored, already-pinned
  `ggml-org/whisper.cpp v1.9.4` and the existing `whisper-cpp` adapter
  unmodified — whisper.cpp's `whisper_init_from_file_with_params` detects
  a GGML file's quantization type from its own header; Q8_0/Q5_1 support
  has existed in `ggml`/`whisper.cpp` since 2023, long before v1.9.4.
  **Zero adapter or runtime code change required for this candidate.**
- **Download/source:**
  `https://huggingface.co/ggerganov/whisper.cpp/resolve/0b364b566045a405be7225ee1e415a073e04da77/ggml-base.en-q8_0.bin`.
- **License:** MIT — identical `cardData.license: "mit"` already verified
  live for this exact repository in `benchmarks/licenses/license-record.md`'s
  `whisper-base-en-ggml`/`whisper-small-en-ggml` rows; this is the same
  repository, same license, same upstream (OpenAI Whisper, MIT code /
  Apache-2.0 weights card), only a different file within it.
- **Redistribution rights:** identical to the already-approved-for-license
  `whisper-base-en-ggml`/`whisper-small-en-ggml` rows (`permitted`) —
  Q8_0 is a lossy compression of the same MIT-licensed weights, not a
  different training artifact with different rights.
- **Exact payload size:** 81,781,811 bytes (≈ 78.0 MiB) — well within the
  120 MiB (125,829,120 B) ceiling, with ~44 MiB of headroom (vs.
  `whisper-base-en-ggml`'s 147,964,211 B, ~22.1 MiB over).
- **SHA-256:** `a4d4a0768075e13cfd7e19df3ae2dbc4a68d37d36a7dad45e8410c9a34f8c87e`
  (Hugging Face LFS `oid`, verified at both the adding commit and current
  `main`).
- **Architecture/runtime implications:** identical inference path to
  `whisper-base-en-ggml` (same `whisper_full` full-utterance decode, same
  `simulated-rolling-window` adapter labeling, same non-streaming
  architecture) — only the weight precision changes (8-bit block
  quantization vs. the original fp16 weights). Expected effect: a small,
  well-documented WER regression (published community benchmarks for
  Q8_0 typically show < 1% relative WER change vs. fp16 for `.en` models;
  this session measures the real number rather than assuming it) and
  materially lower RSS at load time than the fp16 file, though runtime
  RSS is dominated by decode buffers, not just the resident weight file.
- **Why it may improve mistake preservation:** it does not change
  `whisper-base-en-ggml`'s language-model grammar-correction behavior at
  all — this candidate is added to test whether that model can *fit the
  payload gate* at acceptable accuracy loss, not to fix the
  grammar-correction failure mode. If its MPR/false-correction/silence
  numbers land close to `whisper-base-en-ggml`'s (0.5926 / 0.1528 / 6
  tokens), the same MPR shortfall applies and this candidate will also
  fail the fidelity gate — that is a real, expected possible outcome and
  is reported honestly either way.
- **Silence/VAD handling:** identical to `whisper-base-en-ggml` (relies on
  `whisper_full`'s internal `no_speech_thold`/`suppress_nst` behavior,
  unmodified); no change expected or introduced.
- **Expected latency:** identical adapter/architecture to
  `whisper-base-en-ggml` (`simulated-rolling-window`); a smaller
  quantized weight file decodes somewhat faster per token, which may
  modestly help RTF/CPU time but is not expected to change the
  fundamentally different (full-utterance decode) latency profile that
  already fails the ≤ 1500 ms p95 final-after-endpoint gate for both
  whisper candidates.
- **Decision: accepted for focused-subset evaluation.** This is the
  single most surgical possible addition: reuses the existing adapter,
  existing runtime build, existing license verification, and differs from
  an already-measured candidate by exactly one already-published,
  same-provenance file. No new runtime, no new adapter, no new license
  research risk.

## Candidates considered but not investigated further (reasoning only)

- **A new runtime/adapter (e.g. Vosk/Kaldi, NeMo Parakeet, Moonshine):**
  would require vendoring a new pinned runtime, writing a new adapter
  process, a new `build.md`, and full license/provenance research from
  zero — a materially larger engineering process than the task's
  instruction to "prefer the smallest number of strong candidates."
  Deferred unless the surgical whisper.cpp quantization path is
  conclusively rejected by real benchmark evidence.
- **`whisper-small.en` quantizations:** checked and rejected on payload
  alone before any license work — `ggml-small.en-q5_1.bin` is
  190,098,681 B (≈ 181 MiB) and `ggml-small.en-q8_0.bin` is
  264,477,561 B (≈ 252 MiB), both still far over the 120 MiB ceiling even
  at the most aggressive commonly-published quantization.
- **`whisper-tiny.en` (fp16 or quantized):** fits the payload gate
  (77,704,715 B fp16; smaller still quantized) but is a strictly
  lower-capacity model than `base.en`, which already has the second-worst
  MPR among the original five candidates — not pursued ahead of the
  `base.en` quantization, which offers strictly more capacity at a
  comparable or smaller size than several `tiny` variants once quantized.

## Focused-subset evaluation: `whisper-base-en-q8-ggml`

Model file fetched and checksum-verified
(`mistaken-bench fetch --candidate whisper-base-en-q8-ggml` → `VERIFIED`).
License record added as its own standalone row in
`benchmarks/licenses/license-record.md`; `mistaken-bench licenses --check`
→ `PASS (6 candidate(s) complete)`. `validate-corpus` unaffected and still
clean — no corpus, manifest, or scoring file was touched.

Representative focused subset (real human corpus, `asap` pace, 1
repetition, `mac-arm64`), covering the categories the task required:
`mistake-tense` (20, includes the 8×48kHz duplicates), `mistake-minimal-pair`
(8), `fluent-control` (16), `fast-speech` (12), `noise-silence` (12,
includes the 4 physical-silence clips) — 68 of 130 clips.

### Step 1 — baseline (no decoding-parameter changes)

| Metric | `whisper-base-en-q8-ggml` (baseline) | `whisper-base-en-ggml` fp16 sibling, same conditions |
|---|---|---|
| MPR (combined subset) | 0.5323 | 0.5323 |
| False-correction rate | 0.0968 | 0.0968 |
| WER | 0.0923 | 0.0940 |
| Silence hallucination tokens | 2 | 6 |

Per-condition MPR, false-correction, and WER are **bit-identical** to the
already-fully-benchmarked fp16 sibling on every non-silence condition —
Q8_0 quantization is genuinely near-lossless for this model on this
corpus. Only the `noise-silence` condition differs (fewer hallucinated
tokens even before any tuning: 2 vs 6). This confirms the quantization
fixes the payload gate without a fidelity regression, but inherits
`whisper-base-en-ggml`'s exact MPR/false-correction shortfall.

### Step 2 — decoding-parameter tuning (new adapter/harness capability)

Discovered while inspecting `whisper_full_params` defaults in the vendored
`ggml`/`whisper.cpp` source: **`suppress_nst` (non-speech-token
suppression) defaults to `false`** in the library, and the adapter never
set it explicitly — every previously frozen Whisper candidate has run
with this suppression *off* the entire time. Implemented three new
optional, backward-compatible decoding fields
(`DecodingDescriptor`/`Job`: `suppressNst`, `noSpeechThold`,
`initialPrompt`; whisper-cpp adapter only, absent = library default,
zero effect on any other candidate) and tested three configurations on
the same 68-clip subset:

| Configuration | MPR | False-corr. | WER | Silence tokens |
|---|---|---|---|---|
| Baseline (Step 1) | 0.5323 | 0.0968 | 0.0923 | 2 |
| `suppressNst: true, noSpeechThold: 0.3` | 0.5323 (unchanged) | 0.0968 (unchanged) | **0.0889** (slightly better) | **0 (fixed)** |
| `initialPrompt` (anti-grammar-correction instruction) | 0.5161 (worse) | 0.1129 (worse) | 0.1128 (worse) | 6 (worse) |
| `suppressNst` + `noSpeechThold` + `initialPrompt` combined | 0.5161 (worse) | 0.1129 (worse) | 0.1504 (much worse) | 28 (much worse) |

**Findings:**

- `suppressNst: true` + `noSpeechThold: 0.3` is a **genuine, adopted
  improvement**: it eliminates silence hallucination on this subset (2→0
  tokens) with no measured downside (MPR/false-correction unchanged, WER
  marginally better). Adopted into
  `benchmarks/candidates/whisper-base-en-q8-ggml.json`'s frozen decoding
  block as this candidate's evaluated configuration.
- The `initialPrompt` anti-correction instruction **does not work and
  makes results worse** across every metric. Whisper's `initial_prompt`
  mechanism is a vocabulary/topic-context primer, not an
  instruction-following interface (the underlying model was never
  instruction-tuned); prepending an imperative sentence measurably
  degrades decoding rather than suppressing grammar correction. **Not
  adopted.**
- Combining the prompt with `suppressNst` is **worse than either
  change alone** on silence handling specifically (28 hallucinated tokens
  vs. 2 baseline) — a real, disclosed negative interaction, not a
  fabricated or assumed one.

### Verdict for `whisper-base-en-q8-ggml`

With its best evaluated configuration (`suppressNst: true,
noSpeechThold: 0.3`, now the candidate's frozen decoding block):

| Gate | Best-config result | Threshold | Result |
|---|---|---|---|
| Payload | 81,781,811 B (≈ 78.0 MiB) | ≤ 120 MiB | **PASS** (was the blocking gate for the fp16 sibling) |
| Silence hallucination | 0 tokens (this subset) | = 0 | **PASS** (was failing at 2/6) |
| MPR overall | 0.5323 (this subset) | ≥ 0.90 | **FAIL by a wide margin** |
| False-correction rate | 0.0968 (this subset) | ≤ 0.05 | **FAIL** (roughly 2× the ceiling) |
| WER (fluent-control) | 0.0286 | ≤ 0.12 | PASS |

**Decision: rejected before a full 3-repetition Mac-arm64 run.** Per the
task's explicit instruction to reject candidates early when they clearly
cannot approach the frozen gates: MPR and false-correction are the two
gates this whole remediation exists to satisfy, and both are unchanged
from the already-fully-measured fp16 sibling (0.5323 vs. the fp16
sibling's full-corpus 0.5926 — same order of magnitude, both roughly 0.35
below the 0.90 gate). No full benchmark run was performed because the
focused subset already conclusively demonstrates this candidate cannot
pass; spending ~10 minutes of compute on `asap`×3 plus ~40 minutes on
`realtime`×1 for a foregone-conclusion result would contradict the task's
efficiency instruction.

## Outcome of this research phase

One candidate added and evaluated: **`whisper-base-en-q8-ggml`**. It fixes
the payload gate that blocked its fp16 sibling and, with a newly
discovered and adopted `suppressNst`/`noSpeechThold` tuning, also fixes
the silence-hallucination gate — but it does not approach the fidelity
(MPR ≥ 0.90) or false-correction (≤ 0.05) gates, which remain identical
to its already-measured fp16 sibling. **No candidate — original five or
this new sixth — clears every Mac-arm64 gate.** See
`benchmarks/reports/approval.md` for the consolidated decision and the
next legitimate model-selection direction.
