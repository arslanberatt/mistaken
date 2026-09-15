# Spec 05 Remediation — Human Corpus Recording Checklist

This is an operator-facing summary of `protocol.md`. `protocol.md` is the
normative document; if anything here appears to conflict with it, follow
`protocol.md` and treat this file as wrong until corrected. Nothing below
adds a new requirement or loosens an existing one.

Branch: `spec/05-remediation`. Worktree: see the session report. Do not
record into the `main` checkout.

## 0. Before you start

- [ ] You are one real human being (or, ideally, two — see "Second speaker"
      below) who can give informed consent to being recorded.
- [ ] You have read `benchmarks/corpus/protocol.md` in full.
- [ ] You have a quiet room, no HVAC noise directly over the mic, and no
      other people/media running except when a condition explicitly calls
      for it.
- [ ] You have one microphone you will use for every clip from a given
      speaker profile (built-in laptop mic, USB condenser, or headset mic
      are all acceptable — just stay consistent per profile).
- [ ] Your recording app can produce **16-bit PCM (`pcm_s16le`), mono**
      WAV files at **16000 Hz** (primary set) and **48000 Hz** (8-clip
      duplicate subset, see §7 below). Do not enable noise suppression,
      normalization, compression/limiting, de-essing, or EQ anywhere in the
      capture chain.

## 1. Consent (do this first, per speaker, before any recording)

Per `protocol.md` §3, before recording begins each speaker must give
informed consent to:

- (a) the purpose: local ASR benchmark development for the Mistaken
  project;
- (b) that clips are processed only on local machines and never uploaded;
- (c) that the manifest commits the verbatim transcript of what they say,
  but never the audio itself.

Then:

- [ ] Assign the speaker an opaque profile id already reserved in the
      manifest: `sp-01` (primary speaker) and, if a second person
      participates, `sp-02` (second speaker, used for `system-playback`
      clips).
- [ ] Do **not** record any name, gender, age, nationality, accent label,
      or other personal attribute anywhere in `benchmarks/**`.
- [ ] Only after consent is actually given, edit
      `benchmarks/corpus/manifest.json` → `speakerProfiles[].consentGiven`
      to `true` for that profile id. A profile with `consentGiven: false`
      must not be recorded (protocol.md §3). Update: this repository's
      manifest now has both `sp-01` and `sp-02` at `consentGiven: true`
      with real-recording provenance notes — informed consent was
      obtained before recording began and the corpus is complete; a new
      recording session for a different profile still follows this
      checklist from a `false` starting point.

### Second speaker (`sp-02`)

`system-playback` clips need a second-speaker-style utterance (see §6
below). Two options, both valid per `protocol.md` §6b:

- A second real person consents and speaks as `sp-02`, either live in the
  room at a natural distance from the mic, or recorded separately and
  played back through a loudspeaker for the primary mic to capture.
- If only one speaker is available, that speaker records the
  `system-playback` lines and plays them back through a loudspeaker
  themself. This is allowed, but it is a corpus-composition limitation
  that must be disclosed in the eventual report (Spec 05 §15, "Corpus
  bias"; `protocol.md` §3: "If only one speaker profile is available for a
  recording session, that limitation is recorded in the benchmark report").

## 2. What to record — exact utterance list

Every prompt is already written and committed. Do not write new prompts or
edit existing ones; read each one exactly as delivered.

- [ ] All 122 files under `benchmarks/corpus/prompts/<condition>-<nn>.md`
      (one prompt = one clip). Each file states its condition, assigned
      speaker profile, delivery notes, and (for `noise-silence` and
      `system-playback`) recording-method notes.
- [ ] The 8-clip 48 kHz duplicate subset: re-read `mistake-tense-01.md`
      through `mistake-tense-08.md` again, this time recording at 48000 Hz
      instead of 16000 Hz. Same script, same speaker, different sample
      rate only.

Per-condition counts (must match exactly; `validate-corpus` enforces the
floor for each):

| Condition | Clips | Source |
|---|---:|---|
| `mistake-tense` | 12 | `prompts/mistake-tense-01..12.md` |
| `mistake-agreement` | 8 | `prompts/mistake-agreement-01..08.md` |
| `mistake-article` | 6 | `prompts/mistake-article-01..06.md` |
| `mistake-preposition` | 8 | `prompts/mistake-preposition-01..08.md` |
| `mistake-order` | 6 | `prompts/mistake-order-01..06.md` |
| `mistake-minimal-pair` | 8 | `prompts/mistake-minimal-pair-01..08.md` |
| `fillers-repetition` | 10 | `prompts/fillers-repetition-01..10.md` |
| `fluent-control` | 16 | `prompts/fluent-control-01..16.md` |
| `fast-speech` | 12 | `prompts/fast-speech-01..12.md` |
| `system-playback` | 16 | `prompts/system-playback-01..16.md` |
| `noise-silence` | 12 | `prompts/noise-silence-01..12.md` (8 with a real noise bed, 4 pure silence — see §4) |
| `long-turn` | 8 | `prompts/long-turn-01..08.md` |
| **Total primary** | **122** | ≥ 20 minutes combined |
| `mistake-tense-*-48k` duplicate | 8 | Same scripts as `mistake-tense-01..08`, re-recorded at 48 kHz |
| **Grand total** | **130** | manifest entries |

## 3. Reading rules (per clip)

- [ ] Read the prompt's `text` block exactly as written, including any
      deliberately incorrect grammar (`have went`, `didn't knew`, missing
      articles, wrong prepositions, etc.). **Do not silently correct it
      while reading.**
- [ ] Follow the prompt's own delivery note (conversational pace, fast
      pace, "leave the sentence incomplete", etc.).
