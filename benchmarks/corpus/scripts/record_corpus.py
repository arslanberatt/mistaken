#!/usr/bin/env python3
"""Interactive local recording walker for the Spec 05 human corpus.

This is disposable operator tooling, not part of the frozen Spec 05 harness
contract. It does not read or write anything outside `benchmarks/corpus/`
and it never touches `manifest.json`. Its only job is to make the manual
steps in `RECORDING_CHECKLIST.md` (read protocol.md first) less tedious:

  - Walk the 130 manifest clips (122 primary + 8 mistake-tense 48 kHz
    duplicates) in manifest order, one at a time.
  - Show the exact prompt text, condition, speaker profile, and required
    sample rate for the current clip, straight from `manifest.json` and
    the matching `prompts/*.md` file -- never a re-typed copy.
  - Record with `ffmpeg` (macOS `avfoundation` input) only when the
    operator explicitly presses Enter to start, and only until the
    operator explicitly presses Enter again to stop (SIGINT to ffmpeg,
    which finalizes the WAV normally). Nothing records on its own.
  - Let the operator play back the take (`afplay`) and choose to keep it,
    re-record it, or move on -- before anything lands in
    `benchmarks/corpus/clips/<file>.wav` (the exact filename the manifest
    already declares).
  - Apply zero processing: no filters, no gain, no resampling beyond the
    plain sample-rate the manifest already asks for at capture, no
    trimming. (Leading/trailing silence trimming, if desired, stays a
    manual step per protocol.md section 2 -- this script does not trim.)

This script is macOS-specific (`avfoundation` + `afplay`) because that is
the only host this corpus is being recorded on. It is intentionally not a
cross-platform tool.

Usage:
    python3 benchmarks/corpus/scripts/record_corpus.py [--device N]

After a session, finish with:
    python3 benchmarks/corpus/scripts/register_clips.py
    ./benchmarks/harness/target/release/mistaken-bench validate-corpus
"""

from __future__ import annotations

import argparse
import json
import re
import signal
import subprocess
import sys
import time
import wave
from pathlib import Path
from typing import Optional

SCRIPT_DIR = Path(__file__).resolve().parent
CORPUS_DIR = SCRIPT_DIR.parent
MANIFEST_PATH = CORPUS_DIR / "manifest.json"
PROMPTS_DIR = CORPUS_DIR / "prompts"
CLIPS_DIR = CORPUS_DIR / "clips"

META_LINE_RE = re.compile(r"^\*\*([^:*]+):\*\*\s*(.*)$")
TEXT_BLOCK_RE = re.compile(r"```text\n(.*?)\n```", re.DOTALL)
DEVICE_LINE_RE = re.compile(r"^\[AVFoundation indev[^\]]*\]\s*\[(\d+)\]\s+(.+)$")


def load_manifest() -> dict:
    return json.loads(MANIFEST_PATH.read_text())


def parse_prompt(prompt_path: Path) -> dict:
    """Extracts metadata fields and the verbatim text block from a prompt
    file. Never mutates the file. Returns {} fields for anything absent
    (e.g. physical-silence prompts have no text block)."""
    raw = prompt_path.read_text()
    fields: dict[str, str] = {}
    for line in raw.splitlines():
        m = META_LINE_RE.match(line.strip())
        if m:
            key = m.group(1).strip().lower()
            val = m.group(2).strip().strip("`")
            fields[key] = val
    text_match = TEXT_BLOCK_RE.search(raw)
    fields["_text"] = text_match.group(1).strip() if text_match else ""
    return fields


def list_avfoundation_audio_devices() -> list[tuple[int, str]]:
    proc = subprocess.run(
        ["ffmpeg", "-f", "avfoundation", "-list_devices", "true", "-i", ""],
        capture_output=True,
        text=True,
    )
    devices: list[tuple[int, str]] = []
    in_audio = False
    for line in proc.stderr.splitlines():
        if "AVFoundation audio devices:" in line:
            in_audio = True
            continue
        if "AVFoundation video devices:" in line:
            in_audio = False
            continue
        if in_audio:
            m = DEVICE_LINE_RE.match(line)
            if m:
                devices.append((int(m.group(1)), m.group(2).strip()))
    return devices


def choose_device(preselected: Optional[int]) -> int:
    devices = list_avfoundation_audio_devices()
    if not devices:
        print("error: no AVFoundation audio input devices found.", file=sys.stderr)
        sys.exit(2)
    if preselected is not None:
        if preselected not in (idx for idx, _ in devices):
            print(f"error: --device {preselected} not in {devices}", file=sys.stderr)
            sys.exit(2)
        return preselected
    print("\nAvailable audio input devices:")
    for idx, name in devices:
        print(f"  [{idx}] {name}")
    if len(devices) == 1:
        idx = devices[0][0]
        print(f"Only one device found; using [{idx}] {devices[0][1]}.")
        return idx
    while True:
        choice = input("Use which device index? > ").strip()
        try:
            idx = int(choice)
        except ValueError:
            continue
        if idx in (i for i, _ in devices):
            return idx


