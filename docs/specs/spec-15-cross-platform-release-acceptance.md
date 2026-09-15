# Spec 15 - Cross-Platform Release Acceptance

## 1. Status, Ownership, Base, and Gates

- **Status:** Authored; **HARD BLOCKED** until Spec 05 approves a production model, Spec 12 returns production `PASS`, and Specs 13/14 produce eligible artifacts. Not implemented.
- **Implementation owner:** One Spec 15 branch with exactly one writer, who is also the repository integration owner. This is the only spec that may fix a defect anywhere in the application and must never run concurrently with another writer. High-capability implementation and independent high-capability review are mandatory.
- **Required base:** One clean integration SHA containing implemented, reviewed, merged, and evidenced Specs 01–14. Specs 13/14 must share the same production-approved post-Spec-12 base. Development-only evidence, no-authorization records, unmerged work, or different packaging bases do not satisfy this dependency.
- **Allowed implementation predecessors:** Specs 13 and 14. Everything else is inherited transitively through their production-approved Spec 12 base.
- **Parallel safety:** None. No other spec, worktree, or writer may touch the repository while acceptance runs, because every measurement binds one immutable source SHA and two immutable artifacts. Any landing branch invalidates artifact-dependent criteria and requires new artifacts/evidence.
- **Production ASR gate:** `RELEASE-READY` requires exactly one Spec 05 `ProductionApproved` candidate passing every unchanged quality, performance, license, provenance, redistribution, and platform gate; Spec 12 and both artifacts must carry its exact approval identity. `DevelopmentOnly`, a blocked approval, temporary files, or `NON-RELEASE EVIDENCE` forces hard `BLOCKED` preflight and can never be waived by downstream success.
- **Host gate:** Both a qualifying macOS host (Apple Silicon, macOS 13.x floor plus current development host) and qualifying Windows hosts (x64 Windows 11 plus Windows 10 22H2 floor) are required. No platform substitutes for another.
- **Credential gate:** A `Developer ID Application` identity with Apple notarization and an Authenticode identity with RFC 3161 timestamping are external prerequisites. Unsigned, ad-hoc, or notary-blocked artifacts are local evidence only and never release-ready.
- **Publication gate:** A distribution destination and credentials are external inputs. Release assets are produced only after candidate eligibility; upload occurs only with an explicit user-supplied destination. Otherwise publication is exactly `BLOCKED`; guessed/personal/silent upload is prohibited.
- **Successor gate:** None. Spec 15 is terminal and the only point at which Mistaken may be called release-ready.
- **Review level:** High implementation and review. This spec owns the public claim that two signed artifacts use production-approved local ASR, behave identically, run locally, and preserve spoken English without application correction.

## 2. Goal and User-Visible / Measurable Result

Prove that one production-approved Mistaken release candidate — one source revision and two signed platform artifacts — is coherent, then either declare it release-ready with evidence or refuse to.

The visible result:

- One release SHA, one version `0.1.0`, and exactly two distributable artifacts: `Mistaken_0.1.0_aarch64.dmg` (macOS, Apple Silicon) and `Mistaken_0.1.0_x64-setup.exe` (Windows, x64).
- Both artifacts carry `ProductionApproved` maturity, the same Spec 05 approval digest, model id/files/digests, runtime/configuration identity, delivery manifest, and notices; a temporary adapter or unexplained payload difference is a hard blocker.
- A user on either platform installs from a real download path on a clean account, launches with the network disconnected, speaks into the microphone, plays system audio, and sees a live dual-source transcript where their own speech renders as plain lines and computer speech renders with a `- ` prefix.
- The same scripted conversation produces the same transcript structure on both platforms: same speaker prefixes, same first-seen final ordering rules, same verbatim preservation of incorrect English, same final-only `Copy All` output shape.
- Neither installed application makes a non-loopback network attempt, writes a transcript or audio byte to disk, or asks for a permission unrelated to microphone and (on macOS) screen recording.
- `release/manifest.json`, `release/CHECKSUMS.txt`, `release/RELEASE_NOTES.md`, `release/ROLLBACK.md`, and `release/evidence/**` record the candidate identity, both per-host results, every gate value against its threshold, and a single closed verdict: `RELEASE-READY`, `NOT-RELEASE-READY`, or `BLOCKED`.
- The repository's own documentation matches the shipped product: every spec's evidence record is filled from real runs, and the context files and tracker describe what actually shipped.

Measurable result: one verdict per host and one cross-host verdict, each computed from committed, redacted, schema-validated evidence in which every applicable criterion in section 12 has a recorded observed value, a threshold, a run id, a host id, and the release SHA.

## 3. Verified Current Behavior

Verified while authoring this spec:

- `/Users/berat/mistaken` does not exist. `/Users/berat/mistaken-context` is a documentation-only staging bundle containing the six context documents, `spec-plan.md`, and Specs 01-15. There is no application repository, Git history, build, artifact, signature, release tree, or acceptance result to inspect yet. Every field in section 16 is therefore `Pending`.
- The staging bundle is mirrored into iCloud Drive and has no version control, which already caused a mid-session working-copy divergence. This spec's reconciliation step exists partly because documentation truth must end up in the canonical repository rather than in a synced folder.
- `spec-plan.md` assigns Spec 15 the inventory row "The same release candidate passes the full macOS and Windows user flow, source-separation corpus, offline/privacy checks, artifact manifest, and rollback checklist", ownership of "Release evidence, final cross-platform integration fixes, context/tracker reconciliation", dependencies `13, 14`, and review level `High`. Wave 9 contains one terminal running Spec 15 alone, and the plan states that Spec 15 "does not accept platform-specific success claims without the recorded commands, artifact checksums, hardware/OS identity, and exercised user flow" and that it "is the only point at which Mistaken can be called release-ready".
- `spec-plan.md` lists "Distribution/signing identities" as a product decision whose latest resolution point is "Before signed completion of Specs 13/14"; it does not name a publication destination, release host, download page, or update channel anywhere. There is consequently no repository-sanctioned place to upload an artifact, and inventing one would be a silent product decision.
- `spec-plan.md` requires a high-model review "During Spec 15: full release acceptance and context reconciliation", and its merge rules require that cross-platform claims come from the named platform.
- Spec 12 produced the reusable acceptance machinery this spec builds on: `acceptance/README.md`, `acceptance/scenarios/**`, `acceptance/scripts/macos/**`, `acceptance/scripts/windows/**`, a closed `acceptance/evidence/schema.json` with `<host-profile>.json`, `summary.md`, `artifacts.sha256`, and ignored `acceptance/runs/**`. Its schema is explicitly closed - unknown top-level fields fail validation - so this spec adds a separate release evidence schema instead of mutating a frozen one.
- Spec 12 froze the release inputs consumed here: version `0.1.0` across `package.json`, `src-tauri/Cargo.toml`, and `tauri.conf.json`; exactly one Spec 05-approved candidate with runtime tag, model id, per-file relative path/byte size/SHA-256, decoding method, provider, and thread count; the single resource mapping `$RESOURCE/resources/models/<model_id>/`; `src-tauri/resources/licenses/THIRD_PARTY_NOTICES.txt`; and every committed lockfile. It also requires a cross-host `PASS` before packaging began.
- Spec 12 froze the numeric gates that remain unchanged here: MPR >= 0.90, false-correction rate <= 0.05, `mistake-tense` and `mistake-minimal-pair` MPR >= 0.85, full-corpus WER <= 0.25, fluent-control WER <= 0.12, zero non-empty final on physical silence, noise insertion <= 0.02 tokens/s, median first partial <= 900 ms, p95 final-after-endpoint <= 1500 ms, RTF <= 0.6 single and <= 0.9 dual, peak RSS <= 700 MB single and <= 1.4 GB dual, sustained CPU <= 60% of one core per active stream, RSS growth after minute 5 <= 5% with flat thread and handle counts.
- Spec 12 defined the three-scenario source-separation protocol reused here: microphone-only with at least 30 local corpus utterances played from a separate physical speaker, system-only with at least 30 utterances through the current default output endpoint, and dual-source with distinct scripted sets and at least 10 intentional overlaps, scored for source identity, ordering, formatting, MPR/false correction, and latency, with any attributed crossover, native `- `, rewritten text stage, or relaxed threshold failing.
- Spec 13 produces the macOS handoff: final Spec 13 commit SHA and exact Spec 12 base SHA; final `.app`/DMG name, byte size, SHA-256, normalized digest, local immutable location; mounted bundle manifest and approved-model verification result; target, minimum OS, effective `Info.plist` keys, embedded entitlements, linked dependencies, architecture, icon result; Developer ID class/Team ID, secure timestamp, notary submission id/status/log digest, stapler and Gatekeeper results or the exact blocker; real-host install/runtime/permission/lifecycle/offline results; complete toolchain and build command/environment record without credentials; High/Medium review disposition and cleanup state. Spec 13 explicitly excludes "Public upload, CDN/domain setup, download page, release notes, cross-platform checksum publication, final release tag, or release-ready declaration. Spec 15 owns them."
- Spec 14 produces the symmetric Windows handoff: final Spec 14 commit SHA and exact Spec 12 base SHA; setup executable name, byte size, SHA-256, local immutable location; installed payload manifest, approved-model verification, measured model/WebView2/total byte shares; target, supported floors, version resource metadata, linked dependencies, install mode, install path, registry/shortcut inventory, icon result; Authenticode certificate subject/thumbprint/expiry, digest and timestamp facts, `signtool verify` result, observed SmartScreen/Defender behavior or the exact blocker; real-host install/launch/offline-flow/permission/lifecycle/uninstall results for Windows 11 and, when available, Windows 10 22H2; toolchain, staging-digest, and build command/environment record without credentials; High/Medium review disposition and cleanup state. Spec 14 carries the same "Spec 15 owns them" exclusion list.
- Both packaging specs state that a SmartScreen reputation prompt (Windows) or a Gatekeeper first-launch dialog on a validly signed artifact is an observation for Spec 15, not grounds for disabling a protection.
- Both packaging specs state that if a signing certificate or artifact is compromised, "Spec 15 owns public rollback and release response", and that reverting a packaging commit before Spec 15 revokes that platform's handoff.
- Neither packaging spec ships an updater: Tauri Updater, Sparkle, update endpoints, update signing keys, differential artifacts, and background checks are excluded on both platforms. Any rollback plan authored here must therefore assume the user replaces the application manually.
- `architecture.md` invariants remain binding at release: no paid API/subscription/account/API key/backend/internet for core transcription, separate microphone and system sources, source-based speaker separation, no grammar correction or semantic cleanup, no silent cloud fallback, no transcript persistence, no audio upload, bounded buffers, non-blocking UI thread, platform capture behind one internal boundary, replaceable ASR runtime, verified model-weight license, and `microphone = normal text` / `system = "- " prefix` final formatting.
- `project-overview.md` success criteria remain the user-visible target: install and launch on supported macOS and Windows versions without an account or API key, run the core workflow with the network disconnected, capture both sources simultaneously without mixing, see stable finalized segments, stop and copy the whole conversation in one action, and paste into Obsidian with speaker formatting intact.
- `ui-context.md` visual and accessibility invariants remain binding: dark-only workspace language, transcript dominant, no chat bubbles, no avatars, no AI-gradient aesthetic, no account/cloud UI, system speech marked only by `- `, interim text visually distinguishable, actionable errors near the affected control, visible keyboard focus, status never by color alone, accessible button names, reduced-motion respect.
- `code-standards.md` testing priorities remain binding: product invariants over coverage, microphone segments without prefix, system segments with `- `, no interim duplication of final text, no grammar correction by application logic, `Copy All` preserving line ordering and speaker formatting, `Clear` leaving no in-memory transcript state, repeatable Start/Stop without leaking a capture session, microphone and system streams never swapped, and the network-disabled core workflow still working.
- `ai-workflow-rules.md` requires that the current unit works end to end within its scope, that no `architecture.md` invariant was violated, that no paid/cloud fallback was introduced, that `progress-tracker.md` reflects completed work, that TypeScript checks and `npm run build` pass, that `cargo check` passes for relevant Rust changes, that platform-specific checks pass when native code changed, that no transcript persistence was introduced, and that no recognized text is grammar-corrected or rewritten.