- [ ] Fillers (`um`, `uh`, stutters, repeated words) in a prompt are spoken
      as marked, not smoothed over.
- [ ] If what you actually said differs from the prompt text (a stumble,
      an extra filler, a repeated word you didn't plan), that is fine —
      but you must later transcribe **what was actually said**, not the
      original prompt, into the clip's `reference` field (see §6). The
      `reference` must always match the real audio.
- [ ] Trim only leading/trailing silence beyond roughly 300 ms of natural
      lead-in/lead-out. Never trim or edit internal pauses, false starts,
      or fillers inside an utterance.

## 4. Silence and noise clips (`noise-silence`, 12 total)

- [ ] 8 clips (`noise-silence-01` through `08`): read the prompt sentence
      with a mild, realistic noise bed audible under your speech (typing,
      distant traffic, soft music, a fan — see each prompt's `Background`
      line for the specific suggestion). Do not apply noise suppression
      afterward.
- [ ] 4 clips (`noise-silence-09` through `12`): **no speech at all.**
      Leave the microphone recording an empty, genuinely quiet room for
      at least 10 seconds. This must be real room noise floor (HVAC hum,
      faint ambient sound), not a digitally-generated silent/zero file —
      the point is to measure whether a candidate hallucinates words out
      of a real noise floor. Their `reference` in the manifest is already
      the empty string; leave it empty.

## 5. `system-playback` clips (16 total)

Per `protocol.md` §6b: these simulate the system-audio path by recording
a second speaker's voice through a loudspeaker rather than directly into
the mic. For each `system-playback-01..16` prompt:

- [ ] Have the `sp-02` line played through a loudspeaker (either a second
      person speaking live in the room at a natural distance from the
      primary recording mic, or a separately recorded take played back
      through a speaker) and capture it with the same primary recording
      microphone used for every other clip.
- [ ] Register the resulting clip with `speakerProfileId: "sp-02"`
      (already set in the manifest).

## 6. After recording each clip

- [ ] Save the WAV file as `benchmarks/corpus/clips/<condition>-<nn>.wav`
      (48 kHz duplicates: `benchmarks/corpus/clips/mistake-tense-<nn>-48k.wav`).
      This directory is Git-ignored — files here are never committed.
- [ ] Listen back and confirm the transcript. If what you said differs at
      all from the prompt text, edit that clip's `reference` field in
      `benchmarks/corpus/manifest.json` to match what was actually said
      (and, if a deliberate error phrase moved or changed, update its
      `errorSpans` token indices/`spoken` text to match — `validate-corpus`
      checks that every `errorSpans[].spoken` is a verbatim, in-order
      substring of the normalized `reference`).

## 7. Register checksums and durations (mechanical step, scripted)

Once some or all clips are recorded, run:

```bash
python3 benchmarks/corpus/scripts/register_clips.py
```

This recomputes each present clip's SHA-256 and duration exactly the way
the harness will, and writes those two fields into
`benchmarks/corpus/manifest.json`. It does **not** touch `reference`,
`errorSpans`, `speakerProfileId`, or `consentGiven` — those stay under
your manual control (see §6 and §1). Add `--dry-run` to preview without
writing.

## 8. Validate

```bash
cd benchmarks/harness && cargo build --release && cd ../..
./benchmarks/harness/target/release/mistaken-bench validate-corpus
```

- [ ] Fix-and-revalidate (re-record, re-transcribe, or re-run the register
      script) until this exits zero with zero warnings. Every failure
      names the exact clip and the exact rule violated — nothing is a
      silent skip.
- [ ] Confirm `git status` / `git check-ignore benchmarks/corpus/clips/*`
      shows the recorded audio is ignored, not staged — no `.wav` file is
      ever committed.

## 9. What happens after a clean `validate-corpus`

A clean corpus unblocks the `mac-arm64` benchmark run (this Mac qualifies:
see the session report). It does **not** by itself unblock approval — Spec
05 also requires a real `win-x64` reference-host run and a candidate that
clears every frozen gate on both hosts (see the session report's Part 3/4
for the current Windows-host and candidate-fidelity status).
