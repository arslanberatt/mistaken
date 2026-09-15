# Mistaken Benchmark Corpus — Recording Protocol

This protocol governs how the Spec 05 benchmark corpus is produced. It exists
so a second operator can reproduce an equivalent corpus from this document
alone, and so every clip's provenance, consent, and format are verifiable.

## 1. Purpose

The corpus exists to measure whether a candidate local ASR configuration
recognizes deliberately incorrect spoken English **without silently
correcting it**. Recognition accuracy alone (WER) is insufficient for this
product; the corpus is designed to expose grammar auto-correction,
acoustically confusable minimal pairs, disfluency handling, and hallucination
on non-speech audio.

## 2. Recording environment

- One quiet room per session; no open windows, no HVAC directly above the
  microphone, no concurrent media playback in the room except for the
  `system-playback` condition, which is recorded separately (see §6).
- One microphone per speaker profile, used for every clip from that profile.
  Built-in laptop microphone, USB condenser, or headset microphone are all
  acceptable as long as the same device is used for the whole profile.
- Recording format: 16-bit PCM (`pcm_s16le`), mono, 16000 Hz for the primary
  set. The `mistake-tense` duplicate subset (see §7) is additionally recorded
  at 48000 Hz mono to exercise the recognizer's resampling path.
- No post-processing: no noise suppression, no normalization/loudness
  matching, no compression/limiting, no de-essing, no EQ.
- Trimming is limited to leading/trailing silence beyond roughly 300 ms of
  natural lead-in/lead-out; internal pauses, false starts, and fillers inside
  an utterance are never trimmed or edited.

## 3. Speaker profiles and consent

- Every recorded speaker is assigned an opaque profile id (`sp-01`, `sp-02`,
  …) before recording starts. No name, gender, age, nationality, accent
  label, or other personal attribute is recorded anywhere in this subtree.
- Before recording begins, the speaker gives informed consent to: (a) the
  purpose of the recording (local ASR benchmark development for the Mistaken
  project), (b) the fact that clips are processed only on local machines and
  never uploaded, and (c) the fact that the manifest commits the verbatim
  transcript of what they say but never the audio itself.
- The `consentGiven` boolean per profile is recorded in
  `benchmarks/corpus/manifest.json` at the profile-registry level maintained
  by the operator (see the profile ledger kept alongside the manifest). A
  profile with `consentGiven: false` is not recorded.
- If only one speaker profile is available for a recording session, that
  limitation is recorded in the benchmark report as a corpus-composition
  limitation (see Spec 05 §15, "Corpus bias").

## 4. Prompt delivery

- Each clip has a corresponding prompt file at
  `benchmarks/corpus/prompts/<condition>-<nn>.md` containing the exact
  sentence(s) to speak, in reading order, plus any delivery notes (e.g.
  "speak at conversational pace", "speak quickly", "leave the sentence
  incomplete").
- The speaker reads the prompt naturally. Deliberately incorrect grammar in a
  prompt is spoken exactly as written — the speaker does not "fix" it while
  reading. Filler prompts (`um`, `uh`, repeated words) are spoken as marked.
- The verbatim transcript of what was actually said becomes the clip's
  `reference` field in the manifest. If the spoken take differs from the
  prompt (extra filler, a stumble), the manifest `reference` records what was
  actually said, not the original prompt text — the reference must always
  match the real audio.

## 5. File naming and registration

- Clip audio: `benchmarks/corpus/clips/<condition>-<nn>.wav` (Git-ignored;
  never committed).
- After recording, the operator runs `mistaken-bench validate-corpus`, which
  computes each clip's SHA-256 and duration and checks it against the
  manifest entry. A clip is registered in the manifest only after validation
  confirms format, duration bounds, checksum consistency, prompt linkage,
  `errorSpans` token-index correctness, and (for `noise-silence` physical
  silence clips) an empty `reference`.
- Fix-and-revalidate continues until `mistaken-bench validate-corpus` exits
  zero with zero warnings. Validation failures name the exact clip and the
  exact rule violated.

## 6. Condition taxonomy

| Condition prefix | Clips | Content requirement |
|---|---|---|
| `mistake-tense` | 12 | Wrong verb tense/form: `have went`, `didn't knew`, `was gone yesterday`. |
| `mistake-agreement` | 8 | Subject–verb and plural agreement errors. |
| `mistake-article` | 6 | Missing or wrong articles. |
| `mistake-preposition` | 8 | Wrong or missing prepositions. |
| `mistake-order` | 6 | Non-native word order and incomplete restarts. |
| `mistake-minimal-pair` | 8 | Acoustically risky pairs: `went`/`won't`, `can`/`can't`, `knew`/`new`, `their`/`there`. |
| `fillers-repetition` | 10 | `um`, `uh`, stutters, repeated words, self-corrections. |
| `fluent-control` | 16 | Grammatically correct speech, to bound the false-correction measurement. |
| `fast-speech` | 12 | Rapid delivery of the same error families as above. |
| `system-playback` | 16 | Second-speaker style utterances captured from loudspeaker playback (see §6b), for later `- ` source work. |
| `noise-silence` | 12 | Room noise, typing, music bed, plus ≥ 4 clips of ≥ 10 s physical silence with `expectPhysicalSilence: true` and an empty `reference`. |
| `long-turn` | 8 | 60–180 s continuous speech mixing conditions above, for drift and memory-growth measurement. |

Total: 122 clips, ≥ 20 minutes.

### 6a. Physical-silence clips

At least 4 `noise-silence` clips are ≥ 10 seconds of genuine room silence
with no speech at all: microphone left recording an empty quiet room. Their
manifest `reference` is the empty string and `expectPhysicalSilence` is
`true`. Any non-empty recognizer output on these clips is a hallucination by
definition, not a misrecognition.

### 6b. `system-playback` recording

These 16 clips are recorded by playing a second speaker's utterance through
a loudspeaker and capturing it with the recording microphone, simulating the
system-audio path this corpus does not otherwise exercise (Spec 05 does not
capture live system audio; Spec 07/08 own that native path). The source
utterance may be spoken by a second profile speaker directly into a separate
recorder and then played back, or spoken live in the room by a second
speaker at a natural conversational distance from the primary recording
microphone; either way, the resulting clip is registered exactly like any
other clip, with `speakerProfileId` referring to the second-speaker profile.

## 7. Sample-rate duplicate subset

Eight `mistake-tense` clips are additionally re-recorded at 48000 Hz mono
using an identical script, registered as their own manifest entries with
`sampleRateHz: 48000`, to exercise the recognizer's internal resampling path
(`sherpa-onnx` resamples internally to 16 kHz; nothing in Mistaken performs
resampling itself — see `docs/context/progress-tracker.md`, "No resampling
crate, one rate per stream").

## 8. What this protocol forbids

- No noise suppression, AGC, or loudness normalization at capture or after.
- No editing of internal pauses, fillers, or disfluencies.
- No "fixing" a deliberately incorrect prompt while reading it.
- No recording of real names, addresses, credentials, or third-party
  conversation content — every clip is a scripted prompt or a controlled
  noise/silence capture.
- No demographic, accent, or identity metadata anywhere in the subtree.
- No audio, ever, committed to Git (`benchmarks/.gitignore` enforces this;
  `mistaken-bench validate-corpus` and repository review both confirm no
  `.wav` is staged).
