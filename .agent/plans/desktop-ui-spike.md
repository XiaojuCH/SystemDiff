# First guided desktop capture workflow

Status: In progress
Owner: Primary agent
Last updated: 2026-08-26

## Goal

Turn the already validated SystemDiff capture-and-diff core into its first ordinary-user desktop workflow. The change answers one question—"What changed on my Windows PC?"—without requiring PowerShell, JSON files, file pickers, Snapshot terminology, or manual before/after bookkeeping.

This plan implements GitHub Issue #13. It deliberately pauses collector expansion and proves a narrow Windows desktop vertical slice before the project invests in history, dashboards, installers, or additional platform coverage.

## User-visible outcome

A user can open a real Windows desktop window and complete this guided flow:

1. Select **Start capture** on a calm Ready screen.
2. Install, run, or change the software they want to inspect.
3. Select **Finish & Compare**.
4. Read grouped Startup and Windows Services results, with plain-language change descriptions and an optional Technical details disclosure.
5. Start a new capture or cancel safely.

The workflow is available in English and Simplified Chinese based on the user's browser/Windows language. It uses local, read-only SystemDiff collectors, never asks for elevation, and performs no network or telemetry activity.

## Current architecture and context

- `systemdiff-windows::capture_snapshot` is the in-process synchronous orchestration boundary for the implemented Registry startup and Windows Services collectors. Desktop must call it directly and must not shell out to `systemdiff.exe`.
- `systemdiff-diff::diff_snapshots` owns compatibility, deterministic identity, coverage-aware Added/Removed/Modified/Inconclusive classification, and the invariant that incomplete coverage cannot create a false removal.
- `systemdiff-report` already owns terminal-facing human and technical evidence mapping. A narrow locale-neutral desktop presentation module belongs there so Rust, not TypeScript, continues to own meaning.
- The CLI's timestamp, bounded serialization, input loading, and explicit output-path workflow are private to `systemdiff-cli`. The desktop needs its own backend-only ephemeral session/storage boundary; the CLI workflow remains unchanged.
- Snapshot and Diff wire schema v1 are not changed. The desktop presentation DTO is a separate, versioned IPC contract.
- Real Windows Services capture is intentionally `Partial` because SCM enumeration can omit protected services. The desktop presents that as a calm visibility notice while technical details retain exact coverage and diagnostics.
- The broad `before-v1.json` fixture contains planned Scheduled Task evidence and is not suitable for capability claims or screenshots. Desktop tests use focused Registry/Services data.
- The existing portable artifact is CLI-only. A desktop development build must not silently replace or contaminate its packaging, dependency notice, or verification pipeline.

### Phase 0 OSS reference study

The study was limited to six public projects and extracted interaction/architecture principles only. No artwork, icons, layouts, or assets will be copied.

1. **LocalSend** — its Receive/Send workflow hides discovery, networking, and transfer protocol details behind one task and a small number of durable states. Adopt the low cognitive load and truthful waiting/progress surfaces; reject its persistent navigation and transfer-specific percentages.
2. **Files** — its Windows 11 typography, spacing, subtle content surfaces, local banners, and operation feedback feel familiar without explaining a new data model. Adopt surface hierarchy and page-local notices; reject tabs, dual panes, command palettes, and an extensible file-manager shell.
3. **DevToys** — its constrained UI components and Smart Detection keep tool internals behind consistent input/output presentation. Adopt stable typed presentation and restrained Fluent patterns; reject the tool catalog, marketplace, split editor, and developer-dense layout.
4. **UniGetUI** — it successfully normalizes multiple CLI package managers into recognizable objects and actions. Adopt the principle that a GUI can rename and progressively disclose existing core capability without reimplementing it; reject raw tables, bulk selection, filters, and many toolbar commands.
5. **Spacedrive** — its Rust core to typed TypeScript boundary keeps frontend code from redefining backend semantics. Adopt a checked narrow DTO; reject its daemon, persistent library/location model, CQRS scale, job history, and onboarding tax.
6. **WinUtil** — it demonstrates discoverable Windows terminology and standard utility affordances. Adopt only recognizable Windows language/help cues; explicitly reject its admin-first, write-heavy, high-density toolbox and preset aesthetic.

Public sources reviewed include each project's current repository README/screenshots and relevant public UI/architecture documentation: LocalSend `home_page.dart` and transfer progress, Files v4 status-center documentation, DevToys GUI/Smart Detection guidance, UniGetUI pages and CLI reference, Spacedrive architecture guide, and WinUtil user guide/architecture.