Nothing about release readiness is assumed from this authoring pass. Every value in section 16 comes from the real repository, the real signed artifacts, the real hosts, and the real installed flows.

## 4. Scope

### In scope

- Establishing one release candidate identity: one release SHA, version `0.1.0`, and exactly two artifacts rebuilt or verified as originating from that single SHA.
- Verifying that the Spec 13 and Spec 14 handoffs are mutually consistent: same Spec 12 base SHA, same frozen inputs, same approved model identity and digests, same notice digest, same version triple, and no divergent shared file.
- Reconciling the two packaging artifact manifests into one cross-platform release manifest with a closed schema, including the documented platform-specific difference set.
- Re-running the complete user flow on the **installed, distributed** artifacts on both platforms: clean-account install from a real download path, offline launch, microphone-only, system-only, and dual-source capture, Start -> Stop -> Start, Clear, Copy All, close/exit/signal shutdown, relaunch, and removal/uninstall.
- Re-running Spec 12's source-separation corpus protocol against the installed artifacts on both hosts and scoring source identity, ordering, formatting, fidelity, and latency against Spec 05's unchanged thresholds.
- Cross-platform parity verification: behavioral parity of states and controls, platform-correct shortcut mapping, transcript formatting parity, accessibility parity across VoiceOver and Narrator, privacy parity, lifecycle parity, and removal parity.
- One installed-artifact 60-minute dual-source soak per host on the final release artifacts.
- Final cross-platform integration fixes: root-causing and fixing any defect found here, then rebuilding both artifacts from the new release SHA and re-running every invalidated criterion on both hosts.
- Producing `release/**`: closed-schema release manifest, recomputed checksum file, release notes including known local limitations, rollback and release-response checklist, a manual publication checklist, a release evidence tree, and a redacted summary.
- Creating one annotated local tag `v0.1.0` on the release SHA, unpushed unless explicitly requested.
- Reconciling repository documentation with shipped reality: filling each spec's evidence record from real runs, correcting any context statement that no longer matches the product, and writing the final tracker state.
- Emitting one closed cross-host verdict with reasons, blockers, and open-finding counts.

### Out of scope

- Any new product feature, UI surface, command, event, setting, capability, permission, or platform target. Spec 15 fixes defects; it does not add behavior.
- Transcript persistence, transcript history, export formats, cloud sync, account, backend, database, remote API, telemetry, analytics, crash reporting, remote logging, or usage measurement of any kind.
- An updater, update endpoint, update signing key, differential artifact, background update check, or version-check request. A future version remains a manually installed artifact.
- Selecting/promoting a model, relaxing any Spec 05 gate, editing the corpus/scorer, substituting third-party numbers, or treating `NON-RELEASE EVIDENCE` as approval. Production approval is a read-only predecessor input.
- Intel/universal macOS artifacts, Windows ARM64 or 32-bit artifacts, Mac App Store, Microsoft Store, App Sandbox, MSIX, Homebrew, winget, Chocolatey, Sparkle, or a second installer per platform.
- Lowering a platform floor, adding an entitlement or hardened-runtime exception, adding a registry/startup entry, adding elevation, or bundling a new system dependency to make a gate pass.
- Marketing copy, landing page design, screenshots for stores, pricing, license-key infrastructure, or support tooling.
- Choosing a distribution host, domain, or account on the user's behalf, or uploading any artifact anywhere without an explicit user-supplied destination.
- Editing Spec 12's frozen `acceptance/evidence/schema.json`, thresholds, or scenario definitions to accommodate release-time measurements.
- Localizing the application UI, adding a second language, or changing user-visible copy except to correct a defect found by this spec's own criteria.

## 5. Owned Files and Forbidden Concurrent Files

### Primary owned paths

- `release/README.md` - one operator entry point: prerequisites, ordered run sequence, verdict semantics, evidence layout, cleanup, and publication gating.
- `release/schema.json` - the closed release evidence schema. It reuses Spec 12 field names where meanings are identical but is a separate file, because Spec 12's schema is frozen and rejects unknown top-level fields.
- `release/manifest.json` - the reconciled cross-platform release manifest validated against `release/schema.json`.
- `release/CHECKSUMS.txt` - SHA-256 lines for exactly the two distributable artifacts, recomputed from the immutable artifact copies.
- `release/RELEASE_NOTES.md` - version, platforms, floors, what the product does, known local limitations, permission requirements, privacy statement, and the explicit no-updater statement.
- `release/ROLLBACK.md` - withdrawal, certificate/identity compromise response, revert order, user-facing statement template, and rehearsal record.
- `release/PUBLICATION.md` - the manual publication checklist and the recorded destination decision or its exact blocker.
- `release/evidence/<host-profile>.json` - per-host release results, redacted.
- `release/evidence/summary.md` - rendered gate table: criterion, platform, threshold, observed value, verdict, run id, host id, release SHA.
- `release/evidence/parity.json` - the cross-platform parity comparison, including the documented allowed-difference set.
- `release/scripts/macos/**`, `release/scripts/windows/**` - installed-artifact preflight, install, offline-run, sampling, parity capture, soak, removal, and cleanup scripts. They invoke Spec 12's committed scenario definitions read-only.
- `release/runs/**` - ignored raw output: traces, process logs, screenshots, temporary inventories, local hypotheses.
- This spec's own section 16 evidence fields.

### Conditionally owned paths (defect fixes only, as serialized integration owner)

- Any application source, test, configuration, or documentation path required to fix a defect this spec's criteria detected. Each such edit must name the failed criterion, the root cause, and the regenerated evidence.
- `src-tauri/tauri.macos.conf.json`, `src-tauri/tauri.windows.conf.json`, `Info.plist`, `icon.icns`, `icon.ico`, `distribution/macos/**`, `distribution/windows/**` - only to fix a release-blocking packaging defect; both platform artifacts are then rebuilt from the new release SHA and both platforms' artifact-dependent criteria rerun.
- `docs/context/**`, `docs/specs/**` - reconciliation with shipped reality, including each spec's evidence record. Corrections are recorded with their reason; no historical decision is rewritten to look like it was always correct.
- Version strings remain `0.1.0` for this release. A defect fix never bumps the version; it produces a new release SHA and new artifact digests under the same version, and every superseded artifact is discarded rather than shipped.

### Forbidden concurrent files

- Everything. No other spec, branch, or writer may modify any repository path while Spec 15 runs. If a concurrent change lands, every criterion that depends on a build, artifact, signature, checksum, installed run, parity comparison, soak, or verdict is invalidated and rerun from a new release SHA.
- Spec 12's `acceptance/evidence/schema.json`, `acceptance/scenarios/**` thresholds, and `benchmarks/**` gate definitions, approval record, and license record are read-only inputs even for the integration owner. A measurement that cannot pass without editing one of them is a product failure.

## 6. Contracts Consumed and Produced

### Consumed

