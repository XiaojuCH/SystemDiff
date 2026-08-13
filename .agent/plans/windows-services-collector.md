# Windows Services Collector v1

Status: In progress
Owner: primary agent
Last updated: 2026-08-13

## Goal

Issue #11 adds SystemDiff's second real Windows Collector: read-only, deterministic Win32 service configuration evidence through `windows.services` v1. The implementation must broaden the usable Snapshot -> Diff pipeline without weakening the existing rule that incomplete visibility cannot become a confirmed removal.

## User-visible outcome

`systemdiff snapshot -o <path>` captures Registry Run/RunOnce evidence and Windows service configuration visible to the current token. Default Diff output gives ordinary users a dedicated Windows service section with factual Added/Modified/Removed/Inconclusive wording and field-level changes; `--technical` exposes the raw service configuration and coverage evidence; `--json` remains deterministic and language-neutral. The portable Windows x64 Developer Preview contains and verifies both implemented Collectors.

## Current architecture and context

- `systemdiff-core` already has a draft `WindowsService` artifact, but it has no Service-specific validation or canonical identity contract. Its optional fields cannot distinguish known absence from a failed query.
- At plan creation, `systemdiff-windows::capture_snapshot` ran only `RegistryStartupCollector` and `mvp_collector_plans` marked Services planned; implementation now runs both independently.
- `systemdiff-diff` indexes `(collector_id, scope_id, artifact_kind, canonical_id)`. It confirms a one-sided absence only when the relevant scope is complete on both sides. Directly observed same-identity evidence can still be Modified under partial coverage.
- `EnumServicesStatusExW` explicitly and silently omits a service when the caller lacks `SERVICE_QUERY_STATUS`. A successful enumeration therefore is not proof that the service database was exhaustively visible to the current token.
- The current broad fixtures use placeholder scope `machine.win32`, plaintext lowercase identities, complete coverage, and a draft Service wire shape. They are not evidence of a shipped Collector contract.
- The portable verifier currently assumes Registry is the only implemented/enabled Collector.

## Constraints

- Production is query-only. Do not bind or call service creation, deletion, configuration mutation, start/stop/pause, control, or elevation APIs.
- Use `OpenSCManagerW`, `EnumServicesStatusExW`, `OpenServiceW`, `QueryServiceConfigW`, `QueryServiceConfig2W`, and `CloseServiceHandle`; do not shell out or parse localized command output.
- Request only `SC_MANAGER_ENUMERATE_SERVICE` and `SERVICE_QUERY_CONFIG`.
- Enumerate active and inactive Win32 services. Include own-process, shared-process, interactive modifiers, and modern per-user service/instance modifiers when their native type also identifies a Win32 service. Exclude kernel, file-system, recognizer, and other driver-only types by native type bits, never by name.
- Preserve API text as evidence. Do not expand variables, parse command lines, resolve/hash executables, resolve accounts/resources, infer publisher, or classify risk.
- Reject malformed UTF-16 for the affected item. Never use lossy replacement as evidence.
- Snapshot/report output is unredacted and may expose service accounts, paths/arguments, dependency/vendor names, load groups, and descriptions.
- No default CI or local validation may create, delete, reconfigure, start, or stop a service or require administrator privileges.

## Implementation steps

1. Finalize the pre-v0.1 Service wire contract and tests.
   - Keep existing fields and add `load_order_group: Option<String>` plus `tag_id: Option<u32>` because `QUERY_SERVICE_CONFIGW` returns them and public Collector docs already promise them.
   - Treat missing/empty load group and zero tag as known configured absence only after a successful base query.
   - Do not add serde defaults for the new fields: older draft service artifacts lacking them are rejected rather than silently reinterpreted. Update every committed draft fixture and document the deliberate compatibility impact. Keep Snapshot schema v1 because no public v0.1 format has shipped.
   - Validate non-empty service name, no embedded NUL, bounded strings/dependencies, a Win32 base type with no driver-only bits, Service Collector/artifact/scope association, and the v1 canonical identity. Preserve start type and delayed-auto-start independently: real Windows validation showed the delayed flag can remain set on a currently non-automatic service, so rejecting that combination would discard evidence.
2. Define identity and field semantics.
   - Service name is the only identity input. Display name, status, PID, config, and enumeration order never affect identity.
   - v1 canonical identity is lowercase hexadecimal SHA-256 over domain `systemdiff.windows-services.identity.v1\0`, the UTF-16 unit count as little-endian `u32`, and exact service-name UTF-16 units as little-endian bytes.
   - SCM preserves service-name case and compares names case-insensitively, but Microsoft exposes no documented persistent cross-platform canonical token. v1 therefore does not perform Unicode/NLS folding. It can false-split a hypothetical returned casing change; this conservative limitation is documented and any correction requires a new Collector version/compatibility decision.
   - Preserve complete per-user `_LUID` suffixes.
   - Preserve dependency order, casing, and `SC_GROUP_IDENTIFIER` (`+`) prefixes exactly as the configured MULTI_SZ. Do not sort, deduplicate, strip, or case-normalize dependencies.
