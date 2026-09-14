# Building the sherpa-onnx adapter

This is an online (one-time, explicitly operator-initiated) staging step.
Every *measured* benchmark run afterward executes with networking disabled
(Spec 05 section 10, "Offline and privacy").

## 1. Vendor the pinned upstream source

```bash
cd benchmarks/.vendor
git clone --depth 1 --branch v1.13.8 https://github.com/k2-fsa/sherpa-onnx.git sherpa-onnx
```

Record the exact commit built (the tag `v1.13.8` may be re-tagged upstream;
this repository's evidence record captures the resolved commit SHA at
implementation time). `benchmarks/.vendor/` is Git-ignored — this checkout
never enters version control and is never modified in place (it is
read-only, protected third-party source per `docs/context/ai-workflow-rules.md`,
"Protected Files").

## 2. Install a local C++ toolchain and CMake

macOS: `brew install cmake` (Xcode Command Line Tools provide `clang`/`cc`).
No other local dependency is required (Spec 05 section 10, "Permissions").

## 3. Configure and build the adapter

```bash
cd benchmarks/adapters/sherpa-onnx
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j
```

This pulls in the vendored sherpa-onnx source as a CMake subdirectory (the
same integration pattern sherpa-onnx's own `c-api-examples/` use), builds
`sherpa-onnx-core`/`sherpa-onnx-c-api` from source with the C API enabled
and every optional subsystem this benchmark does not need disabled
(Python bindings, tests, PortAudio, JNI, WebSocket server, GPU, TTS, speaker
diarization, example binaries), and produces
`benchmarks/adapters/sherpa-onnx/build/sherpa-onnx-adapter`.

The first configure downloads a prebuilt static ONNX Runtime archive from
the `csukuangfj/onnxruntime-libs` GitHub release matching the host platform
(`onnxruntime-osx-arm64-static_lib-*.zip` on `mac-arm64`); this is the one
network access this staging step performs, and it is separate from every
measured run.

## 4. Verify the adapter binary

```bash
file benchmarks/adapters/sherpa-onnx/build/sherpa-onnx-adapter
```

should report a native Mach-O (macOS) or ELF/PE (other hosts) executable.
The harness's `mistaken-bench run` command refuses to start a candidate
whose adapter binary is missing, naming the exact expected path and this
file.

## 5. Protocol summary

The adapter reads exactly one job line (Spec 05 section 6) from stdin,
locates `encoder*.onnx` / `decoder*.onnx` / `joiner*.onnx` / `tokens.txt`
inside the job's `model.modelDir`, builds a native streaming
`SherpaOnnxOnlineRecognizer` (greedy_search, endpoint detection enabled with
Mistaken's frozen 2.4 s / 1.0 s / 15 s rule thresholds), feeds the clip's
WAV in `chunkMs` blocks (sleeping between chunks only in `realtime` pace),
and emits `ready` / `partial` / `final` / `metrics` / `error` NDJSON events
to stdout exactly as documented. It performs no network access and applies
no text transformation beyond what the recognizer itself returns.