### Adopted principles

1. **One job, three durable states.** Ready → Capturing → Results; busy and recoverable error are transient substates, not new product areas.
2. **Five-second primary action.** The Ready screen has one dominant call to action and one sentence explaining the product.
3. **Linear guidance, not domain training.** Say “start recording, make a change, finish and compare,” not Snapshot, Collector, schema, scope, or canonical identity.
4. **Progressive evidence disclosure.** Ordinary view shows the recognizable object, what changed, useful value, and location. Raw UTC, collector status/diagnostics, hashes, and exact evidence live in Technical details.
5. **Truthful activity feedback.** Capture/compare use an indeterminate busy state; no fabricated percentage or instant-cancel claim.
6. **Coverage is a notice, not an error.** Account visibility limits remain visible and factual but do not become a red error page or a confirmed removal.
7. **Rust owns meaning; React owns language and layout.** Change classification, changed service fields, coverage mapping, and session transitions are decided in Rust and represented by stable semantic IDs.
8. **Real workflow as presentation.** README imagery comes only from the working implementation and shows the core task, not future mock functionality.

### Explicit anti-patterns

1. Empty app shells: unused sidebar entries, dashboards, settings centers, metrics, or charts.
2. Raw evidence as the default interface: tables of collectors, schemas, canonical IDs, JSON, or raw UTC timestamps.
3. Scareware semantics: fake security scores, malicious/safe claims, warning walls, hacker styling, or AI gradients.
4. Dishonest state: fake percentages, silent long-running work, treating Partial as failure, or treating Inconclusive as Removed.
5. Premature platform architecture: daemon/CQRS/history, plugin UI framework, toolbox navigation, large state libraries, or generic filesystem/shell IPC.

### SystemDiff information hierarchy

```text
SystemDiff
└── Promise: See what changed on your Windows PC.
    ├── Ready
    │   ├── Start capture
    │   ├── Checks: Startup entries; Windows services; Scheduled tasks — Coming soon
    │   └── Trust: Local only · Read-only · No telemetry
    ├── Capturing
    │   ├── Before capture completed
    │   ├── Install, run, or change the app
    │   ├── Finish & Compare
    │   └── Cancel
    ├── Busy / recoverable error
    │   ├── Honest indeterminate activity or short explanation
    │   ├── Retry / Return
    │   └── Collapsed technical details
    └── Results
        ├── Confirmed change summary
        ├── Startup group
        ├── Windows Services group
        ├── Calm coverage notice
        ├── Optional exact Technical details
        └── New capture
```

### Tauri spike assessment and decision

Adopt Tauri 2.11.5 with React 19, TypeScript, and Vite. As of 2026-08-20 it is an actively maintained MIT/Apache-2.0 project, supports the current Rust toolchain, uses the system WebView, and provides the narrow Rust command boundary this repository needs. React is reasonable for future localization and contributor access, but this slice adds no router, global state library, design-system runtime, or large i18n framework.

Microsoft documents WebView2 support for Windows 10 SAC 1709 and later, so there is no known API-level blocker. Tauri notes that WebView2 is normally preinstalled only on Windows 10 1803 and later; a clean 1709 machine therefore cannot be assumed to have the runtime. This development slice targets machines with the Evergreen Runtime installed and does not claim installer/bootstrap coverage or fully validate the 1709 baseline. Formal distribution must later test missing-Runtime behavior and choose a Runtime installation strategy.

The local validated prerequisites are Rust 1.97.1 stable MSVC, Node 22.23.1/npm 12.0.1, VS 2022 MSVC tools 14.41, Windows SDK 22621, and WebView2 Runtime 151.0.4129.93. The project will use a locked local `@tauri-apps/cli` dev dependency rather than a global `cargo-tauri` installation.

Security decision:

- bundle only local Vite assets and set an explicit production CSP;
- one `main` window, no remote URLs, remote fonts/scripts/iframes, asset protocol, or global Tauri injection;
- no shell, filesystem, HTTP, dialog, opener, updater, or network plugin;
- expose only typed session commands with no path, URL, shell command, raw Snapshot JSON, or generic read/write input;
- use an explicit Tauri capability for the main window and restrict registered application commands at build time;
- run synchronous capture/diff work through `tauri::async_runtime::spawn_blocking` and enforce concurrency in Rust state, not only disabled buttons;
- render observed strings as React text nodes, never `dangerouslySetInnerHTML`;
- keep `asInvoker`; release builds use the Windows GUI subsystem so double-click does not flash a console.