3. Add a pure `services` Collector module and fake-source tests.
   - Add `WindowsServicesCollector`, a stable descriptor, `ServiceDataSource`, raw UTF-16/config records, bounded collection, identity grouping, deterministic sorting, diagnostics, and aggregate outcome assembly.
   - Use one scope, `current_token.win32`, to name the actual visibility boundary.
   - Use atomic per-service evidence: base config, description, and delayed-auto-start must all query and strictly decode successfully before emitting an observation. Any query denial/failure/malformed/over-limit result omits that service, marks the scope partial, emits an item diagnostic, and retains complete siblings. Consequently `None` means known configured absence, never unreadable.
   - Even a successful real enumeration remains `partial` with one stable `service_visibility_best_effort` diagnostic because API success cannot prove exhaustive visibility. Synthetic tests may explicitly use complete coverage only to validate generic Diff Removed semantics.
4. Add the Win32 SCM adapter behind `cfg(windows)`.
   - Add only the `Win32_System_Services` windows-rs feature needed by these APIs.
   - Own SCM and service handles in private non-Copy RAII wrappers; close each successful open exactly once. Borrow no handle beyond its valid owner.
   - Enumerate with `SC_ENUM_PROCESS_INFO`, `SERVICE_WIN32`, and `SERVICE_STATE_ALL`. Treat returned PID/state/checkpoint/wait-hint as transient plumbing and never retain them.
   - Implement aligned, initialized native buffers; validate struct arrays, pointer ranges/alignment, byte counts, termination, checked conversions, pagination resume progress, and returned service counts before copying evidence.
   - `EnumServicesStatusExW` pages are capped at the documented 256 KiB API maximum. Retain entries returned with `ERROR_MORE_DATA`, reuse the returned resume handle, and require bounded forward progress.
   - Use the documented probe/read pattern for `QueryServiceConfigW` and the two approved `QueryServiceConfig2W` levels only: `SERVICE_CONFIG_DESCRIPTION` and `SERVICE_CONFIG_DELAYED_AUTO_START_INFO`. Each query buffer is capped at the documented 8 KiB API maximum.
   - Read a complete configuration bundle until two consecutive reads agree, using at most three complete reads. A vanishing or continuously changing service produces a deterministic item diagnostic; the Collector does not claim an atomic system Snapshot.
5. Apply SystemDiff resource budgets.
   - At most 4,096 enumerated services and 64 pages/progress steps.
   - At most 32 KiB of retained UTF-16/text evidence per service and 16 MiB across the Collector.
   - The 256 KiB enumeration and 8 KiB query ceilings follow the documented APIs; count/evidence budgets are SystemDiff capture limits, not Windows platform limits.
   - Select retained observations deterministically by canonical identity before applying the aggregate budget. An over-limit item is omitted with partial coverage; complete siblings remain.
6. Integrate orchestration and reporting.
   - Mark Services implemented, run Registry and Services independently, and assemble both outcomes deterministically.
   - Add dedicated human service rendering with friendly known start-type labels, exact field-level Modified output, calm Inconclusive text, and hostile terminal-string escaping.
   - Expand technical output to every wire field, raw numeric values, identity/scope/version, coverage, and native diagnostics. Unknown numeric values remain numeric and never panic.
   - Keep Diff production semantics and JSON schema unchanged apart from the deliberate embedded Service artifact fields.
7. Add focused synthetic fixtures and cross-platform tests.
   - Cover Added under synthetic complete coverage, field-level Modified (start type, binary, description-only), Removed under synthetic complete coverage, and appearance/disappearance under partial coverage as Inconclusive.
   - Cover access denied, vanishing/query-changing service, sibling preservation, invalid UTF-16/MULTI_SZ, empty/multiple/group dependencies, unknown raw constants, per-user suffixes, driver filtering, pagination/growth/invalid native pointers, limits, identity collisions, ordering, control/bidi escaping, and deterministic JSON.
8. Update portable verification and documentation.
   - Require both implemented Collector lines and parse the generated Snapshot for Registry and Services runs rather than matching a singleton Registry array.
   - Retain artifact-only execution, exact archive allowlists, checksum, static CRT, `asInvoker`, unsigned state, and import allowlist. Measure the actual final PE before changing any import expectation.
   - Remove the obsolete Issue #9 branch trigger from CI; feature PRs still run Windows/Ubuntu Rust gates, while the exact Issue #11 branch gets a narrow, visibly named candidate artifact for downloaded-binary verification.
   - Update README EN/ZH, Quickstart, Collector/data-format/architecture/threat-model/roadmap docs, project state, and privacy wording without broad marketing rewrites.
