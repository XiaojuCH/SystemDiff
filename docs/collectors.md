# Collector catalog and platform notes

## Collector contract

Collectors translate messy Windows APIs into deterministic typed observations. Each Collector declares a stable ID/version, description, privilege expectations, scopes, aggregate status, observations, and diagnostics.

A successful API call is not proof of complete system coverage. Collectors continue across readable scopes/items and record access denial, concurrent mutation, disappearing objects, unavailable services, and invalid data.

No MVP Collector writes to the system or executes observed evidence.

## Registry startup entries

Planned ID: `windows.registry.startup`

v0.1 covers only the four documented Run/RunOnce logical locations under HKLM and the current HKCU. It is not a complete persistence scan.

Implementation source:

- `RegOpenKeyExW`, `RegQueryInfoKeyW`, and `RegEnumValueW`.
- On 64-bit Windows, enumerate HKLM `Software` in explicit 64-bit and 32-bit views; do not address `Wow6432Node` directly.
- On Windows 7 and later, HKCU `Software` is shared and is collected once.
- Preserve the numeric Registry type, a typed safe-decode outcome, original unexpanded `REG_EXPAND_SZ`, and original names/casing. Do not assume every value is UTF-16LE text.
- Compute SHA-256 over the complete native value bytes before decoding or truncation so undecoded values still compare reliably.
- Retain a raw prefix only when it adds concrete forensic value, encode it as lowercase hex, enforce the 4 KiB schema limit, and record truncation; ordinary decoded Run values do not need duplicated raw payloads by default.
- Treat value order as unstable and bound any retry when a key changes during enumeration.
- A missing Run key is a complete empty scope, not an error.

RunOnce `!` and `*` prefixes are recorded as derived evidence. Environment expansion, command-line parsing, executable discovery, hashing, and signature checks are later enrichment stages.

Official references: [Run and RunOnce](https://learn.microsoft.com/windows/win32/setupapi/run-and-runonce-registry-keys), [WOW64 affected keys](https://learn.microsoft.com/windows/win32/winprog64/shared-registry-keys), [alternate registry views](https://learn.microsoft.com/windows/win32/winprog64/accessing-an-alternate-registry-view), [RegEnumValueW](https://learn.microsoft.com/windows/win32/api/winreg/nf-winreg-regenumvaluew).

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
- Privileged tests that create temporary keys/services/tasks are opt-in, use disposable names, and never touch real Run/RunOnce locations.