## Constraints

- Production remains read-only, offline-first, non-elevated, and without telemetry, accounts, cloud, or network clients.
- Never execute or expand observed Registry/service command data.
- Frontend cannot supply an arbitrary filesystem path or receive a temporary evidence path.
- Temporary Snapshots are unredacted sensitive evidence. Use an application-local owned root, collision-safe create-new operations, exact known filenames, path-containment/reparse checks, and explicit cleanup. Do not recursively delete broad or unverified paths.
- Session state is single-active-session and backend-authoritative. Duplicate or invalid transitions return stable typed conflicts.
- `spawn_blocking` work cannot be promised to cancel immediately. Cancel discards a completed before capture; a currently executing native capture is allowed to finish safely before cleanup/state recovery.
- Preserve raw UTC in technical evidence. React may render the already validated timestamps in local time for ordinary view.
- Do not change Snapshot/Diff v1, collector behavior, CLI commands, Registry/service coverage, or existing CLI package contents.
- Keep the desktop Rust package and lockfile separate from the root Cargo workspace so Tauri dependencies do not enter CLI portable packaging metadata or `THIRD_PARTY_LICENSES.txt`. Validate it explicitly in CI. Path dependencies still reuse the shared Rust crates.
- Default CI must not write Registry data or require administrator privileges.
- Scheduled Tasks, history/baselines, settings, dashboard, charts, risk/signature/rules/AI, redaction/export, updater, installer/signing, and official Release are non-goals.

## Implementation steps

1. Create Issue #13, record this plan, create `feat/desktop-capture-workflow`, and accept ADR 0003 with the documented WebView2/runtime and security limits.
2. Add a locale-neutral, serializable desktop presentation model to `systemdiff-report`. It includes a contract version, raw UTC bounds, Rust-owned counts/change kinds, stable group/message/field IDs, display evidence, coverage notices, and on-demand technical text. Add deterministic semantic tests for Registry, Services, coverage, empty, fallback, and hostile evidence cases.
3. Scaffold `apps/desktop` with locked Tauri 2/React/TypeScript/Vite dependencies. Keep the Rust package outside the root workspace with its own lockfile and validation. Audit direct and resolved dependency licenses/security before claiming completion.
4. Implement a pure Rust desktop session state machine and storage layer. Use backend-created app-local session roots, bounded Snapshot serialization/loading, create-new exact filenames, cleanup on finish/cancel/new capture, and conservative startup recovery of only verified owned entries.
5. Expose narrow async Tauri commands for state, start, finish, cancel, and technical details. Perform capture/diff in a blocking worker, prohibit concurrent operations in Rust, and return a small typed recoverable error model.
6. Implement the three-state React UI with typed localization dictionaries for `en-US` and `zh-CN`, navigator-language selection, local timestamp display, an honest indeterminate busy state, duplicate-action prevention, calm coverage notices, and expandable technical evidence. Use restrained Fluent-inspired CSS with responsive light/dark support and no unused navigation.
7. Add a guarded mutation-only HKCU Run dogfood harness. It refuses an existing `SystemDiffDogfood` value, writes only the exact synthetic value under explicit test gates, removes only exact matching data, and is not linked into production Rust/Tauri code.
8. Update CI to validate npm lock install, TypeScript checks, frontend build, desktop Rust fmt/clippy/tests, and a Windows Tauri no-bundle build while preserving existing root Rust and CLI portable jobs. Do not publish a desktop artifact or official bundle in this PR.
9. Run the real desktop app on Windows. Verify Ready → Start → mutation → Finish → Results, no UI freeze, correct Registry Added evidence, exact cleanup, English and Chinese UI, release double-click/no-console behavior, and technical coverage. Capture a real screenshot only after this passes.
10. Update both READMEs, `apps/desktop/README.md`, architecture/roadmap/threat model, ADR 0003, and `PROJECT_STATE.md` with factual implemented/dev-only status and the real screenshot.
11. Run full local validation, an independent reviewer focused on product/security/lifecycle/semantics, and a stranger-first manual test. Fix High/Medium findings and only actionable Low findings within scope.
12. Inspect the final diff, use the `systemdiff-pr` workflow, commit, push the feature branch, create a PR closing #13, and wait for all required and desktop CI checks to pass. Do not merge.

