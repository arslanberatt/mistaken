# Spec 02 — Transcript Domain and Workspace

## 1. Status, Ownership, Base, and Gates

- **Status:** Authored; ready for cross-spec integration review. Not yet implemented.
- **Implementation owner:** One Spec 02 branch/worktree with one writer.
- **Expected branch:** `spec/02-transcript-workspace`
- **Expected worktree:** A dedicated path created from the final Spec 01 baseline; never the canonical integration checkout used by another writer.
- **Base SHA:** Assigned from the final clean `main` SHA produced by Spec 01. It is not available during authoring and must be recorded before implementation changes begin.
- **Allowed predecessor:** Spec 01 only. It must be completed, reviewed, committed, and merged.
- **Parallel compatibility:** May run beside Specs 03 and 05 only because this spec’s owned paths are disjoint from their owned paths and all shared dependencies/contracts are supplied by the Spec 01 baseline.
- **Merge gate:** The Spec 01 transcript/runtime types, global tokens, clipboard-write plugin, and frontend test harness must exist exactly as frozen below. If the baseline does not provide them, the integration owner fixes Spec 01 before creating Wave 2 worktrees; the Spec 02 worker must not edit shared manifests, capabilities, Tauri entrypoints, test configuration, or shared types.
- **Review level:** Medium implementation with mandatory high-capability cross-spec review before Wave 2 starts.

## 2. Goal and User-Visible Result

Replace the bootstrap screen with Mistaken’s single-window transcript workspace and implement the frontend transcript domain against volatile React memory.

The committed product state is honest: because native runtime/audio work is not yet integrated, the actual app opens to the complete empty workspace with unavailable source controls and a disabled `Start Listening` action. It does not simulate recording or ship sample transcript content.

The complete Spec 02 behavior is exercised with deterministic local transcript data through tests and a temporary visual verification fixture:

- microphone and system segments render in stable first-seen order;
- system segments alone receive the visible `- ` prefix;
- interim segments update in place and are visually distinguished;
- final segments cannot be changed by later updates;
- `Copy All` writes plain text containing finalized segments only;
- `Clear` removes all in-memory transcript state, with confirmation when any finalized content exists.

No native command, audio capture, ASR, persistence, backend, account, or network behavior is introduced.

## 3. Verified Current Behavior

Verified while authoring this spec:

- `/Users/berat/mistaken` still does not exist. There is no application code to inspect yet.
- `/Users/berat/mistaken-context` is documentation-only and is not a Git repository.
- Spec 01 is authored but not implemented. Its reviewed contract now supplies:
  - the canonical Tauri 2/React/TypeScript/Tailwind repository;
  - the exact semantic tokens from `ui-context.md`;
  - `TranscriptSource`, `TranscriptSegment`, and `CaptureStatus` shared types;
  - Vitest, jsdom, React Testing Library, jest-dom, and user-event infrastructure;
  - Lucide React;
  - the official Tauri clipboard-manager plugin registered with `clipboard-manager:allow-write-text` only.
- Spec 01’s temporary bootstrap screen and its behavior test are intentionally superseded by this spec.
- The product context fixes microphone text as unprefixed, system-audio text as `- ` prefixed, transcript state as in-memory only, and transcript text as read/copy-only in V1.
- `spec-plan.md` resolves two previously listed questions: V1 has no manual transcript editing, and `Clear` requires confirmation when finalized content exists.
- No transcript reducer, serializer, workspace component, clipboard behavior, UI interaction test, native event feed, microphone list, system-audio status, or model status currently exists.
- The official Tauri clipboard plugin supports plain-text writes on both macOS and Windows and enables no clipboard operation by default; Spec 01 grants only write-text.
- Browser `navigator.clipboard.writeText` requires a secure context and can reject with `NotAllowedError`. This spec therefore consumes the explicit Tauri plugin rather than adding an unreliable browser fallback.

Disk state and the implemented Spec 01 baseline are authoritative at application time. If they differ from this section, reconcile the relevant spec/context before editing.

## 4. Scope

### In scope

- Replace the Spec 01 bootstrap composition in `src/App.tsx` with the real single-window workspace composition.
- Create the transcript feature under `src/features/transcript/`.
- Consume the shared transcript/runtime types from `src/types/`; do not redefine them.
- Implement a pure transcript session reducer/hook with stable segment identity, first-seen ordering, interim replacement, final immutability, and clear behavior.
- Implement pure display formatting and finalized-transcript clipboard serialization.
- Render empty, populated, interim, and capture-status presentation states without chat UI.
- Render compact top, source, transcript, and bottom action regions defined by `ui-context.md`.
- Render honest disabled source/start controls in the committed app until native integration supplies availability and callbacks.
- Implement `Copy All` through the preconfigured Tauri clipboard write API with pending, success, and failure states.
- Implement `Clear`, including deterministic confirmation when finalized content exists.
- Add durable domain and user-interaction tests; remove the superseded Spec 01 bootstrap-screen test instead of re-pinning obsolete copy.
- Visually verify the empty committed app and populated/interim states at initial and minimum window sizes.
- Verify real macOS clipboard output from the actual Tauri window using a temporary deterministic fixture that is removed before commit.
- Record Spec 02 evidence and create one focused local commit. Do not push.

### Out of scope

- Custom Tauri commands/events, IPC schemas/wrappers, native runtime state, or subscriptions. Spec 03 owns them.
- Actual microphone enumeration/selection, capture, permission handling, or device disconnect behavior. Spec 04 owns them.
- Actual system-audio availability/capture or OS permissions. Specs 07 and 08 own them.
- ASR/model status, model files, partial/final native emissions, aggregation across two recognizers, or capture lifecycle. Specs 05, 06, 09, and 10 own them.
- A production fixture, simulated listening, timer-driven fake transcript, fake microphone option, fake model-ready state, or enabled Start/Stop control without a real runtime callback.
- Transcript editing, content normalization, grammar correction, spell correction, punctuation rewriting, speaker inference, diarization, timestamps in copied output, confidence values, or hidden metadata.
- Auto-follow, `Jump to latest`, global keyboard shortcuts, time-limited copy feedback, final accessibility hardening, or final Clear dialog treatment. Spec 11 owns these refinements; this spec still provides a usable baseline.
- Transcript/history persistence, browser storage, files, database, account, backend, sync, upload, analytics, telemetry, or cloud fallback.
- New dependencies, package/lockfile edits, Cargo edits, capability edits, Tauri entrypoint edits, or global token changes in the Spec 02 worktree.