| Source | Frozen contract consumed |
| --- | --- |
| Spec 01 | Repository layout, tooling pins, npm/Cargo scripts, least-privilege capability and CSP baseline, write-only clipboard plugin, test harness |
| Spec 02 | Transcript reducer/formatter/serializer: stable partial/final semantics, immutable finals, first-seen final ordering, `- ` prefix only from `source === "system"`, final-only `Copy All`, in-memory `Clear` |
| Spec 03 | Four application commands, six main-window events, validated DTOs, monotonic revisions, structured error codes, capability boundary |
| Spec 04 | Real CPAL microphone capture, 100 x 20 ms reusable blocks (2 s), non-blocking drop-newest overflow |
| Spec 05 | Approved candidate identity, benchmark corpus/scorer/gates, license and provenance verdict, required attribution |
| Spec 06 | One approved local runtime/model/configuration, compiled-in manifest, first-load digest verification, 30 x 100 ms inference stage (3 s), no rewrite features, one rate per stream |
| Spec 07 | macOS ScreenCaptureKit adapter, macOS 13.0 floor, Screen Recording permission truth, audio-only capture |
| Spec 08 | Windows WASAPI loopback adapter, API floor 15063 with tested floor 19045/Windows 11, no permission gate, honest continuous timeline and counters |
| Spec 09 | Atomic requested-source start, isolated pipelines over one shared recognizer, structural attribution, emission-order aggregation, <= 10 s total queued audio |
| Spec 10 | Bounded per-source recovery (3 attempts, 500 ms/2 s/5 s), visible degradation, watchdogs, one idempotent shutdown, 60-minute soak gate |
| Spec 11 | `Cmd/Ctrl + Enter` and `Cmd/Ctrl + Shift + C` shortcuts, 64 px auto-follow with detached-scroll stability, named `role="log"` region with silenced interim subtrees, bounded frontend listeners |
| Spec 12 | Cross-host `PASS` report, closed evidence schema, scenario protocol, successor freeze manifest (version, model identity and digests, resource path, notice digest, root/shared manifests, lockfiles), and every numeric gate |
| Spec 13 | macOS artifact manifest: SHAs, bundle inventory, plist/entitlements, architecture/floor, Developer ID/notary/staple/Gatekeeper facts, installed-flow results, toolchain record, review disposition, or exact blocker |
| Spec 14 | Windows artifact manifest: SHAs, payload inventory, version resource, install mode/path/registry/shortcuts, WebView2 mode/version, Authenticode/timestamp/verify facts, SmartScreen observation, installed and uninstall results, staging digests, toolchain record, review disposition, or exact blocker |

### Predecessor consistency contract

Before any release work runs, the following must hold exactly; each failure has a named consequence:

1. Spec 13 and Spec 14 record the **same** post-Spec-12 base SHA. Different bases mean the two artifacts are not one product: both packaging branches are recreated from one reviewed SHA and both artifacts rebuilt.
2. Spec 12's cross-host report is production `PASS`, its successor freeze recomputes identically, and it records `ProductionApproved` maturity plus the exact Spec 05 approval digest. A development-only/no-authorization record stops release acceptance.
3. Both packaging manifests report `ProductionApproved`, the same Spec 05 approval digest, model id/files/digests, runtime/configuration identity, delivery manifest, and notice digest. Any difference or temporary-adapter artifact is a hard blocker.
4. Neither packaging spec left an unresolved High/Medium finding or reports a runnable product failure as `BLOCKED`.
5. Both packaging manifests point at artifacts that still exist at their recorded immutable locations with their recorded byte sizes and SHA-256 values. A missing or mutated artifact is rebuilt, not re-described.

### Produced - release identity

```jsonc
{
  "schemaVersion": 1,
  "product": { "name": "Mistaken", "version": "0.1.0" },
  "release": {
    "sha": "<release commit SHA>",
    "tag": "v0.1.0",
    "tagPushed": false,
    "canonicalRoot": "<repository root>",
    "branch": "<branch>",
    "cleanState": true,
    "spec12BaseSha": "<SHA>",
    "spec13Sha": "<SHA>",
    "spec14Sha": "<SHA>",
    "rebuiltAfterFix": false
  },
  "frozenInputs": {
    "asrMaturity": "production-approved",
    "spec05ApprovalDigest": "<64 hex>",
    "modelId": "<approved id>",
    "runtimeTag": "<tag>",
    "modelFileCount": 0,
    "modelFiles": [{ "relativePath": "<path>", "bytes": 0, "sha256": "<64 hex>" }],
    "delivery": { "mode": "bundled | separately-provisioned-local", "manifestDigest": "<64 hex>", "runtimePath": "<frozen path>" },
    "noticeDigest": "<64 hex>",
    "lockfileDigests": { "<path>": "<64 hex>" },
    "spec12FreezeManifestDigest": "<64 hex>"
  },
  "artifacts": [
    {
      "platform": "macos",
      "name": "Mistaken_0.1.0_aarch64.dmg",
      "bytes": 0,
      "sha256": "<64 hex>",
      "normalizedPayloadDigest": "<64 hex>",
      "target": "aarch64-apple-darwin",
      "minimumOs": "13.0",
      "signing": {
        "method": "developer-id",
        "teamId": "<id>",
        "certificateExpiry": "<date>",
        "hardenedRuntime": true,
        "secureTimestamp": true,
        "notarySubmissionId": "<id>",
        "notaryStatus": "Accepted",
        "notaryLogDigest": "<64 hex>",
        "stapled": true,
        "gatekeeper": "accepted"
      }
    },
    {
      "platform": "windows",
      "name": "Mistaken_0.1.0_x64-setup.exe",
      "bytes": 0,
      "sha256": "<64 hex>",
      "normalizedPayloadDigest": "<64 hex>",
      "target": "x86_64-pc-windows-msvc",
      "supportedFloor": "10.0.19045",
      "installMode": "currentUser",
      "webview2": { "mode": "offlineInstaller", "version": "<pv value>" },
      "signing": {
        "method": "authenticode",
        "digestAlgorithm": "sha256",
        "certificateSubject": "<subject>",
        "certificateThumbprint": "<thumbprint>",
        "certificateExpiry": "<date>",
        "timestamp": { "present": true, "authority": "<rfc3161 authority>", "time": "<utc>" },
        "signtoolVerify": "pass",
        "smartScreenObserved": "<observation>"
      }
    }
  ],
  "hosts": [
    {
      "hostId": "mac-arm64-floor | mac-arm64-dev | win-x64-11 | win-x64-22h2",
      "platform": "macos | windows",
      "os": "<edition/version/build>",
      "cpu": "<model/cores>",
      "ramGb": 0,
      "power": "AC",
      "verdict": "PASS | FAIL | BLOCKED"
    }
  ],
  "gates": [
    {
      "criterion": 0,
      "platform": "macos | windows | both | integration",
      "name": "<criterion name>",
      "threshold": "<exact threshold or expected observable>",
      "observed": "<measured value or observation>",
      "verdict": "PASS | FAIL | BLOCKED | NOT-APPLICABLE",
      "hostId": "<host or integration>",
      "runId": "<run id>"
    }
  ],
  "parity": {
    "transcriptFormatting": "PASS",
    "controlsAndStates": "PASS",
    "shortcutMapping": "PASS",
    "accessibility": "PASS",
    "privacy": "PASS",
    "lifecycle": "PASS",
    "removal": "PASS",
    "allowedDifferences": ["<documented platform-specific difference>"]
  },
  "blockers": [{ "criterion": 0, "missing": "<external prerequisite>", "checked": "<method>", "nextAction": "<exact action>" }],
  "review": { "reviewer": "<identity>", "highOpen": 0, "mediumOpen": 0, "rerunIds": ["<run id>"] },
  "publication": { "destination": null, "executed": false, "blocker": "<exact missing input or null>" },
  "verdict": "RELEASE-READY | NOT-RELEASE-READY | BLOCKED",
  "verdictReasons": ["<reason>"]
}
```

The schema is closed. Contradiction tests reject `RELEASE-READY` with any `FAIL`/`BLOCKED` gate; ASR maturity other than `production-approved`; missing/mismatched Spec 05 approval digest; a temporary adapter or non-release evidence reference; open High/Medium finding; unsigned/ad-hoc artifact; fewer than two artifacts; mismatched Spec 12 bases; invalid publication state; invalid signature/notary state; or artifact checksum absent from `CHECKSUMS.txt`.

### Produced - verdict semantics

- `PASS` per host: every applicable criterion for that platform has a recorded observed value meeting its threshold, and no High or Medium finding is open against it.
- `FAIL` per host: any runnable defect. A defect is always `FAIL`, never `BLOCKED`, regardless of how small or how late it is found.
- `BLOCKED` per host: an unavailable external prerequisite, or the inherited production-ASR/predecessor gate, prevents mandatory work after every reachable criterion is completed. A runnable product defect remains `FAIL`, never `BLOCKED`.
- `RELEASE-READY` cross-host: ASR maturity is `production-approved`, the Spec 05 approval digest matches every freeze/artifact, both platforms `PASS`, parity/review/docs/assets are complete, and the local tag exists.
- `NOT-RELEASE-READY`: any runnable `FAIL`, open High/Medium finding, unreconciled claim, missing evidence, or artifact defect.
- `BLOCKED` cross-host: no runnable `FAIL`, but production ASR/predecessor or an allowed external prerequisite prevents a mandatory criterion. It is not release-ready and must not be distributed.

Publication is tracked separately only after release eligibility. Missing publication destination cannot conceal an unresolved production-ASR gate or make a development artifact releasable.

## 7. User Flow and Developer Verification Flow

### Operator preflight

