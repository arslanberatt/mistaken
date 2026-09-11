# Mistaken

Mistaken is a local desktop transcription application for macOS and Windows.
It captures the user's microphone and the computer's system audio as two
independent sources, transcribes both locally on-device, and renders a raw
live conversation transcript with no grammar correction. See
[`docs/context/project-overview.md`](docs/context/project-overview.md) for
the full product description.

This is the Wave 1 bootstrap baseline (Spec 01): a real Tauri 2 desktop
window that honestly reports the runtime is ready while audio capture and
transcription are not configured yet. No audio, ASR, or IPC feature exists
in this baseline.

## Prerequisites

- Node (see [`.nvmrc`](.nvmrc)) and npm.
- Rust, installed via [rustup](https://rustup.rs) with the toolchain pinned
  in [`rust-toolchain.toml`](rust-toolchain.toml) (includes `rustfmt` and
  `clippy`).
- macOS: Xcode Command Line Tools. Windows: Microsoft C++ Build Tools and
  WebView2 (validated by a later platform spec).

## Local Commands

```bash
npm ci                # install from the committed lockfile
npm run dev            # Vite development server (frontend only)
npm run typecheck      # strict TypeScript check, no emit
npm run lint           # oxlint
npm run build          # typecheck + production Vite build
npm test               # Vitest suite (jsdom)
npm run tauri dev       # real desktop development launch
npm run tauri build -- --debug --no-bundle   # native/frontend integration build
```

Rust commands run from `src-tauri`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo check
```

## Offline and Privacy Baseline

- No account, authentication, backend, or database.
- No required internet connection after dependencies are installed.
- No transcript, model, recording, or telemetry file is created at this
  stage.
- The only registered native plugin is the official Tauri clipboard manager,
  scoped to plain-text writes only.

## Canonical Documentation

- [`docs/context/`](docs/context/) — product, architecture, UI, code
  standards, progress tracker, spec plan, and workflow rules.
- [`docs/specs/`](docs/specs/) — every implementation spec (01–15).
- [`AGENTS.md`](AGENTS.md) — required agent entrypoint and workflow rules.

This repository is the canonical source once its baseline commit exists;
`/Users/berat/mistaken-context` remains the immutable original staging
bundle.
