# ASR Build Notes — Offline Linking and Staged Archives

## Overview

Mistaken uses the official `sherpa-onnx` Rust crate (`v1.13.8`) with static linking (`default-features = false`, `features = ["static"]`) for local speech recognition.

The upstream `sherpa-onnx-sys` crate build script downloads a prebuilt native library archive from GitHub Releases at build time if no local override is configured. To ensure deterministic, reproducible, and offline-capable builds—and to avoid unverified binary downloads during compilation—Mistaken builds must set `SHERPA_ONNX_ARCHIVE_DIR` (or `SHERPA_ONNX_LIB_DIR`) to point to a locally staged, checksum-verified archive.

Prebuilt binary archives are never committed to version control. They are staged in a local directory (e.g., `src-tauri/.vendor/sherpa-onnx-archives/` which is ignored by `.gitignore`) or in an external cache directory.

## Pinned Archive Identities

### macOS (Apple Silicon, `aarch64-apple-darwin`)

- **Archive filename:** `sherpa-onnx-v1.13.8-osx-arm64-static-lib.tar.bz2`
- **Byte size:** `20,965,936` bytes
- **SHA-256 checksum:** `9091bf160dc7fdacedbc906b212badf53c2993f4e5277a0e03998e96c31d60da`
- **Release source:** `https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-osx-arm64-static-lib.tar.bz2`

### Windows (x64 MSVC, `x86_64-pc-windows-msvc`)

- **Archive filename:** `sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2`
- **Byte size:** `123,206,268` bytes
- **SHA-256 checksum:** `56ffcf3c454c1f14f7bc9887286cc8143e7e542dc632804e1c447d5f8d534eaf`
- **Release source:** `https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2`

## Staging and Verification Procedure

### macOS

1. Stage the archive in a local directory (e.g. `src-tauri/.vendor/sherpa-onnx-archives/`):
   ```bash
   mkdir -p src-tauri/.vendor/sherpa-onnx-archives
   cd src-tauri/.vendor/sherpa-onnx-archives
   curl -LO https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-osx-arm64-static-lib.tar.bz2
   ```

2. Verify file size and SHA-256 digest:
   ```bash
   test $(stat -f%z sherpa-onnx-v1.13.8-osx-arm64-static-lib.tar.bz2) -eq 20965936
   echo "9091bf160dc7fdacedbc906b212badf53c2993f4e5277a0e03998e96c31d60da  sherpa-onnx-v1.13.8-osx-arm64-static-lib.tar.bz2" | shasum -a 256 -c -
   ```

3. Export the environment variable before building:
   ```bash
   export SHERPA_ONNX_ARCHIVE_DIR="$(pwd)/src-tauri/.vendor/sherpa-onnx-archives"
   ```

### Windows (PowerShell)

1. Stage the archive:
   ```powershell
   New-Item -ItemType Directory -Force -Path src-tauri\.vendor\sherpa-onnx-archives
   cd src-tauri\.vendor\sherpa-onnx-archives
   Invoke-WebRequest -Uri "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2" -OutFile "sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2"
   ```

2. Verify file size and SHA-256 digest:
   ```powershell
   (Get-Item sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2).Length -eq 123206268
   (Get-FileHash -Algorithm SHA256 sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2).Hash -eq "56FFCF3C454C1F14F7BC9887286CC8143E7E542DC632804E1C447D5F8D534EAF"
   ```

3. Set the environment variable before building:
   ```powershell
   $env:SHERPA_ONNX_ARCHIVE_DIR = "$PWD\src-tauri\.vendor\sherpa-onnx-archives"
   ```

## Offline Build Verification

With `SHERPA_ONNX_ARCHIVE_DIR` set and network disabled:
```bash
cargo build --manifest-path src-tauri/Cargo.toml
```
The build completes successfully without network access because `sherpa-onnx-sys` extracts the prebuilt libraries directly from the staged archive.