1. Confirm no other writer or worktree is active. List all worktrees and branches, confirm one clean checkout, and record canonical root, branch, and base SHA. Any second active writer stops the run.
2. Re-read canonical context, `spec-plan.md`, Specs 01-15, current source and tests, manifests and lockfiles, Spec 12's freeze and evidence, both packaging manifests, and the installed version-matched Tauri 2 documentation under `node_modules`. Installed documentation overrides general examples.
3. Verify the predecessor consistency contract in section 6 item by item. Record each comparison, not a summary claim.
4. Recompute Spec 12's successor freeze manifest from the release SHA and compare every field and lockfile digest. Refuse to proceed on mismatch.
5. Record both macOS hosts and both Windows hosts: OS edition/version/build, CPU model/cores, RAM, power state, audio input and output devices, and relevant privacy/permission state. Record which host is the floor host for each platform.
6. Verify that both artifacts exist at their recorded immutable locations with matching byte sizes and SHA-256 values. Copy each into a run-scoped read-only release-candidate location; never work directly on the packaging spec's immutable copy.
7. Confirm no credential, private key, keychain item, certificate password, notary token, or destination secret is printed, logged, or written into evidence at any point.

### Candidate identity flow

1. Derive the release SHA. If both packaging commits sit on one clean integration SHA with no subsequent change, that SHA is the release SHA. If any change is needed, apply it, then rebuild **both** artifacts from the new SHA before any further measurement.
2. Verify each artifact's provenance against the release SHA using its packaging manifest: recorded base SHA, toolchain record, and normalized payload digest. An artifact whose recorded source SHA differs from the release SHA is discarded and rebuilt on its own platform host.
3. Reconcile the two payload inventories. Compute the intersection (model files, notices, application code identity, version resources) and the documented platform-specific difference set (bundle vs installer layout, `.icns` vs `.ico`, `Info.plist` vs version resource, ScreenCaptureKit vs WASAPI adapter code, embedded WebView2 offline installer, DMG vs NSIS container metadata). Any difference outside that set is a release-blocking defect.
4. Recompute every model file digest and the notice digest from inside each installed artifact - not from the source tree - and compare both platforms against the frozen manifest.
5. Write `release/CHECKSUMS.txt` from freshly computed digests of the two release-candidate copies, then re-verify each line by independent recomputation.

### Installed user flow - per platform, per host

1. Install from a real download path on a clean standard user account: macOS via a quarantined DMG in Finder, dragging to Applications; Windows via the downloaded setup executable as a standard user with no elevation. Record the exact install path, prompts observed, and Gatekeeper/SmartScreen/Defender behavior verbatim, without disabling or bypassing any protection.
2. Disconnect all non-loopback networking, then launch the installed application for the first time. Record every permission prompt, its trigger, and its timing. Microphone access is requested only from an explicit user action; on macOS, Screen Recording is requested only when the user enables system audio.
3. Run microphone-only capture with at least 30 local corpus utterances played from a separate physical speaker into the selected microphone, including incorrect forms, minimal pairs, fillers, repetitions, a long turn, and physical silence.
4. Run system-only capture with at least 30 utterances through the current default output endpoint, confirming the platform adapter receives the real system path.
5. Run dual-source capture with distinct scripted sets, at least 10 intentional overlaps, and periods where only one source speaks.
6. Exercise Start -> Stop -> Start at least three times, then Clear with finalized content present (confirming the confirmation step), then Copy All, then paste into a plain local text surface and into Obsidian if present on the host.
7. Exercise shutdown three ways: window close, application quit/exit, and a console/termination signal. Relaunch after each and confirm no residual state, no orphaned process, no leaked device handle, and no restored transcript.
8. Exercise degradation and recovery honestly where inducible: unplug or switch the microphone, change the default output endpoint, and revoke a permission between runs. Confirm bounded retry with visible status, survivor continuity, and no transcript rewrite.
9. Run the installed 60-minute dual-source soak, sampling process-tree RSS, thread and handle/descriptor counts, per-source counters, and audio-time drift at least every five minutes.
10. Remove the application: macOS drag-to-Trash removal plus residue inventory; Windows uninstall via Settings/Apps plus directory, registry, shortcut, and WebView2 classification.
11. Repeat the full sequence on the platform's floor host. A floor-host omission blocks only the floor claim and never substitutes for it.

### Parity flow

1. Run one identical scripted conversation set on both platforms with the same utterance order and the same source assignment. Capture each platform's `Copy All` output through the external observer path, never through a new application command.
2. Compare the two outputs structurally: line count, line ordering rule, speaker prefix placement, blank-line handling, whitespace normalization, and absence of any correction. Text differences attributable to recognition are expected; structural differences are failures.
3. Compare visible states and controls: idle, starting, listening, degraded, reconnecting, stopping, error, and empty state; button enablement; error placement; Clear confirmation; Jump to latest; interim styling. Record platform-specific permission copy as an allowed difference and any other divergence as a failure.
4. Compare shortcut behavior: `Cmd + Enter` / `Ctrl + Enter` and `Cmd + Shift + C` / `Ctrl + Shift + C`, with platform-native copy, select-all, and text navigation still working, and no global hotkey registered.
5. Compare accessibility: with VoiceOver on macOS and Narrator on Windows, confirm the named transcript log region, once-only final announcements, silent interim subtrees, focusable and correctly named controls, and status conveyed without color alone.
6. Compare privacy observations: process-tree network attempts, file writes, browser storage, application-owned storage, logs, and clipboard writes. Both must show zero non-loopback attempts and zero transcript/audio persistence.
7. Write `release/evidence/parity.json` with each comparison, its verdict, and the documented allowed-difference set.

### Release artifact flow

1. Validate `release/manifest.json` against `release/schema.json`, then run the contradiction tests in section 6.
2. Render `release/evidence/summary.md` from the manifest so every criterion appears with threshold, observed value, verdict, host, and run id.
3. Write `release/RELEASE_NOTES.md`: version, supported platforms and floors, what the product does, permission requirements, the local-only privacy statement, the explicit statement that there is no updater and no account, and the known limitations - DRM-protected system audio is indistinguishable from silence on Windows, Windows loopback captures the whole session mix, Remote Desktop redirects audio, macOS requires Screen Recording permission for system audio, recognition may still misrecognize speech, and the transcript is not saved by the application.
4. Write `release/ROLLBACK.md` and rehearse it as a dry run: withdraw the download, publish a replacement statement, revert the release commit and tag in a recorded order, invalidate published checksums, and - for a compromised signing identity - preserve hashes and submission ids, revoke through the issuer's process, and rebuild under a new identity. The rehearsal records the exact commands and their observed effects on local state without publishing anything.
5. Write `release/PUBLICATION.md` with the destination decision. If the user supplies a destination and credentials, execute the checklist, record the published URLs and the checksum publication, then verify a real download-and-install from the published location on one host per platform. If not, record the exact missing input as `BLOCKED` and upload nothing.
6. Create the annotated tag `v0.1.0` on the release SHA locally. Do not push the tag or the branch unless explicitly requested.

### Documentation reconciliation flow

1. Fill every spec's section 16 evidence record from real runs, including SHAs, commands, hosts, measurements, and review dispositions. A `Pending` field in any spec at this point is a release blocker.
2. Compare each context document against the shipped product and correct any statement that no longer matches: stack versions, platform floors, permission behavior, model identity, capability set, shortcut list, and invariant wording. Record each correction with its reason.
3. Confirm that no context or spec document claims a capability the release does not have, and that no shipped behavior is undocumented.
4. Write the final `progress-tracker.md` state: release SHA, tag, both artifact digests, per-host verdicts, cross-host verdict, publication state, and the open product decisions that remain after V1.
5. Confirm the staging bundle at `/Users/berat/mistaken-context` is no longer authoritative and that canonical documentation lives in the repository.

### Verification and review flow

1. Run the complete check matrix on both hosts after the final source change: typecheck, lint, frontend tests and build, Rust format/check/clippy/tests, platform adapter checks, Spec 12 frozen consistency checks, packaging tests, both package builds, and the final installed smoke. Record exact commands, tool versions, and exit codes.
2. Rebuild and re-run every artifact-dependent check after any source mutation. A measurement taken before the final mutation is void.
3. Conduct the independent high-capability review across candidate identity, payload reconciliation, signatures and trust, installed flows, parity, soak results, privacy evidence, documentation reconciliation, verdict semantics, and blocker honesty. Fix every High and Medium finding, regenerate affected artifacts and evidence, and record rerun ids.
4. Scan committed content and Git objects for model weights, secrets, private paths, transcripts, audio, raw traces, and staged installers.
5. Fill this spec's section 16, create one focused local commit unless directed otherwise, verify clean status, and report branch, base SHA, release SHA, tag, and push status.

## 8. UI Behavior, States, Tokens, and Accessibility

Spec 15 adds no UI. It verifies that the shipped UI behaves identically across platforms and fixes it when it does not.

### Verified states

- Idle, starting, listening, degraded, reconnecting, stopping, and error states render with the same structure, the same control enablement, and the same copy on both platforms, except for platform-specific permission guidance.
- The empty state explains how to begin without promotional language, and the optional technical line remains small and muted.
- The transcript remains the dominant surface: no chat bubbles, no avatars, no gradient headings, no glowing cards, no decorative animation, and no account or cloud affordance on either platform.
- Microphone segments render with no prefix; system segments render with `- `. Interim text is visually distinguishable from final text and remains readable while muted.
- Errors appear near the affected control with actionable local wording. macOS permission guidance never appears on Windows, and Windows failure wording never borrows macOS permission language.
- Status is never conveyed by color alone on either platform.

### Verified tokens

- Both platforms resolve the same semantic CSS custom properties from the same token set; no feature component contains a hardcoded color value. Any platform-conditional style is limited to native affordances such as scrollbar or focus-ring rendering and is recorded as an allowed difference.
- Dark-only V1 is confirmed on both platforms; no light theme path is shipped or reachable.

