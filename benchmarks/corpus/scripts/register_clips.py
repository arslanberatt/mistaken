#!/usr/bin/env python3
"""Register real recorded clips into benchmarks/corpus/manifest.json.

Spec 05's harness (`mistaken-bench validate-corpus`) checks each clip's
*declared* sha256/durationMs against the *recomputed* value from the actual
WAV file (see benchmarks/harness/src/manifest.rs `validate_with_files`). It
does not write those values back into the manifest -- Spec 05's own
"Ordered Implementation Plan" step 8 says the operator "register[s]
durations and checksums in the manifest" as a manual step. This script
automates exactly that mechanical step, and nothing else:

  - It recomputes sha256 and durationMs from the real WAV file bytes/header
    using the identical method the harness uses (whole-file SHA-256;
    duration = floor(frames * 1000 / sample_rate)).
  - It writes those two fields back into the matching manifest.json clip
    entry.
  - It DOES NOT touch `reference`, `errorSpans`, `condition`, `promptFile`,
    `speakerProfileId`, or `expectPhysicalSilence`. Per protocol.md section
    4, the `reference` text must match what the speaker *actually said*,
    which only a human reviewer can confirm against the recording. Editing
    that field is the operator's job, not this script's.
  - It DOES NOT touch `speakerProfiles[].consentGiven`. Consent is an
    explicit human act (protocol.md section 3) and is never inferred from
    the presence of an audio file.

Usage:
    python3 benchmarks/corpus/scripts/register_clips.py [--dry-run]

Run from the repository root (or anywhere; paths are resolved relative to
this script's location). After running, always finish with:

    ./benchmarks/harness/target/release/mistaken-bench validate-corpus

validate-corpus remains the single source of truth for "the corpus is
ready"; this script only removes the tedious/error-prone part of getting
there.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import wave
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
CORPUS_DIR = SCRIPT_DIR.parent
MANIFEST_PATH = CORPUS_DIR / "manifest.json"
CLIPS_DIR = CORPUS_DIR / "clips"


def sha256_file(path: Path) -> str:
    # Matches benchmarks/harness/src/manifest.rs::sha256_file: hash of the
    # raw file bytes, not the decoded PCM samples.
    h = hashlib.sha256()
    h.update(path.read_bytes())
    return h.hexdigest()


def wav_duration_ms(path: Path, expected_rate: int) -> tuple[int, int, int]:
    """Returns (duration_ms, sample_rate, sample_width_bytes)."""
    with wave.open(str(path), "rb") as w:
        frames = w.getnframes()
        rate = w.getframerate()
        width = w.getsampwidth()
        channels = w.getnchannels()
    if channels != 1:
        raise ValueError(f"expected mono, got {channels} channel(s)")
    if width != 2:
        raise ValueError(f"expected 16-bit PCM (2 bytes/sample), got {width * 8}-bit")
    if rate != expected_rate:
        raise ValueError(f"expected {expected_rate} Hz, got {rate} Hz")
    # Integer floor division, matching manifest.rs:
    # (reader.duration() as u64 * 1000) / spec.sample_rate.max(1)
    duration_ms = (frames * 1000) // max(rate, 1)
    return duration_ms, rate, width


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="report what would change without writing manifest.json",
    )
    args = parser.parse_args()

    if not MANIFEST_PATH.is_file():
        print(f"error: manifest not found at {MANIFEST_PATH}", file=sys.stderr)
        return 2

    manifest = json.loads(MANIFEST_PATH.read_text())
    clips = manifest["clips"]
    clips_by_file = {c["file"]: c for c in clips}

    updated: list[str] = []
    unchanged: list[str] = []
    missing: list[str] = []
    errors: list[str] = []

    for clip in clips:
        rel_file = clip["file"]
        wav_path = CORPUS_DIR / rel_file
        if not wav_path.is_file():
            missing.append(clip["id"])
            continue
        try:
            duration_ms, rate, _width = wav_duration_ms(wav_path, clip["sampleRateHz"])
        except (wave.Error, ValueError) as exc:
            errors.append(f"{clip['id']}: {exc}")
            continue
        new_sha256 = sha256_file(wav_path)
        old_sha256 = clip["sha256"]
        old_duration = clip["durationMs"]
        if new_sha256 == old_sha256 and duration_ms == old_duration:
            unchanged.append(clip["id"])
            continue
        if not args.dry_run:
            clip["sha256"] = new_sha256
            clip["durationMs"] = duration_ms
        updated.append(
            f"{clip['id']}: sha256 {old_sha256[:12]}...->{new_sha256[:12]}..., "
            f"durationMs {old_duration}->{duration_ms}"
        )

    # Extra files on disk not referenced by any manifest entry -- almost
    # always a naming mistake (protocol.md section 5 file-naming rule).
    extras: list[str] = []
    if CLIPS_DIR.is_dir():
        for wav_path in sorted(CLIPS_DIR.glob("*.wav")):
            rel = f"clips/{wav_path.name}"
            if rel not in clips_by_file:
                extras.append(rel)

    if updated and not args.dry_run:
        MANIFEST_PATH.write_text(json.dumps(manifest, indent=2) + "\n")

    print(f"Registered/updated: {len(updated)}")
    for line in updated:
        print(f"  UPDATED {line}")
    print(f"Already correct:    {len(unchanged)}")
    print(f"Missing from disk:  {len(missing)} / {len(clips)} total manifest entries")
    if errors:
        print(f"Format errors:      {len(errors)}")
        for line in errors:
            print(f"  ERROR {line}")
    if extras:
        print(f"Unreferenced files in clips/ (naming mismatch?): {len(extras)}")
        for line in extras:
            print(f"  EXTRA {line}")

    if args.dry_run and updated:
        print("\n(--dry-run: manifest.json was NOT written)")

    print(
        "\nNext: run "
        "./benchmarks/harness/target/release/mistaken-bench validate-corpus "
        "to confirm, then review `reference`/`errorSpans` against what was "
        "actually spoken, and set speakerProfiles[].consentGiven only after "
        "real informed consent (protocol.md section 3)."
    )

    return 1 if (errors or missing) else 0


if __name__ == "__main__":
    raise SystemExit(main())
