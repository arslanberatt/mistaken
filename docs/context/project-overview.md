# Mistaken

## Overview

Mistaken is a lightweight desktop transcription application for macOS and Windows, designed primarily for English speaking practice. It captures the user's microphone and the computer's system audio as two independent audio sources, transcribes both locally on the device, and renders a clean live conversation transcript. The user's microphone speech is shown as normal text, while speech coming from the computer is shown with a `- ` prefix. Mistaken does not intentionally correct grammar, rewrite sentences, replace incorrectly spoken words, or perform English coaching inside the app. Its job is to preserve the raw spoken conversation as faithfully as the local speech-recognition model can recognize it, then let the user copy the transcript into Obsidian for analysis by a separate skill/agent.

## Goals

1. Produce high-quality, low-latency English transcription entirely on-device without any paid API, API key, cloud transcription service, account, or required internet connection.
2. Capture microphone audio and computer/system audio separately so the application can deterministically distinguish the user from the remote/system speaker.
3. Preserve spoken grammar mistakes, repetitions, hesitations, filler words, false starts, and unusual word choices instead of intentionally cleaning them up.
4. Provide a minimal notepad-like live transcript that can be copied in one action and pasted directly into an Obsidian note.
5. Support both macOS and Windows from one product codebase while keeping platform-specific audio capture behind native adapters.
6. Keep the product local-first: no database, no cloud history, no authentication, and no automatic upload of recordings or transcripts.

## Core User Flow

1. User opens Mistaken.
2. Mistaken checks that the required local speech-recognition model is available.
3. User selects a microphone input if more than one is available.
4. User enables system/computer audio capture.
5. User presses `Start Listening`.
6. Mistaken starts two independent audio pipelines:
   - Microphone stream -> user speech
   - System audio stream -> remote/system speech
7. Each stream is normalized and sent to its own local speech-recognition session.
8. Partial transcription appears while speech is still in progress.
9. Finalized microphone speech is appended as normal text.
10. Finalized system-audio speech is appended with a `- ` prefix.
11. Mistaken never runs a grammar-correction pass over the transcript.
12. User presses `Stop` when the conversation ends.
13. User reviews the transcript and presses `Copy All`.
14. User pastes the transcript into an Obsidian note.
15. A separate Obsidian skill/agent analyzes and edits the note if requested.

Example output:

```text
I was thinking about going there tomorrow.

- What are you going to do there?

Um, I don't know. I have went there once before but I didn't stayed long.

- Why not?

I didn't knew anyone there.
```

## Features

### Local Speech Transcription

- Speech-to-text runs locally on the user's machine.
- Primary runtime: `sherpa-onnx` native inference.
- The ASR implementation must be behind an internal abstraction so the model/runtime can be benchmarked or replaced without rewriting the rest of the application.
- `whisper.cpp` may be used as a benchmark candidate or fallback implementation, but it is not a cloud dependency.
- No Deepgram, Azure Speech, OpenAI transcription API, Google Speech-to-Text, AssemblyAI, or other metered transcription service is allowed in the core application.
- No API key is required for transcription.
- No internet connection is required once the application and its local model are installed.
- The exact default model must be chosen only after accuracy, speed, memory usage, installer size, and model-license checks.

### Raw Speech Preservation

Mistaken must not intentionally convert incorrect English into correct English.

If the recognizer returns:

```text
I didn't knew anyone there.
```

Mistaken must not rewrite it to:

```text
I didn't know anyone there.
```

The same rule applies to:

- Incorrect verb forms
- Incorrect word choice
- Repeated words
- Filler words such as `um` and `uh` when the selected model recognizes them
- False starts
- Incomplete sentences
- Hesitations
- Informal spoken English

Important limitation: speech recognition is probabilistic. Mistaken can prevent its own post-processing from correcting English, but it cannot guarantee that the underlying ASR model will never mishear or normalize a spoken word. Model selection and benchmarking must therefore include intentionally incorrect English and confusing word pairs.

### Microphone Capture

- Capture the user's selected microphone.
- Keep microphone audio isolated from system audio.
- Do not merge microphone and system audio before speech recognition.
- Prefer native/local capture with predictable PCM output.
- Normalize audio into the format required by the local ASR pipeline.
- Handle device disconnects without crashing the application.

### Windows System Audio

- Use Windows WASAPI loopback capture for system audio.
- System audio remains a separate source from the microphone.
- Initial implementation may capture the selected/default render endpoint.
- Per-application process capture may be added only if explicitly included in the current implementation scope.