9. Validate locally and on a real Windows host.
   - Run format, locked Clippy, all workspace tests, three service-fixture report modes, `collectors`, package/verifier checks, Markdown/parity/stale/secret scans, and `git diff --check`.
   - Run two short-window, read-only real Snapshots. Report only counts/status/diagnostic summaries and non-sensitive generic facts; never upload or commit the real inventory. Delete exact temporary files.
   - Confirm both Collectors exist, at least one service is normally observed when the host allows it, no driver-only artifact is emitted, ordering is stable, and no obvious spurious configuration modifications occur. Do not make zero changes a flaky invariant.
10. Request an independent reviewer focused on unsafe/pointer correctness, access rights, coverage/no-false-removal, evidence/identity/unknown semantics, resource limits, privacy, read-only boundaries, rendering, and portable integration. Resolve every High/Medium finding and actionable Low issue.
11. Commit, push `feat/windows-services`, create a ready PR closing Issue #11, and wait for required Windows/Ubuntu CI plus the applicable portable candidate/download verification. Stop with the PR unmerged.

## Affected files and modules

- `.agent/plans/windows-services-collector.md`
- `.agent/PROJECT_STATE.md`
- `.github/workflows/ci.yml` only for stale candidate trigger and directly relevant validation
- `crates/systemdiff-core/src/lib.rs`
- `crates/systemdiff-core/tests/`
- `crates/systemdiff-windows/Cargo.toml`
- `crates/systemdiff-windows/src/lib.rs`
- new `crates/systemdiff-windows/src/services.rs`
- new or focused native adapter code under `crates/systemdiff-windows/src/`
- `crates/systemdiff-diff/tests/`
- `crates/systemdiff-report/src/lib.rs` and tests
- `crates/systemdiff-cli/src/main.rs` and tests
- focused synthetic service fixtures under `fixtures/`
- `scripts/verify-windows-preview.ps1`
- `README.md`, `README.zh-CN.md`, `packaging/windows/QUICKSTART.md`
- relevant `docs/` architecture, data-format, Collector, threat-model, and roadmap files

## Test strategy

- Pure core tests validate the new required wire fields, known absence, service evidence invariants, identity, artifact association, round-trip, and deliberate rejection of old draft Service shapes.
- Pure Collector tests use a fake source for order independence, exact UTF-16 identity, per-user names, dependency semantics, atomic observation policy, access/error mapping, mutation, collisions, and deterministic budgets.
- Native state-machine tests inject enumeration/query calls to cover `ERROR_MORE_DATA`, `ERROR_INSUFFICIENT_BUFFER`, pagination/resume, zero progress, returned counts, pointer/alignment/range checks, termination, and caps without writing SCM state.
- Diff tests cover every change kind and ensure partial/denied/missing Services coverage cannot confirm a one-sided absence or appearance.
- Report tests cover Added/Modified/Removed/Inconclusive, exact changed fields, all technical fields, unknown constants, and control/bidi escaping.
- Windows-only integration captures real read-only evidence and reopens/validates it; default CI never writes services or needs elevation.
- Portable verification executes only the packaged/downloaded executable, requires both Collectors, parses a real read-only Snapshot, and preserves binary/package security gates.

Required commands include:

```text
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
```

## Risks

- SCM silently omits inaccessible status objects. Mitigation: real scope is always partial/best-effort, so missing evidence is never confirmed Removed.
- An optional field could confuse query failure with absence. Mitigation: emit only an atomically complete selected configuration bundle.
- Native buffers contain interior pointers. Mitigation: aligned initialized storage, checked pointer/range/termination validation, copy before buffer release, pure state-machine tests, and narrow unsafe review.
- Service configuration can mutate between calls. Mitigation: bounded consecutive-equality reads, diagnostics, no atomicity claim.
- Case-insensitive SCM identity lacks a documented durable independent canonical token. Mitigation: exact evidence hash, explicit false-split limitation, versioned future correction.
- Service evidence can increase Snapshot size and disclose accounts/paths/descriptions. Mitigation: retained-evidence budgets, 64 MiB output cap, unredacted warning, no real inventory in fixtures/logs/artifacts.
- A permanent partial scope means real one-sided Service changes are conservative Inconclusive. This is intentional until Windows exposes or SystemDiff can justify a stronger visibility contract.

## Rollback and compatibility

The Collector and renderer can be reverted without writing system state. The wire additions are a deliberate pre-v0.1 correction: committed draft Service fixtures are migrated, and older draft Service documents missing `load_order_group`/`tag_id` are rejected. Snapshot document schema remains v1 because no public stable v0.1 schema has shipped. The Registry Collector and existing Registry-only report behavior remain unchanged.

