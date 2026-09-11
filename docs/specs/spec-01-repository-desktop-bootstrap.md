# Spec 01 — Repository and Desktop Bootstrap

## 1. Status, Ownership, Base, and Gates

- **Status:** Authored; ready for cross-spec integration review. Not yet implemented.
- **Implementation owner:** One terminal/session in the canonical checkout. No concurrent writers during this bootstrap.
- **Staging source:** `/Users/berat/mistaken-context`
- **Canonical repository target:** `/Users/berat/mistaken`
- **Base branch:** `main`
- **Base SHA:** Not applicable. The target repository does not exist yet.
- **Allowed implementation predecessors:** None.
- **Execution gate:** Specs 01–15 must exist under `/Users/berat/mistaken-context/specs`, their shared contracts must have passed the high-capability integration review required by `spec-plan.md`, and the source context bundle must be internally consistent before this spec is applied.
- **Successor gate:** Specs 02, 03, and 05 may begin only from the clean baseline commit produced by this spec.
- **Review level:** High. This spec fixes repository layout, tooling, shared entrypoints, security baseline, and the SHA from which parallel work starts.

## 2. Goal and Visible Result

Create Mistaken’s dedicated Git repository as a reproducible Tauri 2 desktop application using React, TypeScript, Vite, Tailwind CSS, and a Rust native core.

The visible result is a real native desktop window titled **Mistaken**. It uses the approved dark visual foundation and truthfully reports that the desktop runtime is ready while audio capture and transcription are not configured yet. It contains no fake controls, demo greeting, chat UI, cloud/account affordance, or simulated transcript.

The measurable result is:

1. `/Users/berat/mistaken` is a clean `main`-branch Git repository with a recorded baseline SHA.
2. The canonical context and all staged specs are committed inside that repository.
3. Frontend and Rust baseline checks pass from a fresh dependency install.
4. `npm run tauri dev` opens the native Mistaken window on the implementation host.
5. Closing and reopening the app leaves no helper process, listener, temporary transcript, database, or network dependency behind.

## 3. Verified Current Behavior

Verified while authoring this spec:

- `/Users/berat/mistaken` does not exist.
- `/Users/berat/mistaken-context` exists, contains only planning/context documents, and is not a Git repository.
- No Mistaken application code, package manifest, Cargo manifest, test suite, database schema, environment file, or generated artifact exists.
- The planning bundle defines Tauri 2 + React + TypeScript + Vite + Tailwind as the baseline and a Rust native core as the owner of native work.
- The current macOS host has Node `v24.15.0`, npm `11.12.1`, and Xcode `26.3` available.
- `rustc` and `cargo` are not installed on the current host. Rust installation is therefore a concrete bootstrap prerequisite, not an assumed dependency.
- The current official `create-tauri-app` React TypeScript template includes a demo `greet` command and the opener plugin. Neither serves a Spec 01 requirement and both must be removed.
- Tauri 2 capability files scope permissions to named windows/webviews. A baseline with no frontend-native feature does not need filesystem, shell, opener, dialog, HTTP, clipboard, or process permissions.
- Tailwind’s current Vite integration uses the maintained `@tailwindcss/vite` plugin and `@import "tailwindcss"`.

Authoritative product and architecture behavior remains the content under `/Users/berat/mistaken-context`; no prior implementation report exists.

## 4. Scope

### In scope

- Create `/Users/berat/mistaken` without modifying another checkout.
- Initialize Git with `main` as the initial branch.
- Import the canonical context documents and staged specs.
- Add a root agent entrypoint that enforces the documented read order and spec workflow.
- Bootstrap Tauri 2, React, strict TypeScript, Vite, Tailwind CSS, and the minimal Rust application.
- Pin the JavaScript and Rust toolchains/dependency resolution needed for reproducible parallel worktrees.
- Establish baseline npm scripts and Rust quality commands.
- Create the single native main window and the honest dark bootstrap screen.
- Establish a least-privilege Tauri capability and CSP baseline.
- Freeze the small shared TypeScript contracts, test harness, and dependency set required for Wave 2 branches to remain disjoint.
- Register the official Tauri clipboard manager for plain-text writes only; this is the complete native prerequisite for Spec 02 `Copy All`, not a clipboard-reading feature.
- Remove all generated template/demo behavior that is not part of Mistaken.
- Verify frontend compilation, linting, Rust formatting/linting/checking, native compilation, native launch, close, and relaunch.
- Update the canonical progress tracker and this spec’s implementation evidence after successful application.
- Create one focused local baseline commit and report its branch and SHA. Do not push.

### Out of scope

- Transcript workspace, responsive application composition, empty/listening/error transcript states, or operational controls. Spec 02 owns them.
- Typed IPC lifecycle commands/events and shared transcript contracts. Spec 03 owns them.
- Microphone enumeration/capture, PCM conversion/buffering, microphone permissions, devices, or disconnect handling. Spec 04 owns them. System-audio capture and its platform permissions belong to Specs 07–08; shared orchestration/resampling belongs to Specs 06 and 09 as specified.
- ASR adapters, model discovery/download/storage, recognizer workers, inference, benchmark selection, model identifiers, or transcript streaming. Spec 05 owns benchmark/license approval, Spec 06 owns first local microphone ASR/model lifecycle, and Specs 09–10 own dual-source orchestration and resilience.
- Transcript copy/clear UI and formatting behavior, settings, onboarding, recovery workflows, packaging, signing, installers, updater, telemetry, CI/CD, and release automation. Later specs own them; this spec only prepares the reviewed shared clipboard/test dependency boundary needed for parallel Wave 2 work.
- Backend, account, authentication, database, cloud service, remote API, API key, analytics, crash reporting, or networked runtime behavior.
- Final product iconography and distributable bundles. Specs 13 and 14 own final release assets.
- Placeholder directories, stub commands, no-op modules, fake responses, mocked native behavior, or TODO scaffolding for later specs.

