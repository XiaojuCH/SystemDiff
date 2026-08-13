# Project state

Last updated: 2026-08-13

## Current phase

The public foundation, pre-collector hardening, and first real Windows product slice are on `main`. PR #6 is merged: SystemDiff can capture Registry Run/RunOnce startup evidence, write a Snapshot, and compare a real before/after pair. The current `feat/human-readable-diff` work for Issue #7 is the first product-presentation pass: default human-readable Diff output, an explicit technical text mode, and a truthful stranger-first repository front page. v0.1 remains incomplete.

## Implemented components

- Public repository `XiaojuCH/SystemDiff` with Apache-2.0 licensing, contributor/security documentation, issue forms, required Windows/Ubuntu CI, Dependabot, Secret Scanning, Push Protection, Private Vulnerability Reporting, and an active `main` ruleset.
- Rust workspace boundaries for versioned evidence, Windows collection, deterministic Diff, rules, reports, and CLI composition.
- Draft v1 Snapshot and Diff documents with bounded/header-first Snapshot input, strict UTC timestamps, deterministic serialization, and deliberate Collector-version compatibility checks.
- Coverage-aware comparison: incomplete, unavailable, unsupported, or permission-denied scope coverage cannot silently become a Removed finding.
- `windows.registry.startup` v1 using query-only Win32 Registry APIs, explicit Registry views, scoped diagnostics, bounded mutation/resource handling, strict native-data decoding, lossless UTF-16 value names, and complete-value SHA-256.
- `systemdiff snapshot -o <path>` with canonical UTC metadata, bounded serialization, and create-new output semantics.
- Default human-readable Registry Diff output, explicit `--technical` evidence output, and unchanged `--json` machine output on the current Issue #7 feature branch.
- Registry-only synthetic before/after fixtures and a dual-gated test-only real HKCU E2E. The real E2E observed exactly one expected Added startup value, zero Removed changes, and verified exact-data cleanup; production Rust has no Registry write path.
- A truthful Registry-only README demo whose transcript is regression-tested and whose static visual is derived from that exact output.
- Project-scoped Codex agents, three repeated-workflow skills, living ExecPlans, architecture/format/Collector/threat-model documentation, and synthetic cross-platform tests.

## Known limitations

- Only Registry Run/RunOnce collection is implemented. Services, Scheduled Tasks, rules/explanations, sanitization, installation/package delivery, and the desktop app are unavailable.
- There is no official binary release. Current users must build the development CLI from source.
- The current minimum is Windows 10 version 1709 or Windows Server 2016 version 1709. ARM64 v1 collects HKCU Shared scopes but reports HKLM alternate-view coverage as unsupported until those views are represented and tested.
- Snapshot files are unredacted and can contain usernames in paths, command strings, software details, and other sensitive host evidence. They must be reviewed before sharing.
- Draft v0.1 diffs assume the same Windows installation and the same user/principal context. Cross-host and cross-user identity are intentionally out of scope.
- Registry lookup is case-insensitive, but Collector v1 identity uses exact UTF-16 units because no documented durable cross-platform canonical token is available. A returned casing change can appear as a visible Removed + Added pair.
- The desktop app is a proposed future boundary, not a generated Tauri application.
- No dedicated private Code of Conduct reporting channel is published. GitHub Private Vulnerability Reporting is only for product security reports.
- No CODEOWNERS file is committed during the solo-maintainer stage.

## Decisions affecting current work

- SystemDiff remains offline-first and read-only; evidence is never executed, remediated, or uploaded by default.
- Core and JSON identifiers remain language-neutral. Human presentation is layered over exact technical text and versioned JSON rather than replacing evidence.
- Default terminal output is plain text with no ANSI/color dependency. Snapshot-derived control characters are escaped before display.
- Technical text rendering receives the validated before/after Snapshots so Collector versions and diagnostics are reported from evidence rather than inferred; the Diff v1 JSON schema remains unchanged.
- Snapshot files are capped at 64 MiB at the CLI boundary. Registry capture limits are 8 MiB native data per value, 32 MiB retained name-and-value evidence per Collector run, and 4,096 values per scope. These are SystemDiff resource limits, not Windows platform limits.
- Unknown cross-version comparisons for the same Collector ID are rejected by default. A future verified compatible pair remains possible, but no migration framework exists.
- Registry views, RunOnce prefixes, and value names retain their documented/evidence semantics; no command parsing, environment expansion, executable resolution, signature check, or risk inference occurs.
- Normal changes to `main` go through pull requests and the two required checks: `Rust (windows-latest)` and `Rust (ubuntu-latest)`.

## Next milestone

Finish review and CI for Issue #7 without merging automatically. After that, plan the lightest credible portable developer preview so strangers can try the real Registry workflow without a Rust toolchain. Windows Services remains the next Collector candidate, but it is not started in the presentation PR.

## Major unresolved questions

- What genuine, monitored private channel should receive Code of Conduct reports?
- What publisher-signing and portable-build process can support a trustworthy first Windows developer preview?
- What explicitly versioned identity upgrade should eventually address Registry value-name casing without hiding raw evidence or coupling Diff to mutable Windows NLS behavior?
- What bounded/archive policy should apply to Scheduled Task raw XML before that Collector is implemented?
- What minimum supported Rust version will be tested and documented?
- Should the first desktop spike confirm React/Vite or compare one smaller frontend alternative before accepting ADR 0003?
