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

## Snapshot requirements

A snapshot records:

- document and schema version;
- SystemDiff version and UTC RFC 3339 capture time;
- Windows version/build/architecture when available, without requiring hostname or machine ID;
- privilege/elevation state;
- enabled Collector IDs;
- Collector ID/version, aggregate status, scope coverage, and diagnostics;
- redaction status and optional policy identifier;
- typed observations.

Diagnostics include a stable code, collection stage, and optional Win32/HRESULT numeric value. Localized error messages are for humans only.

## Registry value evidence

Registry artifacts preserve the value name and the numeric Windows Registry type code independently from interpretation. The draft typed decoding supports strings, unexpanded expandable strings, multi-strings, DWORDs, and QWORDs; binary, unknown, or malformed values can remain undecoded with an explicit `not_applicable`, `unsupported_type`, or `invalid_data` status. The type code remains authoritative, and decoded kinds are validated against it, so a future Collector never needs to coerce every value into UTF-16LE text.

Every Registry artifact includes lowercase SHA-256 of the complete native value bytes. This keeps two undecoded values distinguishable even when neither retains raw bytes, and keeps truncated prefixes from hiding a changed suffix. Optional raw evidence is limited to the first 4 KiB, records captured/original byte counts and truncation, and uses validated lowercase hex rather than JSON arrays of byte integers. Hex costs two JSON characters per byte but needs no ambiguous binary codec and remains substantially smaller than integer arrays.

Hashes and raw prefixes are still sensitive: low-entropy values can be guessed, and Registry data may contain paths, usernames, commands, or secrets. Raw evidence is included only for a concrete forensic reason rather than duplicated for every decoded value, and both forms must be covered by redaction/share policy.

## Observation identity

The logical key is:

```text
collector_id + scope_id + artifact_kind + canonical_identity
```

Collectors own canonicalization and version it through their Collector version. Raw casing and display values remain in evidence. Duplicate keys make the snapshot invalid for diffing.

## Diff semantics

Output order is stable. A change contains a deterministic document-local opaque change ID, artifact key, and one of:

- Added: after evidence only;
- Removed: before evidence only;
- Modified: both evidence values;
- Unchanged: both values, only when explicitly requested;
- Inconclusive: an absence cannot support a stronger claim because coverage is incomplete or a Collector/scope is missing.

Missing evidence becomes Added/Removed only when the relevant scope is complete and compatible on both sides. Permission-denied or partial scopes produce Inconclusive results or coverage warnings. When the same identity is directly observed on both sides, changed evidence is Modified even if the broader scope is partial; the coverage warning remains.

If the same Collector ID has different Collector versions in the two snapshots, draft v1 rejects the diff instead of guessing compatibility. This is the current conservative default, not a permanent claim that cross-version comparison is impossible. A future version may explicitly register and test backward-compatible comparison semantics for particular Collector versions; no compatibility or migration framework exists yet. A Collector that exists on only one side remains a local coverage problem and produces Inconclusive absence where evidence is affected.

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