## 5. Owned Files and Forbidden Concurrent Files

### Owned during implementation

After Spec 01 creates the canonical repository, Spec 02 owns only:

- `src/App.tsx`
- deletion/replacement of `src/App.test.tsx`
- `src/features/transcript/**`
- `docs/specs/spec-02-transcript-domain-workspace.md` for its own pre-commit evidence

Recommended feature files, subject to existing repository conventions:

- `src/features/transcript/transcript-domain.ts`
- `src/features/transcript/use-transcript-session.ts`
- `src/features/transcript/transcript-clipboard.ts`
- `src/features/transcript/TranscriptWorkspace.tsx`
- `src/features/transcript/transcript-domain.test.ts`
- `src/features/transcript/TranscriptWorkspace.test.tsx`

Do not split components further unless the rendered behavior makes a second focused component materially clearer. No one-line wrapper or component-per-row abstraction is required.

### Consumed unchanged

- `src/main.tsx`
- `src/index.css` and its semantic tokens/global reset
- `src/types/transcript.ts`
- `src/types/runtime.ts`
- `src/test/setup.ts`
- `vitest.config.ts`
- root TypeScript/Vite/lint configuration
- `package.json` and `package-lock.json`
- all `src-tauri/**` files

### Forbidden concurrent files

The Spec 02 worker must not modify:

- `src/lib/tauri/**`, owned by Spec 03
- Tauri command/event payload names or custom IPC contracts, owned by Spec 03
- `src/types/**`, frozen by Spec 01 for Wave 2
- `src/main.tsx`, `src/index.css`, global tokens, or shared UI/test configuration
- `package.json`, `package-lock.json`, `vite.config.ts`, `vitest.config.ts`, or lint/TypeScript configuration
- `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`, `src-tauri/capabilities/**`, or `src-tauri/tauri.conf.json`
- `benchmarks/**`, owned by Spec 05
- another spec file
- `docs/context/progress-tracker.md` while Wave 2 has parallel writers; the integration owner updates it after review/merge

If a missing shared dependency/type/token blocks implementation, stop that path and report it to the integration owner. Do not resolve the blocker by creating a second type, direct raw invoke, browser clipboard fallback, hardcoded color, or opportunistic manifest edit.

## 6. Contracts Consumed and Produced

### Contracts consumed from Spec 01

The baseline must export these exact shared shapes from `src/types/`:

```ts
export type TranscriptSource = "microphone" | "system";

export interface TranscriptSegment {
  id: string;
  source: TranscriptSource;
  text: string;
  startedAtMs: number;
  endedAtMs?: number;
  isFinal: boolean;
}

export type CaptureStatus =
  | "idle"
  | "starting"
  | "listening"
  | "stopping"
  | "error";
```

It also consumes:

- the semantic CSS variables from `ui-context.md`;
- the official `@tauri-apps/plugin-clipboard-manager` `writeText` API;
- `clipboard-manager:allow-write-text` scoped to the main window;
- Lucide icons and the baseline Vitest/jsdom/Testing Library setup;
- the `main` window and root React mount from Spec 01.

### Transcript session contract produced

```ts
export interface TranscriptSessionError {
  readonly code: "segment_identity_conflict";
  readonly segmentId: string;
}

export interface TranscriptSessionState {
  readonly segments: readonly TranscriptSegment[];
  readonly lastError: TranscriptSessionError | null;
}

export type TranscriptSessionAction =
  | { readonly type: "segment/received"; readonly segment: TranscriptSegment }
  | { readonly type: "session/cleared" };
```

Required reducer semantics:

1. A segment ID is unique for the lifetime of the current in-memory session.
2. The first occurrence of an ID is appended; its array position is the stable first-seen order.
3. An existing interim segment may be replaced only by a newer interim or final segment with the same ID, source, and `startedAtMs`.
4. Replacement preserves the original array position.
5. Once `isFinal` is true, every later update for that ID is ignored and the existing object/text remains unchanged.
6. A same-ID update that changes source or `startedAtMs` preserves every existing segment reference and sets `lastError` to `{ code: "segment_identity_conflict", segmentId }`; it must not silently swap source identity.
7. An accepted new/interim/final segment clears a prior `lastError`; an ignored post-final update returns the existing state unchanged.
8. `session/cleared` returns `{ segments: [], lastError: null }` and releases references to all prior segments.
9. The reducer does not sort by text/timestamp, infer speakers, normalize content, persist data, or call native/browser APIs.
10. Unchanged segments retain object identity so memoized rows do not rerender on an unrelated interim update.

`TranscriptSessionError` remains feature-local and must not be added to or confused with Spec 03’s shared IPC/runtime error contract.

### Formatting contract produced

```ts
formatTranscriptSegment(segment: TranscriptSegment): string
serializeFinalTranscript(segments: readonly TranscriptSegment[]): string
```

Rules:

- microphone: return `segment.text` exactly;
- system: return ``- ${segment.text}`` exactly;
- do not trim, case-fold, correct, punctuate, rewrite, or infer source from text;
- serialization includes finalized segments only, in array order;
- join segments with exactly one blank line (`\n\n`);
- produce no leading/trailing newline and no metadata, timestamp, JSON, Markdown speaker label, confidence, or interim marker;
- no finalized segments produces the empty string;
- allocate the complete clipboard string only when Copy All is requested, not on every transcript render.

### Clipboard contract produced

```ts
export type ClipboardWriter = (text: string) => Promise<void>;
```

- Production passes the Tauri plugin’s plain-text `writeText` through this boundary.
- Tests inject a deterministic writer and assert the exact consumer-observed text.
- Copy runs only from an explicit user action.
- Copy never reads the clipboard.
- A rejection produces visible non-blocking failure text and leaves transcript state unchanged; no `document.execCommand`, hidden textarea, browser clipboard, shell, or filesystem fallback is allowed.

### Workspace presentation boundary produced

`TranscriptWorkspace` accepts transcript state plus an explicit runtime view/callback boundary. It must not import future raw IPC event names or own native lifecycle logic.

Minimum inputs:

- `segments: readonly TranscriptSegment[]`
- `sessionError: TranscriptSessionError | null`
- `captureStatus: CaptureStatus`
- model, microphone, and system-audio display labels
- elapsed milliseconds
- explicit `canStart`, `onStartRequested`, and `onStopRequested` values
- `onClearRequested`
- `writeClipboard: ClipboardWriter`

The committed `App` supplies empty transcript state, `captureStatus="idle"`, truthful unavailable/not-connected labels, `canStart=false`, and no fake start/stop callback. Later specs map native state into this presentation boundary.

After Specs 02 and 03 merge, Spec 04 is the first successor allowed to make a focused integration edit to `App` and the microphone portion of `TranscriptWorkspace`: it replaces the unavailable microphone label with a real selector/Refresh/PCM-test control while preserving this spec’s transcript reducer, formatting, clipboard, Clear, layout, and disabled production-transcription contracts. Specs 07 and 08 do not edit the frontend workspace during their parallel Wave 3 branches.

## 7. User Flow and Developer Verification Flow

### Committed user flow after Spec 02

1. User opens the real Mistaken desktop app.
2. The app displays the complete single-window workspace.
3. The top bar shows `Mistaken` and `Local • Runtime unavailable` or equivalent truthful reviewed wording.
4. The source bar shows no available microphone and system audio not connected.
5. The transcript area shows the practical empty state:
   - `Ready to transcribe locally.`
   - `Choose your microphone, then start listening.`
   - `System audio will appear with a "-" prefix.`
   - `No audio or transcript is uploaded.`
6. Elapsed time is `00:00:00`.
7. `Start Listening`, `Clear`, and `Copy All` are disabled because no runtime or transcript exists.
8. Closing and reopening returns to the same empty state; no transcript was stored.

### Deterministic transcript flow

Use test/temporary verification data, never committed production seed data:

1. Receive final microphone segment `I actually have went there yesterday.`
2. Receive final system segment `Why did you go there?`
3. Receive interim microphone segment `Um, because my friend...`.
4. Receive a newer interim with the same ID; the row changes in place without duplication.
5. Receive a final update with the same ID and the intentionally incorrect text `Um, because my friend invited me and I didn't knew anyone there.`.
6. Attempt another update to that final ID; visible/final text remains unchanged.
7. Receive final system segment `Oh, okay.` and one remaining interim microphone segment.
8. Copy All. The system clipboard contains only finalized content:

```text
I actually have went there yesterday.

- Why did you go there?

Um, because my friend invited me and I didn't knew anyone there.

- Oh, okay.
```

9. Cancel one Clear confirmation and verify all content remains.
10. Confirm Clear and verify the empty state returns immediately.

### Developer verification flow

1. Create the Spec 02 branch/worktree from the recorded Spec 01 SHA and verify owned paths.
2. Run the Spec 01 checks before editing; do not start from a failing baseline.
3. Replace the obsolete bootstrap test with domain/workspace behavior tests.
4. Exercise reducer, serializer, Copy All, Clear, and accessibility behavior through semantic user interactions.
5. Run the committed empty app in the actual Tauri window.
6. For populated/interim visual and real clipboard verification, temporarily seed `App` through the public reducer/session boundary with the deterministic segments above.
7. Use the actual Copy All button, then inspect the macOS clipboard externally and compare exact bytes/text to the expected output.
8. Inspect initial and minimum window sizes in the native app and the Vite surface; use browser automation for the web surface’s visual/interaction verification.
9. Remove the temporary seed/wiring, rerun the suite and production build, and verify the committed app is empty/honest.
10. Record pre-commit evidence in this spec, commit locally, and report final SHA/status to the integration owner.

## 8. UI Behavior, States, Tokens, and Accessibility

### Layout

Use the single-window structure from `ui-context.md`:

```text
Top bar
Source bar
Scrollable transcript surface (dominant/flexible)
Bottom action bar
```

- At `1040 × 720`, all regions remain compact and the transcript owns most vertical space.
- At the `720 × 520` minimum, status/source/action content may wrap or shorten labels, but transcript content and critical action names remain visible without horizontal page overflow.
- Transcript line length is constrained to approximately `70–80ch` with `16–18px` system-sans text and about `1.65` line height.
- Transcript content is selectable but not editable and renders with preserved whitespace (`white-space: pre-wrap`) so display does not silently collapse recognized text.
- No card grid, chat bubble, avatar, speaker color, gradient, glow, glass effect, marketing hero, or decorative animation.

### Token usage

Consume the exact Spec 01 variables from `ui-context.md`:

- backgrounds: `--bg-base`, `--bg-surface`, `--bg-elevated`
- text: `--text-primary`, `--text-secondary`, `--text-muted`, `--text-interim`
- borders: `--border-default`, `--border-strong`
- interaction/state: `--accent-primary`, `--accent-hover`, `--state-error`, `--state-warning`, `--state-success`, `--selection`
- fonts: `--font-sans`, `--font-transcript`, `--font-mono`

Feature components use Tailwind utilities backed by these variables; they do not introduce literal color values or edit the global token set.

### Transcript states

1. **Empty:** practical local-transcription instructions and privacy line; no promotional copy.
2. **Final content:** primary text color; stable and selectable.
3. **Interim content:** `--text-interim`, same source-prefix rule, and a visible compact `Interim` text marker so state is not communicated by color alone.
4. **System source:** visible `- ` prefix in the same typography/color treatment as microphone text; no avatar or inferred name.
5. **Domain invariant rejection:** preserve the prior segment/source and expose concise status that a transcript update was rejected; do not display or copy the invalid replacement.

The transcript container uses an appropriate named log/region semantic. Live announcements must be polite and must not duplicate the same final segment. Spec 11 may tune high-frequency announcement behavior after real events exist.

### Capture/action states

- `idle`: show `Start Listening`; it is enabled only when explicit `canStart=true` and a real callback exists.
- `starting`: show `Starting…`, disabled.
- `listening`: show `Stop`, enabled only when a real stop callback exists.
- `stopping`: show `Stopping…`, disabled.
- `error`: show an explicit error status; retry remains disabled unless a later runtime contract explicitly allows it.
- The committed Spec 02 app is idle but unavailable, so Start is disabled with visible or described explanatory text.
- Do not use global Space handling. Shortcuts belong to Spec 11.

### Clear states

