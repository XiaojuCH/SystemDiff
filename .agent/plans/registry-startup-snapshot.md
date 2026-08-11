# Registry startup Collector and Snapshot CLI vertical slice

Status: In progress
Owner: Codex
Last updated: 2026-08-11

## Goal

Deliver GitHub issue [#5](https://github.com/XiaojuCH/SystemDiff/issues/5): the first real, read-only Windows product path that captures the documented current-user and local-machine Run/RunOnce Registry locations into a deterministic Snapshot and exposes it as `systemdiff snapshot -o <path>`. The resulting before/after files must produce a trustworthy Diff for a real startup-entry change without claiming certainty when a Registry scope was denied, unstable, unsupported, or otherwise incomplete.

This is a Registry startup vertical slice, not another bootstrap or generic hardening project. Any schema or orchestration work below is included only where the first Collector demonstrates a concrete blocker.

## User-visible outcome

On a supported Windows installation, a user can run:

```text
systemdiff snapshot -o before.json
# install software or perform another controlled change
systemdiff snapshot -o after.json
systemdiff diff before.json after.json
```

The Snapshot records real Run/RunOnce evidence visible to the current token. The Diff can show an Added, Removed, or Modified Registry startup entry when coverage supports that conclusion, while permission or concurrent-mutation gaps remain explicit. `collectors` reports the Registry startup Collector as implemented and leaves Services and Scheduled Tasks as planned.

## Current architecture and context

- The repository is at public `main` commit `c38f2d423af6e1c0d2519657c9bdda31103beab1`; PR #4 is merged, issue #3 is closed, and the merge commit passed Windows and Ubuntu CI.
- `systemdiff-core` already owns the draft-v1 Snapshot envelope, Collector contract, coverage/status validation, Registry startup artifact, native type code, complete-content hash field, typed decode outcome, bounded raw evidence, Run/RunOnce kind, and structured marker semantics.
- `systemdiff-diff` already validates Snapshot coverage, rejects duplicate identities, sorts deterministically, and produces Inconclusive absence when scope coverage is not complete. No production Diff redesign is required.
- `systemdiff-report::write_json` already renders any serializable value to a stream. It must not open output paths.
- `systemdiff-cli` owns parsing and file I/O but has no `snapshot` subcommand, Snapshot assembly path, clock boundary, or no-overwrite output helper.
- `systemdiff-windows` contains descriptors only. It has no Windows dependency, Registry adapter, platform metadata provider, or implemented Collector.
- The current `Collector` trait is synchronous and sufficient. This work does not add async, plugins, runtime discovery, or a generic dependency-injection framework.

Read-only planning used the project explorer, windows-researcher, and test-engineer roles. Authoritative references:

- [Run and RunOnce Registry Keys](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)
- [Registry Keys Affected by WOW64](https://learn.microsoft.com/en-us/windows/win32/winprog64/shared-registry-keys)
- [Accessing an Alternate Registry View](https://learn.microsoft.com/en-us/windows/win32/winprog64/accessing-an-alternate-registry-view)
- [RegOpenKeyExW](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regopenkeyexw)
- [RegQueryInfoKeyW](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regqueryinfokeyw)
- [RegEnumValueW](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regenumvaluew)
- [RegCloseKey](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regclosekey)
- [Registry Key Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-key-security-and-access-rights)
- [Registry Value Types](https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-value-types)
- [Registry Element Size Limits](https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-element-size-limits)
- [IsWow64Process2](https://learn.microsoft.com/en-us/windows/win32/api/wow64apiset/nf-wow64apiset-iswow64process2)
- [windows-rs Registry bindings](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/Registry/index.html)

Dependency investigation on 2026-08-11 found:

- `windows` 0.62.2: Microsoft-maintained, MIT OR Apache-2.0, Rust 1.82, with narrow Win32 features available.
- `sha2` 0.11.0: RustCrypto-maintained, MIT OR Apache-2.0, Rust 1.85; fixed-size SHA-256 does not require its default `alloc`/OID features.
- `windows-version` 0.1.7: Microsoft-maintained, MIT OR Apache-2.0, Rust 1.74, and narrowly exposes current Windows version/build information.

The official `windows-registry` safe crate was considered but is not suitable for this Collector boundary. Its current value iterator converts names with `String::from_utf16_lossy` and turns any non-success enumeration result into iteration termination rather than exposing the exact error/retry state. SystemDiff needs lossless names, raw bytes, scoped diagnostics, `ERROR_MORE_DATA`, and concurrent-mutation handling, so the implementation will use narrow lower-level `windows` bindings behind a project-owned safe adapter.

## Constraints

- Remain offline-first and read-only. Production code opens Registry keys with query-only rights and never calls a Registry create/set/delete API.
- Do not execute, expand, parse, resolve, hash, sign-check, classify, or remediate commands referenced by startup values.
- Use Unicode Win32 APIs through `windows-rs`; do not shell out to or parse `reg.exe`, PowerShell, WMIC, or localized text.
- The current token is the collection boundary. Do not enumerate other users, load profiles, introduce SID/machine identity, or imply cross-user/cross-machine comparability.
- Every key/view is an independent scope. One denied or unstable scope cannot erase unrelated evidence or abort the Snapshot.
- A missing key is a complete empty scope. An unreadable or unstable key is not absence and must not create a false Removed change.
- Keep unsafe code confined to small `systemdiff-windows` adapters with documented buffer/ownership invariants. Core and CLI remain `forbid(unsafe_code)`.
- Default CI remains non-elevated and never writes Run/RunOnce. Mutation-based E2E is separately gated, disposable, and manually authorized.
- Do not implement Services, Scheduled Tasks, files, signatures, executable hashing, command parsing, risk rules, GUI, telemetry, cloud, or remediation.
- Do not introduce async, a general Windows API mock layer, a universal OS abstraction, a plugin ABI, or a configurable global resource-policy framework.
- Draft v1 may change before v0.1, but the schema change must be explicit, documented, fixture-tested, and limited to blockers exposed by real collection.

## Windows API and scope strategy

### Scope/view matrix

Use these exact paths:

```text
Software\Microsoft\Windows\CurrentVersion\Run
Software\Microsoft\Windows\CurrentVersion\RunOnce
```

On x64 Windows, emit six fixed scopes in this order:

| Scope ID | Hive | View/access | Kind |
| --- | --- | --- | --- |
| `current_user.shared.run` | HKCU | no WOW selector; `Shared` | Run |
| `current_user.shared.run_once` | HKCU | no WOW selector; `Shared` | RunOnce |
| `local_machine.registry32.run` | HKLM | `KEY_WOW64_32KEY`; `Registry32` | Run |
| `local_machine.registry32.run_once` | HKLM | `KEY_WOW64_32KEY`; `Registry32` | RunOnce |
| `local_machine.registry64.run` | HKLM | `KEY_WOW64_64KEY`; `Registry64` | Run |
| `local_machine.registry64.run_once` | HKLM | `KEY_WOW64_64KEY`; `Registry64` | RunOnce |

On a sole-view x86 Windows installation, emit four scopes: the two HKCU Shared scopes plus `local_machine.native.run` and `local_machine.native.run_once` opened without a WOW selector and labeled `Native`. Never call both WOW selectors on 32-bit Windows and pretend the duplicated physical view is two observations.

Use `IsWow64Process2(GetCurrentProcess(), ...)` to obtain the native machine architecture rather than process pointer width. The initial implementation targets Windows 10 version 1709 / Server version 1709 or later, the documented floor for that API. `windows-version` supplies version/build metadata; unsupported/unknown platforms fail honestly rather than silently guessing topology.

Microsoft documents that `KEY_WOW64_32KEY` can select different 32-bit stores on ARM depending on the calling process architecture. Until SystemDiff defines and tests an ARM build/view contract, an ARM64 or unknown native machine still collects HKCU Shared but marks the four HKLM alternate scopes `Unsupported` with stable scoped diagnostics. It must not mislabel those scopes as x64-compatible evidence.

### Opening and ownership

For each target, call:

```text
RegOpenKeyExW(predefined_root, path, 0, KEY_QUERY_VALUE | selector)
```

`RegQueryInfoKeyW` and `RegEnumValueW` require only `KEY_QUERY_VALUE`; do not request `KEY_READ`, `KEY_ALL_ACCESS`, write access, ownership privileges, or SACL access. Current-token HKCU uses `HKEY_CURRENT_USER`; no impersonation is introduced.

The raw `HKEY` generated by windows-rs is copyable and does not close itself. A private, non-Copy `OwnedRegistryKey` in the Win32 adapter takes ownership only after successful `RegOpenKeyExW` and calls `RegCloseKey` in `Drop`. Predefined root handles are borrowed and never wrapped or closed. Drop does not panic; explicit API failures remain numeric Win32 codes rather than being coerced into HRESULT or localized message parsing.

### Registry data-source boundary

Use one narrow Collector-specific source interface, not one trait per API:

```text
RegistryDataSource
  detect_layout() -> RegistryLayout
  read_key_once(target, limits) -> KeyReadAttempt
```

`KeyReadAttempt` represents Missing, a completed raw attempt, a mutation/anomaly, or a structured source failure. A raw attempt contains before/after key metadata and records with complete UTF-16 name code units, numeric Registry type, and complete native data bytes. Handles and pointers never cross the adapter boundary. Scripted fakes return attempts from a queue for deterministic cross-platform tests.

## Buffer, allocation, and concurrent-mutation strategy

For each whole-key attempt:

1. Open a fresh handle with the exact target selector.
2. Call `RegQueryInfoKeyW` for value count, maximum value-name length, maximum value-data length, and last-write `FILETIME`.
3. Treat all maxima as momentary, bounded allocation hints. Maximum name length is UTF-16 code units excluding NUL, so the initialized name buffer uses checked `max + 1` growth within the Windows value-name limit. Never allocate the reported maximum data length up front: one oversized value must not prevent enumeration of normal siblings.
4. Enumerate indices from zero until `ERROR_NO_MORE_ITEMS`; do not stop only at the initial value count because enumeration is unordered and mutable.
5. Probe each index with `lpData = NULL` and a data-length pointer to obtain the complete name, native type, and required data byte count without allocating the value payload. Reset every capacity variable before every `RegEnumValueW` call. A successful name length excludes the NUL; the required data length includes any stored terminators.
6. If the required data length exceeds the per-value limit, emit no incomplete observation, mark the scope Partial with `registry_value_too_large`, increment the index, and continue. If it fits, allocate only that value's checked size and read the same index again. Accept the record only when the second call succeeds and its complete name/type still match the probe.
7. On `ERROR_MORE_DATA`, discard the undefined data buffer and retry the same index within the per-index limit. Use the returned required data length where provided; for a name shortage, re-query metadata and use bounded geometric growth because Microsoft does not promise that the returned name length is the required capacity. A value that grows beyond the cap is omitted with a scoped diagnostic while later indices remain eligible for collection.
8. Count aggregate native value-name and value-data bytes only for retained complete records. A record that would exceed the Collector budget is omitted and diagnosed without consuming the remaining budget, so smaller later siblings can still be retained.
9. After enumeration, query count and last-write time again. Only an attempt with matching before/after count and `FILETIME`, an enumerated count consistent with the final metadata count, no duplicate exact identity, and no enumeration anomaly is considered stable. Reaching the explicit 4,096-value cap while metadata reports additional values is instead a diagnosed resource-limit `Partial` result and does not trigger pointless retries.
10. Retry the entire scope with a fresh handle at most twice after the initial attempt (three total attempts). Discard earlier attempts rather than mixing their values.
11. If all attempts are unstable, retain only complete records from the final attempt, mark the scope `Partial`, and add `registry_changed_during_scan`. This is a best-effort consistency check, never an atomic-snapshot claim; count/time equality cannot detect every ABA race. Static resource omissions remain Partial but do not force pointless whole-scope retries.

Initial constants to validate with tests and serialized-size measurements:

- 8 MiB maximum native data bytes for one Registry value;
- 32 MiB maximum aggregate retained native Registry value-name and value-data evidence for the Collector;
- 4,096 values per scope;
- three per-index buffer-growth attempts before the whole-key attempt is treated as unstable/resource-limited;
- the existing 64 MiB maximum serialized Snapshot input/output.

All `u32`/`usize` conversions, additions, multiplications, and index increments are checked. Exceeding a limit produces a stable diagnostic and Partial scope; SystemDiff never hashes a truncated value and calls it complete. Tests pair an oversized value with a normal sibling and require the normal evidence to survive. The CLI serializes through a 64 MiB capped writer before creating the destination, so rejection is bounded in memory and an emitted Snapshot is always small enough for its own reader.

## UTF-16, value names, identity, and decoding

### Lossless value-name evidence

Real Win32 enumeration returns UTF-16 code units, while the current `value_name: String` cannot represent an unpaired surrogate without replacement and possible identity collision. Because issue #5 requires the complete original name, this Collector exposes a concrete draft-schema blocker.

Replace the string field with a minimal tagged `RegistryValueName` wire value:

```text
decoded { value: String }
invalid_utf16 { utf16le_hex: String }
```

Valid names retain their exact original casing/text. Invalid names retain all original UTF-16LE code units as validated lowercase hex; no lossy replacement string is emitted. The default/unnamed value is a valid decoded empty string. Core validation and fixtures enforce exactly one tagged form and bounded valid hex. RunOnce marker classification works from the authoritative UTF-16 units, so an ASCII leading `!` or `*` remains interpretable even if later units are invalid.

### Canonical identity and ordering

Add one Registry-startup identity helper, not a generic identity subsystem. Scope already fixes hive/view/key/kind. Collector v1 deliberately performs no Unicode normalization, case mapping, or RunOnce-prefix stripping: `canonical_id` is a domain-separated SHA-256 over the exact authoritative UTF-16 code units, encoded as a little-endian `u32` code-unit count followed by each `u16` in little-endian order, after the ASCII domain bytes `systemdiff.registry-startup.identity.v1` and a zero separator.

Registry value lookup is case-insensitive, but Microsoft does not specify its Unicode comparator or guarantee the casing returned by `RegEnumValueW`. A build-26100 isolated HKCU experiment found that `RegSetValueExW` with alternate casing updated one existing value while enumeration retained its first casing. `CompareStringOrdinal(TRUE)` supplies authoritative pairwise ordinal case-insensitive comparison, but it cannot independently generate a cross-platform wire token; Microsoft does not guarantee that `LCMapStringEx`, linguistic sort keys, or NLS hashes match Registry lookup, and those alternatives risk hidden false merges.

Collector v1 therefore keeps exact UTF-16 as a conservative, explicitly limited evidence identity. It avoids merging distinct raw evidence, is independent of Rust/Unicode table versions, and treats valid and invalid UTF-16 uniformly. If Windows ever returns different casing for one logical value, v1 may produce a visible false split; this is a known limitation, not a claim that Registry logical identity is case-sensitive. Changing the algorithm requires a new Collector version with verified compatibility semantics. The `!` or `*` unit always participates. If distinct exact names ever produce the same digest, omit the colliding group, keep unrelated observations, mark the scope Partial, and emit `registry_identity_collision` rather than applying last-write-wins. Tests fix exact vectors for casing, delimiter-like characters, Unicode, invalid units, the unnamed/default value, and `Foo`/`!Foo`/`*Foo`. The empty name is ordinary evidence with a stable identity and `NoDocumentedPrefix` for RunOnce; it is not marker-only corruption.

Sort targets by the fixed scope table and observations by `ArtifactKey`. Sort coverage by scope ID and diagnostics by `(scope_id, code, stage, native_code)`. Never depend on `RegEnumValueW` order or localized messages.

### Native data and decoding

The exact `data[..returned_byte_count]` from a successful enumeration is authoritative. Compute SHA-256 before decoding or terminator checks, including embedded/terminal NUL and zero-length data.

Decode strictly:

| Native type | Rule |
| --- | --- |
| `REG_SZ` (1) | even byte length, UTF-16LE, documented termination; one logical string |
| `REG_EXPAND_SZ` (2) | same strict string rules; preserve `%VAR%` unexpanded |
| `REG_DWORD` (4) | exactly 4 little-endian bytes |
| `REG_DWORD_BIG_ENDIAN` (5) | exactly 4 big-endian bytes |
| `REG_MULTI_SZ` (7) | even UTF-16LE bytes; require the documented double-NUL termination, including for an empty list; preserve item order |
| `REG_QWORD` (11) | exactly 8 little-endian bytes |
| known non-decoded types such as `REG_NONE`, `REG_BINARY`, `REG_LINK`, and resource types | `NotApplicable` |
| unknown numeric type | `UnsupportedType` |

Odd byte counts, invalid UTF-16, malformed termination, or wrong integer length become `InvalidData`; they do not make a readable scope incomplete because the complete bytes were captured for hashing and the native type, complete hash, and decode status remain available. Zero-length binary/none data is valid non-decoded evidence and uses the SHA-256 of empty bytes; zero-length supported string/integer data is InvalidData.

Collector v1 sets value-data `raw_evidence` to `None` for every decode status. The complete hash and typed/decode status are sufficient for deterministic comparison, while a blanket prefix for binary, unknown, or malformed values would create privacy and JSON-size costs without a concrete current use. The existing bounded wire field remains available for a future explicitly reviewed forensic policy; this issue does not define one. Lossless UTF-16 hex for an invalid value *name* is authoritative identity evidence, not a value-data raw prefix.

## Coverage, diagnostics, and Snapshot assembly

### Minimal diagnostic schema change

The current `Diagnostic` has no machine-readable scope association. Add `scope_id: Option<String>` so a diagnostic can identify one of the six Registry scopes; `None` remains available for Collector-wide layout errors. Snapshot validation rejects a diagnostic that references an unknown scope. This is an explicit pre-v0.1 draft-v1 schema change justified by real per-scope failure semantics; update fixtures and `docs/data-format.md`. Do not introduce a generic error taxonomy.

Stable initial codes include:

- `registry_access_denied`
- `registry_open_failed`
- `registry_query_failed`
- `registry_enumeration_failed`
- `registry_changed_during_scan`
- `registry_value_too_large`
- `registry_resource_limit`
- `registry_identity_collision`
- `registry_layout_unsupported`

Messages are short English operator context only. Machine logic uses code, scope, stage, and numeric Win32 code; diagnostics never include value data, names, commands, paths beyond the fixed documented key, usernames, or localized system error text.

Status mapping:

- `ERROR_FILE_NOT_FOUND` from the exact key open: Complete empty scope.
- `ERROR_ACCESS_DENIED`: PermissionDenied plus scoped numeric diagnostic.
- stable enumeration, including values with InvalidData: Complete.
- retained observations after exhausted mutation/resource/item failures: Partial.
- known unsupported topology: Unsupported.
- unexpected open/query/enumeration failure with no usable observations: Failed for that scope.
- `ERROR_KEY_DELETED`: retry as concurrent mutation; exhausted retries become Partial.

Aggregate status is a pure function: all Complete -> Complete; if every scope has the same terminal PermissionDenied/Unsupported/Unavailable/Failed status -> that status; every mixed result -> Partial. Status is never inferred from observation count. Complete or Partial scopes may carry observations; denied/unavailable/unsupported/failed scopes do not.

### Pure Snapshot assembly

Add a small core assembly function that accepts already sampled metadata and `CollectionOutcome` values. It sets document/schema constants, derives enabled Collector IDs and runs, flattens and sorts observations/coverage/diagnostics, calls `Snapshot::validate`, and returns the Snapshot. It owns no clock, Windows API, file path, or runtime Collector discovery.

The CLI/platform layer supplies:

- `systemdiff_version` from package build metadata;
- canonical current UTC formatted with `time` as RFC 3339 `Z` (reader support for `+00:00` remains);
- optional Windows major/minor and build from `windows-version`;
- native architecture from the same platform detection used for Registry topology;
- current-token privilege from `OpenProcessToken`, `GetTokenInformation(TokenUser/TokenElevation)`, and `IsWellKnownSid(WinLocalSystemSid)`, falling back honestly to `Unknown`;
- redaction `Unredacted` with no policy;
- only actually implemented Collector outcomes, initially Registry startup.

No hostname, SID, stable machine token, account name, or network data is added. The sampled privilege value is reused in `CollectionContext`.

## Snapshot CLI composition and output

Add `snapshot -o <path>` while preserving existing commands. On non-Windows platforms the command parses but returns an explicit unsupported-platform error before creating an output file.

Windows flow:

```text
sample time + non-identifying host/privilege metadata
  -> build CollectionContext
  -> run the fixed list of implemented Collectors (Registry only)
  -> pure assemble/sort/validate Snapshot
  -> serialize pretty JSON + newline through a 64 MiB capped memory writer
  -> OpenOptions::create_new(true)
  -> write_all + flush
```

`create_new(true)` atomically rejects an existing path; do not use `exists()` followed by create and do not add `--force`. A small CLI-owned capped `Write` adapter stops serialization before memory exceeds 64 MiB; the report crate remains stream-only. If `write_all` or `flush` fails after this invocation creates the destination, do not delete by pathname because a concurrent rename/replacement could make that path refer to someone else's file. Return an explicit error naming the potentially incomplete output and require user inspection/removal. Tests use a fixed timestamp and injected writer/file helper rather than wall-clock sleeps.

`mvp_collector_plans` gains an Implemented state for Registry while Services and Tasks remain Planned. The Registry descriptor is defined once and reused by the plan and Collector so ID/version/privilege cannot drift.

## Implementation steps

1. Reconfirm issue #5, this approved plan, synchronized `main`, dependency versions/licenses/features, and the supported Windows/architecture assumptions before code changes.
2. Add targeted dependencies: `windows` only for Windows with the minimum Win32 features, `windows-version`, `sha2` without unnecessary default features, and the existing `time` dependency's `std`/`formatting` features only where the CLI writer needs them. Update the architecture dependency table and lockfile.
3. Make the two Collector-driven draft-v1 schema changes in core: scoped diagnostics and lossless tagged Registry value names. Add invariant/fixture/round-trip tests and update data-format documentation.
4. Add the pure core Snapshot assembly function and deterministic ordering tests.
5. Split `systemdiff-windows` into focused modules for descriptors/composition, Registry evidence/Collector logic, the cfg-Windows Win32 adapter, and platform metadata. Keep the public surface minimal.
6. Implement the fixed Registry target planner for x64, x86 sole-view, ARM/unknown unsupported, and layout failure; add cross-platform table tests.
7. Implement exact UTF-16 name identity, RunOnce classification reuse, strict native decoding, SHA-256, the no-value-raw policy, aggregate status, and scripted-source tests before Win32 calls.
8. Implement the cfg-Windows RAII Registry adapter, metadata-first per-index reads, checked buffers, same-index `ERROR_MORE_DATA` retry, resource caps, whole-scope consistency retry, and numeric error mapping. Add narrow Windows read-only smoke tests.
9. Compose the implemented Registry Collector and platform metadata into the pure Snapshot assembler. Ensure unrelated scope results survive denial/failure.
10. Add CLI parsing/runtime for `snapshot -o`, canonical `Z`, capped <=64 MiB serialization, atomic no-overwrite creation, explicit partial-output errors without path deletion, and non-Windows unsupported behavior.
11. Add registry-only synthetic before/after fixtures and Snapshot -> Diff integration: exactly one expected Added change for complete coverage, and Inconclusive rather than false removal for denied/partial coverage.
12. Update canonical English and Chinese README status/quick-start parity, Collector/data-format/architecture/threat-model/roadmap documentation, and `.agent/PROJECT_STATE.md`. Do not market Services/Tasks or GUI as implemented.
13. Run format, Clippy, full workspace tests, three existing CLI smoke commands, new Snapshot command tests, dependency feature inspection, `git diff --check`, Markdown links, privacy/scope review, and independent reviewer analysis.
14. With explicit maintainer authorization, run the safe real-Windows HKCU E2E below, record only non-sensitive counts/status/versions, verify cleanup, and never commit or upload real Snapshot files.
15. Prepare a focused PR linked to #5 and wait for `Rust (windows-latest)` and `Rust (ubuntu-latest)`; do not merge without separate authorization.

## Affected files and modules

Expected production/dependency surface:

- `Cargo.toml`, `Cargo.lock`
- `crates/systemdiff-core/src/lib.rs` and focused core tests
- `crates/systemdiff-windows/Cargo.toml`
- `crates/systemdiff-windows/src/lib.rs`
- new focused Registry/platform modules under `crates/systemdiff-windows/src/`
- `crates/systemdiff-cli/Cargo.toml`
- `crates/systemdiff-cli/src/main.rs` and optionally one focused Snapshot command module

Expected fixtures/integration tests:

- new Registry-only synthetic Snapshot fixtures under `fixtures/snapshots/`
- core, Windows, CLI, and Diff integration tests
- an explicitly gated test-only/manual E2E harness under `scripts/` only after implementation authorization

Expected documentation/state:

- `README.md`, `README.zh-CN.md`
- `docs/architecture.md`
- `docs/data-format.md`
- `docs/collectors.md`
- `docs/threat-model.md`
- `docs/roadmap.md`
- `.agent/PROJECT_STATE.md`
- this ExecPlan

No production changes are expected in `systemdiff-diff`, `systemdiff-report`, `systemdiff-risk`, Services/Tasks artifacts, desktop, CI trigger behavior, or release tooling unless implementation demonstrates a concrete blocker and the maintainer approves the scope change.

## Test strategy

### Cross-platform pure tests

- Target planner: HKCU Shared, x64 explicit Registry32/Registry64, x86 Native, ARM/unknown unsupported, fixed scope order.
- Adapter-independent value sets: missing/empty, one value, multiple unordered values, unnamed value, delimiter-like names, case/Unicode names, and deterministic ordering.
- Decode tables: valid/malformed REG_SZ, EXPAND_SZ, MULTI_SZ, DWORD little/big endian, QWORD, binary/none, unknown type, invalid UTF-16, odd byte count, invalid termination, zero length, embedded NUL, and `raw_evidence: None` for every status.
- Identity: exact Collector-v1 hash vectors, complete name, documented case-insensitive lookup versus the v1 exact-identity limitation, `Foo`/`!Foo`/`*Foo`, invalid UTF-16 units, unnamed/default value round-trip and stable identity, digest collision handling, duplicate prevention, and input-order independence.
- Retry/status: changed -> stable, changed -> changed -> stable, exhausted mutation, missing, denied, unexpected error, too-large value plus normal sibling retention, count/work cap, mixed scope aggregation, and unrelated evidence retention.
- Snapshot assembly: fixed metadata, canonical ordering, unknown privilege, scoped diagnostics, validation, deterministic JSON, and output <= reader maximum.
- CLI: parse `snapshot -o`, non-Windows unsupported, new path success, existing sentinel unchanged, missing parent/directory/open failure, capped-writer overflow before file creation, injected write/flush failure with explicit partial-file warning and no path deletion, canonical `Z`, and generated Snapshot reread/validation.
- Snapshot -> Diff: empty before/one value after yields one Added; partial/denied absence yields Inconclusive/no false Removed; shuffled source produces identical Snapshot and Diff bytes.

### Windows read-only tests

- Compile and exercise the real adapter without elevation.
- Enumerate the documented scopes visible to the runner but do not assert host-specific value counts or names.
- Verify view labels follow detected topology and handles/errors do not panic.
- Do not log Registry value names/data, raw bytes, commands, usernames, SID, or host-specific paths.
- Do not change ACLs or create/delete startup values in the default test suite.

### Required validation commands

```text
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
cargo run --locked -p systemdiff-cli -- diff fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json
cargo run --locked -p systemdiff-cli -- diff --json fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json
cargo run --locked -p systemdiff-cli -- collectors
```

Also run focused generated-Snapshot/reopen/Diff tests, dependency feature trees, `git diff --check`, Markdown-link checks, schema fixture checks, and the real E2E only when separately authorized. Record exact counts/results; never infer success.

## Real Windows E2E procedure

The implementation may add a clearly test-only PowerShell harness, but production Rust must contain no Registry write API. The harness requires both an explicit switch such as `-ConfirmSyntheticRegistryTest` and `SYSTEMDIFF_RUN_SYNTHETIC_E2E=1`; it must not run from PR/push CI or ordinary ignored tests.

Use only:

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\Run
value name: SystemDiffSyntheticE2E-<random GUID without separators>
type: REG_SZ
data: quoted current %SystemRoot%\System32\cmd.exe /d /c exit 0
```

Procedure:

1. Prefer a disposable Windows VM. Create a fresh private temporary directory for `before.json`, `after.json`, and `diff.json`; do not upload artifacts.
2. Confirm the Run key already exists. Read the exact test value first; abort if the name already exists. Never overwrite or delete pre-existing data.
3. Establish `try/finally` cleanup before mutation and compute the exact expected no-op string.
4. Run `systemdiff snapshot -o before.json` and record only the command/exit status and capture timestamp suffix.
5. Create only the test value as REG_SZ and read it back to confirm exact name/type/data. Do not log off or execute the command.
6. Run `systemdiff snapshot -o after.json` and `systemdiff diff before.json after.json` plus JSON output.
7. Assert the expected Registry observation is Added with HKCU, Shared, Run, the complete synthetic name, no RunOnce marker semantics, and zero Removed changes. Any unrelated removal fails acceptance.
8. In `finally`, delete the value only if its current type and full data still exactly match the harness-owned expected value. If another actor changed it, refuse destructive cleanup and require manual inspection.
9. Re-read and require `cleanup_verified_absent=true`. Delete only the harness-owned temporary report files after recording non-sensitive counts. A cleanup failure makes the E2E fail.
10. Provide an idempotent recovery-only mode that removes a leftover value only when its exact type/data match the known no-op. A new run encountering an existing test name aborts rather than overwriting it.

The acceptance record contains commit SHA, UTC start/end, `rustc -Vv`, Windows edition/build/native architecture, standard/elevated only, command exit codes, test totals, `Added=1`, `Removed=0`, expected identity matched, `cleanup_verified_absent=true`, and confirmation that real Snapshots were deleted and never uploaded. It contains no Registry payloads, usernames, SID, hostname, or real report attachments.

## Privacy implications

- Startup values can expose usernames, installation paths, command arguments, tokens, or secrets. Snapshots remain `Unredacted`; CLI/docs must warn that they are sensitive and must not be attached to public Issues.
- Hashes of low-entropy values can be guessed. A hash is comparison evidence, not anonymization.
- Invalid-name UTF-16 hex can be more sensitive than decoded display text, so it appears only when required to preserve authoritative name identity. Collector v1 emits no value-data raw prefixes.
- Diagnostics and CI logs never include observed names or data. Stable scope IDs refer only to the four documented locations/views.
- Host metadata excludes hostname, SID, account name, and stable machine identifiers.
- No network client, telemetry, account, or upload path is introduced.

## Risks

- **False completeness during Registry mutation:** count/FILETIME checks are not atomic. Mitigate with fresh-handle whole-scope retries, bounded per-index growth, Partial on exhausted instability, and no false-removal regressions.
- **FFI memory/logic errors:** lengths mix UTF-16 units and bytes. Mitigate with initialized buffers, checked arithmetic, length validation before slicing, minimal unsafe blocks, RAII handles, and pure decoder tests.
- **Resource exhaustion:** Registry values can be very large and the 64 MiB input limit does not protect collection. Mitigate with per-value/Collector/work caps and Partial rather than truncated hashes.
- **Lossy value names/identity collision:** a Rust String cannot represent every UTF-16 sequence. Mitigate with tagged lossless evidence, an exact version-independent UTF-16 identity algorithm, and explicit digest-collision handling.
- **ARM view ambiguity:** the 32-bit selector varies by calling architecture. Mitigate by marking HKLM unsupported until a tested ARM contract exists.
- **Output overwrite/partial files:** use atomic create-new, capped serialization before creation, and explicit write/flush errors. Never remove a failed output by pathname; leave a potentially partial file for user inspection rather than risk deleting a concurrent replacement.
- **Privacy disclosure:** real Snapshots and raw bytes are sensitive. Mitigate with no uploads, synthetic fixtures, log discipline, and deletion of E2E artifacts.
- **Test harness becomes write functionality:** keep it outside production, dual-gated, HKCU-only, exact-name/data guarded, and manually authorized.
- **Scope growth:** metadata/schema work may tempt generic frameworks. Keep the two schema changes and one assembly helper tied to demonstrated Collector requirements.

## Rollback and compatibility

This work changes the unreleased draft-v1 Registry value-name field and adds an optional Diagnostic scope before v0.1. Update all golden fixtures. Old Registry observations with a bare string name are explicitly rejected by the tagged-name schema; an older Diagnostic with no `scope_id` is intentionally accepted as `None` and documented as Collector-wide. The PR is reversible as one vertical slice; reverting restores the fixture-only state and removes `snapshot` rather than leaving a half-implemented Collector advertised as available.

Once v0.1 ships, the released schema, Collector ID/version, scope IDs, view semantics, identity algorithm, diagnostic codes, and decode rules become compatibility obligations. Future behavior changes require an explicit Collector version and, where wire-incompatible, a new schema route.

No migration framework is introduced in this issue. Existing synthetic Diff semantics and report output must remain deterministic.

## Progress

- [x] 2026-08-11: Squash-merged PR #4, confirmed issue #3 closed, synchronized local/remote main, removed the fully integrated local branch after tree equality, and observed green merge CI on Windows and Ubuntu.
- [x] 2026-08-11: Confirmed no equivalent open or closed Issue and created focused product issue #5.
- [x] 2026-08-11: Completed read-only explorer, Microsoft Windows API, dependency, and test/E2E investigations.
- [x] 2026-08-11: Drafted this implementation plan without production code changes.
- [x] 2026-08-11: Closed four Medium and one Low plan-review findings; the second independent review reported High 0 / Medium 0 / Low 0.
- [x] 2026-08-11: Maintainer approved the schema, Windows floor, ARM limitation, resource budgets, and dual-gated E2E, then authorized implementation.
- [x] 2026-08-11: Verified value-name casing semantics against Microsoft documentation and an isolated build-26100 HKCU experiment; alternate-case `RegSetValueExW` kept one value and first enumeration casing, and exact-scope cleanup was verified.
- [x] Implemented the approved vertical slice: lossless/scoped core schema updates, pure assembly, bounded Win32 Registry adapter, strict normalization, `snapshot` CLI, Registry-only fixtures, and the guarded E2E harness.
- [x] Ran the dual-gated real HKCU E2E: the synthetic value was absent before, present after, classified as exactly one Added change with zero Removed changes, and exact-data cleanup plus temporary-file deletion were verified.
- [x] Addressed the first implementation review (High 1 / Medium 3): native value names now participate in the aggregate retained-evidence budget with order-independent bounded selection, LocalSystem is identified from TokenUser before elevation classification, canonical fixtures use production v1 scopes/identity, and the Win32 buffer/resource state machine has direct regression coverage.
- [x] Hardened the E2E harness with a random guarded value name and non-recursive exact-entry temporary cleanup, then reran the real E2E and recovery check successfully.
- [x] Completed final local gates and third-round independent review; reviewer reported High 0 / Medium 0 / Low 0.
- [x] Created commit `c3966a3`, pushed the feature branch, and opened PR #6 linked to Issue #5.
- [ ] Fix the Ubuntu target-specific dead-code Clippy failure and wait for both required GitHub CI checks.

## Discoveries

- The existing Collector trait, Registry artifact, coverage validation, Diff, and generic JSON reporter are sufficient; redesigning them would be premature.
- `Diagnostic` lacks a structured scope link, which becomes ambiguous as soon as six independent Registry scopes exist.
- Registry value data can be invalid UTF-16 and string types can lack documented terminators; full bytes must be hashed before strict decoding.
- Registry value names arrive as UTF-16 units, and Microsoft does not guarantee that every stored sequence is representable as a Rust String. Lossy conversion would corrupt evidence and identity.
- `RegQueryInfoKeyW` maxima/count/last-write are momentary hints, not an atomic Snapshot. `RegEnumValueW` can require same-index buffer retry and returns values in unspecified order.
- `HKEY` from the low-level windows-rs binding needs project-owned RAII; the official higher-level `windows-registry` iterator's lossy/error-hiding behavior is insufficient for forensic collection.
- A 64 MiB file-reader ceiling does not bound native collection allocation. The Registry Collector needs small, explicit local limits.
- ARM64 `KEY_WOW64_32KEY` semantics depend on calling architecture, so the current Registry32 label is not enough to promise cross-build ARM equivalence.
- Snapshot file no-overwrite belongs in CLI; report remains stream-only. Snapshot assembly can be one pure core function rather than a builder framework.
- Registry value lookup is case-insensitive, but no documented Windows API produces a durable cross-platform token guaranteed equivalent to Registry matching. Exact UTF-16 is the safer v1 evidence identity because a visible false split is preferable to a hidden false merge.
- Native Registry value names are attacker-controlled evidence too; a value-data-only budget leaves a large allocation gap. The Collector budget therefore accounts for retained UTF-16 name bytes and native data bytes together.

## Decisions

- Use lower-level `windows` 0.62.2 bindings with narrow features, a private RAII HKEY, and a narrow Registry source interface. Do not use command output or the current high-level `windows-registry` iterator.
- Use `KEY_QUERY_VALUE` only, plus exactly one WOW selector where required.
- Use six scopes on x64, four on sole-view x86, and honest Unsupported HKLM scopes on ARM/unknown; HKCU Shared remains available.
- Use three total whole-scope attempts and bounded same-index buffer growth. Exhausted mutation/resource conditions produce Partial, never invented absence.
- Add only the two schema changes demonstrated by real collection: scoped diagnostics and lossless tagged Registry value names.
- Hash complete native data bytes before strict decoding. Emit no value-data raw prefix in Collector v1; revisit only for a concrete, privacy-reviewed forensic need.
- Use a fully specified domain-separated SHA-256 over exact UTF-16 name units. Perform no case/Unicode normalization, retain prefix participation, and reject digest collisions.
- Add one pure Snapshot assembly helper and keep clock, Windows metadata, and filesystem output at outer boundaries.
- Use canonical UTC `Z`, serialize through a capped 64 MiB writer before output creation, and create the destination with `create_new(true)`. Report but do not path-delete a partial file after write/flush failure.
- Keep the write-based synthetic procedure test-only, dual-gated, HKCU-only, guarded against pre-existing/mutated values, and absent from default CI.
- Treat Windows 10/Server version 1709 as the initial runtime floor for this vertical slice because it provides `IsWow64Process2`; document the floor rather than guessing on older systems.
- Treat the exact-name identity as a documented Collector-v1 limitation rather than Windows logical-name semantics; do not substitute unverified NLS uppercase/sort/hash tokens.

## Maintainer approvals

Implementation is authorized with these decisions:

1. Minimum runtime is Windows 10 version 1709 / Windows Server 2016 version 1709, aligned with `IsWow64Process2` support.
2. ARM64 v1 collects HKCU Shared and marks HKLM alternate-view coverage Unsupported without expanding `RegistryView`.
3. Tagged lossless `RegistryValueName` and scoped `Diagnostic` are approved pre-v0.1 schema changes.
4. Capture limits are 8 MiB of native data per Registry value, 32 MiB aggregate retained native Registry value-name and value-data evidence, and 4,096 values per scope. These are SystemDiff resource limits, not Windows platform limits.
5. The dual-gated, test-only, HKCU-only write harness is approved. No Registry write API enters the production CLI, Collector, or public library API.

## Final validation

Local validation completed on 2026-08-11 with stable `rustc 1.97.1` (`x86_64-pc-windows-msvc`) on Windows build 26100:

- `cargo fmt --all --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace --all-targets`: 71 passed, 0 failed, 0 ignored.
- Both fixture Diff commands and `collectors`: exit 0. Terminal fixture totals remained Added 2, Modified 1, Removed 1, Inconclusive 1; JSON contained the same five changes; Registry startup reported Implemented while Services and Scheduled Tasks remained Planned.
- The final dual-gated real Windows E2E captured an absent synthetic HKCU Shared Run value before mutation, observed the exact synthetic `REG_SZ` after mutation, classified exactly one Added and zero Removed changes with the expected identity, verified exact-data guarded deletion, deleted both temporary Snapshots, and confirmed absence again through recovery mode. No real Snapshot was retained, uploaded, or committed.
- `git diff --check`, modified-Markdown local-link checks, dependency-feature inspection, production Registry-write API search, local-path/secret scan, and English/Chinese README factual-parity review passed.
- The final independent reviewer reported High 0 / Medium 0 / Low 0 after confirming enumeration-count mismatch retry, resource-cap semantics, evidence budgets, LocalSystem metadata, canonical fixtures, buffer tests, read-only boundaries, E2E cleanup, schema, privacy, and documentation.

Commit/PR identifiers and the two required remote checks will be recorded in the PR and final maintainer report because a commit cannot contain its own hash.