## 5. File Ownership and Concurrency Boundaries

### Files and directories owned by Spec 01 during implementation

The repository is new, so this spec is the sole owner of the initial baseline:

- `/Users/berat/mistaken/.gitignore`
- `/Users/berat/mistaken/.nvmrc` or the project’s single selected Node-version file
- `/Users/berat/mistaken/rust-toolchain.toml`
- `/Users/berat/mistaken/AGENTS.md`
- `/Users/berat/mistaken/README.md`
- `/Users/berat/mistaken/package.json`
- `/Users/berat/mistaken/package-lock.json`
- `/Users/berat/mistaken/index.html`
- `/Users/berat/mistaken/tsconfig*.json`
- `/Users/berat/mistaken/vite.config.ts`
- the one chosen lint configuration
- `/Users/berat/mistaken/src/main.tsx`
- `/Users/berat/mistaken/src/App.tsx`
- `/Users/berat/mistaken/src/index.css`
- `/Users/berat/mistaken/src/vite-env.d.ts`
- `/Users/berat/mistaken/src/types/transcript.ts`
- `/Users/berat/mistaken/src/types/runtime.ts`
- `/Users/berat/mistaken/src/test/setup.ts`
- `/Users/berat/mistaken/src/App.test.tsx`
- `/Users/berat/mistaken/vitest.config.ts`
- `/Users/berat/mistaken/src-tauri/build.rs`
- `/Users/berat/mistaken/src-tauri/Cargo.toml`
- `/Users/berat/mistaken/src-tauri/Cargo.lock`
- `/Users/berat/mistaken/src-tauri/tauri.conf.json`
- `/Users/berat/mistaken/src-tauri/capabilities/main.json`
- `/Users/berat/mistaken/src-tauri/src/main.rs`
- `/Users/berat/mistaken/src-tauri/src/lib.rs`
- `/Users/berat/mistaken/docs/context/**`
- `/Users/berat/mistaken/docs/specs/**`

Generated build output such as `node_modules`, `dist`, and `src-tauri/target` is not owned source and must be ignored.

### Forbidden changes

- Do not modify, move, delete, or initialize Git inside `/Users/berat/mistaken-context`; it remains the immutable staging/rollback source for this operation.
- Do not touch `/Users/berat/anton/extech` or any unrelated checkout.
- Do not copy or create `.env`, credentials, keys, model files, recordings, transcript fixtures, or machine-specific absolute paths inside committed application configuration.
- Do not retain generated Tauri demo assets, demo commands, opener permissions, or sample links merely because the scaffold produced them.
- Do not add future feature directories with empty modules.

### Post-bootstrap ownership handoff

After the baseline commit:

- Spec 02 owns the first replacement of `src/App.tsx` and the application workspace composition; it consumes, rather than duplicates, the global tokens, shared transcript/runtime types, clipboard-write plugin, and frontend test harness established here.
- Spec 03 owns the first real custom IPC contract, Tauri command/event registration, and any capability changes needed for that contract. It consumes the frozen shared transcript/runtime types and must not redefine them.
- Spec 05 is limited to its benchmark subtree and must not mutate shared root manifests while Specs 02 and 03 run in parallel.
- Root npm/Cargo manifests, lockfiles, global tokens, shared types, test configuration, and canonical context/tracker remain unchanged throughout Wave 2 feature work; any required edit is serialized by the integration owner. Spec 03 is the sole Wave 2 writer for the Tauri library entrypoint, build-time application-command manifest, application permission file, and `main` capability, and may make only the exact reviewed command/event permission changes declared in its spec. Specs 02 and 05 consume every one of those shared native files unchanged.

## 6. Contracts Consumed and Produced

### Contracts consumed

- Product scope and privacy invariants from `project-overview.md`.
- Process boundaries and filesystem layout from `architecture.md`.
- Dark visual tokens and accessibility rules from `ui-context.md`.
- strict TypeScript, Rust error-handling, dependency, testing, and performance rules from `code-standards.md`.
- spec-driven delivery, worktree, merge, and evidence rules from `ai-workflow-rules.md` and `spec-plan.md`.

### Contracts produced

1. **Canonical repository contract**
   - Path: `/Users/berat/mistaken`
   - Default branch: `main`
   - One clean baseline commit; its final SHA is reported in the implementation handoff and becomes the explicit `Base SHA` recorded by each Wave 2 successor before that successor changes code. The committed Spec 01 evidence must not attempt to contain its own self-referential commit hash.