### Verified accessibility

- The transcript exposes one named region with `role="log"` and its implicit polite live semantics for finalized rows, while interim subtrees are excluded from live announcement.
- A finalized segment is announced exactly once under VoiceOver and once under Narrator, with no mirrored live region and no duplicate announcement of interim text.
- Every control has an accessible name, is reachable by keyboard, shows a visible focus indicator, and is operable without a pointer.
- `Jump to latest` is a real focusable control with an accessible name, and auto-follow detachment does not move the scroll position while the user reads.
- Reduced-motion preferences are honored on both platforms; scrolling falls back to instant behavior.
- Text scaling and window resizing at the platform's standard and large settings keep the transcript readable and controls reachable on both platforms.

### Icon reconciliation

- The macOS `icon.icns` and Windows `icon.ico` derive from the same restrained token-based `M` mark. Spec 15 verifies that both render recognizably and unclipped in Finder, Dock, app switcher, macOS permission lists, Explorer, taskbar, Start Menu, Alt-Tab, Settings -> Apps, and the setup executable, and that neither resembles a system or security alert badge. Any visual divergence is reconciled here, since the packaging specs were forbidden from sharing the files.

## 9. Frontend -> Tauri IPC -> Rust / Audio / ASR Data Flow

Spec 15 changes no data flow. It verifies the shipped flow on installed artifacts and documents it as released:

```text
installed application launch
  -> WebView loads local assets only (no remote origin, no CDN, no font/script fetch)
  -> frontend registers the six native event listeners, then calls get_runtime_snapshot
  -> snapshot reports model presence, permission posture, and device availability from real native state
  -> user selects a microphone and optionally enables system audio, then starts capture
  -> start_capture requests exactly the chosen sources; all requested sources must reach a live stream or the start fails and rolls back
  -> per source: platform capture -> 100 x 20 ms capture pool -> 30 x 100 ms inference stage -> its own OnlineStream on the one shared recognizer
  -> transcript events carry source, segment id, text, and audio-derived timings; no PCM and no audio level crosses IPC
  -> reducer applies partials by segment id, finalizes immutably, and keeps first-seen final ordering
  -> formatter renders microphone lines plain and system lines with "- "
  -> Copy All serializes finals only through the write-only clipboard boundary
  -> stop_capture or shutdown releases both sources exactly once
```

Verified properties on both platforms, from the installed artifact:

- Exactly four application commands and six events exist in the shipped binary; no diagnostic, dump, export, telemetry, or debug command is reachable.
- The main capability grants only those commands plus listen permission; the frontend cannot emit, read the filesystem, or make network calls.
- Queued audio never exceeds 10 seconds in total across both sources; overflow drops newest and remains visible through counters rather than blocking.
- No decoding or capture work runs on the UI thread; the interface stays responsive during dual capture and during the soak.
- No stage between recognizer output and rendered text performs correction, substitution, punctuation restoration, inverse text normalization, or hotword replacement. The exercised path is inspected in the shipped artifact, not only in source.
- Structural attribution holds end to end: a source-mismatched block is a fatal internal error, and no text-derived heuristic assigns a source.
- The `- ` prefix exists only in the frontend formatter; no native payload in either artifact contains it.

## 10. Platform, Permissions, Offline, Privacy, and Fallback

### Platform matrix released

| Platform | Target | Declared floor | Tested hosts | System audio path |
| --- | --- | --- | --- | --- |
| macOS | `aarch64-apple-darwin` | macOS 13.0 | current Apple Silicon development host plus a macOS 13.x floor host | ScreenCaptureKit, audio-only, Screen Recording permission required |
| Windows | `x86_64-pc-windows-msvc` | Windows 10 22H2 (build 19045) / Windows 11 | Windows 11 x64 plus a Windows 10 22H2 x64 floor host | WASAPI loopback on the default render endpoint, no permission prompt |

Intel and universal macOS builds, Windows ARM64 and 32-bit builds, and the Windows 15063-19044 API-floor range remain out of scope and are stated as untested rather than implied to work.

### Permissions released

- macOS: microphone access with the exact purpose string, requested only on an explicit user action; Screen Recording, requested only when the user enables system audio, with an honest restart-required state. No camera, location, contacts, photos, automation, accessibility, or full-disk access. No App Sandbox, no hardened-runtime exception without direct evidence, no `get-task-allow`.
- Windows: no permission prompt for loopback capture; microphone access governed by the OS desktop privacy setting with behavior reported from observable native results. No elevation, service, scheduled task, startup entry, driver, audio filter, virtual device, firewall rule, or third-party COM registration.
- Both: the installed application requests nothing at launch. Every prompt is traceable to a user action, and the count of launch-time prompts is recorded as zero on both platforms.

### Offline behavior released

- Installation works with networking disconnected on both platforms: macOS DMG install needs no network, and the Windows setup embeds the Evergreen WebView2 offline installer.
- First launch and the complete core flow work with all non-loopback networking disabled on both platforms.
- The release build performs no model download, no license check, no update check, no telemetry, no crash upload, no font or asset fetch, and no remote logging. Process-tree tracing records zero non-loopback attempts, and the absence is proven by both prevention (no route) and observation (attempt tracing).
- The only tolerated local traffic is loopback required by the platform WebView, which is recorded and classified rather than denied.

### Privacy released

- No transcript, audio sample, or recognition hypothesis is written to disk by the application on either platform. Content-aware sentinel scanning covers application-owned storage, WebView storage, logs, temporary directories, and crash-report locations.
- The clipboard is written only by explicit `Copy All`, through a write-only boundary that never reads clipboard content.
- No account, login, profile, device identifier, analytics identifier, or usage counter exists. Nothing about device choice, permission state, failure history, or session content persists across launches.
- Unavoidable OS or WebView metadata (cache directories, GPU shader caches, WebView2 user-data folder) is classified with its owner and content type rather than claimed absent.
- Release evidence itself is redacted: no private path, transcript text, audio, credential, or raw trace is committed. Raw runs stay in ignored locations with their digests recorded.

### Fallback rules

- There is no cloud fallback, no remote recognizer, no alternate model download, and no degraded online mode. A missing or mismatched model is an explicit local error.
- A failing source degrades visibly and bounded-ly; the survivor continues. Exhausted recovery is terminal with the underlying reason preserved.
- A missing external prerequisite produces an explicit `BLOCKED` field after every reachable criterion passes; it never produces a substituted mechanism, a relaxed gate, or a silent downgrade.
- If a platform cannot be verified on its floor host, the floor claim is `BLOCKED` and the release notes state the tested hosts exactly, rather than implying broader support.

## 11. Resource Lifecycle, Bounds, Errors, and Recovery

### Release-run lifecycle

- Every measurement belongs to one run id bound to the release SHA, one host id, and one artifact digest. A run whose artifact or source changes afterwards is void and rerun; evidence is never edited to match a later build.
- Artifact copies used for verification are read-only. Verification never mutates the candidate; a failed run produces a new run id, and a fixed defect produces a new release SHA and new artifact identities.
- Installed test copies, mounted images, extracted payloads, registry exports, traces, temporary accounts, keychain items, and clipboard content are enumerated when created and removed at the end of the run. Anything that cannot be removed is recorded as retained OS state with its owner.
- Host settings changed for verification - network state, default audio device, microphone privacy setting, security settings, display scaling, assistive technology - are restored, and any unrestorable change is recorded.

### Bounds verified

- Total queued audio across both sources stays at or below 10 seconds; the 2 s capture pool and 3 s inference stage per source remain fixed-capacity with drop-newest overflow.
- Peak process-tree RSS stays within 700 MB single-stream and 1.4 GB dual-stream; sustained CPU stays within 60% of one core per active stream.
- During the installed 60-minute dual soak, RSS growth after minute 5 stays within 5%, native thread and handle/descriptor counts stay flat, per-source counters remain consistent, audio-time drift stays within its recorded tolerance, and repeated recovery leaves no allocation ratchet. The transcript is the only permitted monotonic growth.
- Normal-flow runs require zero drops; drops are permitted only under deliberate overload and must remain visible through counters and the degraded state.

### Errors and evidence handling

- Release scripts fail closed on malformed manifest input, missing unit, unknown host, mismatched SHA/model/version, incomplete trace, clock discontinuity, insufficient samples, dropped observer event, or dirty final tree. A partial run cannot produce `PASS`.
- The closed schema rejects unknown fields, missing hashes or units or host facts, inconsistent verdict combinations, stale SHAs, wrong artifact digests, skipped applicable criteria, and unresolved High/Medium findings.
- High and Medium findings block the release verdict. Low findings may remain only with explicit impact, owner, and rationale recorded in the final review record.
- A failed signing, notarization, or packaging run is never repaired in place: keep its ignored raw evidence, fix the source or configuration, and create a new artifact identity.

### Recovery paths for the release itself

- Defect found before publication: fix at the root cause, rebuild both artifacts from the new release SHA, rerun every invalidated criterion on both platforms, and supersede the previous candidate entirely. Two candidates never coexist as releasable.
- Defect found after publication: execute `release/ROLLBACK.md` - withdraw the download, publish the replacement statement, invalidate published checksums, revert the release commit and tag in the recorded order, and treat the next candidate as a fresh release SHA under a new version decision.
- Signing identity or artifact compromise: stop distribution immediately, preserve artifact hashes, signature metadata, and notary submission ids, revoke through the issuer's documented process, rebuild under a new identity, and publish a user-facing statement. No updater exists, so the statement must tell users to remove and reinstall manually.

## 12. Numbered Measurable Acceptance Criteria