- Empty transcript: disabled.
- Only interim content: one Clear activation clears immediately because no finalized content exists.
- Any finalized content: first activation reveals an inline confirmation with `Clear transcript` and `Cancel`; no destructive action occurs yet.
- Cancel preserves all segments and restores focus to the original Clear button.
- Confirm clears final and interim segments and returns focus to a stable workspace target.
- Escape cancels the inline confirmation.
- Clear does not imply stopping capture and never touches a database/file.

### Copy states

- No finalized content: disabled even if an interim row exists.
- Ready: label `Copy All`.
- Pending: prevent duplicate requests and expose `Copying…`.
- Success: non-blocking `Copied` status in an `aria-live="polite"` region.
- Failure: visible `Could not copy transcript.` status; transcript remains intact and the action can be retried.
- Feedback resets when finalized transcript content changes or the session is cleared. Spec 02 uses no timeout; Spec 11 owns brief timed feedback.

### Accessibility

- Semantic `header`, labeled source region, named transcript region/log, and footer/action region.
- Buttons are native buttons with accessible names and visible focus indicators.
- Disabled actions expose why they are unavailable without relying on tooltip-only content.
- Icons are `aria-hidden` when adjacent text supplies the name.
- Status is conveyed by text plus icon where used, never color alone.
- Interim state has a textual marker in addition to muted color.
- Confirmation controls are reachable in logical tab order; opening/canceling manages focus predictably.
- Required text/control contrast meets WCAG AA.
- Text scaling and the minimum window size do not hide transcript or confirmation actions.
- Reduced motion produces no animation; this spec does not require animation.

## 9. Frontend → Tauri IPC → Rust / Audio / ASR Data Flow

### Production state after Spec 02

```text
React App
  → useTranscriptSession(initial empty state)
  → TranscriptWorkspace
      → renders honest unavailable runtime/source status
      → renders empty transcript state
      → Clear disabled
      → Copy All disabled
      → Start Listening disabled
```

There is no custom Tauri IPC, native event subscription, Rust/audio/ASR work, poll, timer, stream, worker, or runtime simulation.

### Deterministic domain flow

```text
TranscriptSegment
  → segment/received action
  → pure reducer
      → append first ID in first-seen order
      → replace matching interim in place
      → freeze final against later updates
  → TranscriptWorkspace rows
```

### Copy flow

```text
explicit Copy All click
  → filter final segments
  → format by structural source metadata
  → join with one blank line
  → ClipboardWriter
  → Tauri clipboard-manager writeText
  → Copied or actionable failure status
```

The plugin call remains behind the feature-local `ClipboardWriter` boundary. It is not a new custom command and does not enter Spec 03’s raw command/event namespace.

### Clear flow

```text
Clear click
  → finalized content exists?
      yes → confirmation → confirm
      no  → immediate clear
  → session/cleared action
  → empty React state
```

Later native transcript events will dispatch the same `segment/received` action through an integration owner. This spec neither names those events nor couples the reducer to Tauri payload transport.

## 10. Platform, Permissions, Offline/Privacy, and Fallback Rules

### Platform behavior

- React/domain behavior is platform-neutral and must pass the same deterministic tests on macOS and Windows-capable CI/hosts when available.
- Real UI and clipboard smoke for this implementation is required on the current macOS Apple Silicon host.
- Official Tauri clipboard-manager support covers macOS and Windows, but Spec 02 must not claim real Windows verification without a Windows run; final cross-platform proof belongs to Specs 14–15.
- Use platform-neutral `Cmd/Ctrl` wording only in documentation; no shortcuts are implemented here.

### Permissions

- Consume only `clipboard-manager:allow-write-text` from Spec 01.
- Never request clipboard read, image, HTML, clear, filesystem, shell, process, dialog, HTTP, microphone, system-audio, screen-recording, accessibility, or notification permission.
- Copy must originate from the visible user action.
- Do not weaken CSP or add remote origins.

### Offline/privacy

- Rendering, reducer updates, Clear, and Copy All require no network.
- Transcript and feedback exist only in React memory.
- Do not call localStorage, sessionStorage, IndexedDB, Cache Storage, File System Access, Tauri filesystem APIs, or a database.
- Do not log transcript text to console, test reports, analytics, telemetry, or persisted error files.
- Test fixture text is synthetic and committed only inside test source; it is not user transcript history.

### Fallback

- Clipboard rejection shows the defined failure status and keeps transcript state. Do not use browser clipboard, deprecated `execCommand`, a hidden editable element, a shell command, a temporary file, or a cloud service as fallback.
- Missing native runtime keeps Start/source controls unavailable. Do not simulate readiness.
- Missing Spec 01 dependencies/contracts is an integration blocker fixed before worktree creation, not a reason for Spec 02 to edit shared files.

## 11. Resource Lifecycle, Bounded State, Errors, and Recovery

- Session memory contains only `TranscriptSegment` objects and small UI status values; no PCM, model buffers, native handles, or persisted copies.
- Finalized segments remain immutable and retain object identity until Clear/unmount.
- Interim replacement changes one logical row and must not duplicate it.
- The transcript array is finite only by the current app process/session. Spec 02 introduces no secondary cache or duplicate formatted transcript string. Clipboard serialization allocates once per explicit copy request and releases the string after the promise settles.
- This is not an audio spec, so it owns no ring buffer/queue or overflow strategy. Bounded audio behavior remains mandatory in later native specs.
- At most one clipboard write may be pending from the workspace. Repeated clicks while pending are ignored/disabled.
- Copy promise fulfillment/rejection after unmount must not recreate state, leak a listener, or surface an unhandled rejection.
- Copy success is shown only for the finalized-content snapshot actually written. If finalized content changes while the promise is pending, do not claim the newer content was copied.
- Clipboard failure is recoverable by retry; no transcript mutation occurs.
- Clear confirmation holds no duplicate transcript copy. Cancel releases confirmation state; confirm releases prior segment references.
- Closing the app discards all state by process teardown. Relaunch starts empty.
- No interval, timeout, global listener, subscription, worker, observer, or native handle is introduced in Spec 02. Spec 11 may add a bounded feedback timeout with explicit cleanup.

## 12. Numbered Measurable Acceptance Criteria

