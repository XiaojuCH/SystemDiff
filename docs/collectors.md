# Collector catalog and platform notes

## Collector contract

Collectors translate messy Windows APIs into deterministic typed observations. Each Collector declares a stable ID/version, description, privilege expectations, scopes, aggregate status, observations, and diagnostics.

A successful API call is not proof of complete system coverage. Collectors continue across readable scopes/items and record access denial, concurrent mutation, disappearing objects, unavailable services, and invalid data.

No MVP Collector writes to the system or executes observed evidence.

## Registry startup entries

Implemented ID/version: `windows.registry.startup` v1

The pre-v0.1 implementation covers only the four documented Run/RunOnce logical locations under HKLM and the current HKCU. It is not a complete persistence scan. The current minimum platform is Windows 10 version 1709 or Windows Server 2016 version 1709, matching the documented minimum for `IsWow64Process2`.

Implementation source:

- `RegOpenKeyExW`, `RegQueryInfoKeyW`, and `RegEnumValueW`.
- On 64-bit Windows, enumerate HKLM `Software` in explicit 64-bit and 32-bit views; do not address `Wow6432Node` directly.
- On Windows 7 and later, HKCU `Software` is shared and is collected once.
- On ARM64, v1 still collects both HKCU Shared scopes but reports all HKLM alternate-view scopes as `unsupported`. This is a conservative Collector limitation, not a claim that Windows on ARM lacks alternate Registry views.
- Emit `Registry32` only after explicitly selecting `KEY_WOW64_32KEY`, and emit `Registry64` only after explicitly selecting `KEY_WOW64_64KEY`. These labels describe logical Registry views, not processor-specific physical stores.
- Emit `Shared` only for keys Microsoft documents as shared. Emit `Native` only when the target key has one system view and no WOW64 alternate logical views. A Collector must never use `Native` as a shortcut for omitting a selector on a redirected key, because the resulting default depends on process bitness.
- Preserve the numeric Registry type, a typed safe-decode outcome, original unexpanded `REG_EXPAND_SZ`, and exact UTF-16 value-name code units. Valid Unicode names use a decoded string; invalid UTF-16 names use lossless lowercase UTF-16LE hex rather than replacement characters. Do not assume every value is UTF-16LE text.
- Compute SHA-256 over the complete native value bytes before decoding or truncation so undecoded values still compare reliably.
- Registry startup Collector v1 does not retain a value-data raw prefix. Typed evidence, decode status, numeric type, and a complete native-byte SHA-256 avoid duplicating potentially sensitive bytes. The schema's bounded optional raw-evidence field remains available only for a later, separately reviewed privacy need.
- Treat value order as unstable and bound any retry when a key changes during enumeration.
- A missing Run key is a complete empty scope, not an error.
- Every scope is independent. Access denial, concurrent mutation, invalid data, or a SystemDiff capture limit produces a scoped status/diagnostic without discarding complete sibling evidence or aborting unrelated scopes.

SystemDiff capture limits are 8 MiB of native data per Registry value, 32 MiB of retained native value-name and value-data evidence for this Collector, and 4,096 enumerated values per scope. These are product resource budgets, not Windows Registry platform limits. An omitted over-limit item makes its scope `partial`; it is never represented by a truncated hash or incomplete observation, and normal sibling observations remain available.

RunOnce prefix evidence follows Microsoft's documented value-name behavior:

- By default, a RunOnce value is deleted before its command runs. A leading `!` defers deletion until after the command runs.
- Run and RunOnce keys are ignored in Safe Mode by default. A leading `*` makes a RunOnce value run in Safe Mode.
- Microsoft does not define combined, repeated, or marker-only prefix forms. Preserve those complete names as raw evidence and mark their structured interpretation `undocumented`; do not infer that both documented behaviors apply.

The complete value name, including `!` or `*`, remains evidence and part of canonical identity. `Foo`, `!Foo`, and `*Foo` cannot collapse to the same observation. An empty/default value name is ordinary observable evidence with a stable identity and no inferred prefix semantics.

Windows Registry value lookup is case-insensitive. Collector v1 nevertheless hashes the exact authoritative UTF-16 code units with a versioned, domain-separated SHA-256 because Microsoft does not expose a documented, durable canonical form suitable for cross-platform Snapshot diffing. This conservative pre-v0.1 limitation avoids silently merging distinct raw evidence but could show a visible Removed + Added pair if Windows ever returns different display casing for the same logical value across captures. A future correction requires a new Collector version and explicit compatibility tests; v1 does not use ad hoc Unicode folding. Environment expansion, command-line parsing, executable discovery, file hashing, and signature checks are later enrichment stages.