def check_consent_gate(manifest: dict) -> None:
    print("\n" + "=" * 72)
    print("CONSENT CHECK (protocol.md section 3 / RECORDING_CHECKLIST.md section 1)")
    print("=" * 72)
    profiles = manifest.get("speakerProfiles", [])
    any_true = False
    for profile in profiles:
        pid = profile.get("id", "?")
        consent = profile.get("consentGiven")
        print(f"  speakerProfiles[].id={pid!r}  consentGiven={consent!r}")
        if consent is True:
            any_true = True
    print()
    print("This script will NEVER read or write speakerProfiles[].consentGiven.")
    print("It does not know whether real informed consent has actually been given.")
    print()
    print("Fields YOU must personally edit in benchmarks/corpus/manifest.json,")
    print("and ONLY after the named speaker has actually given informed consent")
    print("per protocol.md section 3, are exactly:")
    for profile in profiles:
        pid = profile.get("id", "?")
        print(
            f'    speakerProfiles[] entry with "id": "{pid}"  ->  set "consentGiven": true'
        )
    if any_true:
        print(
            "\nNote: at least one profile above already shows consentGiven: true."
        )
    print(
        "\nA profile left at consentGiven: false must not be recorded "
        "(protocol.md section 3)."
    )
    print(
        "This gate does not block recording mechanically -- it exists so you "
        "cannot proceed without reading it."
    )
    answer = input(
        "\nType 'yes' to confirm every speaker you are about to record has "
        "already given real informed consent out loud, right now, before any "
        "microphone opens: "
    ).strip().lower()
    if answer != "yes":
        print("Consent not confirmed. Exiting without recording anything.")
        sys.exit(1)


def wav_duration_seconds(path: Path) -> float:
    with wave.open(str(path), "rb") as w:
        frames = w.getnframes()
        rate = w.getframerate()
    return frames / float(rate) if rate else 0.0


def record_take(device_index: int, sample_rate: int, dest: Path) -> bool:
    """Records until the operator presses Enter. Returns True if a file was
    written. Recording never starts until the operator presses Enter here,
    and never stops until the operator presses Enter again."""
    input(
        f"\nPress ENTER to START recording now at {sample_rate} Hz "
        f"(device [{device_index}])... "
    )
    dest.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        "ffmpeg",
        "-hide_banner",
        "-loglevel",
        "warning",
        "-f",
        "avfoundation",
        "-i",
        f":{device_index}",
        "-ac",
        "1",
        "-ar",
        str(sample_rate),
        "-c:a",
        "pcm_s16le",
        "-y",
        str(dest),
    ]
    proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL)
    # Give ffmpeg a moment to actually open the device before we allow stop.
    time.sleep(0.3)
    print("Recording... speak now. Press ENTER to STOP.")
    input()
    proc.send_signal(signal.SIGINT)
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.terminate()
        proc.wait(timeout=5)
    if not dest.is_file() or dest.stat().st_size == 0:
        print("Recording failed: no output file written.")
        return False
    dur = wav_duration_seconds(dest)
    print(f"Saved take: {dest}  ({dur:.2f}s)")
    return True


def play_take(path: Path) -> None:
    if not path.is_file():
        print("Nothing to play yet -- no take recorded for this clip.")
        return
    subprocess.run(["afplay", str(path)])


def staging_path(final_path: Path) -> Path:
    return final_path.with_name(f".staging-{final_path.name}")


def describe_clip(clip: dict, prompt: dict, index: int, total: int) -> None:
    condition = clip["condition"]
    speaker = clip["speakerProfileId"]
    rate = clip["sampleRateHz"]
    is_48k_dup = clip["id"].endswith("-48k")
    is_silence = clip.get("expectPhysicalSilence", False)

    print("\n" + "=" * 72)
    print(f"Clip {index + 1}/{total}   id={clip['id']}")
    print(f"Condition:      {condition}")
    print(f"Speaker profile: {speaker}")
    print(f"Sample rate:    {rate} Hz")
    print(f"Destination:    benchmarks/corpus/{clip['file']}")

    if is_48k_dup:
        base_id = clip["id"][: -len("-48k")]
        print(
            f">>> 48 kHz DUPLICATE RECORDING <<<  same script as '{base_id}', "
            f"record at 48000 Hz only. Do not change the words."
        )

    if condition == "system-playback":
        method = prompt.get("recording method", "")
        print(
            ">>> SYSTEM-PLAYBACK CLIP <<< capture via loudspeaker playback "
            "(protocol.md section 6b): either a second person speaks this "
            "line live at a natural distance from the primary mic, or a "
            "separately recorded take is played back through a speaker and "
            "captured by the primary mic."
        )
        if method:
            print(f"Recording method note: {method}")

    if condition == "noise-silence":
        if is_silence:
            print(
                ">>> PHYSICAL SILENCE CLIP <<< Do NOT speak. Leave the "
                "microphone recording a genuinely quiet room for at least "
                "10 seconds. No digitally-generated silence."
            )
        else:
            bg = prompt.get("background", "")
            print(">>> NOISE-BED CLIP <<< speak with a real, mild noise bed audible.")
            if bg:
                print(f"Background note: {bg}")

    delivery = prompt.get("delivery")
    if delivery:
        print(f"Delivery note:  {delivery}")

    text = prompt.get("_text", "")
    if text:
        print("\nTEXT TO READ (read exactly as written, including any errors):")
        print(f"  {text}")
    elif not is_silence:
        print("\n(No prompt text block found -- check the prompt file.)")


