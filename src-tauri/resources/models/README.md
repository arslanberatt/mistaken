# Local ASR Models

## Status: Development ASR • Not release approved

This directory holds local speech recognition models for Mistaken.

Under the project's ASR maturity contract (see `docs/context/architecture.md`), the model configured here is a **DEVELOPMENT ASR ADAPTER** authorized solely for building and verifying the audio pipeline, IPC, lifecycle, and transcript workspace architecture.

Spec 05 remains **BLOCKED — no production candidate approved**. This development model is **not** production approved.

## Pinned Development Model: `sherpa-zipformer-en-20M-2023-02-17-int8`

- **Candidate ID:** `sherpa-zipformer-en-20M-2023-02-17-int8`
- **Upstream Model Repository:** `csukuangfj/sherpa-onnx-streaming-zipformer-en-20M-2023-02-17`
- **Revision:** `d42f2d9f7ca24806fb667456a18a9f1b60f70d16`
- **Runtime:** `sherpa-onnx` `v1.13.8` (Apache-2.0, static linking)
- **License Status:** `permitted-with-attribution` (Apache-2.0)
- **Measured Benchmark Fidelity:** Real-human-corpus Mistake Preservation Rate (MPR) is **0.3241** (fails the unchanged Spec 05 production gate of >= 0.90).

## Required Model Files and Checksums

Model files are placed under `resources/models/sherpa-zipformer-en-20M-2023-02-17-int8/`:

| File | Bytes | SHA-256 Checksum |
|---|---|---|
| `encoder-epoch-99-avg-1.int8.onnx` | 42,845,182 | `3810755ce7c3ab26b42a8bcf39d191308fa27fb0f53358823ba46141d03b7eb3` |
| `decoder-epoch-99-avg-1.int8.onnx` | 539,499 | `21e2a2acd961b3ac72f55be2f10f1a285e1b0b0ba010d7c0b6eab141411b163c` |
| `joiner-epoch-99-avg-1.int8.onnx` | 259,572 | `e085d73b593cf9b0707f370dbd656d58327d3fe36d80d849202ef81df02cb01e` |
| `tokens.txt` | 5,048 | `49e3c2646595fd907228b3c6787069658f67b17377c60aeb8619c4551b2316fb` |

All four files must match these exact sizes and SHA-256 digests. Mistaken verifies these checksums on first load and refuses to load any altered or unexpected file.

## Attribution

This model is derived from the Next-gen Kaldi project (k2-fsa / sherpa-onnx) under the Apache License 2.0.
Original repository: https://github.com/k2-fsa/sherpa-onnx
Model repository: https://huggingface.co/csukuangfj/sherpa-onnx-streaming-zipformer-en-20M-2023-02-17
