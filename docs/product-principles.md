# Product principles

These principles are product constraints, not marketing language. A change that conflicts with them requires explicit maintainer review and, when architectural, an ADR.

## Simple outside, powerful inside

SystemDiff should answer “What changed on my Windows system?” without requiring ordinary users to understand Windows internals. Explanations use plain language, calibrated attention levels, and no fearmongering. “Unusual” never means “malicious.”

Advanced users must be able to inspect the exact evidence: paths, registry locations, raw before/after values, timestamps, hashes and signature metadata when available, collector and rule identifiers, structured JSON, and deterministic CLI output. The GUI must layer explanation over evidence, never replace or hide it.

## Offline-first

Core collection, diffing, analysis, and reporting work locally. No account is required. System data is not uploaded by default. The MVP has no telemetry. Any future cloud or AI feature must be optional, explicit, and separable from the local core.

## Read-only by default

The initial product observes and reports. It does not delete files or registry entries, kill processes, remove services, modify scheduled tasks or firewall rules, disable security products, or automatically “clean” a system.

Remediation, if ever considered, is a separate product boundary requiring explicit maintainer approval, a dedicated threat model, and safeguards independent from evidence collection.

## Evidence before judgment

A finding contains:

1. what changed;
2. the underlying evidence;
3. why it may matter;
4. a confidence or classification;
5. an optional plain-language explanation.

Prefer `informational`, `expected`, `noteworthy`, `suspicious`, and `unknown` to simplistic safe/malicious labels. Rules enrich evidence; they do not rewrite it.

## Graceful privilege handling

SystemDiff works without administrator privileges wherever possible. A collector or scope that cannot be read reports `unavailable`, `partial`, `permission_denied`, `unsupported`, or `failed` with stable diagnostics. One collector failure does not destroy an otherwise useful snapshot.

Incomplete coverage must not be interpreted as a confirmed removal.

## Privacy-aware reports

Snapshots and reports may expose usernames, paths, installed software, network configuration, hashes, hostnames, task definitions, and other sensitive data. Schema design includes redaction status from the beginning. Sharing guidance assumes reports are sensitive until a tested sanitizer says otherwise.

## Stable machine-readable data

Snapshot and diff formats are explicit, versioned documents. Deterministic output and compatibility fixtures allow professional users to build reliable tooling. Public schema changes are reviewed and documented; released schema versions are never silently redefined.

## Localization without semantic drift

Machine-readable identifiers, schema fields, rule IDs, and evidence remain language-neutral. User-facing applications are designed for at least `en-US` and `zh-CN`; explanations are localized outside business logic.
