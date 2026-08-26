# Project state

Last updated: 2026-08-26

## Current phase

The public foundation, Registry startup and Windows Services Collectors, human-readable Diff, and portable Windows x64 CLI Developer Preview are on `main`. The current `feat/desktop-capture-workflow` work implements Issue #13's first guided desktop vertical slice while reusing the existing Rust capture, Diff, and report semantics. It remains a source-built development app rather than a distributed desktop preview. v0.1 remains incomplete.

## Implemented components

- Public repository `XiaojuCH/SystemDiff` with Apache-2.0 licensing, contributor/security documentation, issue forms, required Windows/Ubuntu CI, Dependabot, Secret Scanning, Push Protection, Private Vulnerability Reporting, and an active `main` ruleset.
- Rust workspace boundaries for versioned evidence, Windows collection, deterministic Diff, rules, reports, and CLI composition.
- Draft v1 Snapshot and Diff documents with bounded/header-first Snapshot input, strict UTC timestamps, deterministic serialization, and deliberate Collector-version compatibility checks.
- Coverage-aware comparison: incomplete, unavailable, unsupported, or permission-denied scope coverage cannot silently become a Removed finding.
- `windows.registry.startup` v1 using query-only Win32 Registry APIs, explicit Registry views, scoped diagnostics, bounded mutation/resource handling, strict native-data decoding, lossless UTF-16 value names, and complete-value SHA-256.
- `windows.services` v1 uses query-only SCM APIs, atomic selected-field observations, strict UTF-16 handling, bounded resources/mutation reads, and permanently conservative current-token partial coverage.
- `systemdiff snapshot -o <path>` with canonical UTC metadata, bounded serialization, and create-new output semantics.
- Default human-readable Registry and Services Diff output, explicit `--technical` evidence output, and unchanged `--json` machine output.
- A release-mode Windows x64 Developer Preview pipeline with packaging-only static MSVC CRT, an explicit `asInvoker` manifest, exact package/checksum verification, and a later artifact-download smoke job.
- Registry-only synthetic before/after fixtures and a dual-gated test-only real HKCU E2E. The real E2E observed exactly one expected Added startup value, zero Removed changes, and verified exact-data cleanup; production Rust has no Registry write path.
- A truthful Registry-only README demo whose transcript is regression-tested and whose static visual is derived from that exact output.
- On the current Issue #13 branch, a Tauri 2 + React/TypeScript desktop development app provides a single-session Ready → Capturing → Results workflow in `en-US` and `zh-CN`. Rust owns capture, Diff semantics, coverage, technical evidence, session state, bounded backend-only temporary storage, and cleanup; React owns localization and layout.
- The desktop ordinary presentation uses a versioned locale-neutral DTO, stable semantic IDs, fixed Startup/Windows Services groups, calm coverage notices, and on-demand exact technical text without exposing Snapshot paths or raw documents to the web frontend.
- Project-scoped Codex agents, three repeated-workflow skills, living ExecPlans, architecture/format/Collector/threat-model documentation, and synthetic cross-platform tests.

## Known limitations

- Registry Run/RunOnce and Windows Services are implemented on `main`. Scheduled Tasks, rules/explanations, sanitization, and an installer remain unavailable.
- There is no official binary Release or Authenticode signing. The Developer Preview is an expiring GitHub Actions artifact that requires GitHub sign-in, and clean-machine validation remains a gate for an official alpha.
- The current minimum is Windows 10 version 1709 or Windows Server 2016 version 1709. ARM64 v1 collects HKCU Shared scopes but reports HKLM alternate-view coverage as unsupported until those views are represented and tested.
- Snapshot files are unredacted and can contain usernames, service accounts, paths/arguments, descriptions, software details, and other sensitive host evidence. They must be reviewed before sharing.
- Draft v0.1 diffs assume the same Windows installation and the same user/principal context. Cross-host and cross-user identity are intentionally out of scope.
- Registry lookup is case-insensitive, but Collector v1 identity uses exact UTF-16 units because no documented durable cross-platform canonical token is available. A returned casing change can appear as a visible Removed + Added pair.
- SCM service-name comparison is also case-insensitive. Services v1 preserves exact returned UTF-16 evidence and accepts the same conservative casing-only false-split limitation rather than applying unverified Unicode/NLS normalization.
- The desktop app is currently development-only: there is no signed/bundled installer, desktop CI artifact, updater, history, import/export, or clean-machine WebView2 bootstrap. Windows 10 1709 is not yet a validated desktop distribution baseline; this slice requires an installed WebView2 Runtime.
- Desktop session Snapshots are unredacted sensitive local evidence. Normal finish/cancel/new-capture/stable-exit paths remove verified files and Results surface cleanup failure, while in-flight exit or crash recovery is conservative and may leave local evidence for the next startup.
- No dedicated private Code of Conduct reporting channel is published. GitHub Private Vulnerability Reporting is only for product security reports.
- No CODEOWNERS file is committed during the solo-maintainer stage.

