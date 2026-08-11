# Project state

Last updated: 2026-08-11

## Current phase

The public repository foundation is complete. The initial commit is on `origin/main`, the first GitHub CI run passed, and the repository's baseline security and branch rules are active. The current phase is pre-collector hardening before implementation of the first real Windows Collector.

## Implemented components

- Durable product, architecture, roadmap, threat-model, and ADR documentation.
- Project-scoped Codex agents and three repeated-workflow skills.
- Public GitHub repository `XiaojuCH/SystemDiff`, community templates, baseline CI, and dependency update policy.
- A draft v1 domain/schema skeleton, deterministic diff boundary, JSON/terminal reporting boundary, rule interface, Windows collector descriptors, and CLI fixture-diff path.
- Root `Cargo.lock` generated with Cargo 1.97.1; CI mirrors the local format, Clippy, test, and CLI fixture smoke commands.
- The workspace passes rustfmt, Clippy with warnings denied, all-target workspace tests, and the three documented CLI fixture/status commands on stable `x86_64-pc-windows-msvc` (`rustc 1.97.1`).
- The first GitHub CI run passed on Windows and Ubuntu. The active `main` ruleset requires pull requests, resolved review threads, and the `Rust (windows-latest)` and `Rust (ubuntu-latest)` checks; it blocks deletion and non-fast-forward updates while retaining an explicit maintainer bypass.
- GitHub Secret Scanning, Push Protection, and Private Vulnerability Reporting are enabled.
- Registry evidence keeps the native type code, validated typed decode status/value, a full-content SHA-256, and an optional validated 4 KiB lowercase-hex raw prefix rather than assuming every value is a UTF-16LE string.
- The bootstrap foundation has passed real-toolchain validation and independent architecture, security, and maintainability review.

## Known limitations

- No Windows collector calls an operating-system API yet.
- `systemdiff snapshot` is intentionally unavailable; the end-to-end MVP is not complete.
- The desktop app is a documented future boundary, not a generated Tauri application.
- Redaction metadata exists in the schema, but sanitization is not implemented.
- The bootstrap CLI currently reads an entire JSON file before validation; header-first version routing, RFC 3339 timestamp validation, and input resource limits are not implemented.
- Draft fixtures and wire types may change before v0.1; after v0.1, v1 compatibility becomes a release obligation.
- Draft v0.1 diffs assume the same Windows installation and the same user/principal context. Cross-host and cross-user identity are out of scope; no SID hash, machine token, or identity framework exists.
- No dedicated private Code of Conduct reporting channel is published. GitHub Private Vulnerability Reporting is available only for product security reports.
- No CODEOWNERS file is committed during the solo-maintainer stage.

## Decisions affecting current work

- Rust owns the shared domain, diff, rule, and reporting logic; Windows API access is isolated in one crate.
- Tauri 2 with React and TypeScript is proposed for v0.2 and will be validated after the CLI MVP.
- Snapshot and diff JSON are separately versioned documents with deterministic serialization expectations.
- Collector failures and privilege limitations are recorded per collector/scope and must not invalidate unrelated evidence.
- Unknown cross-version comparisons for the same Collector ID are rejected by default; future explicitly verified compatible version pairs remain possible, but no compatibility framework exists yet.
- v0.1 comparison is limited to before/after snapshots from the same Windows installation and user/principal context.
- SystemDiff remains offline-first and read-only; evidence is never executed or remediated.
- Apache-2.0 is the repository license.
- Normal changes to `main` go through pull requests and the active required checks; maintainer bypass is reserved for exceptional recovery.

## Next milestone

Complete the focused pre-collector hardening work: bounded snapshot input, document-header routing, UTC RFC 3339 validation, explicit Registry view semantics, and correct structured RunOnce prefix evidence. Only then specify and implement the first real Registry Run/RunOnce Collector and `snapshot` CLI path.

## Major unresolved questions

- What genuine, monitored private channel should receive Code of Conduct reports?
- What minimum supported Windows versions and architectures will v0.1 promise?
- Which Registry value types or decode failures warrant including the optional raw prefix rather than only typed evidence plus the full-content hash?
- What bounded/archive policy should apply to Scheduled Task raw XML before that Collector is implemented?
- What minimum supported Rust version will be tested and documented?
- Should the first desktop spike confirm React/Vite or compare one smaller frontend alternative before accepting ADR 0003?
