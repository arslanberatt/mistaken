# Agent Entrypoint

This file is the required starting point for any agent (human or AI) working
in this repository. Read it before touching source, and follow it exactly.

## 1. Required Context Read Order

Read these files, in this order, before implementing anything:

1. `docs/context/project-overview.md` — what Mistaken is and what is in/out of scope.
2. `docs/context/architecture.md` — stack, boundaries, data flow, invariants.
3. `docs/context/ui-context.md` — visual language, layout, states, UI conventions.
4. `docs/context/code-standards.md` — implementation conventions.
5. `docs/context/progress-tracker.md` — current unit, completed work, next work, open decisions.
6. `docs/context/spec-plan.md` — spec inventory, dependency graph, parallel waves, file ownership.
7. `docs/context/ai-workflow-rules.md` — rules for how the coding agent should work.
8. The exact spec file under `docs/specs/` you are implementing.
9. Current Git status (`git status`, `git log -1`) and the relevant existing code/tests/manifests.

If implementation and documentation disagree, resolve the conflict in the
relevant `docs/context/` file first. Do not silently pick one.

## 2. Spec Workflow

- Mistaken is built spec-by-spec from `docs/specs/`. Each spec is a complete,
  independently verifiable vertical slice with its own acceptance criteria,
  file ownership list, and verification map.
- Implement only the requested spec. Do not start a later spec, and do not
  widen scope into files owned by another spec.
- A spec starts only after every spec it depends on (see the dependency graph
  in `docs/context/spec-plan.md`) has passed its acceptance checks, been
  reviewed, and been merged into the integration branch (`main`).
- Follow the spec's own "Implementation Order" section step by step.
- Record verification evidence directly in the spec's
  "Implementation evidence" section and update `docs/context/progress-tracker.md`
  before considering the work complete.

## 3. One-Writer and Worktree Rules

- A physical checkout has exactly one writer. Two concurrent writers must
  never share one checkout.
- Parallel specs each use their own Git branch and worktree, created from one
  recorded base SHA:

  ```bash
  cd /Users/berat/mistaken
  BASE_SHA=$(git rev-parse HEAD)
  git worktree add ../mistaken-spec-02 -b spec/02-transcript-workspace "$BASE_SHA"
  ```

- Root dependency manifests/lockfiles, shared Tauri entry points, common
  IPC/event contracts, common audio/ASR traits, root application composition,
  global tokens, and `docs/context/**` are single-writer files per wave. A
  parallel implementer that discovers a required shared-file change records
  it and yields that path to the integration owner instead of editing it
  opportunistically.
- Each implementation changes only its owned paths, records verification
  evidence, and commits locally. Do not push unless explicitly requested.
- Worktrees are removed only after their commit is merged and evidence is
  preserved.

## 4. Verification Obligations

Before any spec is considered done:

- The smallest practical verification path for that spec's exact scope has
  been run for real (not asserted).
- `npm run typecheck`, `npm run lint`, `npm run build`, and `npm test` pass.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test`, and `cargo check` pass from `src-tauri` for any Rust change.
- Platform-specific checks pass when native/platform code changed.
- No invariant in `docs/context/architecture.md` was violated.
- `docs/context/progress-tracker.md` reflects completed work, discoveries,
  and next work.
- No transcript persistence, grammar correction, or semantic rewriting of
  recognized speech was introduced.

## 5. Non-Negotiable Product Rules

Unless the user explicitly changes the specification, the following are
banned everywhere in this repository:

- BMAD or any other external agentic-framework scaffolding.
- Any paid or metered cloud/transcription API, cloud AI service, or API key
  required for core functionality.
- A required internet connection for the core transcription flow.
- A backend, account/authentication system, or transcript database.
- Automatic grammar correction or semantic rewriting of recognized speech.
- Mixing microphone and system audio before transcription, or inferring
  source identity from transcript text instead of structural metadata.

See `docs/context/ai-workflow-rules.md` for the complete rule set.