### macOS System Audio

- Use Apple ScreenCaptureKit for system audio capture.
- Request only the permissions required by macOS.
- Keep captured system audio separate from microphone audio.
- Exclude Mistaken's own playback from capture when technically appropriate to avoid feedback loops.

### Speaker Separation

Speaker separation is source-based, not guessed by an AI model for the normal two-person use case.

```text
Microphone -> User -> normal line
System audio -> Other speaker -> "- " line
```

Example:

```text
I want to visit Russia next year.

- Why Russia?

Because I never went there before.
```

This source-based rule is a core product invariant.

Multiple remote speakers inside the same system-audio stream are not required for the first version. Local speaker diarization can be evaluated later if needed.

### Live Transcript Workspace

The interface behaves like a minimal live notepad.

Required controls:

- `Start Listening`
- `Stop`
- `Clear`
- `Copy All`
- Microphone selector
- System-audio status
- Local-model status

Behavior:

- Partial/interim text may appear while a sentence is being recognized.
- Finalized text becomes stable transcript content.
- Microphone lines have no speaker prefix.
- System-audio lines use `- `.
- The transcript is editable only if an explicit manual-edit feature is later added; automatic correction is forbidden.
- `Clear` removes the current in-memory session after user action.

### Temporary Session State

- Current transcript lives in application memory.
- No transcript database is created.
- No user account is created.
- No cloud session is created.
- Closing the app may discard the current transcript.
- Copying the transcript is the user's persistence workflow.

Suggested internal representation:

```ts
type TranscriptSource = "microphone" | "system";

type TranscriptSegment = {
  id: string;
  source: TranscriptSource;
  text: string;
  startedAtMs: number;
  endedAtMs?: number;
  isFinal: boolean;
};
```

### Obsidian Workflow

Mistaken and the Obsidian analysis skill are separate systems.

```text
Conversation
    ↓
Mistaken
    ↓
Raw local transcript
    ↓
Copy All
    ↓
Obsidian note
    ↓
User invokes skill/agent
    ↓
Grammar / vocabulary / speaking analysis
```

Mistaken must not require access to the user's Obsidian vault.

## Scope

### In Scope

- macOS desktop application
- Windows desktop application
- Tauri 2 desktop shell
- React + TypeScript UI
- Rust native layer
- Microphone capture
- Windows system audio capture
- macOS system audio capture
- Separate microphone and system audio pipelines
- Local/offline English speech recognition
- Streaming/near-real-time transcript updates
- Source-based user/remote-speaker separation
- `- ` prefix for system-audio speech
- Raw-transcript preservation policy
- Start / Stop / Clear / Copy All controls
- In-memory session state
- Local ASR model loading
- Graceful permission/device/model error states

### Out of Scope

- Paid transcription APIs
- Metered cloud AI services
- API keys for core functionality
- Required internet connection
- User accounts
- Login / registration
- PostgreSQL
- SQLite transcript storage
- Cloud transcript history
- Automatic cloud sync
- Backend server
- Grammar correction inside Mistaken
- Vocabulary correction inside Mistaken
- Automatic rewriting or cleanup
- English scoring
- Flashcards
- Spaced repetition
- Obsidian vault modification
- Pronunciation coaching in V1
- Mobile applications
- Payments / subscriptions
- Multi-user collaboration
- Mandatory multi-speaker diarization inside system audio

## Success Criteria

1. Mistaken installs and launches on supported macOS and Windows versions without requiring an account or API key.
2. The application can run its core transcription workflow with the network disconnected.
3. Microphone audio and system audio can be captured at the same time without being merged into one source.
4. Microphone speech appears as normal transcript text.
5. System-audio speech appears with a `- ` prefix.
6. Speech begins appearing while the conversation is happening and finalized segments remain stable afterward.
7. Mistaken performs no grammar-correction or sentence-rewriting pass on recognized text.
8. A test corpus containing intentionally incorrect English is used during ASR model selection.
9. A test set containing confusing word pairs and pronunciation mistakes is used to compare candidate local models.
10. The selected default model meets an agreed accuracy/latency target on the user's target hardware before it is made the default.
11. The selected runtime and bundled model have licenses that permit the intended distribution model; runtime license and model-weight license are checked separately.
12. The user can stop transcription and copy the entire conversation in one action.
13. Clearing or closing the current session does not leave a transcript in an application database because no transcript database exists.
14. The copied output can be pasted directly into an Obsidian note and retains the user/system speaker formatting.