1. **Production predecessors and base — integration:** Specs 01–14 are reviewed/merged/evidenced; Spec 05 names one gate-passing `ProductionApproved` candidate; Spec 12 is production `PASS` and its freeze recomputes; Specs 13/14 share that base and approval digest; the manifest records canonical/base/release SHAs and a clean tree. Any `DevelopmentOnly` or blocked-ASR input makes this criterion `BLOCKED` and forbids release work.
2. **Serialized single-writer ownership - integration:** Exactly one writer owns the repository for the whole run; no other branch or worktree modifies any path; every changed path is either under `release/**` or tied to a named failed criterion with a recorded root cause; and Spec 12's frozen schema, scenario thresholds, and `benchmarks/**` records are unmodified.
3. **One release candidate identity - integration:** Exactly two distributable artifacts exist, both provably built from the single release SHA, both at version `0.1.0`, each recorded with name, byte size, SHA-256, normalized payload digest, target, floor, and immutable location; any artifact from a different SHA is discarded and rebuilt rather than described.
4. **Frozen production input parity — both platforms:** Both artifacts report `production-approved` maturity and the same Spec 05 approval digest, model identity/files/digests, runtime/configuration, delivery manifest, and notices; no temporary adapter or model weight committed to Git exists.
5. **Payload reconciliation - both platforms:** The two payload inventories differ only within the documented platform-specific set (bundle vs installer layout, `.icns` vs `.ico`, `Info.plist` vs version resource, platform system-audio adapter code, embedded WebView2 offline installer, container metadata); every other difference is recorded as a defect and fixed, and neither payload contains source, tests, `.env`, credentials, absolute developer paths, benchmark recordings, transcripts, audio, raw traces, updater metadata, or unexpected third-party binaries.
6. **macOS artifact trust - macOS:** `Mistaken_0.1.0_aarch64.dmg` and the contained `Mistaken.app` are signed with a valid `Developer ID Application` identity, have the hardened runtime enabled with no unevidenced exception and no App Sandbox or `get-task-allow`, carry a secure timestamp, have an `Accepted` notarization with recorded submission id and reviewed log digest, are stapled, and pass Gatekeeper assessment; every application-owned Mach-O is arm64 only with the 13.0 minimum declared.
7. **Windows artifact trust - Windows:** `Mistaken_0.1.0_x64-setup.exe` and the installed application executable carry SHA-256 Authenticode signatures with an RFC 3161 timestamp from a recorded authority, `signtool verify /pa /all` passes on both, the certificate subject/thumbprint/expiry are recorded, and the observed SmartScreen/Defender behavior is recorded verbatim with no protection disabled or bypassed.
8. **Clean-account install - macOS:** On a fresh standard user account, a quarantined DMG install through Finder mounts, shows exactly one `Mistaken.app` plus the Applications target, installs by drag, launches with zero Gatekeeper bypass and zero launch-time permission prompts, and records the exact prompts and install path.
9. **Clean-account install - Windows:** On a fresh standard user account, the downloaded setup executable installs per-user under `%LOCALAPPDATA%` with no UAC prompt and no administrator requirement, creates only the recorded shortcuts and `HKCU` uninstall entry, provisions WebView2 without internet access, and launches with zero launch-time permission prompts.
10. **Installed offline core flow - macOS:** With all non-loopback networking disabled, the installed application completes microphone-only, system-only, and dual-source capture with zero non-loopback network attempts observed in the process tree, zero transcript or audio bytes written anywhere, and permission prompts occurring only on the explicit user actions that require them.
11. **Installed offline core flow - Windows:** The same three flows complete on the installed Windows application with all non-loopback networking disabled, zero non-loopback attempts observed with process/path attribution, zero transcript or audio persistence, and no permission prompt for loopback capture.
12. **Source-separation corpus on installed artifacts - both platforms:** Spec 12's protocol runs against the installed artifacts on both platforms with at least 30 scored utterances per source, at least 10 intentional overlaps in the dual run, and periods of single-source speech; source identity is correct for every scored utterance with zero cross-attribution, zero native `- ` in any payload, and stable first-seen final ordering.
13. **Fidelity and latency on installed artifacts - both platforms:** Spec 05's unchanged gates hold per platform on the final artifacts: MPR >= 0.90, false-correction rate <= 0.05, `mistake-tense` and `mistake-minimal-pair` MPR >= 0.85, full-corpus WER <= 0.25, fluent-control WER <= 0.12, zero non-empty final on physical silence, noise insertion <= 0.02 tokens/s, median first partial <= 900 ms, and p95 final-after-endpoint <= 1500 ms, each recorded with its measured value, sample count, and host.
14. **Resource gates on installed artifacts - both platforms:** RTF <= 0.6 single-stream and <= 0.9 dual-stream, peak process-tree RSS <= 700 MB single and <= 1.4 GB dual, sustained CPU <= 60% of one core per active stream, and total queued audio <= 10 seconds with zero drops in normal flow and visible fixed-capacity behavior under deliberate overload.
15. **Transcript output parity - both platforms:** For one identical scripted conversation, both platforms produce structurally identical `Copy All` output - same line count, same ordering rule, same speaker prefix placement, same whitespace handling, finals only - with no correction, rewriting, punctuation restoration, or word substitution applied by application logic on either platform.
16. **Controls, states, and shortcut parity - both platforms:** Idle/starting/listening/degraded/reconnecting/stopping/error and empty states, control enablement, error placement, Clear confirmation with finalized content, and `Jump to latest` behave identically; `Cmd/Ctrl + Enter` toggles Start/Stop and `Cmd/Ctrl + Shift + C` runs Copy All on the correct platform modifier; no global hotkey is registered; native copy/select-all/text navigation still work; and the only recorded differences are the documented platform-specific permission guidance.
17. **Accessibility parity - both platforms:** With VoiceOver on macOS and Narrator on Windows, the named transcript log region is exposed, each finalized segment is announced exactly once, interim mutations are silent, every control has an accessible name with visible keyboard focus, status is conveyed without color alone, reduced-motion is honored, and the interface remains usable at the platform's large text/scaling setting.
18. **Lifecycle parity - both platforms:** Start -> Stop -> Start repeats at least three times without leaking a session, device handle, thread, or transcript state; window close, application exit, and a termination signal each release both sources exactly once; relaunch starts clean with no restored transcript and no orphaned process; and inducible device/permission failures degrade visibly with bounded retry and never rewrite finalized content.
19. **Installed 60-minute dual soak - both platforms:** On each platform's primary host, a 60-minute continuous dual-source run on the installed artifact samples at least every five minutes and shows RSS growth after minute 5 within 5%, flat native thread and handle/descriptor counts, consistent per-source counters, audio-time drift within the recorded tolerance, no allocation ratchet after recovery, and no drop in normal flow.
20. **Removal parity - both platforms:** macOS removal by moving the app to Trash leaves an inventoried residue set classified by owner, and Windows uninstall removes the application directory, embedded model, notices, shortcuts, and `HKCU` uninstall entry while the Microsoft-owned WebView2 runtime and its user-data folder are classified as retained OS state; neither platform leaves transcript or audio content behind.
21. **Floor-host runs - both platforms:** The full installed flow runs on the macOS 13.x Apple Silicon floor host and the Windows 10 22H2 x64 floor host with recorded OS build, hardware, devices, and results; a missing floor host blocks only that platform's floor claim after every other criterion on that platform passes, and the release notes then state the tested hosts exactly.
22. **Release manifest and checksums - integration:** `release/manifest.json` validates against `release/schema.json`, contains exactly two artifacts with matching digests, records every criterion in this section with threshold, observed value, verdict, host, and run id, passes every contradiction test, and `release/CHECKSUMS.txt` contains independently recomputed SHA-256 lines that match both artifacts verbatim.
23. **Release notes correctness - integration:** `release/RELEASE_NOTES.md` states version, both targets and floors, tested hosts, required permissions per platform, the local-only privacy statement, the explicit absence of an account, updater, and cloud service, and the known local limitations including DRM-protected system audio indistinguishable from silence on Windows, whole-session-mix loopback capture, Remote Desktop audio redirection, macOS Screen Recording requirement for system audio, probabilistic recognition, and no application-side transcript saving; every claim traces to a recorded observation.
24. **Rollback rehearsal - integration:** `release/ROLLBACK.md` defines withdrawal, replacement statement, checksum invalidation, revert order for the release commit and tag, and compromise response with hash/submission-id preservation and issuer revocation, and its dry-run rehearsal is recorded with exact commands and observed local effects while publishing nothing and assuming no updater.
25. **Publication honesty - integration:** Publication executes only with a user-supplied destination and credentials, in which case the published URLs, published checksums, and one real download-and-install verification per platform are recorded; otherwise `publication.executed` is `false` with the exact missing input recorded as `BLOCKED`, nothing is uploaded, no destination is invented, and no credential appears in evidence.
26. **Documentation reconciliation - integration:** Every spec's evidence record is filled from real runs with no `Pending` field remaining, every context document matches shipped reality with each correction and its reason recorded, no document claims a capability the release lacks, no shipped behavior is undocumented, the final tracker records release SHA, tag, both artifact digests, per-host verdicts, cross-host verdict, publication state, and remaining product decisions, and canonical documentation lives in the repository rather than the staging bundle.
27. **Complete check matrix - both platforms:** After the final source change, typecheck, lint, frontend tests and build, Rust format/check/clippy/tests, both platform adapter check sets, Spec 12 frozen consistency checks, both packaging test sets, both package builds, and the final installed smoke all pass on their respective hosts with exact commands, tool versions, and exit codes recorded, and no changed lockfile or unintended shared-source change remains.
28. **High review and final verdict — integration:** Independent review covers ASR maturity/approval identity, payload/delivery reconciliation, artifact trust, installed flows, parity, resources, privacy, docs, verdict semantics, and blockers; every High/Medium finding is fixed with regenerated evidence. `RELEASE-READY` additionally requires `production-approved` maturity, matching Spec 05 approval digests, both platforms/parity `PASS`, complete docs/assets, and the unpushed local tag; unresolved production ASR yields only `BLOCKED`.