Official references: [Run and RunOnce](https://learn.microsoft.com/windows/win32/setupapi/run-and-runonce-registry-keys), [WOW64 affected keys](https://learn.microsoft.com/windows/win32/winprog64/shared-registry-keys), [alternate registry views](https://learn.microsoft.com/windows/win32/winprog64/accessing-an-alternate-registry-view), [RegEnumValueW](https://learn.microsoft.com/windows/win32/api/winreg/nf-winreg-regenumvaluew), and the documented [case-insensitive Registry value-name lookup](https://learn.microsoft.com/dotnet/api/microsoft.win32.registrykey.getvalue).

## Windows services

Planned ID: `windows.services`

v0.1 covers Win32 service configuration and explicitly excludes drivers.

Implementation source:

- `OpenSCManagerW` and `EnumServicesStatusExW` for enumeration.
- `OpenServiceW`, `QueryServiceConfigW`, and narrowly selected `QueryServiceConfig2W` levels for configuration.
- Service name is identity; display name is evidence, not identity.
- Stable configuration includes raw type/start/error-control values, binary path, account, dependencies, load group/tag, delayed-auto-start, and description when readable.
- PID, checkpoint, wait hint, and transient run status do not participate in the default configuration diff.
- A service that vanishes between enumeration and query produces an item diagnostic rather than failing the Collector.

`EnumServicesStatusExW` can omit services the caller cannot query. Non-elevated coverage is therefore current-token/best-effort even if enumeration returns success. Per-user services with `_LUID` suffixes retain their full names; explanation rules may reduce noise, but collection never merges them.

Official references: [EnumServicesStatusExW](https://learn.microsoft.com/windows/win32/api/winsvc/nf-winsvc-enumservicesstatusexw), [QUERY_SERVICE_CONFIGW](https://learn.microsoft.com/windows/win32/api/winsvc/ns-winsvc-query_service_configw), [service access rights](https://learn.microsoft.com/windows/win32/services/service-security-and-access-rights), [per-user services](https://learn.microsoft.com/windows/application-management/per-user-services-in-windows).

## Scheduled tasks

Planned ID: `windows.scheduled_tasks`

v0.1 uses Task Scheduler 2.0 COM, connects with the current token, recursively traverses folders, and includes hidden tasks.

- Use `ITaskService`, `ITaskFolder`, and `IRegisteredTask`; do not parse `schtasks.exe` or PowerShell output.
- Balance COM initialization on the Collector thread with an RAII guard.
- Use the full API-returned task path as the initial identity and sort folder/task results.
- Preserve enabled/hidden state, principals, actions, triggers, relevant settings, registration metadata, schema version, and raw XML as sensitive local evidence.
- Exclude last/next run time, state, missed runs, and last result from default configuration comparison.
- Continue across denied folders/tasks and mark the scope partial.

Ordinary users often cannot read tasks created by other principals. The product must say that current-token coverage is incomplete rather than showing a clean bill of health.

Official references: [Task Scheduler interfaces](https://learn.microsoft.com/windows/win32/taskschd/task-scheduler-interfaces), [ITaskService::Connect](https://learn.microsoft.com/windows/win32/api/taskschd/nf-taskschd-itaskservice-connect), [GetTasks](https://learn.microsoft.com/windows/win32/api/taskschd/nf-taskschd-itaskfolder-gettasks), [task security contexts](https://learn.microsoft.com/windows/win32/taskschd/security-contexts-for-running-tasks).

## Targeted filesystem (post-MVP candidate)

Do not recursively hash an entire system drive. Any future Collector starts with explicit roots, exclusions, link/reparse-point handling, size limits, hashing policy, privacy review, and deterministic fixture strategy.

## Test expectations

- Pure normalization/identity tests run without Windows.
- Synthetic fixtures cover unordered input, invalid encodings, access denial, pagination, concurrent deletion/change, and incomplete coverage.
- Default Windows smoke tests are read-only and non-elevated.
- Default CI never writes Registry, service, or task state.
- The only write-capable Registry E2E is the test-only `scripts/run-registry-startup-e2e.ps1` harness. It requires an environment-variable gate and an explicit switch, writes one clearly synthetic HKCU Run value, refuses any existing matching value, verifies exact type/data before guarded cleanup, needs no administrator access, and is not linked into the production CLI or Collector API.
