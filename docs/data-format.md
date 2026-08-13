# Data format

## Status

The repository contains a **draft v1** wire model used to prove boundaries and fixtures. It is not a released compatibility promise until v0.1. After v0.1, an existing schema version must never be redefined in place.

## Document envelope

Snapshots and diffs are separate document families:

```json
{
  "document_type": "systemdiff.snapshot",
  "schema_version": 1,
  "systemdiff_version": "0.0.0-dev",
  "captured_at": "2026-08-11T00:00:00Z"
}
```

A diff uses `systemdiff.diff` and its own schema version. Application versions and schema versions evolve independently.

Readers inspect `document_type` and `schema_version` before deserializing the remaining body. Unknown major versions are rejected with a clear error. Additive top-level metadata may be tolerated, but an unknown typed artifact is never silently discarded.

The CLI accepts and generates Snapshot files up to 64 MiB. Input handling checks metadata before allocating the full input, performs a bounded read of at most the supported maximum plus one byte, and rechecks the actual byte count before JSON decoding. Output is fully serialized through a capped writer before a destination is created. The destination uses create-new semantics and is never overwritten; a write/flush failure reports that the newly created path may be incomplete rather than deleting by pathname. This fixed ceiling provides headroom for targeted v0.1 evidence while bounding parser amplification; it is not a streaming parser or a configurable resource-policy framework.

After the bounded read, the core first deserializes a minimal header containing only `document_type` and `schema_version`. Only `systemdiff.snapshot` schema v1 is then routed to the current `Snapshot` wire type. This header pass still scans the bounded JSON to skip unrelated fields; it does not construct a generic JSON DOM or the full Snapshot body before routing.

## Snapshot requirements

A snapshot records:

- document and schema version;
- SystemDiff version and a valid UTC RFC 3339 capture time;
- Windows version/build/architecture when available, without requiring hostname or machine ID;
- privilege/elevation state;
- enabled Collector IDs;
- Collector ID/version, aggregate status, scope coverage, and diagnostics;
- redaction status and optional policy identifier;
- typed observations.

Diagnostics include a stable code, collection stage, and optional Win32/HRESULT numeric value. Localized error messages are for humans only.

Readers accept known UTC expressed with `Z` or `+00:00` and reject non-zero offsets. RFC 3339 `-00:00` means that the local offset is unknown, so it is not accepted as a known UTC assertion. Snapshot readers preserve the original valid wire string; SystemDiff-generated Snapshots emit canonical `Z`.

## Registry value evidence

Registry artifacts preserve the value name and the numeric Windows Registry type code independently from interpretation. `value_name` is a tagged lossless value: valid Unicode is serialized as `{"encoding":"decoded","value":"..."}`, while invalid UTF-16 is preserved as exact lowercase UTF-16LE hex. Empty/default value names are valid evidence and round-trip without special casing. The draft typed value-data decoding supports strings, unexpanded expandable strings, multi-strings, DWORDs, and QWORDs; binary, unknown, or malformed values can remain undecoded with an explicit `not_applicable`, `unsupported_type`, or `invalid_data` status. The type code remains authoritative, and decoded kinds are validated against it, so the Collector never needs to coerce every value into UTF-16LE text.

Each startup artifact explicitly identifies whether it came from a `run` or `run_once` key. A Run entry has no RunOnce prefix semantics. A RunOnce entry records exactly one structured interpretation derived from the complete value name:

- `no_documented_prefix` for an ordinary name;
- `defer_deletion_until_after_run` for a leading `!`;
- `run_in_safe_mode` for a leading `*`;
- `undocumented` for combined, repeated, or marker-only prefix forms that Microsoft does not define.

The structured interpretation never replaces or strips the complete `value_name`. The full name, including any prefix, remains raw evidence and part of the Collector-owned canonical identity. `Foo`, `!Foo`, and `*Foo` therefore remain distinct observations. The draft schema rejects a `startup_kind` inconsistent with the final `Run`/`RunOnce` key-path component, a Run entry carrying RunOnce semantics, or a RunOnce interpretation inconsistent with its raw name.

Every Registry artifact includes lowercase SHA-256 of the complete native value bytes. This keeps two undecoded values distinguishable even when neither retains raw bytes. Optional raw evidence is schema-bounded to the first 4 KiB, records captured/original byte counts and truncation, and uses validated lowercase hex rather than JSON arrays of byte integers. Registry startup Collector v1 always emits `raw_evidence: null`: no concrete need currently outweighs the privacy and Snapshot-size cost of duplicating native bytes.

Registry startup collection also applies explicit SystemDiff resource budgets: 8 MiB of native data per value, 32 MiB of retained native value-name and value-data evidence across the Collector, and 4,096 values per scope. These are capture limits, not Windows platform limits. Exceeding one creates a scoped diagnostic and `partial` coverage; a value without complete native evidence is not emitted or hashed as if complete, while fully captured sibling observations are retained.