## 13. Acceptance Criterion -> Verification / Test Mapping

| # | Verification method | Evidence |
| --- | --- | --- |
| 1 | Inspect Spec 05 approval/maturity, Spec 12 production report/freeze, worktrees, packaging manifests, and clean state | Approval digest/maturity, base/release SHAs, per-field freeze comparison |
| 2 | Worktree listing, changed-path classification, digest comparison of frozen schema/scenario/benchmark files | Writer inventory, criterion-attributed paths, frozen-file digests |
| 3 | Artifact provenance check against release SHA, digest/size recomputation, discard/rebuild log | Artifact identities and locations |
| 4 | Recompute production maturity/approval/model/delivery/notice values from both artifacts; scan Git | Cross-platform frozen-input table, zero temporary-adapter finding |
| 5 | Inventory diff of both payloads against the documented difference set, forbidden-content scan | `parity.json` payload section, allowed-difference list, scan findings and dispositions |
| 6 | `codesign` display/verify, entitlement enumeration, `lipo`/Mach-O inspection, `notarytool` log review, `stapler`, `spctl` assessment | Signature authority chain, entitlement list, architecture output, submission id and log digest, staple and Gatekeeper results |
| 7 | `signtool verify /pa /all` on setup and application executables, certificate and timestamp inspection, protection-behavior observation | Verify output, certificate facts, timestamp authority and time, verbatim SmartScreen/Defender observation |
| 8 | Fresh standard account, quarantined Finder install, mounted-volume inventory, prompt log | Install transcript, volume inventory, prompt count and triggers, install path |
| 9 | Fresh standard account, download-path install, registry/shortcut export, WebView2 `pv` before/after, prompt log | Install transcript, per-user path, registry and shortcut inventory, WebView2 state, prompt count |
| 10 | Network-disabled installed run with process-tree attempt tracing, file-write tracing, storage and log inspection | Attempt counts with attribution, write inventory, sentinel scan result, prompt timeline |
| 11 | Same methods with Windows Filtering Platform event correlation and process/path attribution | Attempt counts with process and path fields, write inventory, sentinel scan result |
| 12 | Spec 12 scenario definitions run against installed artifacts, external observer capture, scoring | Per-utterance source identity table, overlap set, ordering check, native `- ` scan |
| 13 | Benchmark scorer re-run on installed-artifact output with repetitions, per-host aggregation | Gate table with measured values, thresholds, sample counts, host, run id |
| 14 | Process-tree CPU/RSS sampling, RTF computation, queue counter inspection, deliberate overload run | Sampled series summaries, RTF values, counter snapshots, overload observation |
| 15 | Identical scripted conversation both platforms, structural comparison of `Copy All` output | Both outputs' structural summaries, diff of structure, correction-stage scan |
| 16 | Scripted UI walkthrough of every state on both platforms, shortcut matrix, native-editing checks | State-by-state screenshots/descriptions, shortcut result matrix, allowed-difference list |
| 17 | VoiceOver and Narrator sessions, announcement counting, focus and naming audit, scaling run | Assistive-technology versions, announcement counts, focus/name findings, scaling observations |
| 18 | Repeated Start/Stop cycles, three shutdown paths, relaunch inspection, induced device/permission failures | Cycle log, shutdown/relaunch results, handle and process checks, recovery timeline |
| 19 | 60-minute installed dual soak per platform with >= 5-minute sampling | RSS/thread/handle series, counter consistency, drift values, drop counts |
| 20 | macOS removal plus residue inventory, Windows uninstall plus directory/registry/shortcut/WebView2 classification | Residue inventories per platform with owner classification, transcript/audio absence proof |
| 21 | Full installed flow on each floor host with recorded hardware/OS identity | Floor-host profiles and per-criterion results, or the exact floor blocker |
| 22 | Schema validation, contradiction test suite, independent checksum recomputation | Validation output, contradiction test results, checksum file and recomputation log |
| 23 | Claim-by-claim traceability review of release notes against recorded observations | Notes file with per-claim evidence references |
| 24 | Dry-run rehearsal of the rollback checklist with local-only effects | Rehearsal command log, observed local effects, publish-nothing confirmation |
| 25 | Destination decision record; if supplied, publication execution plus real download-and-install verification | Publication record with URLs and checksum publication, or exact `BLOCKED` field; credential-free evidence |
| 26 | Spec-by-spec evidence completeness check, context-to-reality comparison, tracker finalization | Completeness report, correction list with reasons, final tracker state, canonical-location confirmation |
| 27 | Execution of the full check matrix on both hosts after the final source change | Command, tool version, and exit code table per host; lockfile and shared-source diff check |
| 28 | Independent high-capability review, finding disposition, rerun regeneration, tag creation, verdict computation | Reviewer identity, findings with dispositions and rerun ids, tag object details, verdict with reasons |

Permanent tests required for release semantics rather than one-off scripts: release schema validation, verdict contradiction rejection, checksum-to-manifest consistency, frozen-input parity comparison across two payload inventories, allowed-difference set enforcement, and evidence-completeness checking. They must assert observable rejection or computed outcomes, never source text or mock forwarding.

## 14. Ordered Implementation Plan

1. Confirm serialized ownership: one clean checkout, no other worktree or writer, recorded canonical root, branch, and base SHA.
2. Re-read canonical context, `spec-plan.md`, Specs 01-15, source and tests, manifests and lockfiles, Spec 12 freeze/evidence, both packaging manifests, and installed Tauri 2 documentation.
3. Verify production ASR maturity and Spec 05 approval digest first, then every predecessor-consistency item and the Spec 12 freeze. On `DevelopmentOnly` or blocked approval, emit the exact blocker and stop before copying artifacts or creating a release tag.
4. Record all four host profiles and verify both production-approved artifacts exist unmutated; copy only eligible artifacts into read-only release-candidate locations.
5. Derive the release SHA and verify both artifacts' provenance against it; rebuild any artifact whose source SHA differs.
6. Author `release/schema.json` and the permanent tests for schema validation and verdict contradiction rejection.
7. Author `release/README.md` and the platform script sets for installed preflight, install, offline run, sampling, parity capture, soak, removal, and cleanup, invoking Spec 12's scenario definitions read-only.
8. Reconcile both payload inventories and frozen inputs, recomputing model and notice digests from inside the installed artifacts; classify every difference and fix any defect at the source.
9. Verify macOS artifact trust: signature, entitlements, hardened runtime, architecture and floor, notarization log, stapling, Gatekeeper.
10. Verify Windows artifact trust: signatures and timestamps on both executables, `signtool verify /pa /all`, certificate facts, and verbatim protection-behavior observation.
11. Run the clean-account install and offline first-launch flows on the macOS primary host, then the full three-scenario corpus, scoring, and resource sampling.
12. Repeat step 11 on the Windows primary host with Windows-native tracing and device handling.
13. Run the parity flow across both platforms with one identical scripted conversation, then the controls/states/shortcut, accessibility, privacy, and lifecycle comparisons; write `parity.json`.
14. Run the installed 60-minute dual soak on each platform's primary host and record the sampled series.
15. Run removal/uninstall and residue classification on both platforms.
16. Run the full installed flow on each floor host, or record the exact floor blocker after all other criteria on that platform pass.
17. Root-cause and fix every defect found. For each fix: name the failed criterion, fix at the source, rebuild **both** artifacts from the new release SHA, and rerun every invalidated criterion on both platforms.
18. Run the complete check matrix on both hosts after the final source change, and rebuild and re-run every artifact-dependent check.
19. Generate `manifest.json`, `CHECKSUMS.txt`, `evidence/<host-profile>.json`, `evidence/summary.md`, and `evidence/parity.json`; validate the schema and run the contradiction tests.
20. Write `RELEASE_NOTES.md` with per-claim traceability and `ROLLBACK.md`, then rehearse the rollback checklist as a local-only dry run.
21. Write `PUBLICATION.md`; execute publication only with a user-supplied destination and verify a real download-and-install per platform, otherwise record the exact `BLOCKED` field and upload nothing.
22. Reconcile documentation: fill every spec's evidence record, correct context statements with recorded reasons, confirm no undocumented behavior and no overclaimed capability, and write the final tracker state.
23. Conduct the independent high-capability review; fix every High and Medium finding, regenerate affected artifacts and evidence, and record rerun ids.
24. Scan committed content and Git objects for weights, secrets, private paths, transcripts, audio, raw traces, and staged installers.
25. Remove temporary installs, mounts, exports, accounts, keychain items, clipboard content, and scratch output; restore host settings and record anything retained.
26. Compute the final cross-host verdict, create the annotated local tag `v0.1.0` on the release SHA, fill section 16, create one focused local commit unless directed otherwise, verify clean status, and report branch, base SHA, release SHA, tag, and push status. Do not push unless explicitly requested.

## 15. Risks, Rollback, Cleanup, and Preservation Rules

### Risks and mitigations