## Affected files and modules

- `.agent/plans/desktop-ui-spike.md` and `.agent/PROJECT_STATE.md`
- `docs/adr/0003-desktop-stack.md`, `docs/architecture.md`, `docs/roadmap.md`, `docs/threat-model.md`
- `crates/systemdiff-report/src/lib.rs` and a focused presentation module/tests
- `crates/systemdiff-core/src/lib.rs` only for an equivalent current-stable Clippy compatibility fix discovered by remote CI
- `apps/desktop` frontend, Tauri backend, separate Cargo/npm lockfiles, and developer documentation
- `.github/workflows/ci.yml`
- a guarded test-only desktop dogfood script under `scripts/`
- `README.md`, `README.zh-CN.md`, and a real screenshot under `docs/assets/`

No Collector, Snapshot/Diff schema, rule, risk, CLI workflow, or portable CLI package content is expected to change.

## Test strategy

### Rust presentation

- Registry Added maps to the stable Startup group, Rust-owned Added semantics, decoded command, and exact location.
- Service Modified lists only fields that actually changed.
- One-sided evidence under partial coverage remains Inconclusive, confirmed count remains zero, and a calm Services visibility notice is emitted.
- Empty results still contain stable Startup and Windows Services groups/empty-state IDs.
- Unknown artifact fallback is visible and never panics or disappears.
- Control and bidi characters in evidence are display-hardened; exact technical evidence remains available.
- DTO semantic/message IDs and JSON field names are deterministic and localization-neutral.

### Session and storage

- Ready → busy → Capturing → busy → Results and New capture.
- Duplicate/illegal Start, Finish, Cancel, and technical-detail transitions.
- Capture, diff, validation, serialization, load, and cleanup failure recovery.
- Create-new collisions and bounded evidence input/output.
- Cancel, successful finish, new capture, startup recovery, and window-close cleanup.
- Owned-root containment, exact allowlist, unexpected entry, symlink/reparse-point, and cleanup-error behavior.
- Raw UTC is preserved and no frontend-supplied path enters storage.

### Frontend

- Typecheck and production Vite build are mandatory. Add only a lightweight lint/test tool if it does not materially expand dependencies.
- Manually verify Ready, scanning, Capturing, comparing, Results, empty results, Registry Added, Service Modified, Inconclusive/coverage, and recoverable error.
- Verify `en-US`, `zh-CN`, locale fallback, local-time rendering, responsive width, light/dark preference, keyboard focus, and busy-button disabling.

### Windows integration and dogfood

- Real window launch through Tauri and a release `--no-bundle` build.
- GUI Start → guarded HKCU `SystemDiffDogfood` mutation → GUI Finish → exact Added startup result → exact-data cleanup.
- No administrator privileges, production write API, CLI workflow, JSON, file picker, or console is required for the user path.
- Inspect PE subsystem/no-console and run the generated executable with installed WebView2.

### Commands

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets

npm ci --prefix apps/desktop
npm run check --prefix apps/desktop
npm run build --prefix apps/desktop

cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --all --check
cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets
npm --prefix apps/desktop run tauri -- build -- --no-bundle
```

The exact CI and manual dogfood commands will be recorded with observed output in Final validation. No success will be inferred.

## Risks

- **UI freeze or duplicate capture:** synchronous Win32 collectors can block. Use async commands plus `spawn_blocking` and an atomic Rust session transition before work begins.
- **Sensitive temp evidence left behind:** create only under an owned root, use create-new exact names, clean explicitly, and conservatively recover verified stale directories. Do not claim LocalAppData is encryption.
- **False removal in the UI:** presentation must consume `ChangeKind` and coverage from Rust and never reclassify in TypeScript.
- **WebView2 unavailable on old Windows:** Win10 1709 may require Evergreen Runtime. Keep this PR dev-only and do not claim clean-machine distribution support.
- **IPC/content escalation:** remote content, wide capabilities, generic paths, shell/fs/http plugins, or unescaped HTML would expand trust. Reject them and review the generated capability/CSP.
- **CLI artifact regression:** a Tauri workspace dependency graph can pollute existing CLI license/package checks. Use a separate desktop Cargo lock/workspace boundary and explicitly rerun existing package/download verification.
- **Dependency size/advisories:** audit actual Cargo/npm lockfiles; do not hide High/Medium issues behind a generic Tauri allowlist.
- **Crash during native capture:** blocking native work is not instantly abortable. Do not promise it; serialize session operations and finish cleanup when the task returns.
- **Misleading screenshot:** take it only from the validated real app with current Registry/Services capability and Scheduled Tasks visibly marked Coming soon.

## Rollback and compatibility

The desktop app is additive and development-only. Reverting its app directory, presentation DTO, documentation, and desktop CI leaves the CLI, Collectors, Snapshot/Diff schema, fixtures, and existing portable artifact unchanged. There is no user database or migration. Ephemeral session files are not a supported interchange format and are deleted after use.

The new presentation IPC model begins at contract version 1 but is not the public Snapshot/Diff wire schema. Future desktop compatibility changes must be deliberate and frontend/backend versions ship together. ADR 0003 records the durable stack choice and may be superseded rather than rewritten if the desktop architecture later changes.

## Progress

- [x] 2026-08-20: Synced clean `main` at `ff51d9433bd605406543f59ec8a10054fb4e442a` with `origin/main`.
- [x] 2026-08-20: Completed six-project stranger-first UI/architecture research and recorded adopted/rejected patterns above.
- [x] 2026-08-20: Verified Tauri 2.11.5/WebView2 compatibility, security constraints, current dependency state, and local Windows prerequisites.
- [x] 2026-08-20: Mapped capture, diff, report, session, CI, fixtures, packaging, and documentation boundaries.
- [x] 2026-08-20: Created GitHub Issue #13 for the guided desktop workflow.
- [x] 2026-08-20: Created `feat/desktop-capture-workflow` and implemented the locale-neutral presentation contract in `systemdiff-report`.
- [x] 2026-08-21: Implemented and tested the backend-only session storage/state machine, root lock, conservative recovery, and five no-argument Tauri commands.
- [x] 2026-08-21: Implemented and validated the bilingual React UI, responsive Fluent-inspired styling, local-time display, honest busy/error states, and on-demand technical evidence.
- [x] 2026-08-21: Completed three real Windows guarded dogfood runs, exact Registry cleanup, GUI subsystem verification, and a real English Results screenshot.
- [x] 2026-08-26: Completed final local validation and independent review with no remaining High/Medium findings.
- [ ] Commit, push, create the PR, and confirm authoritative remote CI.

## Discoveries

- The current CLI package license verifier considers all non-workspace packages returned by root Cargo metadata. Adding Tauri to the root workspace would contaminate CLI-only dependency notices even if the CLI does not link Tauri. A separate desktop Cargo workspace/lockfile is the smallest non-regressive boundary for this slice.
- Windows Services capture is intentionally Partial because successful SCM enumeration can omit inaccessible services. A calm coverage notice is expected during normal desktop use, not an exceptional error state.
- Tauri's `spawn_blocking` work generally cannot be aborted after it starts. UI Cancel semantics must not claim immediate native cancellation.
- WebView2 supports Windows 10 1709 at the platform level, but only 1803+ is normally preinstalled. A naked development executable is not a clean-machine distribution strategy.
- `localhost` and a Vite server explicitly bound to `127.0.0.1` are not necessarily the same endpoint on an IPv6-preferring host; keep the Tauri development URL and Vite bind address exactly aligned.
- A permanent zero-byte root lock file is intentionally not session evidence. The process holds an advisory file lock for its lifetime so a second instance cannot run stale-session recovery against an active capture.
- GitHub's `stable` channel advanced to Rust/Clippy 1.98.0 during final validation and enabled `chunks_exact_to_as_chunks` under `-D warnings`. Replacing one existing fixed-width decode iterator with Clippy's equivalent `as_chunks::<4>()` form keeps stable CI green without pinning an obsolete toolchain or changing decoding semantics.

## Decisions

- Accept ADR 0003: Tauri 2 + React + TypeScript is the desktop stack, subject to the local-only, narrow-IPC, installed-WebView2 boundary in this plan.
- Keep desktop session orchestration inside the desktop Rust backend until another consumer exists; do not add a general session crate.
- Put locale-neutral presentation semantics in `systemdiff-report`, the existing report/presentation owner.
- Keep the desktop Rust package outside the root Cargo workspace and validate it explicitly so the existing CLI artifact remains unchanged.
- Keep frontend localization as small typed dictionaries. Do not add a router, state library, design system, or general i18n framework.
- Persist ephemeral before/after evidence only in a backend-owned app-local directory because the requested acceptance explicitly requires collision-safe local storage and cleanup. Do not expose paths to React or add history.
- Use a real screenshot rather than a GIF if animation tooling would enlarge scope.

## Final validation

Local toolchain used on 2026-08-21:

- `rustc 1.97.1 (8bab26f4f 2026-07-14)`, Cargo 1.97.1, rustfmt 1.9.0-stable, Clippy 0.1.97;
- Node.js 22.23.1 and npm 12.0.1;
- WebView2 Runtime 151.0.4129.93 in the validated development environment.

Completed local checks:

- `npm ci --prefix apps/desktop`: passed; 152 packages installed and 153 audited by the install command.
- `npm run check/lint/build --prefix apps/desktop`: all passed after review fixes; the final Vite build emitted a 216.95 kB JavaScript asset (67.07 kB gzip) and 9.41 kB CSS asset (2.99 kB gzip).
- `npm audit --prefix apps/desktop --audit-level=low --json`: zero info/low/moderate/high/critical vulnerabilities in the locked npm graph.
- root `cargo fmt --all --check` and `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- root `cargo test --locked --workspace --all-targets`: 119 passed, zero failed.
- after remote Clippy 1.98 exposed one new lint in pre-existing core decode code, the equivalent one-line iterator update passed root fmt, Clippy, and all 119 tests locally; authoritative 1.98 confirmation is delegated to the replacement GitHub run.
- desktop Cargo fmt/Clippy checks: passed; desktop `cargo test --locked ... --all-targets`: 16 passed, zero failed, including managed bootstrap errors, storage-initialization routing, visible cleanup state, and normal/deferred shutdown semantics.
- `npm --prefix apps/desktop run tauri -- build -- --no-bundle`: passed; produced `apps/desktop/src-tauri/target/release/systemdiff-desktop.exe`, 8,930,304 bytes. Direct PE parsing reported PE32+ magic `0x020B` and subsystem `2` (Windows GUI, no console subsystem).
- CLI human Diff, JSON Diff, and Collector-list smoke commands: passed.
- existing CLI package and downloaded-artifact-style verifier: passed. It retained the exact two-file outer artifact and five-file portable ZIP, AMD64/asInvoker/uiAccess=false/static-CRT import boundary, all CLI modes, and a real read-only Snapshot smoke.
- desktop Cargo metadata contained 425 registry packages across its all-platform locked graph, with zero missing license metadata and zero Git dependencies. `cargo-audit` was not installed, so no RustSec success is claimed; nested Cargo and npm Dependabot entries were added.
- guarded desktop dogfood script parsed successfully; Markdown relative-link check passed across 33 files; secret/local-path pattern scans found no hits; `git diff --check` passed after final review fixes.