2. **Toolchain contract**
   - npm is the sole JavaScript package manager.
   - The selected Node major is committed in one version file and must support the selected Vite/Tauri toolchain.
   - Rust is installed through official `rustup` with `rustfmt` and `clippy`; `rust-toolchain.toml` pins the exact stable toolchain resolved during implementation.
   - `package-lock.json` and `src-tauri/Cargo.lock` are committed. JavaScript direct dependencies use exact versions; template range drift is normalized after scaffolding.
   - Do not force an independently newer TypeScript/React/Vite version over the official compatible template set. Start from the current official stable React TypeScript template, remove unused packages, resolve once, and lock the verified set.
   - The baseline direct dependency set includes the core `@tauri-apps/api` package, the official Tauri clipboard-manager JavaScript/Rust plugin, `lucide-react`, and the Vitest + jsdom + Testing Library packages required by the reviewed Specs 02–03 contracts. The Rust baseline includes Serde derive support required by Tauri’s typed DTO/error boundary. Do not add shadcn/Radix, a router, a state/schema library, or another clipboard package.
   - The clipboard plugin is registered once in the Rust entrypoint. Its capability grants `clipboard-manager:allow-write-text` only; clipboard read, image, HTML, and clear permissions remain denied by omission.

3. **Command contract**
   - `npm run dev`: Vite development server.
   - `npm run typecheck`: strict TypeScript check without emitting application artifacts.
   - `npm run lint`: the single configured frontend linter; use the official template’s maintained Oxlint convention unless the installed stable template has changed and the replacement is documented.
   - `npm run build`: typecheck plus production Vite build.
   - `npm run tauri dev`: real desktop development launch.
   - `npm run tauri build -- --debug --no-bundle`: native/frontend integration build without claiming release packaging.
   - Rust commands run from `src-tauri`: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo check`.
   - `npm test`: run the real Vitest suite once in the configured jsdom environment; an empty/pass-with-no-tests script is forbidden.
   - The baseline includes one user-observable bootstrap-screen test. Spec 02 replaces that superseded test with durable transcript-domain and workspace behavior tests.

4. **Frontend bootstrap contract**
   - `src/main.tsx` renders exactly one application root.
   - `src/App.tsx` is a minimal, honest launch surface, not the transcript workspace.
   - `src/index.css` defines the shared semantic tokens and imports Tailwind once through the maintained Vite integration.
   - No network fetch, browser storage, Tauri invocation, global mutable state, or hidden background work runs at startup.
   - `src/types/transcript.ts` exports the frozen `TranscriptSource` and `TranscriptSegment` shapes from `code-standards.md`; `src/types/runtime.ts` exports the frozen `CaptureStatus` union. Both are complete Wave 2 contracts, not placeholder feature modules.

5. **Native bootstrap contract**
   - `main.rs` delegates to a library `run()` entrypoint so native logic remains testable as the project grows.
   - `lib.rs` builds the Tauri application and exposes no demo or product command in Spec 01.
   - The official clipboard-manager plugin is the only registered feature plugin and exposes plain-text write only. No demo command or product command exists in Spec 01.
   - Runtime startup may use Tauri’s terminal `.expect(...)` only for the process-fatal application run invariant; feature/runtime paths added later must use typed `Result` errors.

6. **Documentation contract**
   - Context files are copied to `docs/context` with their filenames preserved.
   - `spec-plan.md` becomes `docs/context/spec-plan.md`.
   - All reviewed staged specs become `docs/specs/<original-name>.md`.
   - The source staging bundle remains untouched.
   - After commit, the in-repository copies are canonical and future progress/spec evidence updates happen there.

## 7. User and Developer Flows

### User launch flow

1. The user starts the application through `npm run tauri dev` during development.
2. A single native window opens with title **Mistaken**.
3. The surface is dark and immediately identifies the product.
4. The shell reports **Desktop runtime ready** and states that audio capture and transcription are not configured yet.
5. No button implies recording, transcription, settings, copying, or clearing already works.
6. The user closes the window using the native window control.
7. The process exits cleanly. Relaunching produces the same deterministic screen.

### Developer bootstrap flow

1. Verify the target path is absent. If it exists or contains user data, stop rather than overwrite it.
2. Verify the source context bundle and all 15 reviewed specs exist.
3. Install the missing Rust stable toolchain using official `rustup`, including `rustfmt` and `clippy`; do not use a project-local fake toolchain.
4. Scaffold the current official Tauri 2 React TypeScript application into the exact target path with npm.
5. Initialize Git on `main` if the scaffold did not do so, then remove demo content and unused dependencies.
6. Add Tailwind through `@tailwindcss/vite` and the single CSS import; do not create a legacy Tailwind 3/PostCSS configuration beside it.
7. Apply the repository, security, UI, and tooling decisions in this spec.
8. Copy the reviewed context/spec bundle into the repository.
9. Install from the committed lockfile path, run the full verification map, and launch the real Tauri window.
10. Close and relaunch the app, inspect process/network/filesystem behavior, then update evidence.
11. Create one focused baseline commit and record branch and SHA.

### Future-agent entry flow

1. Read root `AGENTS.md`.
2. Read the six context files in their required order, then `spec-plan.md`, current Git status, the requested spec, and relevant code/tests/manifests.
3. Respect one-writer ownership and the worktree protocol.
4. Implement only the requested vertical spec and record evidence before merging.

## 8. UI States, Tokens, and Accessibility

### Required bootstrap state

Only one state exists in Spec 01: **desktop foundation ready, product features not configured**.

Required visible copy:

- Product label: `Mistaken`
- Primary status: `Desktop runtime ready`
- Supporting truth: `Audio capture and transcription are not configured yet.`

The wording may receive punctuation or capitalization corrections during implementation, but it must not imply that transcription works.

### Layout

- Full-window dark application surface.
- Compact product header at the top.
- A quiet central status region; no fake transcript cards, waveform, source toggles, chat bubbles, animated loader, or action toolbar.
- Initial window: `1040 × 720` logical pixels.
- Minimum window: `720 × 520` logical pixels.
- Resizable and centered; title `Mistaken`; window label `main`.
- The surface must remain readable at the minimum size without horizontal overflow.

### Semantic token foundation

Define and use CSS custom properties matching `ui-context.md`:

- `--bg-base: #0B0D10`
- `--bg-surface: #111419`
- `--bg-elevated: #171B21`
- `--text-primary: #F3F4F6`
- `--text-secondary: #C2C7D0`
- `--text-muted: #7C8491`
- `--accent-primary: #8BDF9B`
- `--accent-hover: #A3E8AF`
- `--border-default: #252A32`
- `--border-strong: #363D48`
- `--state-error: #F07178`
- `--state-warning: #E8B86D`
- `--state-success: #8BDF9B`
- `--text-interim: #8E96A3`
- `--selection: rgba(139, 223, 155, 0.20)`

