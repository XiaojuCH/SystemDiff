# Project state

Last updated: 2026-08-11

## Current phase

Bootstrap foundation validated and stabilized with a real Rust toolchain. Architecture, collaboration workflow, community scaffolding, lockfile, deterministic fixtures, and the minimal Rust workspace are ready for initial-commit review; real Windows collection remains the next engineering phase.

## Implemented components

- Durable product, architecture, roadmap, threat-model, and ADR documentation.
- Project-scoped Codex agents and three repeated-workflow skills.
- GitHub community templates, baseline CI design, and dependency update policy.
- A draft v1 domain/schema skeleton, deterministic diff boundary, JSON/terminal reporting boundary, rule interface, Windows collector descriptors, and CLI fixture-diff path.
- Root `Cargo.lock` generated with Cargo 1.97.1; CI mirrors the local format, Clippy, test, and CLI fixture smoke commands.
- The workspace passes rustfmt, Clippy with warnings denied, all-target workspace tests, and the three documented CLI fixture/status commands on stable `x86_64-pc-windows-msvc` (`rustc 1.97.1`).
- Registry evidence keeps the native type code, validated typed decode status/value, a full-content SHA-256, and an optional validated 4 KiB lowercase-hex raw prefix rather than assuming every value is a UTF-16LE string.
- Final independent initial-commit review reports no remaining high- or medium-severity findings.

## Known limitations

- No Windows collector calls an operating-system API yet.
- `systemdiff snapshot` is intentionally unavailable; the end-to-end MVP is not complete.
- The desktop app is a documented future boundary, not a generated Tauri application.
- Redaction metadata exists in the schema, but sanitization is not implemented.
- The bootstrap CLI currently reads an entire JSON file before validation; header-first version routing, RFC 3339 timestamp validation, and input resource limits are not implemented.
- Draft fixtures and wire types may change before v0.1; after v0.1, v1 compatibility becomes a release obligation.
- No GitHub remote, repository owner, private security contact, or conduct-reporting contact has been configured.
- No CODEOWNERS file is committed until a real repository owner/team with write access is known.

## Decisions affecting current work

- Rust owns the shared domain, diff, rule, and reporting logic; Windows API access is isolated in one crate.
- Tauri 2 with React and TypeScript is proposed for v0.2 and will be validated after the CLI MVP.
- Snapshot and diff JSON are separately versioned documents with deterministic serialization expectations.
- Collector failures and privilege limitations are recorded per collector/scope and must not invalidate unrelated evidence.
- Unknown cross-version comparisons for the same Collector ID are rejected by default; future explicitly verified compatible version pairs remain possible, but no compatibility framework exists yet.
- SystemDiff remains offline-first and read-only; evidence is never executed or remediated.
- Apache-2.0 is the initial repository license choice, pending maintainer confirmation before public launch.

## Next milestone

After maintainer approval and the initial commit, specify and implement the registry Run/RunOnce Collector and real `snapshot` CLI path behind deterministic data-source abstractions, including 32/64-bit Registry view coverage, permission/partial outcomes, fixtures, and non-elevated tests.

## Major unresolved questions

- What GitHub organization/user will own the public repository and future CODEOWNERS entries?
- What private contact should receive security and Code of Conduct reports?
- What minimum supported Windows versions and architectures will v0.1 promise?
- Which Registry value types or decode failures warrant including the optional raw prefix rather than only typed evidence plus the full-content hash?
- What minimum supported Rust version will be tested and documented?
- Should the first desktop spike confirm React/Vite or compare one smaller frontend alternative before accepting ADR 0003?
