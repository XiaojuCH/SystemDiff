# Pre-collector hardening

Status: Complete
Owner: Codex
Last updated: 2026-08-11

## Goal

Harden the untrusted Snapshot-file boundary and the draft Registry startup evidence model before any real Windows Collector is implemented. This plan delivers GitHub issue [#3](https://github.com/XiaojuCH/SystemDiff/issues/3) without expanding into collection, remediation, identity, risk, desktop, or release work.

## User-visible outcome

The existing `systemdiff diff` command will reject oversized Snapshot files before an unbounded full-file allocation, route supported JSON documents by header before constructing a v1 `Snapshot`, and reject invalid or non-UTC `captured_at` values. Contributors implementing the first Registry Collector will have explicit Registry-view contracts and structured, Microsoft-grounded RunOnce prefix evidence that preserves complete value names and collision-free identities.

## Current architecture and context

- `systemdiff-cli::load_snapshot` currently performs `fs::read` followed by direct `serde_json::from_slice::<Snapshot>`.
- `systemdiff-diff::diff_snapshots` is the first production call to `Snapshot::validate`; document type and schema checks therefore occur only after the full v1 wire type is constructed.
- `captured_at` is a `String` whose only current constraint is non-empty.
- `RegistryView` has stable enum variants but no rustdoc definition, and `Native` is ambiguous.
- `RegistryStartupEntry` preserves the complete raw `value_name`, but has no structured Run/RunOnce distinction or prefix semantics.
- ADR 0002 requires header-first routing. The threat model identifies unbounded Snapshot JSON as TM-002.
- Draft schema v1 is explicitly mutable before v0.1; no released compatibility promise is being rewritten.

Read-only investigation used the project explorer, windows-researcher, and test-engineer roles. Microsoft references:

- [Run and RunOnce Registry Keys](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)
- [RunOnce Registry Key](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/runonce-registry-key)
- [Registry Keys Affected by WOW64](https://learn.microsoft.com/en-us/windows/win32/winprog64/shared-registry-keys)
- [Accessing an Alternate Registry View](https://learn.microsoft.com/en-us/windows/win32/winprog64/accessing-an-alternate-registry-view)
- [Registry Key Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-key-security-and-access-rights)

## Constraints

- Remain offline-first, read-only, synchronous, and deterministic.
- Treat Snapshot JSON as untrusted; report precise document and size failures without parsing evidence as commands.
- Do not add a streaming JSON parser, async runtime, configurable policy framework, v2 dispatch registry, or migration framework.
- Accept RFC 3339 `captured_at` values only when they use known UTC (`Z` or `+00:00`); reject non-zero offsets and RFC 3339 `-00:00` unknown-local-offset notation. Preserve the input string rather than canonicalizing read evidence.
- Preserve `RegistryView::{Shared, Native, Registry32, Registry64}`. `Native` must never mean a process-bitness-dependent default view for a redirected key.
- Preserve the full Registry value name in evidence and canonical identity. Do not strip `!` or `*` for identity.
- Do not infer semantics for combined, repeated, or marker-only RunOnce prefixes that Microsoft does not document.
- Do not modify Scheduled Task raw XML, Collector traits, `change_id`, risk behavior, sanitizer design, MSRV policy, GUI/Tauri, or release tooling.
- Do not implement a real Collector, Registry API call, or `systemdiff snapshot`.

## Implementation steps

1. Create issue #3, complete read-only architecture/Windows/test research, and record decisions in this plan.
2. Add a 64 MiB CLI Snapshot input maximum. Open the file, inspect handle metadata, reject an oversized length, then perform a bounded `MAX + 1` read and recheck actual bytes before decoding. Keep a small limit-injected helper for deterministic tests.
3. Add a core `decode_snapshot_document` boundary that deserializes only `document_type` and `schema_version`, explicitly routes `systemdiff.snapshot` v1, then constructs the current `Snapshot`. Return typed header/type/version/body errors.
4. Add standard RFC 3339 parsing in `Snapshot::validate` using the parsing-only surface of the maintained, Apache-2.0 OR MIT `time` crate. Require known UTC and preserve original wire text.
5. Add exact rustdoc and public-document semantics for all four `RegistryView` variants. Define `Registry32`/`Registry64` by the corresponding WOW64 selector, `Shared` by Microsoft's shared-key model, and `Native` as the sole view only where no WOW alternate view exists.
6. Add a required Registry startup kind plus a single optional RunOnce prefix-semantics enum. Run entries carry no RunOnce semantics; RunOnce entries carry exactly one of no documented prefix, deferred deletion, Safe Mode execution, or undocumented syntax. Validate the structured value against the complete raw value name.
7. Update synthetic fixtures and mechanical struct literals. Add tests for size ordering, header routing, UTC timestamp policy, stable RegistryView spellings, RunOnce semantics/round-trip/invariants, and full-name identity separation.
8. Update data-format, Collector, architecture/dependency, and durable project-state documentation without changing excluded subsystems.
9. Run format, Clippy, full workspace tests, existing CLI smoke commands, deterministic fixture checks, Markdown/diff checks, and independent reviewer analysis. Address High/Medium findings in scope.
10. Complete this plan and hand the validated implementation to the repository's normal branch/PR workflow. Track external integration and required-check status in the PR rather than treating unobserved CI as repository-local validation.

## Affected files and modules

Expected production and dependency surface:

- `Cargo.toml`, `Cargo.lock`
- `crates/systemdiff-core/Cargo.toml`
- `crates/systemdiff-core/src/lib.rs`
- `crates/systemdiff-cli/Cargo.toml`
- `crates/systemdiff-cli/src/main.rs`

Expected tests and draft fixtures:

- `crates/systemdiff-core/tests/`
- `crates/systemdiff-risk/src/lib.rs` only for mechanical test-fixture construction
- `fixtures/snapshots/before-v1.json`
- `fixtures/snapshots/after-v1.json`

Expected documentation/state:

- `docs/architecture.md`
- `docs/data-format.md`
- `docs/collectors.md`
- `docs/threat-model.md`
- `.agent/PROJECT_STATE.md`
- this ExecPlan

No production changes are expected in diff, report, risk, Windows, services, tasks, or desktop modules.

## Test strategy

- CLI size helper: below limit, exactly at limit, above limit.
- CLI ordering: a tiny invalid JSON file above an injected small limit returns `too large` before read/decode; the production reader remains bounded after metadata to limit growth races.
- Core document decoder: valid v1 fixture, unknown document type with an invalid body, unsupported version with an invalid body, supported header with invalid v1 body, and malformed/missing header.
- Timestamp validation: `Z`, `+00:00`, fractional seconds, non-zero offset, `-00:00`, malformed text, invalid calendar/time values, and empty input.
- RegistryView: exact serialized names, round-trip, unknown-name rejection, and fixture consistency.
- Registry startup: Run, RunOnce without a documented prefix, `!`, `*`, combined/repeated/marker-only forms as undocumented, raw/structured mismatch rejection, serde round-trip, and distinct `Foo`/`!Foo`/`*Foo` observation keys.
- Regression: existing fixture validation, deterministic shuffled Diff JSON, incomplete-coverage behavior, undecoded Registry hash comparison, Collector-version rejection, report tests, and all three CLI smoke paths.

## Risks

- A metadata-only size check has a growth race. Mitigation: retain metadata preflight but bound the actual read to `MAX + 1` and recheck before decoding.
- Header deserialization still scans the bounded JSON to skip body fields; it does not construct the full `Snapshot`, but it is not a streaming header parser. Documentation must describe this accurately.
- RFC 3339 parsers may normalize `-00:00` to numeric zero. Mitigation: use standard parsing plus an explicit accepted UTC designator policy.
- Adding required Registry fields changes unreleased draft-v1 fixtures. Mitigation: update all golden fixtures and reject missing/inconsistent evidence rather than silently defaulting it.
- Two independent RunOnce booleans could imply undocumented combinations. Mitigation: use one enum and retain uninterpreted raw names.
- `canonical_id` is Collector-owned, so core cannot prove that a future Collector includes the prefix. Mitigation: fixture/tests and docs fix the contract now; real canonicalization remains part of the Collector PR.

## Rollback and compatibility

The change is a focused, reversible draft-v1 update before v0.1. Reverting the PR restores prior fixtures and parsing behavior. Once v0.1 ships, these semantics become compatibility obligations and must not be redefined in place.

Existing supported fixture diffs remain semantically deterministic. Unsupported document families/versions are rejected earlier and more clearly. No cross-version migration behavior is introduced.

## Progress

- [x] 2026-08-11: Confirmed clean synchronized `main`, green CI, active ruleset, and no equivalent issue.
- [x] 2026-08-11: Created focused GitHub issue #3.
- [x] 2026-08-11: Completed explorer, Microsoft Windows, and test-engineer read-only investigations.
- [x] 2026-08-11: Implemented and tested the bounded input and header-routing boundaries.
- [x] 2026-08-11: Implemented UTC timestamp and Registry schema semantics with deterministic tests.
- [x] 2026-08-11: Updated draft fixtures, public technical documentation, the threat model, and durable project state.
- [x] 2026-08-11: Completed local format, Clippy, workspace-test, CLI-smoke, diff, dependency-feature, and Markdown-link validation.
- [x] 2026-08-11: Completed independent read-only review with no High, Medium, or actionable Low findings.
- [x] 2026-08-11: Completed the implementation plan and prepared the reviewed change for the branch/PR workflow.

## Discoveries

- Microsoft documents `!` as deferring RunOnce value deletion until after the command runs, and `*` as allowing RunOnce execution in Safe Mode. It does not define combined/repeated prefixes or their ordering.
- HKLM `Software` is redirected on 64-bit Windows, while HKCU `Software` is shared on supported Windows versions; HKLM startup locations need explicit 32/64 passes and HKCU is collected once.
- Omitting WOW64 selectors on a redirected key binds the result to process bitness and is not stable evidence.
- RFC 3339 `-00:00` represents an unknown local offset, not a known UTC assertion.
- Header-first routing belongs in core so future CLI/desktop readers share version dispatch; filesystem size enforcement remains at the CLI boundary.

## Decisions

- Use a fixed 64 MiB input ceiling. It provides substantial headroom for targeted v0.1 evidence and future bounded task/Registry payloads while placing an auditable limit on parser amplification. A configurable policy is deferred until a demonstrated use case exists.
- Keep `captured_at` as the original wire `String`; validate rather than normalize evidence read from disk. A future writer will emit canonical `Z`.
- Use the `time` crate with only RFC 3339 parsing support. It is maintained, license-compatible, and avoids a custom parser or a larger time framework.
- Define `Native` only as the sole Registry view when no WOW alternate logical views exist. Redirected keys on WOW-enabled systems must use explicit `Registry32`/`Registry64` evidence.
- Add `RegistryStartupKind` (`run`/`run_once`) and a single `RunOncePrefixSemantics` value. Structured semantics enrich but never replace the complete `value_name`.

## Final validation

Local validation completed on Windows with the installed stable MSVC toolchain:

- `cargo fmt --all --check`: passed after applying one formatting-only adjustment with `cargo fmt --all`.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed with no warnings.
- `cargo test --locked --workspace --all-targets`: passed, 39 tests, 0 failed, 0 ignored.
- `cargo run --locked -p systemdiff-cli -- diff fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json`: passed; reported 2 added, 1 inconclusive, 1 modified, and 1 removed change.
- `cargo run --locked -p systemdiff-cli -- diff --json fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json`: passed and emitted the deterministic v1 JSON Diff document.
- `cargo run --locked -p systemdiff-cli -- collectors`: passed and reported the three MVP Collectors as planned.
- `git diff --check`: passed.
- Relative Markdown links in modified project documents: passed.
- `cargo tree -p systemdiff-core -e features`: confirmed that SystemDiff enables only the `time` parsing feature.

The independent reviewer confirmed the five adjudicated hardening items, dependency scope, documentation, draft-schema change, and explicit exclusions. It reported 0 High, 0 Medium, and 0 actionable Low findings.

GitHub Actions had not run for the feature branch when this repository-local plan was completed. The PR records that external Windows and Ubuntu validation; this plan does not claim an unobserved CI result.
