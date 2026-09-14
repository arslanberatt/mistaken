# Building the whisper.cpp adapter

One-time, explicitly operator-initiated staging step (Spec 05 section 10,
"Offline and privacy"); every *measured* run afterward executes with
networking disabled.

## 1. Vendor the pinned upstream source

```bash
cd benchmarks/.vendor
git clone --depth 1 --branch v1.9.4 https://github.com/ggml-org/whisper.cpp.git whisper-cpp
```

`benchmarks/.vendor/` is Git-ignored and never modified in place (protected
third-party source per `docs/context/ai-workflow-rules.md`).

## 2. Install a local C++ toolchain and CMake

Same requirement as the sherpa-onnx adapter: `brew install cmake` on macOS;
no other local dependency.

## 3. Configure and build the adapter

```bash
cd benchmarks/adapters/whisper-cpp
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j
```

This builds `ggml`/`whisper` from source with every optional
subsystem this benchmark does not need disabled (built-in CLI examples,
server, tests, libcurl model download, SDL2 microphone capture) and
produces `benchmarks/adapters/whisper-cpp/build/whisper-cpp-adapter`. No
model weights are fetched by this step — those are staged separately under
`benchmarks/models/<candidate-id>/` and verified by `mistaken-bench fetch`.

## 4. Verify the adapter binary

```bash
file benchmarks/adapters/whisper-cpp/build/whisper-cpp-adapter
```

## 5. Protocol summary and the honest streaming caveat

whisper.cpp has no native streaming recognizer (Spec 05 section 3): this
adapter's `ready` event always reports `"streamingMode":
"simulated-rolling-window"`. It feeds the clip's WAV in `chunkMs` blocks
(sleeping between blocks only in `realtime` pace), re-decodes the last
`windowMs` of accumulated audio from scratch roughly once per second of new
audio and emits that as a `partial`, then — after every sample has been
fed — runs one full-utterance decode over the entire clip and emits it as
the single authoritative `final`. The rolling-window partials exist only to
produce an honestly-labeled simulated first-partial-latency number; they
never drive the scored transcript, and the harness's report renderer marks
every whisper.cpp candidate's latency row with the same caveat inline next
to the number (Spec 05 section 8).

The adapter also carries its own minimal mono 16-bit PCM WAV reader and a
linear resampler, because whisper.cpp's public API expects 16 kHz input and
performs no internal resampling (unlike sherpa-onnx) — this matters only
for the corpus's 8-clip 48 kHz duplicate subset.
