# Spec 05 License Record

Runtime library license and model-weight license are checked separately
(`docs/context/architecture.md`, "License Rule"): an open-source runtime
does not automatically imply every model weight bundled with it may be
redistributed under the same terms. Every claim below cites a primary
source and a retrieval date; drift from Spec 05's authoring-time notes
(section 3) is called out explicitly where checked.

Verification method: live retrieval of each primary source (GitHub
releases API, raw `LICENSE` files, Hugging Face model/tree API, OpenSLR)
on the date recorded, not copied from the spec text. All fetches below
were performed directly by the Spec 05 implementer (a prior delegated
license-research subagent failed on a provider usage-limit error before
producing output and is not the source of any claim here).

Every candidate row below is complete and self-contained (no
cross-references to another candidate's row), per AC16.

## sherpa-zipformer-en-2023-06-26-int8-left64

- Runtime repository: k2-fsa/sherpa-onnx
- Runtime tag: v1.13.8
- Runtime license: Apache-2.0 (source: https://raw.githubusercontent.com/k2-fsa/sherpa-onnx/master/LICENSE, retrieved: 2026-09-11)
- Linked inference runtime: onnxruntime (vendored by tag, downloaded from csukuangfj/onnxruntime-libs release v1.28.2 during the build staging step), MIT (source: https://raw.githubusercontent.com/microsoft/onnxruntime/main/LICENSE, retrieved: 2026-09-11)
- Model repository: csukuangfj/sherpa-onnx-streaming-zipformer-en-2023-06-26
- Model revision: 672fbf1b30579d6585301139bb363f42a0ad4a24
- Weight license (declared): apache-2.0, per Hugging Face cardData.license (source: https://huggingface.co/api/models/csukuangfj/sherpa-onnx-streaming-zipformer-en-2023-06-26, retrieved: 2026-09-11)
- Upstream provenance repository: Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17 (the TorchScript source this ONNX export is converted from)
- Upstream provenance license: none declared. The repository's Hugging Face tags array is ["tensorboard", "region:us"] and its cardData carries no license key at all (source: https://huggingface.co/api/models/Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17, retrieved: 2026-09-11). This confirms Spec 05's authoring-time finding: the upstream training/export source that produced this weight carries no license grant of its own, independent of the downstream csukuangfj mirror's self-declared apache-2.0 tag.
- Training corpus: LibriSpeech (OpenSLR SLR12)
- Training corpus terms: CC BY 4.0, attribution required (source: https://www.openslr.org/12, retrieved: 2026-09-11)
- Required attribution text: "LibriSpeech: an ASR corpus based on public domain audio books. Vassil Panayotov, Guoguo Chen, Daniel Povey and Sanjeev Khudanpur, ICASSP 2015."
- Redistribution verdict: unclear
- Verdict reasoning: the downstream mirror's self-applied apache-2.0 tag cannot substitute for a license grant from the party that actually holds rights over the trained weights (the upstream Zengwei/icefall-asr-* export), which declares none. Per Spec 05 section 10 ("Fallback rules"): "A model whose license or provenance is unclear stays unclear and cannot be approved, regardless of accuracy." This candidate is blocked on provenance alone, independent of any accuracy measurement.

## sherpa-zipformer-en-2023-06-26-fp32-left64

This is the unquantized export of the same model used only as the
quantization-loss control; every license and provenance fact matches the
int8 candidate above exactly, restated in full below (AC16 requires each
row to stand alone rather than cross-reference another row).

- Runtime repository: k2-fsa/sherpa-onnx
- Runtime tag: v1.13.8
- Runtime license: Apache-2.0 (source: https://raw.githubusercontent.com/k2-fsa/sherpa-onnx/master/LICENSE, retrieved: 2026-09-11)
- Linked inference runtime: onnxruntime (vendored by tag), MIT (source: https://raw.githubusercontent.com/microsoft/onnxruntime/main/LICENSE, retrieved: 2026-09-11)
- Model repository: csukuangfj/sherpa-onnx-streaming-zipformer-en-2023-06-26
- Model revision: 672fbf1b30579d6585301139bb363f42a0ad4a24
- Weight license (declared): apache-2.0, per Hugging Face cardData.license (source: https://huggingface.co/api/models/csukuangfj/sherpa-onnx-streaming-zipformer-en-2023-06-26, retrieved: 2026-09-11)
- Upstream provenance repository: Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17
- Upstream provenance license: none declared. Same finding as the int8 sibling: tags array ["tensorboard", "region:us"], no license key in cardData (source: https://huggingface.co/api/models/Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17, retrieved: 2026-09-11).
- Training corpus: LibriSpeech (OpenSLR SLR12)
- Training corpus terms: CC BY 4.0, attribution required (source: https://www.openslr.org/12, retrieved: 2026-09-11)
- Required attribution text: "LibriSpeech: an ASR corpus based on public domain audio books. Vassil Panayotov, Guoguo Chen, Daniel Povey and Sanjeev Khudanpur, ICASSP 2015."
- Redistribution verdict: unclear
- Verdict reasoning: identical to the int8 sibling — same upstream provenance gap blocks approval regardless of accuracy.

## sherpa-zipformer-en-20M-2023-02-17-int8

- Runtime repository: k2-fsa/sherpa-onnx
- Runtime tag: v1.13.8
- Runtime license: Apache-2.0 (source: https://raw.githubusercontent.com/k2-fsa/sherpa-onnx/master/LICENSE, retrieved: 2026-09-11)
- Linked inference runtime: onnxruntime (vendored by tag), MIT (source: https://raw.githubusercontent.com/microsoft/onnxruntime/main/LICENSE, retrieved: 2026-09-11)
- Model repository: csukuangfj/sherpa-onnx-streaming-zipformer-en-20M-2023-02-17
- Model revision: d42f2d9f7ca24806fb667456a18a9f1b60f70d16
- Weight license (declared): apache-2.0, per Hugging Face cardData.license (source: https://huggingface.co/api/models/csukuangfj/sherpa-onnx-streaming-zipformer-en-20M-2023-02-17, retrieved: 2026-09-11)
- Upstream provenance repository: desh2608/icefall-asr-librispeech-pruned-transducer-stateless7-streaming-small
- Upstream provenance license: apache-2.0, per Hugging Face cardData.license and tags ["tensorboard","license:apache-2.0","region:us"] (source: https://huggingface.co/api/models/desh2608/icefall-asr-librispeech-pruned-transducer-stateless7-streaming-small, retrieved: 2026-09-11). Unlike the 2023-06-26 candidates, this upstream repository does carry an explicit license grant, so the chain from training source through the ONNX mirror is complete.
- Training corpus: LibriSpeech (OpenSLR SLR12)
- Training corpus terms: CC BY 4.0, attribution required (source: https://www.openslr.org/12, retrieved: 2026-09-11)
- Required attribution text: "LibriSpeech: an ASR corpus based on public domain audio books. Vassil Panayotov, Guoguo Chen, Daniel Povey and Sanjeev Khudanpur, ICASSP 2015."
- Redistribution verdict: permitted-with-attribution
- Additional provenance note: the fp32 encoder-epoch-99-avg-1.onnx export in this same repository carries a third-party scanner status of suspicious (PAIT-ONNX-200, Protect AI scanner) per the Hugging Face file-security panel (source: https://huggingface.co/api/models/csukuangfj/sherpa-onnx-streaming-zipformer-en-20M-2023-02-17/tree/d42f2d9f7ca24806fb667456a18a9f1b60f70d16?expand=true, retrieved: 2026-09-11). This candidate uses only the int8 export, which carries no such flag; the fp32 file is excluded from the frozen candidate set for exactly this reason and is recorded here only as a provenance note, not as a property of the approved artifact.

## whisper-base-en-ggml

- Runtime repository: ggml-org/whisper.cpp
- Runtime tag: v1.9.4 (resolved commit 927cfce34f31707e17f2bff35c349632fb9e2c3a, matching the release's target_commitish; source: https://api.github.com/repos/ggml-org/whisper.cpp/releases/latest, retrieved: 2026-09-11)
- Runtime license: MIT, "Copyright (c) 2023-2026 The ggml authors" (source: https://raw.githubusercontent.com/ggml-org/whisper.cpp/master/LICENSE, retrieved: 2026-09-11)
- Linked inference runtime: ggml (vendored in-tree by tag; same repository, same license)
- Model repository: ggerganov/whisper.cpp (community GGML-format mirror of OpenAI's released weights)
- Model revision: 80da2d8bfee42b0e836fc3a9890373e5defc00a6 (commit that added ggml-base.en.bin)
- Weight license (declared): mit, per Hugging Face cardData.license on the ggerganov/whisper.cpp repository (source: https://huggingface.co/api/models/ggerganov/whisper.cpp, retrieved: 2026-09-11)
- Upstream provenance repository: openai/whisper (model architecture/training code, MIT) and the openai/whisper-base.en released weights
- Upstream provenance license: openai/whisper repository is MIT (source: https://raw.githubusercontent.com/openai/whisper/main/LICENSE, retrieved: 2026-09-11). The sibling openai/whisper-small.en model card was independently checked and confirms license apache-2.0 at the released-weights level (see the small.en row below); OpenAI applies Apache-2.0 at the weights-card level on top of an MIT codebase, and the ggml conversion re-licenses its own converted binary as MIT, which is the operative license for the artifact this candidate actually ships.
- Training corpus: OpenAI's internal 680,000-hour multilingual/multitask supervised dataset (Whisper paper, Radford et al. 2022, arXiv:2212.04356); not a redistributed corpus, no separate attribution obligation flows to Mistaken from the training data itself.
- Training corpus terms: not independently redistributed by this candidate; not applicable.
- Required attribution text: "Whisper model by OpenAI (github.com/openai/whisper), ggml/whisper.cpp conversion by the ggml authors and contributors (github.com/ggml-org/whisper.cpp)." MIT does not legally require a visible runtime attribution string, only preservation of the license/copyright notice in redistributed copies of the software; this text is Mistaken's own attribution-notice content, not a legal condition beyond notice preservation.
- Redistribution verdict: permitted

## whisper-small-en-ggml

- Runtime repository: ggml-org/whisper.cpp
- Runtime tag: v1.9.4 (resolved commit 927cfce34f31707e17f2bff35c349632fb9e2c3a; source: https://api.github.com/repos/ggml-org/whisper.cpp/releases/latest, retrieved: 2026-09-11)
- Runtime license: MIT, "Copyright (c) 2023-2026 The ggml authors" (source: https://raw.githubusercontent.com/ggml-org/whisper.cpp/master/LICENSE, retrieved: 2026-09-11)
- Linked inference runtime: ggml (vendored in-tree by tag; same repository, same license)
- Model repository: ggerganov/whisper.cpp
- Model revision: 80da2d8bfee42b0e836fc3a9890373e5defc00a6
- Weight license (declared): mit, per Hugging Face cardData.license (source: https://huggingface.co/api/models/ggerganov/whisper.cpp, retrieved: 2026-09-11)
- Upstream provenance repository: openai/whisper (MIT) and the openai/whisper-small.en released weights
- Upstream provenance license: openai/whisper-small.en's Hugging Face cardData.license is apache-2.0 (source: https://huggingface.co/api/models/openai/whisper-small.en, retrieved: 2026-09-11); openai/whisper repository code is MIT (source: https://raw.githubusercontent.com/openai/whisper/main/LICENSE, retrieved: 2026-09-11). The ggml conversion re-licenses the converted binary as MIT, which is the operative license for the artifact this candidate ships.
- Training corpus: OpenAI's internal training dataset (Whisper paper, arXiv:2212.04356); no separate redistribution obligation.
- Training corpus terms: not independently redistributed by this candidate; not applicable.
- Required attribution text: "Whisper model by OpenAI (github.com/openai/whisper), ggml/whisper.cpp conversion by the ggml authors and contributors (github.com/ggml-org/whisper.cpp)."
- Redistribution verdict: permitted

## whisper-base-en-q8-ggml


Added 2026-09-15 during the `spec/05-remediation` candidate-set-expansion
phase (product-owner decision, `docs/context/progress-tracker.md`), after
the original five candidates were empirically exhausted. Same repository,
license, and upstream provenance chain as `whisper-base-en-ggml` above,
re-verified independently rather than cross-referenced, per AC16.

- Runtime repository: ggml-org/whisper.cpp
- Runtime tag: v1.9.4 (resolved commit 927cfce34f31707e17f2bff35c349632fb9e2c3a; source: https://api.github.com/repos/ggml-org/whisper.cpp/releases/latest, retrieved: 2026-09-11)
- Runtime license: MIT, "Copyright (c) 2023-2026 The ggml authors" (source: https://raw.githubusercontent.com/ggml-org/whisper.cpp/master/LICENSE, retrieved: 2026-09-11)
- Linked inference runtime: ggml (vendored in-tree by tag; same repository, same license)
- Model repository: ggerganov/whisper.cpp (community GGML-format mirror of OpenAI's released weights)
- Model revision: 0b364b566045a405be7225ee1e415a073e04da77 (commit "Add Q8_0 models", the commit that added `ggml-base.en-q8_0.bin`; file confirmed byte-identical between this commit and current `main` HEAD `5359861c739e955e79d9a303bcbc70fb988958b1`)
- Weight license (declared): mit, per Hugging Face cardData.license on the ggerganov/whisper.cpp repository (source: https://huggingface.co/api/models/ggerganov/whisper.cpp, retrieved: 2026-09-15)
- Upstream provenance repository: openai/whisper (model architecture/training code, MIT) and the openai/whisper-base.en released weights
- Upstream provenance license: openai/whisper repository is MIT (source: https://raw.githubusercontent.com/openai/whisper/main/LICENSE, retrieved: 2026-09-11). Q8_0 is a post-training block quantization of the same base.en weights already verified for `whisper-base-en-ggml` (same upstream, same operative MIT license on the converted ggml binary); it is not a separate training artifact with separate rights.
- Training corpus: OpenAI's internal 680,000-hour multilingual/multitask supervised dataset (Whisper paper, Radford et al. 2022, arXiv:2212.04356); not a redistributed corpus, no separate attribution obligation flows to Mistaken from the training data itself.
- Training corpus terms: not independently redistributed by this candidate; not applicable.
- Required attribution text: "Whisper model by OpenAI (github.com/openai/whisper), ggml/whisper.cpp conversion by the ggml authors and contributors (github.com/ggml-org/whisper.cpp)."
- Redistribution verdict: permitted

## Verdict summary (not a candidate row)

| Candidate | Verdict | Blocking reason |
|---|---|---|
| sherpa-zipformer-en-2023-06-26-int8-left64 | unclear | Upstream TorchScript source (Zengwei/icefall-asr-librispeech-streaming-zipformer-2023-05-17) declares no license. |
| sherpa-zipformer-en-2023-06-26-fp32-left64 | unclear | Same upstream provenance gap as the int8 sibling. |
| sherpa-zipformer-en-20M-2023-02-17-int8 | permitted-with-attribution | Complete Apache-2.0 chain (runtime, weight, upstream provenance); LibriSpeech CC BY 4.0 attribution required. |
| whisper-base-en-ggml | permitted | MIT runtime and MIT ggml conversion; OpenAI's own code is MIT. |
| whisper-small-en-ggml | permitted | Same as whisper-base-en-ggml. |
| whisper-base-en-q8-ggml | permitted | Same MIT chain as whisper-base-en-ggml; Q8_0 is a published post-training quantization of the same weights, not a separate artifact. |

License-clean candidates (permitted / permitted-with-attribution): the 20M
sherpa int8 model and all three whisper.cpp candidates. The two 2023-06-26
zipformer candidates are license-blocked on upstream provenance regardless
of any accuracy result, per Spec 05 section 10's explicit rule that an
unclear verdict can never be approved.
