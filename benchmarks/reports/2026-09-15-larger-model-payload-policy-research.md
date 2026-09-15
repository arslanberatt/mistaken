# Spec 05 remediation — payload-policy revision and larger-model research (2026-09-15)

## Task 1 — product-owner decision record

**Decision (explicit, product-owner, this session):** Preserving deliberate
grammatical mistakes remains a hard product requirement. The following
gates are **unchanged**: MPR ≥ 0.90, false-correction rate ≤ 0.05, WER
thresholds (≤ 0.25 overall / ≤ 0.12 `fluent-control`), the physical-silence
hallucination requirement (= 0), and every latency/resource gate (RTF,
peak RSS, sustained CPU, RSS-growth). Real evidence across every candidate
evaluated so far — the original five, `whisper-base-en-q8-ggml`, and
`vosk-small-en-us` — showed the ≤ 120 MiB *bundled model payload* gate
empirically prevented every tested architecture from having enough
capacity to satisfy the fidelity objective, independent of whether the
architecture itself was a good fit.

**Cause:** the ≤ 120 MiB ceiling in Spec 05 section 12 AC15 was authored
assuming the model ships bundled inside the initial application
installer. That assumption is revised: the model does **not** have to be
bundled inside the initial installer.

**New rule (replaces the single-number AC15 ceiling with a two-part
packaging rule):**

1. **Initial desktop application/package size remains separately
   controlled** — this is Specs 13/14's installer-size concern, untouched
   by this decision.
2. **Local ASR model resources may exceed 120 MiB when delivered as a
   separately downloadable, checksum-pinned local resource**, provided:
   - inference remains fully local/offline after download (no cloud ASR
     call, ever, at any point);
   - no paid fallback of any kind;
   - license/provenance and redistribution rights are clear (an `unclear`
     verdict still blocks approval regardless of size or accuracy, per
     the unchanged Spec 05 section 10 rule);
   - model files are checksum- and version-pinned (the existing
     `mistaken-bench fetch`/candidate-descriptor mechanism already
     satisfies this — no new mechanism was built because the existing one
     already meets the requirement);
   - the app can verify model integrity before use (already a Spec 06
     concern per the original spec's "lazy verified model load," restated
     here as a constraint the eventual approved candidate must satisfy);
   - the app has a truthful model-download/install lifecycle (a Spec 06
     concern, not built here — Spec 05 does not touch the application);
   - installer size and model size are documented **separately**, never
     summed into one "app size" figure that hides the download.

**This is a packaging-delivery-mechanism decision, not a threshold
relaxation.** No MPR/false-correction/WER/silence/latency/resource
number changed. The resource constraint is **not silently removed**:
`benchmarks/harness/src/scoring/gates.rs`'s `MAX_MODEL_PAYLOAD_BYTES`
constant (120 MiB) is deliberately left unchanged in code this session,
because a new evidence-based number can only be set once a concrete
candidate is approved and its real payload is known — inventing a round
number now, with no candidate to justify it, would itself be an
unevidenced gate change. Every payload figure in this document and in
`reports/approval.md` is reported honestly regardless of the old
120 MiB figure; candidates were not auto-rejected on payload alone during
this session's evaluation, which is the practical effect of the new
policy pending a final number.

## Task 2 — candidate search (payload no longer the primary filter)

Candidates previously excluded from full consideration *primarily*
because of payload, reconsidered under the new policy:

| Candidate | Payload | Why previously excluded | Reconsidered? |
|---|---|---|---|
| `whisper-small-en-ggml` (already in the frozen set) | 487.6 MB | Payload (3.9×) **and** RSS (971.3 MB > 700 MB, unchanged gate) | **No re-test needed** — RSS gate is not relaxed and was already measured failing; payload relaxation cannot fix an RSS failure. Already fully benchmarked (`reports/2026-09-15-mac-arm64.md`): MPR 0.6667, closest of the original five, still far below 0.90. |
| `whisper-base-en-ggml` / `whisper-base-en-q8-ggml` (already in the set) | 148.0 MB / 81.8 MB | Payload only (fp16); RSS passes (505.9 MB) | Already fully/focus-tested this remediation series: MPR 0.5926 / 0.5323, false-correction 0.1528 / 0.0968 — both fail independent of payload. Not re-tested (unchanged candidates, no new evidence to gain). |
| `vosk-model-en-us-0.22-lgraph` | 214.0 MB extracted (130.6 MB compressed) | Payload only (8 MiB over the old ceiling) | **Tested this session** (see Task 3/4 below) — the natural "scale up the best-behaved architecture" experiment enabled directly by the new policy. |
| `vosk-model-en-us-0.22` (full, non-lgraph) | ~1.8 GB | Payload (far over any reasonable ceiling even relaxed) | **Not tested** — see Task 5 stop-condition reasoning: the lgraph result already shows the primary sub-gate (`mistake-tense`) unmoved by a 3× capacity increase, giving no evidence basis to expect a qualitatively different result from a further ~9× jump, and a ~1.8 GB "separately downloaded resource" starts to strain the spirit of "local resource" even under a relaxed rule. |
| A hypothetical larger, clean-licensed sherpa-onnx transducer (e.g. a non-"small" `desh2608`-family checkpoint) | Unknown | Payload | **Searched, not found.** `desh2608` (the only clean-licensed LibriSpeech streaming-transducer publisher found in this whole remediation series) has published only the 20M "small" checkpoint; the larger `pruned_transducer_stateless7_streaming` checkpoints in the icefall ecosystem trace back to `Zengwei`'s repositories, which — as already documented three times in this series — declare no license. |
| Quantized Whisper variants beyond what was already tested | Various | Payload primarily, grammar-normalization secondarily | **Not tested further.** The task's own framing is explicit: "quantized Whisper variants only if their grammar-normalization behavior can plausibly meet MPR." Every Whisper quantization tested so far (fp16 base, fp16 small, Q8_0 base) shows the *same* false-correction rate as its same-size sibling (quantization does not touch the language-model behavior that causes correction), and there is no plausible mechanism by which a *larger* Whisper checkpoint would correct grammar *less* — larger language models are, if anything, more fluent and more likely to "fix" a disfluent sentence, not less. Testing `whisper-medium`/`large` was judged to have a low plausibility of closing the false-correction gap and a near-certain failure on RSS/latency at that size (1.5–2.9 GB, non-streaming full-attention decode), so it was not pursued, consistent with the task's own "only if plausible" instruction. |

## Task 3 — fidelity-first screen: `vosk-en-us-0.22-lgraph`