1. **Predecessor and isolation — all hosts:** Spec 02 starts from the recorded clean Spec 01 SHA in its own branch/worktree; baseline tests pass before editing, and no other writer uses that physical checkout.
2. **Shared contract reuse — platform-neutral:** All transcript and capture-status code imports Spec 01’s `src/types` definitions; no duplicate `TranscriptSource`, `TranscriptSegment`, or `CaptureStatus` declaration exists.
3. **Stable order — platform-neutral:** Receiving new valid IDs renders them in first-seen order regardless of timestamp values, and replacing an interim keeps the same DOM/list position.
4. **Interim/final lifecycle — platform-neutral:** Same-ID/same-source interim updates replace in place without duplication; a final update replaces its interim; every later update to that final ID leaves its text/object unchanged.
5. **Source integrity — platform-neutral:** A same-ID update cannot change source or start identity; it is rejected without altering the original row, and only structurally `system` segments receive `- `.
6. **Raw text preservation — platform-neutral:** Intentionally incorrect grammar, fillers, repetitions, casing, punctuation, and spacing in `segment.text` remain byte-for-byte unchanged apart from the required system prefix and inter-segment clipboard separators.
7. **Clipboard serialization — platform-neutral:** Copy serialization includes finalized segments only, in current stable order, separated by exactly `\n\n`, with no leading/trailing newline or metadata; no finals yields `""`.
8. **Committed empty workspace — macOS native and web surface:** The actual committed app opens with the four-region layout, practical empty/privacy text, unavailable model/microphone/system status, `00:00:00`, and disabled Start/Clear/Copy actions; it contains no sample transcript or fake readiness.
9. **Populated/interim rendering — macOS visual fixture plus automated test:** Deterministic microphone/system final and interim segments render as selectable notepad text, system rows use `- `, interim is muted plus textually marked, and there are no bubbles/avatars/source colors.
10. **Capture presentation — platform-neutral:** The workspace maps idle/starting/listening/stopping/error to the specified action/status labels and disabled states, but the committed app cannot invoke Start/Stop without explicit real callbacks.
11. **Clear behavior — platform-neutral:** Clear is disabled when empty, clears interim-only content immediately, requires confirmation when any final exists, preserves content on Cancel/Escape, and removes every in-memory segment only after confirmation.
12. **Copy behavior — macOS native plus automated test:** Copy All is disabled without finals, performs one write per user activation, writes the exact four-final-segment sample to the real macOS clipboard, shows truthful pending/success feedback, and on injected rejection shows failure without altering transcript state.
13. **Volatile privacy — macOS inspection and platform-neutral source:** Populating, copying, clearing, closing, and reopening create no transcript database/file/browser-storage entry, emit no network request, and never log transcript text.
14. **Accessibility/responsiveness — macOS native/web:** Semantic regions and accessible names are exposed, keyboard focus/confirmation recovery works, status/interim distinctions do not rely on color, contrast is AA, and both empty/populated states remain usable at `720 × 520` and increased text size.
15. **Verification suite — implementation host:** `npm test`, `npm run typecheck`, `npm run lint`, `npm run build`, and `cargo check` pass; the actual Tauri app launches after changes and the temporary populated fixture is absent from the production build/committed source.
16. **Scope and handoff — repository:** Only owned paths plus this spec’s evidence changed, the superseded bootstrap test is removed rather than reworded, no shared manifest/config/native/context path changed, one focused local commit is created, and final SHA/clean status are reported to the integration owner without pushing.

## 13. Acceptance Criterion → Verification/Test Mapping

| AC | Verification or permanent test | Evidence to record |
|---|---|---|
| 1 | Compare worktree root/branch/base SHA; run baseline `npm test` and build before edits | Worktree, branch, base SHA, baseline command results |
| 2 | TypeScript/LSP reference inspection plus duplicate-symbol search scoped to `src` | Imported shared type paths and zero duplicate declarations |
| 3 | Reducer test with timestamps deliberately out of order; workspace row-order assertion before/after interim replacement | Test name and observed order |
| 4 | Reducer test covering interim → interim → final → ignored late update; assert stable length, position, and final object/text | Test result |
| 5 | Reducer invariant test for attempted source/start change; formatting tests for both sources | Rejection result and rendered strings |
| 6 | Unit/UI tests using `I didn't knew`, fillers, repeated words, casing, punctuation, and deliberate spaces | Exact before/after strings |
| 7 | Serializer tests for mixed final/interim, empty input, blank-line separator, and no trailing newline/metadata | Exact serialized output |
| 8 | Launch committed `npm run tauri dev`; inspect actual native window and browser-driven Vite surface | Screenshot/evidence path and exact visible/disabled state |
| 9 | React Testing Library semantic render plus temporary populated visual fixture in native/browser surfaces | Screenshot/evidence path and interim/system observations |
| 10 | Parameterized UI behavior test for five `CaptureStatus` states and callback availability | Action labels, disabled state, callback count |
| 11 | user-event tests for empty, interim-only, Cancel, Escape, and confirmed final-content clear; assert focus | Segment counts and focus targets |
| 12 | Injected writer success/pending/rejection tests; temporary native fixture; click Copy All and compare external `pbpaste` output byte-for-byte | Writer call count/text, real clipboard comparison, feedback states |
| 13 | Source/API inspection; run with network unavailable/blocked; inspect app-data paths before/after populate/copy/clear/close/reopen | Network, storage, filesystem, and logging observations |
| 14 | Role/name-based tests, keyboard-only walkthrough, contrast calculation, reduced-motion/text-scale checks, visual checks at `1040 × 720` and `720 × 520` | Accessibility and resize results |
| 15 | `npm test`; `npm run typecheck`; `npm run lint`; `npm run build`; `cargo check`; committed-state `npm run tauri dev` | Exact commands and exit/launch results; zero fixture wiring in committed diff |
| 16 | Diff ownership review; verify removed obsolete test; commit; `git status --short`; report final SHA | Changed paths, commit SHA, clean status, no push |

Permanent tests must assert consumer-observable domain/UI contracts. Do not assert Tailwind class strings, component file structure, reducer implementation syntax, mock echoes, source text, or incidental DOM nesting.

## 14. Ordered Implementation Plan

