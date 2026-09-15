# AI Workflow Rules

## Approach

Build Mistaken incrementally using a spec-driven workflow. The context files define product behavior, architecture boundaries, UI rules, code standards, and current progress. Implement against these documents instead of inventing product behavior during coding.

The project is local-first and deliberately narrow. The coding agent must optimize for transcript fidelity, clean platform boundaries, deterministic source separation, and maintainable native integration before adding convenience features.

The following product rules are non-negotiable unless the user explicitly changes the specification:

- No paid transcription API
- No metered cloud speech service
- No API key for core functionality
- No required internet connection
- No backend
- No authentication
- No transcript database
- No automatic transcript history
- No automatic grammar correction
- No semantic rewriting of recognized speech
- Microphone and system audio remain separate sources
- Microphone transcript = normal text
- System-audio transcript = `- ` prefix

## Source of Truth

Read these files before implementing related work:

1. `project-overview.md` — what Mistaken is and what is in/out of scope.
2. `architecture.md` — stack, boundaries, data flow, invariants, and technical decisions.
3. `ui-context.md` — visual language, layout, states, and UI conventions.
4. `code-standards.md` — implementation conventions.
5. `progress-tracker.md` — current unit, completed work, next work, and unresolved decisions.
6. `spec-plan.md` — implementation-spec count, dependencies, file ownership, parallel waves, and model assignment.
7. `ai-workflow-rules.md` — rules for how the coding agent should work.

If implementation and documentation disagree, do not silently choose one. Resolve the conflict in the relevant context file first.

## Scoping Rules

- Work on one feature unit per branch and physical checkout. A checkout has exactly one writer.
- Prefer small, end-to-end verifiable increments over broad speculative refactors.
- Do not combine unrelated system boundaries in a single implementation step.
- Platform-specific work should be implemented and tested independently where practical.
- Do not implement future features just because the architecture could support them.
- Do not add dependencies unless they directly solve a current requirement.
- Do not add cloud services as shortcuts for difficult native/local work.
- Do not add persistence unless explicitly approved.
- Do not hide technical failures with automatic fallbacks that violate product invariants.

## Parallel Terminal Rules

Parallel implementation follows `spec-plan.md`.

- Parallel writers are allowed only after Spec 01 creates the dedicated Mistaken Git repository and commits a shared baseline.
- Every concurrent spec uses its own Git branch and worktree. Two writing terminals must never share one physical checkout.
- A spec starts only after all declared predecessors for its current maturity have passed their applicable checks, been reviewed, and been merged into the integration branch. The sole current exception is the documented ASR maturity contract: Specs 06 and 09–11 may proceed in `DEVELOPMENT` mode after the Spec 05 benchmark evidence and product-owner exception merge, while Spec 05 remains `BLOCKED — no candidate approved`.
- Specs may run concurrently only when their exact owned paths are disjoint. Intent or feature names do not override real file-path overlap.
- Each wave starts from one recorded base SHA. A later wave starts from the integration SHA produced after the preceding merges.
- Root manifests and lockfiles, shared Tauri entry points, common IPC/event contracts, common audio/ASR traits, application composition, global tokens, bundled-resource manifests, and context/tracker documents are single-writer files.
- A parallel implementer that discovers a required shared-file change records it and yields that path to the integration owner; it does not edit the file opportunistically.
- One integration owner reviews and merges branches in dependency order, runs combined checks after each merge, and is the only writer to shared context/tracker files during a wave.
- Spec authors may work in parallel because each owns one spec file, but one high-reasoning integration owner must reconcile dependencies, contracts, terminology, and file ownership before implementation begins.
- Each terminal reports its spec ID, absolute worktree, branch, base SHA, final commit SHA, changed paths, exact verification evidence, hardware, OS, and any external blocker.

The exception never propagates release approval. Temporary-adapter transcript-quality results are `NON-RELEASE EVIDENCE`; Spec 12 cannot return `PASS` or authorize packaging, Specs 13–14 cannot produce distributable `PASS` artifacts, and Spec 15 cannot return `RELEASE-READY` until Spec 05 names exactly one production-approved candidate that passes every unchanged gate.

## Recommended Implementation Units

Unless progress or discoveries require a different order, prefer units similar to:

1. Bootstrap Tauri 2 + React + TypeScript + styling.
2. Build the static transcript workspace UI and session state.
3. Define typed Tauri IPC and native runtime state.
4. Enumerate/select microphone devices.
5. Capture microphone PCM locally.
6. Integrate a minimal local ASR proof of concept for microphone speech.
7. Benchmark local ASR candidates using Mistaken-specific incorrect-English samples.
8. Implement Windows system-audio capture.
9. Implement macOS system-audio capture.
10. Run two simultaneous independent ASR streams.
11. Aggregate microphone/system segments into the required transcript format.
12. Add Stop / Clear / Copy All and failure states.
13. Validate offline operation with network disabled.
14. Package/test on supported Windows and macOS targets.

Do not try to implement both native audio platforms, ASR, UI, packaging, and benchmarking in one change.

## When to Split Work

Split an implementation step if it combines:

- Windows audio capture and macOS audio capture
- Audio capture and ASR model benchmarking
- ASR runtime integration and UI redesign
- Native Rust changes and unrelated frontend refactors
- Model packaging and speech-recognition algorithm changes
- Permission handling and unrelated transcript features
- Multiple unresolved technical decisions
- Behavior not clearly defined in the context files