- **Two artifacts from two source revisions:** the most likely way to ship an incoherent product. Mitigation: release identity is a single SHA, provenance is verified per artifact, and any fix rebuilds both sides. A "macOS is already fine" shortcut is prohibited.
- **Platform evidence substitution:** one platform's success standing in for the other. Mitigation: every criterion names its platform, and cross-host verdicts require both hosts' recorded runs.
- **Verifying the wrong thing:** measuring a development build instead of the installed distributed artifact. Mitigation: every runtime criterion is defined against the installed artifact on a clean account, with install path recorded.
- **Gate gaming at the last gate:** promoting a temporary adapter, importing non-release evidence, relaxing thresholds, shortening soak, dropping samples, reducing sources, or editing corpus is invalid. Spec 05/12 gates are read-only and schema tests fail closed on maturity/approval mismatch.
- **Release-time feature creep:** adding a small setting, telemetry ping, update check, or "helpful" correction while fixing defects. Mitigation: scope forbids new behavior; the capability set, command/event count, and correction-free path are re-verified in the shipped artifacts.
- **Security-control theater:** disabling Gatekeeper, SmartScreen, or Defender to produce a clean screenshot. Mitigation: protections stay enabled, observations are recorded verbatim, and bypass is a failure.
- **Credential and destination leakage:** signing secrets, notary tokens, or upload credentials landing in evidence, logs, or commits. Mitigation: credential-free evidence rules, redaction, and a final Git object scan.
- **Silent publication:** uploading to a guessed host or a personal cloud folder. Mitigation: publication requires an explicit user-supplied destination; otherwise it is exactly `BLOCKED` and nothing is uploaded.
- **Documentation drift claimed as reconciliation:** rewriting history so earlier decisions look correct. Mitigation: corrections record the reason and the superseded statement; decisions are amended, not retconned.
- **Rollback that assumes an updater:** planning a recall that silently pushes a fix. Mitigation: the rollback checklist assumes manual reinstall, since no updater ships.
- **Privacy claim overreach:** claiming zero disk writes when the OS or WebView writes metadata. Mitigation: content-aware scanning plus explicit classification of unavoidable OS-owned state.
- **Concurrency creeping back:** a helper branch landing "just a docs fix" during the run. Mitigation: serialized ownership criterion with changed-path classification and artifact invalidation on violation.
- **Floor-host absence treated as pass:** shipping a floor claim tested only on the development host. Mitigation: floor runs are their own criterion; absence is `BLOCKED` and the notes state tested hosts exactly.
- **Soak shortened because it is slow:** truncating the 60-minute installed run. Mitigation: sampling cadence and duration are fixed criteria with recorded series.

### Rollback rules

- Before publication: discard the candidate entirely. A new fix produces a new release SHA, new artifacts, and new evidence; superseded artifacts and checksums are deleted from the release-candidate location, not archived as alternatives.
- After publication: execute `release/ROLLBACK.md` in order - withdraw the download, publish the replacement statement, invalidate published checksums, then revert the release tag and commit locally. Never leave published checksums pointing at a withdrawn artifact.
- Compromise: stop distribution, preserve artifact hashes, signature metadata, and notary submission ids, revoke through the issuer's documented process, rebuild under a new identity, and state plainly that users must remove and reinstall manually.
- Reverting release documentation is part of rollback: the tracker and release evidence must never assert readiness for a withdrawn artifact.

### Required cleanup

- Remove installed test copies, mounted images, extracted payloads, registry exports, temporary user accounts, temporary keychain items, traces, screenshots not retained as evidence, clipboard content, and temporary scripts.
- Restore host network state, default audio devices, microphone privacy settings, security settings, display scaling, and assistive-technology state; record anything that cannot be restored.
- Keep raw runs in ignored locations with their digests recorded; commit only redacted aggregates.
- Leave no `TODO`, placeholder, stub, or uncommitted diff in the release tree or in any spec's evidence record.

### Preservation rules

- Preserve every product invariant at release: local-only core transcription, two separate sources, source-based separation, no correction or rewriting, bounded buffers, in-memory transcript, explicit final-only Copy All, no account/backend/database/updater/telemetry, and verified model-weight licensing with required attribution.
- Preserve Spec 05’s unchanged gates and production approval digest, Spec 12’s production freeze/schema, and Spec 13/14 artifact immutability. `DevelopmentOnly` can never enter `release/**`.
- Preserve artifact-to-source traceability: from each published or candidate artifact digest back to the release SHA, the freeze manifest, and the staged tool digests.
- Preserve the user's environment and unrelated work: never reset or clean unrelated changes, never delete operator credentials, never remove model or corpus files outside the run-scoped scratch area.
- Preserve honesty of scope in the release notes: state the tested hosts and the untested ranges rather than implying broader support.

### Open product questions carried past V1

- Distribution destination, download page, and checksum publication location.
- Whether a future version introduces an update mechanism, and if so under which signing and privacy constraints.
- Intel/universal macOS and Windows ARM64 support, each requiring its own benchmark, runtime archive, and real-host evidence.
- Optional persistence of the selected input device, which remains unspecified and therefore unimplemented.

## 16. Definition of Done and Evidence Record

Spec 15 is complete only when one `ProductionApproved` release candidate — one SHA, version `0.1.0`, two signed artifacts with identical Spec 05 approval/model/runtime/delivery inputs — passes all installed-flow, source-separation, fidelity/latency/resource, parity, accessibility, privacy, lifecycle/removal, and 60-minute soak criteria on both primary/floor platforms; release assets validate; documentation matches; review closes; and exactly one honest verdict is recorded.

`RELEASE-READY` additionally requires production maturity and matching approval digests everywhere. If production ASR remains unresolved, the only permissible outcome is `BLOCKED`; no artifact is eligible for distribution and no release tag is created. No other spec may declare release readiness.

### Required implementation evidence

- **Canonical repository root:** Pending
- **Branch / base SHA / release SHA / final clean Git state:** Pending
- **Serialized ownership confirmation and worktree inventory:** Pending
- **Spec 05 production approval digest/maturity and unchanged-gate result:** Pending
- **Spec 12 cross-host production verdict and freeze recomputation:** Pending
- **Spec 13 SHA / Spec 14 SHA / shared production-approved base:** Pending
- **Artifact table (name, bytes, SHA-256, normalized digest, target, floor, location) x 2:** Pending
- **Frozen input parity (maturity, approval digest, model/runtime/delivery/notices/locks):** Pending
- **Payload reconciliation result and allowed-difference set:** Pending
- **macOS signing/notary/staple/Gatekeeper facts or exact blocker:** Pending
- **Windows Authenticode/timestamp/`signtool verify` facts, SmartScreen observation, or exact blocker:** Pending
- **Host profiles (two macOS, two Windows) with OS build, CPU, RAM, power, devices:** Pending
- **Clean-account install results and prompt timelines per platform:** Pending
- **Offline installed flow results, network attempt counts with attribution, persistence scan results:** Pending
- **Source-separation corpus results per platform (utterance counts, overlaps, cross-attribution, ordering, native `- ` scan):** Pending
- **Fidelity/latency gate table per platform with measured values and sample counts:** Pending
- **Resource gate table per platform (RTF, peak RSS, sustained CPU, queue bounds, drops):** Pending
- **Transcript output parity comparison and correction-stage scan:** Pending
- **Controls/states/shortcut parity matrix:** Pending
- **Accessibility parity results with assistive-technology versions and announcement counts:** Pending
- **Lifecycle parity results (Start/Stop cycles, three shutdown paths, relaunch, induced recovery):** Pending
- **Installed 60-minute dual soak series per platform:** Pending
- **Removal/uninstall residue inventories and classifications:** Pending
- **Floor-host run results or exact floor blockers:** Pending
- **`release/manifest.json` digest, schema validation output, contradiction test results:** Pending
- **`release/CHECKSUMS.txt` content and independent recomputation log:** Pending
- **Release notes per-claim traceability check:** Pending
- **Rollback rehearsal command log and observed local effects:** Pending
- **Publication destination decision, executed URLs and verification, or exact `BLOCKED` field:** Pending
- **Documentation reconciliation report (evidence completeness, corrections with reasons, tracker final state):** Pending
- **Complete check matrix per host (commands, tool versions, exit codes):** Pending
- **Git object and content scan result (weights, secrets, private paths, transcripts, audio, traces, staged installers):** Pending
- **Independent high-capability reviewer, findings, dispositions, rerun ids:** Pending
- **Annotated tag `v0.1.0` object details and push status:** Pending
- **Cleanup and host-setting restoration:** Pending
- **Per-host verdicts and final cross-host verdict with reasons:** Pending
- **Focused local commit:** Pending
- **Push status:** Not pushed unless explicitly requested

### Authoring evidence and sources

- Reviewed `/Users/berat/mistaken-context/project-overview.md`, `architecture.md`, `ui-context.md`, `code-standards.md`, `ai-workflow-rules.md`, `progress-tracker.md`, `spec-plan.md`, and Specs 01-14 in the staging bundle, whose Wave 9 assignment, dependency rows, review gates, merge rules, frozen decisions, and pending product decisions define this spec's limits.
- Consumed the explicit Spec 15 handoff lists, exclusion lists, and successor-gate statements in `spec-13-macos-packaging-distribution.md` and `spec-14-windows-packaging-distribution.md`, including their statements that Spec 15 owns public upload, release notes, cross-platform checksum publication, the final release tag, the release-ready declaration, and public rollback response.
- Consumed Spec 12's committed acceptance layout, closed evidence schema, scenario protocol, successor freeze manifest, and numeric gates as read-only inputs.
- Verified that the application repository is absent and the context bundle remains documentation-only; therefore every field above is `Pending` until Spec 15 runs in the canonical repository against two real signed artifacts on four real hosts.

Authoring this file is not implementation evidence. Every pending field remains pending until Spec 15 is applied in the real repository and the release candidate, both installed flows, both floor-host runs, the parity comparison, the soaks, the review, and the verdict are observed.
