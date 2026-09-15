# Spec 05 remediation — fundamentally different architecture research (2026-09-15)

Authorized by the same product-owner candidate-set-expansion decision as
`benchmarks/reports/2026-09-15-candidate-expansion-research.md` (a
same-family Whisper quantization, evaluated and rejected there). This
document covers the deliberately *architecturally different* search:
CTC-style acoustic transcription, compact transducer variants outside the
already-exhausted family, and Kaldi/Vosk-style literal decoding, per the
task's explicit direction not to spend further time tuning the
already-rejected Whisper/Sherpa-transducer configurations.

## Phase 1 — architectures researched

| Architecture class | Concrete option(s) investigated | Why considered "less generative" |
|---|---|---|
| Streaming CTC (icefall zipformer2-ctc) | `sherpa-onnx-streaming-zipformer-ctc-{zh,zh-xlarge,small-ctc-zh}-*` | Frame-synchronous blank-collapsed decode, no autoregressive joiner |
| Offline CTC (icefall zipformer) | `csukuangfj/sherpa-onnx-zipformer-ctc-en-2023-10-02` | Same decode style, non-streaming |
| Offline CTC (NVIDIA NeMo) | `csukuangfj/sherpa-onnx-nemo-ctc-en-citrinet-512`, `-conformer-large` | Non-autoregressive, no internal neural LM |
| Kaldi HMM-DNN + WFST | `vosk-model-small-en-us-0.15` (alphacep/vosk-api) | Frame-level acoustic model decoded through a fixed WFST graph with an n-gram LM baked in — no neural sequence-to-sequence generation at all |

## Phase 2 — paper screen (candidate matrix)