If a change cannot be verified end to end quickly, the scope is too broad. Split it.

## Handling Missing Requirements

- Do not invent product behavior not defined in the context files.
- If a requirement is ambiguous, choose the smallest behavior consistent with the current product goals and record the decision.
- If the ambiguity can materially affect architecture, add it to `progress-tracker.md` under `Open Questions` before implementation.
- If a model/runtime license is unclear, do not bundle or redistribute it until verified.
- If a platform API cannot meet a requirement, record the limitation instead of silently using a paid/cloud alternative.
- When a target OS version affects the native implementation, record the minimum-version decision explicitly.

## ASR-Specific Rules

- The recognizer output is transcript data, not prose to be polished.
- Never send recognized text through an LLM or grammar corrector before rendering it.
- Never normalize incorrect grammar into correct English.
- Preserve repetitions and filler words when the recognizer outputs them.
- Keep interim text separate from finalized segments.
- Do not mutate finalized transcript segments because later text "sounds more correct".
- If a recognizer revises an interim hypothesis, only the interim segment may change.
- Benchmark candidate models using intentionally incorrect English; normal benchmark speech alone is insufficient for this product.
- Track model/runtime identity during development benchmarks so comparisons are reproducible.
- Keep ASR provider/runtime-specific code behind an adapter.
- Treat `DEVELOPMENT ASR ADAPTER` and `PRODUCTION APPROVED ASR MODEL` as disjoint maturity states. Never infer the latter from working IPC, real microphone output, source separation, lifecycle checks, a signed build, or downstream integration success.
- A temporary development adapter must be fully local, real, checksum/version pinned, license-recorded, visibly labeled `Development ASR • Not release approved`, and replaceable through the existing recognizer abstraction.

## Audio-Specific Rules

- Microphone and system audio are distinct sources.
- Never mix them before ASR for the standard two-source workflow.
- Native capture callbacks must remain lightweight.
- Do not perform expensive inference directly inside the audio callback.
- Use bounded buffering.
- Make the overflow/backpressure strategy explicit.
- Convert audio format in a dedicated normalization stage.
- Avoid temporary audio files unless required for a narrowly scoped debug workflow.
- Debug recordings must never become production behavior without explicit approval.

## Platform Rules

### Windows

- Prefer WASAPI loopback for system audio.
- Keep Windows-only code under the Windows platform adapter.
- Do not require virtual audio cable software for the core flow.

### macOS

- Prefer ScreenCaptureKit for system audio.
- Keep macOS-only code under the macOS platform adapter/native helper.
- Permission-denied behavior must be testable and visible to the user.
- Do not request screen/audio capture permission before it is needed.

## Dependency Rules

Before adding a dependency, confirm:

1. It solves a requirement in the current implementation unit.
2. It can run locally for the required feature.
3. It does not introduce a paid runtime dependency.
4. Its license is compatible with the project.
5. It does not duplicate an existing dependency without a clear benefit.

For ASR models, dependency review must include the model-weight license separately from the runtime library license.

## Protected Files

Do not modify the following unless the current task explicitly requires it:

- Vendored third-party source code
- Local ASR model binaries/weights
- Generated lockfiles except as the result of an intentional dependency change
- Generated Tauri platform artifacts
- `src/components/ui/*` primitives generated by shadcn/ui unless a product-specific extension is needed
- OS entitlement/capability files unless the current task is specifically about the relevant platform permission/capability

Never patch third-party library internals to avoid understanding an integration problem. Fix the integration boundary or pin/replace the dependency.

## Keeping Docs in Sync

Update the relevant context file whenever implementation changes:

- Product scope -> `project-overview.md`
- Architecture/system boundaries -> `architecture.md`
- UI patterns/tokens -> `ui-context.md`
- Code conventions -> `code-standards.md`
- Current work/status/open decisions -> `progress-tracker.md`
- Agent implementation process -> `ai-workflow-rules.md`

A meaningful implementation change is not complete until `progress-tracker.md` is updated.

## Verification Rules

Each implementation unit should have the smallest practical verification path.

Examples:

- UI unit -> render and interact with the exact state being implemented.
- Microphone unit -> enumerate devices, start capture, observe valid PCM levels, stop cleanly.
- ASR unit -> feed known local audio and compare transcript output.
- Windows loopback unit -> play known audio and verify only the system source receives it.
- macOS system-audio unit -> play known audio and verify ScreenCaptureKit audio frames reach the pipeline.
- Dual-stream unit -> speak into mic while system audio plays and verify source identities never swap.
- Offline unit -> disable network and run the complete core flow.

## Before Moving to the Next Unit

1. The current unit works end to end within its defined scope.
2. No invariant defined in `architecture.md` was violated.
3. No paid/cloud fallback was introduced.
4. `progress-tracker.md` reflects completed work, discoveries, and next work.
5. TypeScript checks pass.
6. `npm run build` passes.
7. `cargo check` passes for relevant Rust changes.
8. Relevant platform-specific checks pass when native code changed.
9. No transcript persistence was accidentally introduced.
10. No recognized text is being grammar-corrected or rewritten.
