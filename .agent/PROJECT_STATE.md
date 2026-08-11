# Project state

Last updated: 2026-08-11

## Current phase

The public repository foundation and pre-collector hardening are complete. The first real product slice is implemented on the `feat/registry-startup-snapshot` branch: the read-only CLI captures Registry Run/RunOnce startup evidence and compares before/after Snapshots. Issue #5 is in final review and CI preparation; v0.1 remains incomplete.

## Implemented components

- Durable product, architecture, roadmap, threat-model, and ADR documentation.
- Project-scoped Codex agents and three repeated-workflow skills.
- Public GitHub repository `XiaojuCH/SystemDiff`, community templates, baseline CI, and dependency update policy.
- A draft v1 domain/schema, deterministic diff boundary, JSON/terminal reporting boundary, rule interface, Registry startup Collector, Snapshot assembler, and CLI capture/diff path.
- Root `Cargo.lock` generated with Cargo 1.97.1; CI mirrors the local format, Clippy, test, and CLI fixture smoke commands.
- The workspace passes rustfmt, Clippy with warnings denied, all-target workspace tests, and the three documented CLI fixture/status commands on stable `x86_64-pc-windows-msvc` (`rustc 1.97.1`).
- The first GitHub CI run passed on Windows and Ubuntu. The active `main` ruleset requires pull requests, resolved review threads, and the `Rust (windows-latest)` and `Rust (ubuntu-latest)` checks; it blocks deletion and non-fast-forward updates while retaining an explicit maintainer bypass.
- GitHub Secret Scanning, Push Protection, and Private Vulnerability Reporting are enabled.
- Registry evidence keeps the native type code, validated typed decode status/value, a full-content SHA-256, and an optional validated 4 KiB lowercase-hex raw prefix rather than assuming every value is a UTF-16LE string.
- The CLI rejects Snapshot files larger than 64 MiB before full decoding, bounds the actual read, and routes `document_type`/`schema_version` before constructing Snapshot v1.
- `captured_at` uses standards-based RFC 3339 parsing and accepts only known UTC represented by `Z` or `+00:00`; input evidence remains an unchanged wire string.
- Registry startup evidence distinguishes Run from RunOnce, validates structured `!`/`*` semantics against the complete raw value name, and preserves prefixed names in identity. Registry view labels have explicit, process-bitness-independent acquisition semantics.
- `windows.registry.startup` v1 uses `RegOpenKeyExW`, `RegQueryInfoKeyW`, and `RegEnumValueW` with query-only access, RAII-owned keys, explicit x64 Registry32/Registry64 passes, HKCU Shared scopes, bounded mutation retry, scoped diagnostics, strict native-data decoding, and complete-byte SHA-256.
- Registry value names use a tagged lossless UTF-16 representation. Unnamed/default values remain ordinary evidence; invalid UTF-16 never passes through lossy replacement.
- `systemdiff snapshot -o <path>` emits canonical UTC, unredacted draft-v1 JSON through a 64 MiB capped serializer and create-new output semantics. Existing files are never overwritten.
- SystemDiff capture budgets are 8 MiB native data per Registry value, 32 MiB retained native name-and-value evidence across the Registry startup Collector, and 4,096 values per scope. Over-limit evidence degrades only the affected scope and preserves complete siblings.
- The dual-gated test-only HKCU Run E2E produced exactly one expected Added change and zero Removed changes, then verified exact-data guarded cleanup and deletion of temporary Snapshot files. Default CI and production Rust contain no Registry write path.
- The bootstrap foundation has passed real-toolchain validation and independent architecture, security, and maintainability review.

## Known limitations

