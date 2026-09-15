# Building the Vosk adapter

Spec 05 remediation candidate-expansion adapter (`spec/05-remediation`,
`docs/context/progress-tracker.md`, "candidate set expanded, no gate
relaxed"). This is an online (one-time, explicitly operator-initiated)
staging step. Every *measured* benchmark run afterward executes with
networking disabled (Spec 05 section 10, "Offline and privacy").

## 1. Vendor the pinned prebuilt library and header

Alpha Cephei (the Vosk maintainer) distributes `libvosk` only as a
prebuilt binary bundled inside its official PyPI wheel, NuGet package, or
npm package — there is no buildable-from-source release archive the way
`sherpa-onnx`/`whisper.cpp` publish one. This mirrors an existing Spec 05
precedent: the sherpa-onnx adapter's own build already depends on a
prebuilt static ONNX Runtime archive fetched from a GitHub release
(`csukuangfj/onnxruntime-libs`). Using Alpha Cephei's own official,
versioned, Apache-2.0-licensed prebuilt artifact is the same pattern,
applied to a different upstream project.

Pinned versions used for this adapter's evidence: PyPI wheel
`vosk-0.3.44-py3-none-macosx_10_6_universal2.whl` (x86_64 + arm64
universal2 `libvosk.dyld`, the latest macOS-published wheel at
implementation time — `vosk-api` v0.3.45 exists as a git tag but has no
published macOS wheel, only Linux/Windows) and the `vosk_api.h` header
from git tag `v0.3.43` (the closest prior tag to the 0.3.44 library
release; verified to declare only functions the 0.3.44 binary actually
exports).

```bash
mkdir -p benchmarks/.vendor/vosk/lib benchmarks/.vendor/vosk/include

# 1a. Fetch the official macOS wheel and extract libvosk
python3 -m pip download vosk==0.3.44 --no-deps -d /tmp/vosk_wheel
cd /tmp/vosk_wheel && unzip -o vosk-0.3.44-py3-none-macosx_10_6_universal2.whl -d extracted
cp extracted/vosk/libvosk.dyld /path/to/benchmarks/.vendor/vosk/lib/libvosk.dylib

# 1b. Fetch the matching C API header at the closest prior git tag
curl -sL -o /path/to/benchmarks/.vendor/vosk/include/vosk_api.h \
  https://raw.githubusercontent.com/alphacep/vosk-api/v0.3.43/src/vosk_api.h
```

Record the exact wheel filename and its SHA-256, and the exact header git
tag/commit built (`benchmarks/reports/2026-09-15-candidate-expansion-research.md`
carries this session's recorded values). `benchmarks/.vendor/` is
Git-ignored — this checkout never enters version control and is never
modified in place (it is read-only, protected third-party source per
`docs/context/ai-workflow-rules.md`, "Protected Files").

Verify the vendored library exports every symbol this adapter calls:

```bash
nm -D benchmarks/.vendor/vosk/lib/libvosk.dylib | grep -E \
  "vosk_model_new|vosk_recognizer_new\b|vosk_recognizer_accept_waveform_s|vosk_recognizer_result|vosk_recognizer_partial_result|vosk_recognizer_final_result|vosk_recognizer_free|vosk_model_free|vosk_set_log_level"
```

## 2. Install a local C++ toolchain and CMake

macOS: `brew install cmake` (Xcode Command Line Tools provide `clang`/`cc`).
No other local dependency — libvosk statically links its own Kaldi/OpenFST
dependencies, so nothing beyond the vendored `.dylib` is required.

## 3. Configure and build the adapter

```bash
cd benchmarks/adapters/vosk
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j
```

This links a small adapter executable against the vendored `libvosk`
(dynamic linking is Alpha Cephei's only distributed form — unlike the
static-linked sherpa-onnx/whisper.cpp adapters) and produces
`benchmarks/adapters/vosk/build/vosk-adapter` with an embedded rpath
pointing at the vendored `.dylib`, so it runs without any extra
`DYLD_LIBRARY_PATH` setup.

## 4. Verify the adapter binary

```bash
file benchmarks/adapters/vosk/build/vosk-adapter
otool -L benchmarks/adapters/vosk/build/vosk-adapter | grep libvosk
```

The first command should report a native Mach-O (macOS) or ELF/PE (other
hosts) executable; the second confirms the binary links the vendored
`libvosk.dylib` by its vendored path, not a system-wide copy. The
harness's `mistaken-bench run` command refuses to start a candidate whose
adapter binary is missing, naming the exact expected path and this file.

## 5. Protocol summary

The adapter reads exactly one job line (Spec 05 section 6) from stdin,
loads a `VoskModel` from the job's `model.modelDir`, creates a
`VoskRecognizer` at the clip's real sample rate (Vosk resamples
internally, same "resample once, inside the recognizer" convention as the
sherpa-onnx adapter), feeds the clip's WAV in `chunkMs` blocks of native
16-bit PCM (sleeping between chunks only in `realtime` pace), and emits
`ready` (`streamingMode: "native-streaming"` — Vosk's C API is a genuine
real-time streaming recognizer, not a simulated rolling window) /
`partial` / `final` / `metrics` / `error` NDJSON events to stdout exactly
as documented. `partial`/`final` text is copied verbatim from Vosk's own
`{"partial": "..."}` / `{"text": "..."}` JSON result fields — no
lowercasing, punctuation, spell-correction, or word substitution. It
performs no network access and applies no text transformation beyond what
the recognizer itself returns.