Use the system sans stack for interface and transcript text, and define the project monospace token only for future technical values. The bootstrap screen itself must not invent a competing palette, font family, gradient, glow, glass effect, or large-card hierarchy.

### Accessibility

- Use semantic `header`, `main`, heading, and paragraph elements.
- Product/status content must be available to screen readers in DOM order without ARIA duplication.
- Normal text and status text must meet WCAG AA contrast against their actual backgrounds.
- There are no custom interactive controls in this spec, so no decorative element may receive tab focus.
- Respect reduced-motion preferences; the default implementation should use no animation.
- Browser zoom/text scaling must not clip the required truth/status copy at the minimum window size.

## 9. Frontend → IPC → Rust / Audio / ASR Flow

Spec 01 deliberately establishes only the process shell:

```text
Native process starts
  → Tauri creates window "main"
  → Vite-built local assets load
  → React mounts the static bootstrap surface
  → user closes window
  → native process exits
```

There is no frontend invocation, custom command, emitted event, audio engine, recognizer, worker, model registry, or persistence boundary in this spec.

Required negative design decisions:

- Remove the scaffolded `greet` command, `invoke("greet")`, opener plugin, external link, logos, form, and related CSS.
- Do not invent placeholder command/event names; Spec 03 owns the first typed IPC contract.
- Do not create empty `audio`, `asr`, `commands`, `state`, `features`, `lib/tauri`, or other future modules. The two complete shared TypeScript contract files explicitly assigned to this baseline are the only exception.
- Do not register async tasks, timers, listeners, streams, or workers at startup.

## 10. Platform, Permissions, Offline, Privacy, and Fallback

### Current implementation host

- macOS Apple Silicon is the required Spec 01 smoke-test host.
- Xcode Command Line Tools are already available through the installed Xcode toolchain.
- Rust is missing and must be installed before native verification can pass.

### Windows contract

- The committed source and configuration must remain compatible with a future Windows build.
- Do not add macOS-only frontend branches or hard-coded Unix paths.
- Microsoft C++ Build Tools and WebView2 are documented prerequisites for the later Windows validation environment; their installation is not part of this macOS bootstrap.

### Tauri identity and window policy

- `productName`: `Mistaken`
- `identifier`: `com.mistaken.desktop`
- application version: `0.1.0`
- one local `main` window using the size policy in Section 8
- no remote window URL, additional webview, deep link, tray, global shortcut, menu command, updater, or autostart behavior

### Capability and CSP policy

- Create one capability for window `main`.
- Its only feature permission is `clipboard-manager:allow-write-text`, scoped to `main`, because reviewed Spec 02 must copy plain text without colliding with Spec 03’s Wave 2 native ownership. Clipboard read, image, HTML, and clear permissions plus opener, filesystem, shell, process, dialog, HTTP, notification, and broad plugin defaults are forbidden. If the exact generated Tauri 2 schema requires a core permission solely to render/close the window, list that permission individually and record the evidence.
- Explicitly reference only that capability in Tauri configuration rather than implicitly loading unrelated files.
- Configure a non-null Content Security Policy restricted to bundled/self assets and only the schemes Tauri itself requires. No remote `http:`/`https:` origin, wildcard source, remote font, remote image, or analytics endpoint is allowed.
- Development-only Vite/HMR allowances must not weaken the production CSP.

### Offline and privacy behavior

- The running app must not require a network connection after dependencies are installed.
- Startup must issue no fetch/XHR/WebSocket/event-source request and load no remote asset.
- It must create no transcript, database, cache, model, recording, or telemetry file.
- It must request no microphone, screen/audio-capture, filesystem, accessibility, or notification permission.

### Fallback

If the target path exists, the Rust toolchain cannot be installed, the selected stable scaffold is internally incompatible, or the native window cannot launch, stop with the exact failing command and preserve the source staging bundle. Do not silently fall back to a browser-only app, older Tauri major, Electron, mocked native layer, or weakened checks.