1. After Spec 01 merges, create the Spec 02 branch/worktree from its recorded final SHA; record the exact root, branch, and base SHA.
2. Re-read canonical context, Specs 01–03 and 05, current status, owned source, existing tests, shared types, tokens, plugin registration, and manifests. Run baseline checks.
3. Confirm Spec 01 supplied the frozen shared types, clipboard write plugin/permission, Lucide, and test dependencies. Escalate any gap to the integration owner before editing.
4. Delete the superseded bootstrap-screen test. Add behavior-first domain tests for stable order, interim replacement, final immutability, source integrity, raw preservation, and clipboard serialization.
5. Implement the pure transcript domain/reducer and feature-local typed invariant result. Keep formatting separate from rendering and clipboard transport.
6. Add workspace interaction tests for empty/populated/interim states, capture presentation, Clear confirmation/focus, Copy pending/success/failure, and semantic accessibility.
7. Implement the transcript workspace using the Spec 01 token set and shared types. Keep the transcript dominant, selectable, read-only, and free of chat metaphors.
8. Implement the feature-local clipboard writer boundary and production adapter to the existing Tauri plugin. Never request/read clipboard contents.
9. Replace `src/App.tsx` with the committed empty/unavailable composition. Do not seed sample data or enable native controls.
10. Run focused tests, typecheck, lint, and build; fix behavior/source rather than weakening assertions or suppressing diagnostics.
11. Launch the committed Tauri app and browser Vite surface. Verify empty state, keyboard behavior, computed tokens, initial/minimum sizes, text scaling, and contrast.
12. Temporarily seed the public session boundary with the deterministic transcript fixture. Verify populated/interim visuals and real Copy All through the native app; compare `pbpaste` to the exact expected text.
13. Exercise Clear Cancel/Escape/Confirm in the actual surface. Inspect network, storage, app-data, and console behavior without persisting transcript text in evidence.
14. Remove all temporary fixture/wiring and screenshots/logs from the worktree. Re-run the full checks and relaunch the committed empty app.
15. Review the complete diff for source swaps, final mutation, rewriting, accessibility failures, privacy leaks, unnecessary abstraction/allocation, forbidden path changes, and stale bootstrap behavior. Fix every High/Medium finding.
16. Fill this spec’s pre-commit evidence, create one focused local commit, then report the final commit SHA, clean status, changed paths, host/OS, and exact outcomes to the integration owner. Do not edit the shared tracker or push.

## 15. Risks, Rollback, Cleanup, and Preservation Rules

### Risks and mitigations

- **Wave 2 shared-file collision:** Spec 02 could need dependencies/capabilities also touched by Spec 03. Mitigation: Spec 01 owns the complete shared baseline; Spec 02 treats every root/native/shared contract as read-only.
- **Unreliable browser clipboard:** `navigator.clipboard` can be unavailable or reject outside a secure context. Mitigation: use the official Tauri clipboard plugin supplied by Spec 01; no browser fallback.
- **Overprivileged clipboard:** Reading the clipboard would expose unrelated user data. Mitigation: write-text only, user activation only, and capability inspection.
- **Fake product readiness:** A UI-only unit can appear operational. Mitigation: committed app shows unavailable runtime/source status and disabled Start; fixtures are temporary/test-only.
- **Source identity corruption:** Updating an interim by ID could accidentally swap microphone/system identity. Mitigation: same ID must retain source/start identity; reject conflicts and test the invariant.
- **Final transcript mutation:** Late native hypotheses could rewrite final text. Mitigation: reducer ignores every post-final update for the ID and tests object/text stability.
- **Accidental correction:** Trimming/normalizing text could mask spoken errors or spacing. Mitigation: formatting only adds structural prefix/separators and tests exact raw strings.
- **Copying unstable text:** Interim text could enter Obsidian and later change. Mitigation: Copy All includes finalized segments only and disables when none exist.
- **Destructive clear:** One click could discard finalized content. Mitigation: confirmation for any final; Cancel/Escape and focus behavior are tested.
- **Stale copy claim:** New finals could arrive while a write is pending. Mitigation: success corresponds to the serialized snapshot written and resets when final content changes.
- **Excessive rerender/allocation:** Frequent interim updates could repaint all transcript content or repeatedly serialize it. Mitigation: stable IDs/object references, memoizable rows, and serialization only on click; avoid speculative virtualization.
- **Accessibility noise:** High-frequency interim updates can over-announce. Mitigation: polite named log semantics and no duplicate final; Spec 11 tunes behavior against real event cadence.
- **Privacy leakage through diagnostics:** Test or error output could include user transcript. Mitigation: production errors contain no transcript text; only synthetic fixtures appear in test source.

### Rollback

Before merge:

1. Preserve the Spec 02 evidence outside generated build output.
2. Discard/remove only the dedicated Spec 02 worktree/branch after verifying it contains no user work.
3. Leave the Spec 01 integration baseline and parallel Spec 03/05 worktrees untouched.

After merge, rollback uses Git revert of the focused Spec 02 commit. Do not restore the obsolete bootstrap test/copy as the product UI; if rollback is required for integration, the integration owner must preserve the shared Spec 01 contracts and re-establish an honest non-operational surface.

### Cleanup before completion

- Remove the temporary populated App fixture/wiring.
- Remove screenshots, `pbpaste` captures, network logs, browser storage probes, and generated build output from tracked paths.
- Remove the superseded Spec 01 bootstrap test.
- Remove unused imports/components/helpers discovered during implementation.
- Leave no TODO, fake callback, seeded transcript, timer, listener, disabled test, snapshot dump, or commented alternate implementation.
- Keep the synthetic incorrect-English fixture only where it earns a permanent domain/UI test.

### Preservation rules

- Preserve Spec 01 shared types, tokens, test/tool configuration, clipboard permission, native files, and lockfiles unchanged.
- Preserve transcript text and source metadata exactly.
- Preserve microphone/system separation, no grammar rewrite, no persistence, no cloud/backend/account, and offline operation.
- Preserve other Wave 2 branches/worktrees and unrelated user changes.

## 16. Definition of Done and Evidence Record

Spec 02 is done only when the committed app displays the honest empty workspace, deterministic transcript behavior passes durable tests, populated/interim states have been visually exercised, real Copy All has written the exact expected plain text through the native macOS app, Clear behavior has been exercised, and every acceptance criterion has recorded evidence.

Record before the final commit:

- implementation date
- absolute worktree
- branch and base SHA
- implementation host hardware and macOS version
- changed paths
- baseline check results
- focused test names/results
- exact reducer/serializer behavior results
- empty native-window evidence path/observations
- populated/interim temporary-fixture evidence path/observations
- real clipboard comparison result without persisting user transcript
- Clear Cancel/Escape/Confirm observations
- initial/minimum-size and text-scaling observations
- accessibility/contrast results
- network/storage/app-data inspection results
- `npm test` result
- `npm run typecheck` result
- `npm run lint` result
- `npm run build` result
- `cargo check` result
- committed-state `npm run tauri dev` result
- temporary-fixture removal result
- forbidden-path/diff review
- High/Medium findings and resolutions
- planned commit subject

The post-commit handoff must report the final commit SHA and `git status --short` result. Those self-referential values are not written back into the commit itself.

### Implementation evidence

- **Implementation date:** 2026-09-11
- **Absolute worktree:** `/Users/berat/mistaken-spec-02`
- **Branch:** `spec/02-transcript-workspace`
- **Base SHA:** `a7fe826078bae76c8de3c442fae741f937b271e0` (Spec 01 baseline, `main`)
- **Implementation host:** Apple M4, macOS 15.7.5 (24G624), arm64
- **Changed paths:**
  - `src/App.tsx` (replaced bootstrap composition with the committed transcript workspace)
  - `src/App.test.tsx` (deleted — superseded bootstrap-screen test)
  - `src/features/transcript/transcript-domain.ts` (new)
  - `src/features/transcript/transcript-domain.test.ts` (new)
  - `src/features/transcript/use-transcript-session.ts` (new)
  - `src/features/transcript/transcript-clipboard.ts` (new)
  - `src/features/transcript/TranscriptWorkspace.tsx` (new)
  - `src/features/transcript/TranscriptWorkspace.test.tsx` (new)
  - `docs/specs/spec-02-transcript-domain-workspace.md` (this evidence record)