Added through the normal process: `benchmarks/candidates/vosk-en-us-0.22-lgraph.json`
(17 individually checksummed files, `mistaken-bench fetch` → `VERIFIED`
for all), reusing the already-built `vosk` adapter unmodified (same
runtime, same C API calls — only the model directory differs). License
row added to `benchmarks/licenses/license-record.md` (same first-party
Alpha Cephei Apache-2.0 chain as `vosk-small-en-us`, independently
restated per AC16's no-cross-reference rule).

Focused subset (real human corpus, `asap` pace, 1 repetition): 64 clips —
`mistake-tense` (20, incl. 48kHz duplicates), `mistake-agreement` (8),
`mistake-minimal-pair` (8), `fluent-control` (16), `noise-silence` (12,
incl. the 4 physical-silence clips).

| Condition | MPR | False-correction | WER | Silence hallucination |
|---|---|---|---|---|
| `mistake-tense` | **0.4348** (identical to `vosk-small-en-us`) | 0.0000 | 0.1754 | — |
| `mistake-agreement` | 0.8750 (vs. 0.7500 small) | 0.0000 | 0.0580 | — |
| `mistake-minimal-pair` | **0.5625** (vs. 0.6250 small — worse) | 0.0000 | 0.2184 | — |
| `fluent-control` | n/a | n/a | 0.1714 | — |
| `noise-silence` | n/a | n/a | 0.3768 | **6 tokens** (all 4 physical-silence clips hallucinate 1–2 tokens each; the small model hallucinated 0) |
| **Combined** | **0.5532** | **0.0000** | **0.1922** | **6** |

**Primary question answered: no, this candidate cannot plausibly achieve
MPR ≥ 0.90.** The `mistake-tense` sub-gate — the single most informative
per-condition measurement, since it directly targets the corpus's core
tense-error family — is **bit-identical** between the 68 MiB small model
and the 204 MiB (3× acoustic-model, 3× decoding-graph) lgraph model.
`mistake-minimal-pair` (the other required ≥ 0.85 sub-gate) got *worse*.
False-correction stayed at a perfect 0.0000 (confirming the architecture
genuinely never grammar-corrects, at either size), and WER improved
materially (0.1922 vs. ~0.21 on comparable conditions for the small
model), but neither improvement reaches the fidelity bar, and the
`mistake-tense` result gives no reason to expect a larger jump would.

**New finding, disclosed honestly:** the larger acoustic model
introduces a **silence-hallucination regression** — a more sensitive
73.7 MB acoustic model (vs. 16.0 MB) picks up faint room-noise-floor
signal on all 4 physical-silence clips that the smaller model correctly
recognized as non-speech. This is a real, measured cost of scaling up
this architecture, not a hypothetical one.

## Task 4 — full Mac-arm64 benchmark

**Not run.** Per the task's explicit instruction to reject early when a
candidate is clearly far from the fidelity gates: MPR 0.5532 combined
(0.4348/0.5625 on the two required sub-gates) is not close to 0.90/0.85,
and the identical `mistake-tense` result across a 3× capacity increase
provides no basis to expect the full corpus would tell a different story.
Running the full 3-repetition benchmark (`asap` + `realtime`, each
several minutes given the model's larger load time) would not change
this conclusion.

## Task 5 — stop condition

**Reached.** Model size is demonstrated, with real evidence at two
distinct sizes within the best-performing architecture family found in
this entire remediation effort, **not to be the actual limiting factor**:

- Small Vosk (68 MiB extracted): MPR 0.5349 combined, `mistake-tense`
  0.4348.
- Medium Vosk (204 MiB extracted, ~3× capacity): MPR 0.5532 combined,
  `mistake-tense` **0.4348 — unchanged**.

Combined with the earlier findings that Whisper's false-correction
behavior is architecture-level (trained-in, not fixable by quantization
or prompting) and that the sherpa-onnx streaming transducer's
early-utterance token loss is a property of its specific export (not
fixed by beam search or warm-up padding), **six candidates across three
architecturally distinct families have now been evaluated with real
measured evidence against the real human corpus, and none clears the
fidelity gate** — independent of the payload ceiling, which this session
explicitly relaxed and then still could not use to produce a passing
result.

**No candidate becomes MAC QUALIFIED. The remaining legitimate choices,
none selected by this session, are:**

1. Commission or fund a purpose-built anti-normalization ASR model —
   explicitly trained or fine-tuned to preserve grammatical errors rather
   than correct them. This is the only path identified in this entire
   remediation effort that directly targets the false-correction failure
   mode without also inheriting either Whisper's language-model bias or
   a small-model's raw accuracy ceiling. Out of Spec 05's scope (section 4
   excludes training/fine-tuning); would require a new spec or an
   explicit scope exception.
2. Revise the product's MPR/false-correction fidelity requirement as a
   deliberate, recorded product decision (not a quiet threshold edit) if
   the product can tolerate a lower bar than originally specified.
3. Keep Spec 05 blocked and treat "no currently available local ASR
   model satisfies Mistaken's fidelity promise, at any size, on any of
   the architectures evaluated" as the honest, evidence-backed current
   answer.

See `benchmarks/reports/approval.md` for the consolidated decision record.