Real Windows dogfood (run three times):

1. launched the release GUI directly without a console and confirmed Ready was understandable in both Chinese and English;
2. used the GUI **Start capture** action and reached the responsive Capturing state;
3. separately used the dual-gated test-only harness to add exact `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value `SystemDiffDogfood` with data `cmd.exe /d /c exit 0`;
4. used the GUI **Finish & Compare** action and saw exactly `1 change detected`, `SystemDiffDogfood`, `Added to your startup`, the expected location and command, no observed Service configuration changes, and a calm current-account coverage notice;
5. expanded Technical details and confirmed raw UTC, one Added/zero Removed/zero Modified/zero Inconclusive, exact Registry evidence, all Registry scope coverage, conservative Services coverage, diagnostics, and completed desktop-session evidence cleanup;
6. the harness removed only the exact matching value and verified cleanup. A subsequent Registry read confirmed the value absent. After application exit the session root contained no session directory, only its zero-byte lock file.

The real screenshot is `docs/assets/systemdiff-desktop-results.jpg` (982×752, 54,990 bytes) and contains only the guarded synthetic evidence. Both READMEs use it while clearly labeling the desktop as source-built development software, not a distributed Release.

The independent reviewer initially found four Medium lifecycle/UX issues and one residual Medium bootstrap-error routing issue. The implementation now surfaces cleanup-pending state in the ordinary UI, avoids an incorrect no-change claim for inconclusive-only results, synchronously guards duplicate commands, cleans stable sessions on normal exit, and treats cached bootstrap failures as restart-required rather than offering a false retry. Regression tests cover the fixes. The final independent re-review reported no High or Medium findings; the only remaining Low is a visible but non-obscuring pointer halo in the real screenshot. Authoritative PR checks remain pending and will be appended before the plan is complete.
