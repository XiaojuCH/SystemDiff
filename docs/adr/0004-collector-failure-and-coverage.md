# ADR 0004: Collector failure and coverage semantics

- Status: Accepted
- Date: 2026-08-11

## Context

Windows enumeration is non-atomic and access-control dependent. Registry views differ, service enumeration can omit protected services, task folders can deny access, and objects can change during collection. Treating every empty result as complete would create false removals and misleading security findings.

## Decision

Collectors fail independently and return aggregate and per-scope coverage. Initial statuses are `complete`, `partial`, `permission_denied`, `unavailable`, `unsupported`, and `failed`. Diagnostics use stable codes plus optional numeric Win32/HRESULT values.

Diff may assert Added/Removed only when the relevant Collector scope is complete in both snapshots. Otherwise absence becomes Inconclusive with a coverage reason. Directly observed before/after evidence for the same identity may still establish Modified while retaining a broader coverage warning.

Aggregate and per-scope statuses must be internally consistent: a `complete` Collector has only complete scopes; a `partial` Collector has at least one incomplete scope; unavailable/denied/unsupported/failed Collectors cannot claim a complete scope or emit observations. Within a partial Collector, only `complete` or `partial` scopes may carry observations; denied/unavailable/unsupported/failed scopes cannot. Draft v1 rejects the whole diff when the same Collector ID has different versions rather than guessing compatibility. This is a conservative current default, not a permanent prohibition: a future decision may define explicitly verified backward-compatible comparison semantics for particular versions. No compatibility registry or migration framework is introduced now. One Collector failure never invalidates successful evidence from other Collectors.

The initial Collector API is synchronous. Blocking Windows APIs and COM apartment rules are kept visible rather than hidden behind an async runtime. Concurrency can be added later at the orchestrator boundary if measurements justify it.

## Consequences

- Snapshots carry more operational metadata.
- Reports must explain incomplete visibility without implying danger or safety.
- Diff code and fixtures must test complete/partial transitions.
- Collector implementations cannot treat API success as proof of exhaustive coverage without platform evidence.