- Only Registry Run/RunOnce collection is implemented. Services, Scheduled Tasks, rules/explanations, sanitization, installation/package delivery, and the desktop app are unavailable; the end-to-end v0.1 MVP is not complete.
- The current minimum is Windows 10 version 1709 or Windows Server 2016 version 1709. ARM64 v1 collects HKCU Shared scopes but explicitly reports HKLM alternate-view coverage as unsupported until those view semantics are represented and tested.
- Windows Registry value lookup is case-insensitive, but Collector v1 identity hashes exact UTF-16 code units because there is no documented durable canonical token for independent cross-platform Snapshot comparison. If enumerated display casing changes, v1 may expose a visible false Removed + Added pair; changing this requires a new Collector version.
- Registry startup Collector v1 emits no raw value-data prefixes. Complete native-byte hashes and typed/decode status support deterministic comparison without duplicating potentially sensitive bytes.
- The desktop app is a documented future boundary, not a generated Tauri application.
- Redaction metadata exists in the schema, but sanitization is not implemented.
- The 64 MiB file boundary does not yet impose separate object-count, string-size, nesting-depth, or per-artifact count limits, and parser fuzzing is not configured. A streaming parser is intentionally absent.
- Draft fixtures and wire types may change before v0.1; after v0.1, v1 compatibility becomes a release obligation.
- Draft v0.1 diffs assume the same Windows installation and the same user/principal context. Cross-host and cross-user identity are out of scope; no SID hash, machine token, or identity framework exists.
- No dedicated private Code of Conduct reporting channel is published. GitHub Private Vulnerability Reporting is available only for product security reports.
- No CODEOWNERS file is committed during the solo-maintainer stage.

## Decisions affecting current work

- Rust owns the shared domain, diff, rule, and reporting logic; Windows API access is isolated in one crate.
- Tauri 2 with React and TypeScript is proposed for v0.2 and will be validated after the CLI MVP.
- Snapshot and diff JSON are separately versioned documents with deterministic serialization expectations.
- Snapshot files are capped at 64 MiB at the CLI boundary and routed by a minimal core header before v1 body construction; no configurable resource policy or migration registry exists.
- Valid read-time `captured_at` values must be RFC 3339 known UTC using `Z` or `+00:00`; future SystemDiff writers will emit canonical `Z`.
- Collector failures and privilege limitations are recorded per collector/scope and must not invalidate unrelated evidence.
- Unknown cross-version comparisons for the same Collector ID are rejected by default; future explicitly verified compatible version pairs remain possible, but no compatibility framework exists yet.
- `RegistryView::Shared` follows Microsoft's shared-key model; `Registry32` and `Registry64` require their explicit WOW64 selectors; `Native` is reserved for a sole view where no WOW alternate logical view exists.
- RunOnce `!` means deletion is deferred until after the command runs, `*` means the entry runs in Safe Mode, and undocumented combined/repeated forms are retained without inferred behavior. Complete value names remain identity-bearing evidence.
- Registry startup Collector v1 applies the maintainer-approved 8 MiB/value-data, 32 MiB retained name-and-value evidence/Collector, and 4,096 values/scope product limits; these are not Windows Registry platform limits.
- The production CLI and Collector API remain read-only. The only Registry write is a dual-gated, test-only, HKCU-only E2E harness with refuse-existing and exact-data guarded cleanup behavior.
- Windows 10 version 1709 and Windows Server 2016 version 1709 are the current minimum supported collection platforms. ARM64 HKLM alternate Registry views remain explicitly unsupported in Collector v1 while HKCU Shared collection continues.
- v0.1 comparison is limited to before/after snapshots from the same Windows installation and user/principal context.
- SystemDiff remains offline-first and read-only; evidence is never executed or remediated.
- Apache-2.0 is the repository license.
- Normal changes to `main` go through pull requests and the active required checks; maintainer bypass is reserved for exceptional recovery.

## Next milestone

Complete independent review and both required GitHub checks for Issue #5, then merge only with separate maintainer authorization. Afterward, define the next focused v0.1 Collector issue without starting Services or Scheduled Tasks inside the Registry PR.

## Major unresolved questions

- What genuine, monitored private channel should receive Code of Conduct reports?
- What explicitly versioned, tested identity upgrade should eventually address Registry value-name casing without hiding raw evidence or coupling Diff to Windows NLS behavior?
- What bounded/archive policy should apply to Scheduled Task raw XML before that Collector is implemented?
- What minimum supported Rust version will be tested and documented?
- Should the first desktop spike confirm React/Vite or compare one smaller frontend alternative before accepting ADR 0003?