## 11. Resource Lifecycle, Errors, and Recovery

- Spec 01 allocates no audio buffers, worker threads, model resources, database handles, file watchers beyond the development tooling, or persistent app state.
- The only long-lived application resources are the native process, its main webview, and the Vite development process started by Tauri during development.
- Closing the only window must terminate the native app and the child development process started for that run.
- A second `npm run tauri dev` after clean shutdown must bind its development port and reopen normally.
- Startup failures must be printed to the invoking terminal with a non-zero exit; they must not be converted into an apparently successful blank window.
- Frontend rendering failure must remain visible in development diagnostics; no empty catch/fallback should suppress it.
- Template `expect` is permitted only at the terminal Tauri `run()` boundary because failure there is process-fatal. No runtime product path exists yet.
- Rollback is deletion of the newly created target repository only after verifying it contains no post-bootstrap user work; the preserved staging bundle remains the recovery source.

## 12. Acceptance Criteria

1. **Repository safety:** Applying the spec creates `/Users/berat/mistaken` as a new Git repository on `main` and never overwrites a pre-existing target or modifies `/Users/berat/mistaken-context`.
2. **Canonical docs:** The repository contains the six context files plus `spec-plan.md` under `docs/context` and all 15 reviewed specs under `docs/specs`, with source filenames and content preserved except for explicit implementation-evidence updates made after import.
3. **Agent entrypoint:** Root `AGENTS.md` defines the required context read order, spec creation/application commands, one-writer/worktree rules, verification obligations, and the ban on BMAD/cloud/API-key behavior.
4. **Supported stack:** `package.json`, `package-lock.json`, `Cargo.toml`, and `Cargo.lock` resolve a compatible Tauri 2 + `@tauri-apps/api` + React + strict TypeScript + Vite + Tailwind Vite-plugin stack using npm, with Serde derive support in Rust. The reviewed Wave 2 foundation includes only the official clipboard-write plugin, Lucide, and Vitest/jsdom/Testing Library beyond that core; no opener, router, state/schema library, backend, database, telemetry, audio, or ASR dependency exists.
5. **Pinned toolchains:** The committed Node-version file and exact `rust-toolchain.toml` reproduce the verified toolchains; `rustfmt` and `clippy` are available.
6. **Clean scaffold:** No generated greet command, invoke call, opener plugin/permission, external link, template logo, sample form, lorem/demo copy, placeholder feature module, or stale scaffold styling remains.
7. **Window identity:** Tauri configuration has product name/title `Mistaken`, identifier `com.mistaken.desktop`, version `0.1.0`, a single centered/resizable `main` window initially `1040 × 720`, and minimum `720 × 520`.
8. **Least privilege:** The only active capability targets `main` and grants only `clipboard-manager:allow-write-text` plus any individually evidenced core permission required to render/close the window; clipboard read/image/HTML/clear, filesystem, shell, process, opener, dialog, HTTP, notification, remote-URL, and wildcard permissions are absent. Production CSP is non-null and has no remote origin.
9. **Honest visible state:** The native window visibly contains `Mistaken`, `Desktop runtime ready`, and `Audio capture and transcription are not configured yet.` It exposes no working-looking product control and remains readable at the minimum window size.
10. **Design foundation:** The approved semantic tokens are defined once and used by the bootstrap surface; there is no second palette, hard-coded competing color in component markup, gradient, glow, glass treatment, or non-system interface font.
11. **Accessible surface:** Semantic document structure is correct, required copy meets WCAG AA contrast, no decorative element is focusable, reduced motion yields no animation, and text scaling does not hide required content.
12. **Frontend quality:** `npm test`, `npm run typecheck`, `npm run lint`, and `npm run build` exit successfully from a clean `npm ci` install; the test command exercises the observable bootstrap screen rather than passing with no tests.
13. **Rust quality:** From `src-tauri`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo check` exit successfully.
14. **Native integration:** `npm run tauri build -- --debug --no-bundle` succeeds, and `npm run tauri dev` opens the actual native window; close followed by relaunch succeeds without a stale helper process or occupied dev port.
15. **Offline/privacy baseline:** During the native launch smoke test, the application issues no runtime network request, prompts for no OS permission, and creates no database, model, transcript, recording, telemetry, or other application data file.
16. **Parallel baseline:** After evidence updates, one focused local commit leaves Git status clean. Its final `main` SHA is reported in the implementation handoff, and clean worktrees for Specs 02, 03, and 05 can be created from that exact commit without dependency installation changing a lockfile.

## 13. Acceptance Criteria → Verification Map

| AC | Verification | Evidence to record |
|---|---|---|
| 1 | Preflight path checks; compare source bundle file checksums before and after bootstrap; inspect Git root and branch | Target path decision, source checksum result, `git rev-parse --show-toplevel`, branch |
| 2 | Enumerate expected context/spec filenames and compare each copied file to its staging source before evidence edits | Filename checklist and any intentional post-copy diff |
| 3 | Read root `AGENTS.md` against the required workflow checklist | Checklist result |
| 4 | `npm ci`; inspect direct dependency list and clipboard plugin registration; `cargo metadata --no-deps` | Resolved direct versions, approved Wave 2 foundation packages, and removed template packages |
| 5 | `node --version`, `npm --version`, `rustc --version`, `cargo --version`, `rustup component list --installed` | Exact toolchain versions |
| 6 | Search the application source/manifests for scaffold identifiers and inspect the rendered UI | Zero-match report for greet/opener/template content |
| 7 | Validate `tauri.conf.json` against the generated schema and inspect the live native window | Config values and observed dimensions/title |
| 8 | Inspect the effective Tauri capability set and production configuration; verify write-text is the sole feature permission and no remote source is permitted | Capability permission list and CSP value |
| 9 | Real `npm run tauri dev` visual smoke test at initial and minimum window sizes | Screenshot or equivalent captured visual evidence plus observations |
| 10 | Inspect computed styles/token declarations in the running surface | Token list and representative computed values |
| 11 | Keyboard traversal, screen-reader tree/semantic inspection, contrast calculation, reduced-motion, and increased text-size smoke | Results for each accessibility check |
| 12 | `npm test`; `npm run typecheck`; `npm run lint`; `npm run build` after `npm ci` | Commands, exercised bootstrap-screen assertion, and exit status |
| 13 | `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test`; `cargo check` | Commands and exit status |
| 14 | `npm run tauri build -- --debug --no-bundle`; launch-close-relaunch `npm run tauri dev`; inspect the process tree/port between runs | Build result and two-run lifecycle observations |
| 15 | Run once with network unavailable or blocked; inspect runtime requests, OS permission prompts, and newly created app-data files | Network/permission/filesystem observations and inspected paths |
| 16 | Re-run `npm ci` without lockfile change; create and remove three temporary clean worktrees from the baseline SHA; inspect final Git status | Baseline branch/SHA, worktree results, clean status |

The bootstrap test defends an observable truthfulness contract, not source wiring. Spec 02 must delete or replace it when the bootstrap screen is intentionally superseded; it must not re-pin obsolete copy.

## 14. Implementation Order

1. Re-read all source context documents and reviewed Specs 01–15; reconcile any later cross-spec ownership change before touching the target path.
2. Capture source-bundle checksums and verify `/Users/berat/mistaken` is absent.
3. Install official stable Rust through `rustup` with the minimal profile plus `rustfmt` and `clippy`; record the exact version.
4. Use the current official `create-tauri-app` Tauri 2 React TypeScript npm template to create the exact target path.
5. Initialize/normalize Git on `main`; add the Node and Rust toolchain pins and ignore generated output.
6. Remove template demo UI, `greet` command/invocation, opener plugin/API usage, opener capability, template logos from the launch surface, and all now-unused dependencies/assets.
7. Add the maintained Tailwind Vite integration, shared semantic token foundation, frozen transcript/runtime TypeScript types, Lucide, and the Vitest/jsdom/Testing Library harness with one behavior-level bootstrap test.
8. Register the official clipboard-manager plugin, grant write-text only, and implement the minimal native window configuration, production CSP, Rust entrypoint, and honest bootstrap surface.
9. Add the root `AGENTS.md`; replace the generic generated README with concise Mistaken prerequisites, local commands, offline/privacy baseline, and links to canonical context/specs.
10. Copy reviewed context and spec files into `docs/context` and `docs/specs`, preserving the staging source.
11. Run clean installs and all static/build checks; fix source rather than suppressing diagnostics.
12. Launch the real native app, inspect initial/minimum-size UI, accessibility, network/permission/filesystem behavior, then close and relaunch.
13. Run the no-bundle native build and the temporary worktree/lockfile reproducibility checks.
14. Review the complete repository for correctness, security, accessibility, privacy, stale scaffold content, generated artifacts, and scope leakage; fix every High/Medium finding.
15. Update the in-repository `docs/context/progress-tracker.md` and this spec with all pre-commit evidence. Do not place a fake, placeholder, or self-referential final commit SHA in committed content.
16. Create one focused local baseline commit, run `git rev-parse HEAD`, verify clean status, and report that SHA in the implementation handoff. Use that exact SHA when creating successor worktrees and as the `Base SHA` each successor records on its own branch. Do not push.

## 15. Risks, Rollback, Cleanup, and Preservation

### Risks and mitigations

- **Target-path collision:** A newly appearing `/Users/berat/mistaken` could contain user work. Mitigation: refuse overwrite; do not use force flags or recursive cleanup.
- **Toolchain drift:** Unbounded `latest` ranges could make parallel worktrees differ. Mitigation: begin from the official compatible stable template, commit exact JavaScript dependency versions plus both lockfiles, and pin Node/Rust toolchains.
- **Rust bootstrap failure:** Rust is currently absent. Mitigation: install only through official `rustup`; if installation or native prerequisites fail, report the exact blocker and do not claim a browser-only substitute is complete.
- **Template privilege leakage:** The current template includes opener behavior and a demo command. Mitigation: remove both packages/registrations/permissions and verify zero remnants.
- **False product readiness:** A polished placeholder can imply transcription works. Mitigation: fixed honest copy, no feature controls, no synthetic transcript.
- **Premature architecture:** Empty future modules would freeze the wrong boundaries. Mitigation: create only code executed or configured by this spec; later specs own their modules.
- **Docs divergence:** Staging and repository copies could both be edited. Mitigation: staging stays immutable through bootstrap; after the baseline commit, the repository copy is canonical and the tracker records the transition.
- **Platform overclaim:** A successful macOS launch does not prove Windows audio behavior. Mitigation: this spec claims only cross-platform-compatible bootstrap source and a macOS native smoke; Windows behavioral validation belongs to later platform specs.
- **Release-scope leakage:** A normal `tauri build` can create bundles/installers and invite premature signing work. Mitigation: use `--no-bundle`; Specs 13–14 own packaging.

### Rollback

Before the baseline is handed to successors, rollback is:

1. Confirm the target contains only Spec 01 output and no user work.
2. Preserve the verification log outside the target if failure analysis is needed.
3. Remove the target directory.
4. Leave `/Users/berat/mistaken-context` unchanged.
5. Re-run from the reviewed spec after correcting the identified prerequisite.

After successors begin, rollback is Git-based from the recorded baseline; do not delete the canonical repository.

### Cleanup required before completion

- Remove generated sample UI/assets and all unused direct dependencies.
- Remove build output, screenshots, temporary network logs, temporary app-data probes, and temporary worktrees unless an evidence path is intentionally documented outside Git.
- Keep only source, lockfiles, canonical docs/specs, product-appropriate bootstrap assets, and configuration.
- Update tracker/spec evidence; do not leave TODOs, placeholders, stubs, or an uncommitted diff.

### Preservation rules

- Preserve all staging documents and unrelated user files.
- Preserve the project’s offline-only, no-account, no-database, in-memory transcript, dual-source audio, no-grammar-rewrite, and bounded-memory invariants.
- Preserve the exact baseline SHA as the parent for Wave 2 branches/worktrees.

## 16. Definition of Done and Evidence Record

This spec is done only when all acceptance criteria pass and the actual native app has been launched, closed, and relaunched from `/Users/berat/mistaken`.

Record all evidence knowable before the baseline commit in this section after implementation:

- Implementation date
- Repository path
- Branch
- Planned baseline commit subject
- Node/npm versions
- Rust/Cargo versions and pinned toolchain
- Exact direct JavaScript and Rust framework versions
- `npm ci` result
- `npm run typecheck` result
- `npm run lint` result
- `npm run build` result
- `cargo fmt --check` result
- `cargo clippy --all-targets --all-features -- -D warnings` result
- `cargo test` result
- `cargo check` result
- `npm run tauri build -- --debug --no-bundle` result
- Native initial-size and minimum-size visual observations/evidence path
- Launch-close-relaunch result
- Offline/network, OS-permission, and app-data filesystem observations
- Source-bundle preservation result
- Lockfile reproducibility result
- Temporary Specs 02/03/05 worktree creation result
- Pre-commit scope/status inspection
- High/Medium review findings and resolutions

The post-commit implementation handoff must additionally report the final baseline commit SHA, `git status --short` result, and the SHA used to create the temporary Spec 02/03/05 worktrees. Those values are intentionally not written back into the baseline commit itself.

### Implementation evidence

- **Implementation date:** 2026-09-11.
- **Repository path:** `/Users/berat/mistaken`.
- **Branch:** `main`.
- **Planned baseline commit subject:** `Spec 01: bootstrap Tauri 2 desktop repository baseline`.
- **Node/npm versions:** Node `v24.15.0`, npm `11.12.1` (pinned in `.nvmrc`).
- **Rust/Cargo versions and pinned toolchain:** `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1 (797e8a9bc 2026-08-05)`, installed via official `rustup` (minimal profile) with `rustfmt` and `clippy` components; pinned exactly in `rust-toolchain.toml` (`channel = "1.98.1"`).
- **Exact direct JavaScript framework versions:** `react 19.3.0`, `react-dom 19.3.0`, `@tauri-apps/api 2.11.1`, `@tauri-apps/plugin-clipboard-manager 2.3.3`, `lucide-react 1.45.0`, `@types/react 19.3.0`, `@types/react-dom 19.3.0`, `@vitejs/plugin-react 6.1.1`, `@tailwindcss/vite 4.3.3`, `tailwindcss 4.3.3`, `typescript 6.0.3`, `vite 8.3.0`, `vitest 5.0.0`, `jsdom 30.0.1`, `@testing-library/react 16.3.3`, `@testing-library/jest-dom 7.0.1`, `oxlint 1.82.0`, `@tauri-apps/cli 2.11.4`. All direct dependencies are pinned to exact versions (no `^`/`~`) in `package.json`, resolved once and locked in `package-lock.json`.
- **Exact direct Rust framework versions (from `Cargo.lock`):** `tauri 2.11.5`, `tauri-build 2.6.3`, `tauri-plugin-clipboard-manager 2.3.3`, `serde 1.0.229`, `serde_json` (latest `1.x` resolved). No opener, HTTP, filesystem, shell, dialog, or process plugin is present.
- **`npm ci` result:** Pass. Clean install from the committed lockfile after `rm -rf node_modules`; 128 packages added; no lockfile diff.
- **`npm run typecheck` result:** Pass (`tsc --noEmit`, strict mode, zero errors).
- **`npm run lint` result:** Pass (`oxlint .`, zero warnings/errors).
- **`npm run build` result:** Pass. Typecheck plus `vite build` produced `dist/index.html`, `dist/assets/index-*.css` (11.53 kB), `dist/assets/index-*.js` (220.77 kB).
- **`cargo fmt --check` result:** Pass.
- **`cargo clippy --all-targets --all-features -- -D warnings` result:** Pass, zero warnings.
- **`cargo test` result:** Pass, `0 passed; 0 failed` (no Rust unit tests exist in the Spec 01 baseline scope; the command itself exits successfully).
- **`cargo check` result:** Pass.
- **`npm run tauri build -- --debug --no-bundle` result:** Pass. Produced `src-tauri/target/debug/mistaken` (unoptimized debug binary; no bundle/installer artifact created, consistent with `--no-bundle`).
- **Native initial-size and minimum-size visual observations:** `npm run tauri dev` opened a real native macOS window (verified via `CGWindowListCopyWindowInfo` and a window-scoped `screencapture -l<id>` capture, not a full-screen capture, to avoid capturing unrelated desktop content) titled `Mistaken` at exactly `1040×720` logical pixels, centered on the primary display. The rendered surface showed only the header `Mistaken`, the status `Desktop runtime ready`, and the supporting line `Audio capture and transcription are not configured yet.`, with no button, input, chat UI, or fake transcript. The window was then resized (via Accessibility API) to the configured minimum `720×520`; the same content remained fully readable with no horizontal overflow or clipping. Screenshots were captured to local temporary files (not committed; not part of repository evidence).
- **Launch-close-relaunch result:** Pass. The dev session was stopped (`SIGTERM` via the process supervisor); `lsof -i :1420` and process listing confirmed the Vite dev server, `cargo`-built native binary, and dev-port binding were all fully released with no stale process. A second `npm run tauri dev` immediately re-bound port 1420 and reopened the identical deterministic `1040×720` window with identical content.
- **Offline/network, OS-permission, and app-data filesystem observations:** During the run, the native `mistaken` process opened no TCP/UDP network connection at all (only a local Unix-domain IPC socket and a kernel network-reachability control socket, both non-networked). Its WebKit networking helper process held exactly one established TCP connection, `localhost -> localhost` to the local Vite dev server; zero non-loopback connections were observed. No OS permission prompt appeared (no capability requests filesystem/shell/dialog/HTTP/microphone/screen-capture access). No transcript, database, model, recording, or telemetry file was written by application code. WebKit's own engine created its standard local `NetworkCache`/`WebsiteData`/`ResourceLoadStatistics` directories under `~/Library/Caches/mistaken` and `~/Library/WebKit/mistaken` (LocalStorage/IndexedDB/SearchHistory subdirectories present but empty; NetworkCache contains only cached copies of the app's own bundled JS/CSS/HTML). This is inherent, unavoidable WKWebView engine behavior for any native macOS webview app (dev or production) and contains no transcript or product content; it is classified here rather than denied, consistent with the boundary Spec 12 later formalizes for the full acceptance gate.
- **Source-bundle preservation result:** Pass. SHA-256 checksums of every file under `/Users/berat/mistaken-context` were captured before scaffolding and re-verified identical after the full implementation; the staging bundle was never modified and is not a Git repository.
- **Lockfile reproducibility result:** Pass. After pinning exact dependency versions in `package.json`, `npm install` re-resolved `package-lock.json` once; a subsequent `rm -rf node_modules && npm ci` produced an identical install with no lockfile change.
- **Temporary Specs 02/03/05 worktree creation result:** Per Section 16, this value requires the baseline commit SHA and is intentionally not written back into the baseline commit; it is reported in the post-commit implementation handoff together with the final SHA and `git status --short`.
- **Pre-commit scope/status inspection:** `git add -A --dry-run` was reviewed line by line: exactly 68 paths match the Section 5 ownership list (root config/docs/source plus `docs/context/**`, `docs/specs/**`, `src/**`, `src-tauri/**` excluding `target`/`gen`); `node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`, and `.DS_Store` are confirmed ignored and untracked.
- **High/Medium review findings and resolutions:** One capability finding: the generated Tauri 2 schema's `main` capability requires `core:default` (not a narrower single core permission) for the window to reliably render/close under the official schema; this was evidenced by a clean, error-free `tauri dev` session with the capability limited to `core:default` plus `clipboard-manager:allow-write-text` only, and is recorded here as the evidenced exception permitted by Section 10. No other High/Medium finding was identified.

## Official References Verified During Authoring

- [Create a Tauri project](https://v2.tauri.app/start/create-project/)
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri configuration reference](https://v2.tauri.app/reference/config/)
- [Tauri CLI `dev` and `build` reference](https://v2.tauri.app/reference/cli/)
- [Calling Rust from the frontend](https://v2.tauri.app/develop/calling-rust/)
- [Tailwind CSS Vite installation](https://tailwindcss.com/docs/installation/using-vite)
- [Current official `create-tauri-app` React TypeScript template](https://github.com/tauri-apps/create-tauri-app/tree/dev/templates/template-react-ts)
- [Tauri Clipboard plugin and permissions](https://v2.tauri.app/plugin/clipboard/)
- [Vitest test environments](https://vitest.dev/guide/environment.html)
- [Testing Library query priority](https://testing-library.com/docs/queries/about/)