- **Baseline check results (before editing):** `npm test` 1/1 passed; `npm run typecheck` clean; `npm run lint` (oxlint) clean; `npm run build` succeeded; `cargo check` succeeded (from a clean `npm ci` install in the fresh worktree).
- **Focused test names/results:** 44/44 Vitest tests passed across `transcript-domain.test.ts` (17 tests: reducer first-seen order, interim replacement, final immutability, source/start-identity rejection, lastError clear, unrelated-object-identity retention, raw-text preservation, `formatTranscriptSegment`, `serializeFinalTranscript`) and `TranscriptWorkspace.test.tsx` (27 tests: empty state, populated/interim rendering + row order, domain-invariant rejection status, all five `CaptureStatus` presentations, Clear empty/interim-immediate/confirm/Cancel/Escape/Confirm, Copy disabled/success/failure/pending-dedupe/stale-content-guard, a `React.StrictMode` regression test, and accessibility name/description checks).
- **Exact reducer/serializer behavior results:** All required invariants verified by test, including byte-exact preservation of `"  Um, I actually have went  there yesterday,  didn't I??  "` and rejection of a same-id source/start change without mutating the stored segment (`lastError` set to `{code:"segment_identity_conflict", segmentId}`).
- **Empty native-window evidence:** `/tmp/spec02-evidence/01-empty-state-initial.png` (1040×720) and `/tmp/spec02-evidence/11-final-committed-empty-state.png` (post fixture-removal relaunch) — both show `Mistaken` / `Local • Runtime unavailable`, unavailable mic/system-audio source labels, the exact four-line empty-state copy, `00:00:00`, and disabled `Clear`/`Start Listening`/`Copy All`. `/tmp/spec02-evidence/02-empty-state-min-size.png` shows the same content at the `720×520` minimum with no overflow/clipping.
- **Populated/interim temporary-fixture evidence:** `/tmp/spec02-evidence/03-populated-final-state.png` — deterministic fixture (mounted temporarily in `App.tsx`, removed before commit) rendered the ui-context.md example conversation: unprefixed microphone finals, `- `-prefixed system finals, one muted row carrying a visible `Interim` marker, and proof that a post-final rewrite attempt (`"...didn't knew..."` → `"...didn't know..."`) was rejected — the original incorrect grammar remained on screen.
- **Real clipboard comparison:** Clicked the actual `Copy All` button in the native window (`AXPress` via System Events, then a hardware-level `cliclick` press, both confirmed against the app's own accessibility tree — not a mocked writer) and compared `pbpaste` byte-for-byte against the expected string with `cmp`: **byte-identical, 141 bytes, no trailing newline** (`/tmp/spec02-evidence/expected-copy-all-notrail.txt` vs `/tmp/spec02-evidence/actual-copy-all-clean.txt`). No transcript bytes were persisted into tracked evidence beyond this synthetic fixture text. Screenshot `/tmp/spec02-evidence/08-copy-all-success-confirmed.png` shows the app's own `Copied` success feedback.
- **Defect found and fixed during real-app verification:** the first real Copy All click wrote the correct clipboard bytes but the button stayed on `Copying…` forever. Root cause: `TranscriptWorkspace`'s mount-tracking `useLayoutEffect(() => () => { isMountedRef.current = false }, [])` only ever set the ref to `false` (no `true` setup), so React 18 `StrictMode`'s development-only double-invocation of that effect's cleanup on initial mount permanently tripped the "unmounted" guard, causing every `writeClipboard(...).then()/.catch()` to no-op. Fixed by setting `isMountedRef.current = true` in the effect body before returning the cleanup. Added a permanent regression test, `"resolves a pending copy to success under React.StrictMode (matches real main.tsx mounting)"`, that renders `TranscriptWorkspace` inside `<StrictMode>` (matching real `main.tsx`) and asserts `Copied` appears; verified this test fails on the pre-fix code (reproducing the exact stuck `Copying…` DOM) and passes after the fix.
- **Clear Cancel/Escape/Confirm observations (real app):** `Clear` → confirmation shown (`Clear transcript` + `Cancel`, content preserved, screenshot `09-clear-confirmation-open.png`) → `Cancel` → confirmation dismissed, content preserved, `AXFocused` confirmed `true` on the `Clear` button. Re-opened confirmation → `Escape` (sent only once "mistaken" was confirmed frontmost, since another foreground app on this shared host intermittently stole window focus between commands) → confirmation dismissed, content preserved. Re-opened confirmation → `Clear transcript` → transcript returned to the exact empty state (screenshot `10-clear-confirmed-empty.png`). Focus-restoration-to-`Clear`-button after Cancel/Escape is additionally proven deterministically by the `toHaveFocus()` assertions in `TranscriptWorkspace.test.tsx`, because live `AXFocused` readings on this shared host were periodically unreliable due to real concurrent foreground-app switching outside this session's control.
- **Initial/minimum-size and text-scaling observations:** `1040×720` and `720×520` both verified visually (screenshots above); all four regions remain visible, transcript stays the dominant area, no horizontal overflow. Text-scaling was not exercised with the OS accessibility zoom control (no system-level automation available in this environment); layout uses relative (`rem`/`ch`/Tailwind) units throughout with no fixed pixel heights that would clip growing text, consistent with the `ui-context.md` responsive rules.
- **Accessibility/contrast results:** `header`→`banner`, `section[aria-label="Audio sources"]`→`region`, `section[role="log" aria-label="Transcript"]`→named log, `footer`→`contentinfo` all confirmed via Testing Library role queries. Disabled `Start Listening`/`Clear`/`Copy All` expose `aria-describedby` reasons (verified by `toHaveAccessibleDescription`). Every rendered button has non-empty visible text (no icon-only critical actions). Computed WCAG contrast ratios for every text/background token pair actually used: `textPrimary/bgBase` 17.68:1, `textSecondary/bgBase` 11.47:1, `textMuted/bgBase` 5.16:1, `textInterim/bgBase` 6.52:1, `bgBase/accentPrimary` (Start Listening label) 12.16:1, `stateError/bgBase` 6.80:1, `stateSuccess/bgBase` 12.16:1, `stateWarning/bgBase` 10.66:1 — all exceed the 4.5:1 AA threshold for normal text. No `transition`/`animate`/keyframe classes were introduced anywhere in the new feature code, so reduced-motion has nothing to suppress.
- **Network/storage/app-data inspection results:** source grep of `src/features/transcript/**` and `src/App.tsx` for `localStorage|sessionStorage|indexedDB|fetch(|XMLHttpRequest|console.log|console.warn|console.error|writeTextFile|invoke(` — zero matches. `~/Library/Caches/mistaken` contains only standard WebKit engine cache directories (`WebKit/NetworkCache`, `WebKit/HSTS`, `WebKit/AlternativeServices`, `WebKit/CacheStorage`) — OS/WebView infrastructure metadata, not transcript content. No `~/Library/Application Support/mistaken` (or similarly named) directory exists. No transcript text was written to any tracked file; all clipboard/screenshot evidence lives under `/tmp/spec02-evidence/` outside the repository.
- **`npm test` result:** 44/44 passed (2 test files) — final run after fixture removal.
- **`npm run typecheck` result:** clean (`tsc --noEmit`, no output, exit 0).
- **`npm run lint` result:** clean (`oxlint .`, no findings; one `react(refs)` finding during development — writing a ref during render — was fixed by moving the write into a `useLayoutEffect`, then re-verified clean).
- **`npm run build` result:** succeeded; `vite build` produced `dist/index.html`, `dist/assets/index-*.css` (14.62 kB), `dist/assets/index-*.js` (232.91 kB).
- **`cargo check` result:** succeeded from `src-tauri` (no Rust files changed by this spec). `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` were also run for completeness and all passed with zero warnings/failures.
- **Committed-state `npm run tauri dev` result:** launched successfully; the real native `Mistaken` window opened at `1040×720` showing only the honest empty workspace described above, with no working-looking control, fake transcript, or hidden startup work. Verified twice: once before the temporary fixture was added, and again after the fixture and its `StrictMode` clipboard-bug fix were finalized (screenshot `11-final-committed-empty-state.png`).
- **Temporary-fixture removal result:** `src/App.tsx` was restored to the exact committed empty composition (byte-identical to the pre-fixture version); `git status --short` after removal shows only the intended owned-path changes (see Diff review below). No fixture code, timer, or seeded segment remains in any tracked file.
- **Forbidden-path/diff review:** `git status --short` shows exactly `D src/App.test.tsx`, `M src/App.tsx`, and new files under `src/features/transcript/`, plus this evidence edit to `docs/specs/spec-02-transcript-domain-workspace.md`. No change to `src/lib/tauri/**`, `src/types/**`, `src/main.tsx`, `src/index.css`, `package.json`, `package-lock.json`, `vite.config.ts`, `vitest.config.ts`, lint/TypeScript configuration, any `src-tauri/**` file, `benchmarks/**`, another spec file, or `docs/context/progress-tracker.md`.
- **High/Medium findings and resolutions:**
  1. **High — lint:** ref mutated during render (`react(refs)`). Fixed by moving the assignment into a `useLayoutEffect` keyed on the derived value.
  2. **High — functional bug, found via real-app verification:** `StrictMode` double-invoked mount-cleanup permanently disabled Copy success/failure feedback after the first render (see defect note above). Fixed at the source (`isMountedRef` setup) and covered by a permanent `StrictMode` regression test proven to fail pre-fix and pass post-fix.
  3. **Environmental, not a product defect:** this development host had real concurrent foreground-app activity (another application repeatedly took OS window focus and, once, briefly overwrote the system pasteboard) during manual verification. Every clipboard/keyboard/focus assertion below was re-run only once "mistaken" was confirmed frontmost immediately beforehand, and the byte-exact clipboard comparison and screenshot evidence above were captured in that state.
  4. **Process/tooling note:** `@testing-library/user-event` was not present in the Spec 01 baseline's `package.json`/`node_modules`, although `docs/specs/spec-02-transcript-domain-workspace.md` §3 describes "user-event infrastructure" as supplied. Per §5, this worktree must not edit `package.json`/`package-lock.json` opportunistically, so all interaction tests use `fireEvent`/`act` from the already-present `@testing-library/react`, which fully exercises the same click/keydown code paths. Recorded here for the integration owner; no owned-file or dependency edit was made to work around it.
- **Planned commit subject:** `Spec 02: transcript domain and single-window workspace`

## Official References Verified During Authoring

- [React: Extracting State Logic into a Reducer](https://react.dev/learn/extracting-state-logic-into-a-reducer)
- [Tauri Clipboard plugin](https://v2.tauri.app/plugin/clipboard/)
- [MDN: Clipboard `writeText()` and secure-context requirements](https://developer.mozilla.org/en-US/docs/Web/API/Clipboard/writeText)
- [Vitest test environments](https://vitest.dev/guide/environment.html)
- [Testing Library query priority](https://testing-library.com/docs/queries/about/)