def find_start_index(clips: list[dict]) -> int:
    for i, clip in enumerate(clips):
        if not (CORPUS_DIR / clip["file"]).is_file():
            return i
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--device", type=int, default=None, help="avfoundation audio device index")
    args = parser.parse_args()

    if sys.platform != "darwin":
        print("error: this script only supports macOS (avfoundation + afplay).", file=sys.stderr)
        return 2

    manifest = load_manifest()
    clips = manifest["clips"]
    total = len(clips)

    check_consent_gate(manifest)
    device_index = choose_device(args.device)

    index = find_start_index(clips)
    print(
        f"\nStarting at clip {index + 1}/{total} "
        f"(first clip with no recorded file on disk; use [g] to jump anywhere)."
    )

    while True:
        clip = clips[index]
        final_path = CORPUS_DIR / clip["file"]
        stage_path = staging_path(final_path)
        prompt = parse_prompt(CORPUS_DIR / clip["promptFile"])

        describe_clip(clip, prompt, index, total)

        recorded_count = sum(1 for c in clips if (CORPUS_DIR / c["file"]).is_file())
        print(f"\nProgress: {recorded_count}/{total} clips have a saved file.")

        has_staged = stage_path.is_file()
        has_final = final_path.is_file()
        if has_staged:
            print("Status: a pending take is waiting for [k]eep or [r]e-record.")
        elif has_final:
            print("Status: already recorded and kept. [r] re-records; [k]/[n] just moves on.")
        else:
            print("Status: not yet recorded.")

        cmd = input(
            "\n[r]ecord/re-record  [p]lay last take  [k]eep & next  "
            "[n]ext without keeping  [g]oto  [l]ist progress  [q]uit\n> "
        ).strip().lower()

        if cmd == "r":
            ok = record_take(device_index, clip["sampleRateHz"], stage_path)
            if ok:
                print("Take staged. Use [p] to play it back, [k] to keep it, or [r] to redo it.")
        elif cmd == "p":
            play_take(stage_path if has_staged else final_path)
        elif cmd == "k":
            if has_staged:
                stage_path.replace(final_path)
                print(f"Kept: {final_path}")
                index = min(index + 1, total - 1) if index < total - 1 else index
                if index == total - 1 and (CORPUS_DIR / clips[index]["file"]).is_file():
                    print("\nAll 130 manifest clips now have a saved file on disk.")
            elif has_final:
                index = min(index + 1, total - 1)
            else:
                print("Nothing to keep -- record a take first with [r].")
        elif cmd == "n":
            index = min(index + 1, total - 1)
        elif cmd == "g":
            target = input("Goto clip id or 1-based index: ").strip()
            found = None
            if target.isdigit():
                n = int(target)
                if 1 <= n <= total:
                    found = n - 1
            else:
                for i, c in enumerate(clips):
                    if c["id"] == target:
                        found = i
                        break
            if found is None:
                print("Not found.")
            else:
                index = found
        elif cmd == "l":
            by_condition: dict[str, list[int]] = {}
            for i, c in enumerate(clips):
                by_condition.setdefault(c["condition"], [0, 0])
                by_condition[c["condition"]][1] += 1
                if (CORPUS_DIR / c["file"]).is_file():
                    by_condition[c["condition"]][0] += 1
            print("\nPer-condition progress:")
            for cond, (done, need) in sorted(by_condition.items()):
                print(f"  {cond:24s} {done:3d}/{need:3d}")
        elif cmd == "q":
            print(
                f"\nExiting. {recorded_count}/{total} clips saved so far. "
                "Re-run this script any time to resume; it starts at the "
                "first un-recorded clip."
            )
            return 0
        else:
            print("Unrecognized command.")


if __name__ == "__main__":
    raise SystemExit(main())
