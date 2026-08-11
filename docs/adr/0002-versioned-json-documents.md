# ADR 0002: Explicitly versioned snapshot and diff documents

- Status: Accepted
- Date: 2026-08-11

## Context

Snapshots and diffs are long-lived evidence, issue attachments, test fixtures, and an integration surface for professional users. Application version alone cannot describe wire compatibility. Silent reinterpretation would damage trust.

## Decision

Snapshots and diffs have separate `document_type` and integer `schema_version` fields. Application version remains metadata. Readers inspect the header before routing to explicit versioned wire types.

Serialization is deterministic through fixed struct fields, ordered maps, and explicit sorting. A released schema version is immutable in meaning. Breaking changes receive a new version and retained compatibility fixtures. Unknown typed artifacts are errors rather than silently discarded.

Redaction status is present from v1. Hostname and stable machine ID are excluded from the MVP envelope unless a demonstrated use case outweighs the privacy cost.

## Consequences

- Version routing and historical fixtures add maintenance work.
- Professional tooling can rely on documented formats deliberately.
- Additive metadata should remain possible, while artifact evolution needs explicit compatibility design.
- Draft v1 may change before v0.1; the first release turns it into a compatibility obligation.