| # | Model | Architecture | Runtime | Payload | License (primary source) | Provenance | Streaming | Verdict |
|---|---|---|---|---|---|---|---|---|
| 1 | `sherpa-onnx-streaming-zipformer-ctc-{zh,zh-xlarge,small}-*` | Streaming Zipformer2-CTC | sherpa-onnx (already vendored) | 25 MiB – 728 MiB | Apache-2.0 (repo) | icefall/k2-fsa | Native | **Rejected — no English model exists.** k2-fsa's own pretrained-model docs (`k2-fsa.github.io/sherpa/onnx/pretrained_models/online-ctc/`) list only Chinese (zipformer-ctc) and Russian (t-one-ctc) streaming CTC checkpoints. Training one would be "model fine-tuning/training," explicitly out of Spec 05's scope (section 4). |
| 2 | `csukuangfj/sherpa-onnx-zipformer-ctc-en-2023-10-02` (int8) | Offline Zipformer-CTC | sherpa-onnx (already vendored, `SherpaOnnxOfflineRecognizer` + `zipformer_ctc` config — new C API path, no new runtime) | 70,239,299 B (≈ 67.0 MiB), fits | apache-2.0 (repo tag) | **`Zengwei/icefall-asr-librispeech-zipformer-transducer-ctc-2023-06-13`: no `license:` tag at all** (verified live, 2026-09-15) | Simulated (offline, would need rolling-window adapter) | **Rejected — `unclear` provenance**, the identical gap already found and disclosed for the excluded `2023-06-26`/`2023-02-21` zipformer transducer candidates. Not benchmarked; Spec 05 section 10 blocks an `unclear` verdict regardless of accuracy. |
| 3 | `csukuangfj/sherpa-onnx-nemo-ctc-en-citrinet-512` (int8) | Offline Citrinet (non-autoregressive CTC) | sherpa-onnx (already vendored, `nemo_ctc` offline config — new C API path, no new runtime) | 38,048,112 B (≈ 36.3 MiB), fits | apache-2.0 (repo tag, self-applied by the converter) | NVIDIA NGC `stt_en_citrinet_512` — **its own NGC model card states "License to use this model is covered by the NGC TERMS OF USE unless another License/Terms Of Use/EULA is clearly specified,"** not a clear open-source grant (verified live at `catalog.ngc.nvidia.com`, 2026-09-15); the sibling `stt_en_citrinet_512_ls` (LibriSpeech-only) *does* declare `cc-by-4.0` on its own Hugging Face card, but no public ONNX conversion of that specific `_ls` checkpoint exists, and converting it ourselves would be "export scripting," out of scope (section 4) | Simulated (offline) | **Rejected — `unclear` provenance.** The converter's self-applied `apache-2.0` tag does not match the upstream NGC model's own stated terms. Not benchmarked. |
| 4 | `vosk-model-small-en-us-0.15` | Kaldi TDNN-F chain acoustic model + WFST (`HCLr.fst`/`Gr.fst`) decoding graph with baked-in n-gram LM | **New adapter, new runtime dependency** (`alphacep/vosk-api`, Apache-2.0) | 70,898,967 B extracted (≈ 67.6 MiB), fits | **Apache 2.0, first-party** (Alpha Cephei is both the runtime and model publisher — no third-party provenance chain to check; verified live at `alphacephei.com/vosk/models` and the `vosk_api.h` license header, 2026-09-15) | First-party, no gap | **Native** (Vosk's C API is designed for real-time partial/final streaming) | **Accepted for evaluation.** |

## Rejection reasoning, restated

- **#1 (streaming CTC, native low-latency, the architecturally ideal fit):**
  rejected purely on availability — no English pretrained artifact exists
  to evaluate, and training one is explicitly out of scope.
- **#2 and #3 (offline CTC via sherpa-onnx, zero new runtime risk):**
  both fit the payload gate comfortably, but both hit the exact same
  provenance pattern already documented three times in this remediation
  series (`2023-06-26` and `2023-02-21` zipformer transducers, now a
  zipformer CTC and a NeMo CTC): a downstream Hugging Face converter
  self-applies an `apache-2.0` tag that does not match what the upstream
  training-checkpoint source or NGC model card actually declares. This is
  a systemic characteristic of the hobbyist `csukuangfj`-converted ONNX
  model zoo, not something specific to CTC vs. transducer architectures.
- **#4 (Vosk):** the only candidate in this entire remediation effort
  (original five, `whisper-base-en-q8-ggml`, and this search) with a
  **first-party, single-vendor, unambiguous license chain** — no upstream
  research-checkpoint conversion to verify at all.

No candidate was rejected for cloud dependency (none of the above have
one), non-streaming latency alone (only #2/#3 were non-streaming, and
both were already rejected on license before latency mattered), or
requiring transcript correction (none post-correct text).

## Phase 3 — added through the normal Spec 05 process

- `benchmarks/candidates/vosk-small-en-us.json`: new candidate descriptor,
  `adapter: "vosk"`, 14 individually checksummed model files (Vosk ships a
  multi-file Kaldi model directory, not a single `.onnx`/`.bin`), fetched
  from `https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip`
  and recomputed independently (not copied from any upstream-published
  checksum list — Alpha Cephei does not publish one).
- `benchmarks/licenses/license-record.md`: new standalone `vosk-small-en-us`
  row, `mistaken-bench licenses --check` → `PASS (7 candidate(s) complete)`.
- `benchmarks/adapters/vosk/`: **new adapter** (`src/main.cc`,
  `src/json_lite.h`, `src/wav_reader.h`, `CMakeLists.txt`, `build.md`).
  Links against a vendored, pinned, Git-ignored prebuilt `libvosk.dylib`
  (Alpha Cephei publishes no source-buildable release archive — only a
  prebuilt binary via PyPI/NuGet/npm — so this mirrors the existing
  precedent of the sherpa-onnx adapter's build already depending on a
  prebuilt ONNX Runtime static library fetched from a GitHub release).
  Exact pin: PyPI wheel `vosk-0.3.44-py3-none-macosx_10_6_universal2.whl`
  for the library (latest macOS-published build; `v0.3.45` exists as a
  git tag but ships no macOS wheel) and `vosk_api.h` from git tag
  `v0.3.43` (closest prior tag; diffed line-for-line against `v0.3.45`'s
  header — only one added, unrelated function and a doc-comment fix, no
  change to any function this adapter calls). `mistaken-bench fetch
  --candidate vosk-small-en-us` → `VERIFIED` for all 14 files.
- No existing candidate file, adapter, or evidence record was modified or
  replaced; `benchmarks/candidates/*.json` for the original five and
  `whisper-base-en-q8-ggml` are byte-for-byte unchanged.

## Phase 4 — focused fidelity screen (real human corpus, `asap` pace, 1 repetition)

90 of 130 clips: `mistake-tense` (20, incl. 8×48kHz duplicates),
`mistake-agreement` (8), `mistake-article` (6), `mistake-preposition` (8),
`mistake-minimal-pair` (8), `fluent-control` (16), `fast-speech` (12),
`noise-silence` (12, incl. the 4 physical-silence clips).

| Condition | MPR | False-correction | WER | Silence hallucination |
|---|---|---|---|---|
| `mistake-tense` | 0.4348 | 0.0435 | 0.1813 | — |
| `mistake-agreement` | 0.7500 | 0.0000 | 0.1449 | — |
| `mistake-article` | 0.8571 | 0.0000 | 0.0962 | — |
| `mistake-preposition` | 0.5556 | 0.0000 | 0.3699 | — |
| `mistake-minimal-pair` | 0.6250 | 0.0000 | 0.1724 | — |
| `fluent-control` | n/a (no spans) | n/a | 0.1643 | — |
| `fast-speech` | 0.3913 | 0.0870 | 0.2458 | — |
| `noise-silence` | n/a (no spans) | n/a | 0.3913 | **0** (12/12 clips, including all 4 physical-silence clips) |
| **Combined** | **0.5349** | **0.0349** | **0.2144** | **0** |

**This is the first candidate in the entire remediation series (original
five, plus `whisper-base-en-q8-ggml`) whose focused-subset
false-correction rate (0.0349), overall WER (0.2144), and silence
hallucination (0 tokens) all individually clear their respective gates
(≤ 0.05, ≤ 0.25, = 0).** Concrete example confirming genuinely different
behavior from Whisper: `mistake-tense-02-48k`, reference `"she go to the
store every day last week"`, hypothesis `"she go to the store every day
last week"` — the deliberate agreement error (`"she go"`) is preserved
**exactly**, not silently corrected to `"she goes"`.

**But the primary gate, MPR ≥ 0.90, is not close (0.5349 combined; the
`mistake-tense` and `mistake-minimal-pair` sub-gates, which each require
≥ 0.85, measure 0.4348 and 0.6250).** Inspecting individual hypotheses
shows this is driven by **ordinary small-model misrecognition, not
grammar correction** — e.g. `mistake-tense-03`, reference `"we was gone
yesterday before you arrived"`, hypothesis `"he was gone yesterday before
you arrived"`: `"we"` was misheard as `"he"` (an acoustic confusion of a
short function word), which is exactly the kind of error a 40 MB,
size-constrained acoustic model makes, and is architecturally unrelated
to the grammar-auto-correction failure mode Whisper exhibits. This
confirms the "less generative" hypothesis is directionally correct —
Vosk's false-correction rate is the best measured in this entire
remediation series — but this specific *small* Vosk model's raw acoustic
accuracy is not good enough to close the fidelity gap.

**No larger Vosk model fits the payload gate for this remediation to
try:** `vosk-model-en-us-0.22-lgraph` (128 MiB, "big model with dynamic
graph," WER 7.82 vs the small model's 9.85) is 8 MiB over the 120 MiB
ceiling; the full `vosk-model-en-us-0.22` is 1.8 GiB. This is the same
"the small-enough model is too weak, the accurate-enough model is too
big" tension already documented for the Whisper family and the
sherpa-onnx 20M transducer.

## Phase 5/6 — decision

**No full 3-repetition Mac-arm64 benchmark was run for `vosk-small-en-us`.**
Per the task's explicit instruction to reject early when focused MPR is
nowhere near 0.90: 0.5349 combined (0.4348/0.6250 on the two per-condition
sub-gates) is not close, despite every other individual gate measured
clean on this subset. Running the full corpus would not change that
conclusion, and per Phase 6's stop condition, this session does not keep
searching indefinitely once a genuinely different architecture has been
found, evaluated, and still falls short on the one gate the whole
remediation effort is centered on.

**No candidate — the original five, `whisper-base-en-q8-ggml`, or
`vosk-small-en-us` — clears every Mac-arm64 gate. MAC QUALIFIED status has
not been reached by any candidate.**

See `benchmarks/reports/approval.md` for the consolidated decision and the
exact product-constraint statement for the product owner.