## Decisions affecting current work

- SystemDiff remains offline-first and read-only; evidence is never executed, remediated, or uploaded by default.
- Core and JSON identifiers remain language-neutral. Human presentation is layered over exact technical text and versioned JSON rather than replacing evidence.
- Default terminal output is plain text with no ANSI/color dependency. Snapshot-derived control characters are escaped before display.
- Technical text rendering receives the validated before/after Snapshots so Collector versions and diagnostics are reported from evidence rather than inferred; the Diff v1 JSON schema remains unchanged.
- Snapshot files are capped at 64 MiB at the CLI boundary. Registry capture limits are 8 MiB native data per value, 32 MiB retained name-and-value evidence per Collector run, and 4,096 values per scope. These are SystemDiff resource limits, not Windows platform limits.
- Services capture limits are 4,096 services, 32 KiB retained UTF-16 evidence per service, 16 MiB per Collector, 64 enumeration pages, and documented 256 KiB/8 KiB native enumeration/query buffer ceilings. The real `current_token.win32` scope is always partial because SCM can silently omit services inaccessible for status queries.
- Unknown cross-version comparisons for the same Collector ID are rejected by default. A future verified compatible pair remains possible, but no migration framework exists.
- Registry views, RunOnce prefixes, and value names retain their documented/evidence semantics; no command parsing, environment expansion, executable resolution, signature check, or risk inference occurs.
- Normal changes to `main` go through pull requests and the two required checks: `Rust (windows-latest)` and `Rust (ubuntu-latest)`.
- Developer Preview packaging runs only after those gates on trusted upstream `push` events, uploads an exact ZIP/checksum pair for 14 days, and verifies the downloaded artifact in a fresh Windows job. Normal artifacts come from `main`; the exact active feature branch may temporarily produce a clearly named candidate for pre-merge validation. Fork pull requests cannot enter this upload path.
- The portable build alone uses static MSVC CRT and remains at version `0.0.0`. The package is commit-linked and hashed but is not claimed to be reproducible, signed, released, or permanently downloadable.
- ADR 0003 accepts Tauri 2 + React/TypeScript for the desktop. The app has a separate Cargo workspace/lockfile so Tauri dependencies cannot pollute the CLI package graph, and CI validates frontend and desktop Rust work explicitly.
- Desktop IPC is local-only and capability-scoped: no shell, filesystem, HTTP, dialog, opener, updater, remote-content, arbitrary-path, or Registry/service write command is exposed. Synchronous capture runs on blocking workers behind a Rust-owned single-session state machine.

## Next milestone

Open and validate the PR for Issue #13's independently reviewed guided desktop development workflow, then decide whether it is ready to merge. Scheduled Tasks remains intentionally unstarted; desktop packaging, clean-machine WebView2 handling, signing, and a permanent public download remain future release work.

## Major unresolved questions

- What genuine, monitored private channel should receive Code of Conduct reports?
- What publisher-signing, clean-machine validation, and immutable Release process should support the first official Windows alpha?
- What explicitly versioned identity upgrade should eventually address Registry value-name casing without hiding raw evidence or coupling Diff to mutable Windows NLS behavior?
- What bounded/archive policy should apply to Scheduled Task raw XML before that Collector is implemented?
- What minimum supported Rust version will be tested and documented?
- What desktop packaging and WebView2 Runtime strategy should be validated before offering a public desktop artifact?