Hashes and raw prefixes are still sensitive: low-entropy values can be guessed, and Registry data may contain paths, usernames, commands, or secrets. Raw evidence is included only for a concrete forensic reason rather than duplicated for every decoded value, and both forms must be covered by redaction/share policy.

### Registry view labels

`registry_view` records the Windows view actually used for acquisition:

- `shared`: Microsoft documents the key as shared across WOW64 logical views; it is collected once.
- `registry32`: the 32-bit logical view selected explicitly with `KEY_WOW64_32KEY` where alternate views exist.
- `registry64`: the 64-bit logical view selected explicitly with `KEY_WOW64_64KEY` where alternate views exist.
- `native`: the sole view for a key on a Windows installation where that key has no WOW64 alternate logical views.

`native` never means “omit a WOW64 selector and use the Collector process default” for a redirected key. That behavior changes with process bitness and is not stable evidence.

## Observation identity

The logical key is:

```text
collector_id + scope_id + artifact_kind + canonical_identity
```

Collectors own canonicalization and version it through their Collector version. Raw casing and display values remain in evidence. Duplicate keys make the snapshot invalid for diffing. Diagnostics may carry a `scope_id`; when present, it must reference one of the same Collector run's coverage scopes. A missing `scope_id` remains a collector-wide diagnostic.

For Registry startup entries, Collector v1 computes a domain-separated SHA-256 over the exact value-name UTF-16 code units and their length. Prefixes and empty names are not stripped or normalized. Windows Registry value lookup itself is case-insensitive, but Microsoft does not provide a documented persistent canonical representation that can be generated independently in two Snapshots and compared cross-platform. Exact-code-unit identity is therefore a conservative pre-v0.1 limitation: it prevents false merges but could expose a casing-only logical update as Removed + Added if enumerated casing changes. This is not a permanent claim about Registry identity; changing the algorithm requires a new Collector version and regression fixtures.

For Windows services, Collector v1 uses the same versioned/domain-separated exact-UTF-16 strategy over the service name; display name, PID, state, and configuration are not identity. The service artifact now requires `load_order_group` and `tag_id` fields in addition to raw service/start/error values, binary path, account, ordered dependencies, delayed-auto-start, and description. This is a deliberate pre-v0.1 correction to the draft wire shape; old draft Service objects missing the two fields are rejected, while explicit `null` means the complete query established configured absence. Query failure never becomes `null`: the whole item is omitted and coverage remains partial.

Services v1 retains exact dependency order/casing and documented `+` group prefixes. Its real `current_token.win32` scope is always partial because SCM enumeration may silently omit status-inaccessible services. A same-identity item observed on both sides can still be Modified, but one-sided absence is Inconclusive. Focused synthetic fixtures may use complete coverage solely to regression-test the generic Added/Removed semantics.

## Diff semantics

Output order is stable. A change contains a deterministic document-local opaque change ID, artifact key, and one of:

- Added: after evidence only;
- Removed: before evidence only;
- Modified: both evidence values;
- Unchanged: both values, only when explicitly requested;
- Inconclusive: an absence cannot support a stronger claim because coverage is incomplete or a Collector/scope is missing.

Missing evidence becomes Added/Removed only when the relevant scope is complete and compatible on both sides. Permission-denied or partial scopes produce Inconclusive results or coverage warnings. When the same identity is directly observed on both sides, changed evidence is Modified even if the broader scope is partial; the coverage warning remains.

If the same Collector ID has different Collector versions in the two snapshots, draft v1 rejects the diff instead of guessing compatibility. This is the current conservative default, not a permanent claim that cross-version comparison is impossible. A future version may explicitly register and test backward-compatible comparison semantics for particular Collector versions; no compatibility or migration framework exists yet. A Collector that exists on only one side remains a local coverage problem and produces Inconclusive absence where evidence is affected.

Draft v0.1 comparisons require Snapshot A and B to come from the same Windows installation and the same user/principal context. Cross-host and cross-user comparison is not supported, and the wire format does not introduce a persistent machine token or SID identity subsystem.

Change IDs are opaque references within a Diff document. They must not embed canonical paths, usernames, or other evidence because redaction cannot safely remove data that is duplicated into an identifier.

## Redaction metadata

Every snapshot states whether it is `unredacted`, `redacted`, or `unknown`. A redacted document includes a stable policy ID/version once the sanitizer exists. Editing JSON manually does not make it a verified redacted report.

## Determinism

- Use ordered structures for serialized maps and explicitly sort vectors whose API order is unspecified.
- Exclude collection timestamps, service PIDs, task next-run times, and similar volatile values from default artifact comparison.
- Never base identity on object serialization order or localized display text.
- Golden fixtures test semantic round-trip and byte-stable generated diffs.

## Compatibility policy

Before v0.1, draft fixtures may change with review. Starting at v0.1:

1. keep historical read fixtures;
2. route each supported version to explicit wire types;
3. use a new schema version for breaking changes;
4. document migration or rejection behavior;
5. never silently reinterpret an old field.