## Progress

- [x] 2026-08-13: synchronized clean `main` and `origin/main` at `fa2216427e0d1da2972916c46495978c9a050d0f`; latest main CI and portable verification were green.
- [x] 2026-08-13: confirmed no open PR/Issue and created Issue #11 plus branch `feat/windows-services`.
- [x] 2026-08-13: mapped the existing core/diff/report/CLI/portable boundaries and baseline fixture gaps.
- [x] 2026-08-13: completed initial official API and test-strategy research; baseline format, Clippy, and 88 workspace tests passed on the untouched branch.
- [x] 2026-08-13: implemented the draft Service wire correction, validation, identity, and focused core tests.
- [x] 2026-08-13: implemented the pure Collector and query-only native SCM adapter with RAII handles, bounded native buffers/pagination, strict UTF-16, atomic observations, and deterministic selection.
- [x] 2026-08-13: integrated both Collectors into Snapshot capture, Service report rendering, focused fixtures/tests, portable verification, and public/internal docs.
- [x] 2026-08-13: completed local, real-Windows, portable, and independent-review validation; all review findings were resolved.
- [ ] Commit, push, open PR, and observe final remote CI.

## Discoveries

- `EnumServicesStatusExW` silently omits a service when the caller lacks `SERVICE_QUERY_STATUS`; API success is therefore not exhaustive-visibility evidence.
- `EnumServicesStatusExW` documents a 256 KiB maximum output array and resume semantics with entries potentially returned alongside `ERROR_MORE_DATA`.
- `QueryServiceConfigW` and `QueryServiceConfig2W` require `SERVICE_QUERY_CONFIG`, use documented size probes, and document an 8 KiB maximum output buffer.
- Service names are limited to 256 characters, preserve case, compare case-insensitively, and reject slash/backslash. Display names also preserve case and compare case-insensitively.
- Per-user services expose full `_LUID`-suffixed service/display names and a native per-user service-type modifier; collection must not merge or strip those instances.
- `QUERY_SERVICE_CONFIGW.lpDependencies` is a double-NUL-terminated list of service/group names; group dependencies retain the documented `+` prefix.
- Real read-only validation found Windows service configurations where the delayed-auto-start flag remained true while the current start type was not Automatic. The wire therefore preserves both raw facts independently instead of enforcing a false cross-field invariant.

## Decisions

- Use scope `current_token.win32` and permanently conservative partial coverage for the real v1 adapter.
- Use atomic selected-field observations instead of a generalized per-field acquisition-state framework.
- Add load-order group and tag now, while the schema is still draft, rather than contradict the public Collector contract.
- Preserve exact dependency sequence and evidence; do not invent normalization unsupported by Microsoft.
- Use exact UTF-16 identity with a versioned domain and document the case-only limitation rather than couple Snapshot comparison to mutable/undocumented Unicode or NLS folding.
- Retain unknown raw numeric configuration values for forward compatibility; validate only invariants required to prove the artifact is a Win32 non-driver service.
- Treat successful absence as `None` only after all three selected queries succeed; any base/description/delayed query failure omits the item and retains partial coverage.

## Final validation

Local implementation validation on 2026-08-13:

- `cargo fmt --all --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace --all-targets`: passed, 113 tests, 0 failures.
- Focused Services fixtures ran through human, technical, and JSON CLI modes and produced one factual Added service with complete technical evidence.
- Two back-to-back real read-only Snapshots each retained 341 Services observations; the default Diff reported no confirmed changes and JSON contained no changes. Both temporary Snapshots were deleted exactly. No service was created, modified, started, stopped, or deleted.
- Real coverage was `partial` with `service_visibility_best_effort`; driver-only observations were 0. Item diagnostics showed that selected queries can fail or return evidence outside the accepted contract without aborting siblings.
- Final local portable package verification passed after removing Cargo/Rust/linker tools from `PATH`: AMD64, `asInvoker`, `uiAccess=false`, unsigned, no delayed imports, reviewed imports `advapi32.dll`, `api-ms-win-core-synch-l1-2-0.dll`, `KERNEL32.dll`, and `ntdll.dll`. EXE size was 1,860,608 bytes; ZIP size was 740,539 bytes. Exact outer/inner allowlists and checksum passed, and the packaged binary captured both Collectors.
- `git diff --check`: passed. Repository secret/machine-path scan found no new credential or committed real-host evidence.
- Independent reviewer final result: High 0 / Medium 0 / Low 0. The review-driven fixes made per-service failure diagnostics distinguishable without exposing service names and rendered delayed-auto-start changes independently of start type.
- Final remote CI/artifact results remain pending and will be appended without inference.
