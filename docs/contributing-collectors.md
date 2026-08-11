# Contributing a Collector

Collector contributions are welcome, including API research and fixtures from contributors who do not write Rust.

## Start with a proposal

Open a New Collector proposal before implementation. Describe:

- the Windows subsystem and user problem;
- exact evidence and stable identity;
- official API or data source;
- supported Windows versions/architectures;
- privilege requirements and partial-coverage behavior;
- stability/volatility and normalization rules;
- privacy exposure and redaction needs;
- synthetic sample output and test strategy.

Do not present a location as comprehensive unless the platform documentation supports that claim.

## Design requirements

- Use a stable, namespaced Collector ID and explicit version.
- Prefer documented Unicode Windows APIs over localized command output.
- Convert raw APIs into typed observations; do not make downstream code parse prose.
- Separate original evidence, canonical identity, stable comparison fields, and volatile display fields.
- Continue safely across inaccessible scopes/items and emit stable diagnostic codes.
- Never turn partial coverage into a confirmed absence.
- Do not execute, load, import, or remediate anything found during collection.
- Keep Windows bindings in `systemdiff-windows`; keep diff/rule/UI logic elsewhere.
- Minimize enabled `windows-rs` features and document every unsafe invariant.

## Required tests

- identity and normalization tests;
- deterministic order tests;
- empty/not-present behavior;
- permission denied and partial behavior;
- malformed/invalid platform data;
- concurrent change or disappearing objects where relevant;
- serialization fixture;
- complete → partial diff regression proving no false removal;
- non-elevated default execution.

Real-system integration tests must be read-only by default. Any privileged mutation fixture is opt-in, disposable, narrowly scoped, and reviewed separately.

## Documentation

Update `docs/collectors.md`, data-format documentation when the public artifact changes, privacy/redaction notes, supported platform claims, and relevant README collector tables. If a released schema must break, follow the versioning ADR rather than editing v1 in place.

## Non-code contributions

Useful contributions include official API research, compatibility notes, anonymized or synthetic fixtures, reproduction steps, field privacy analysis, terminology, documentation, and translations. Never attach an unreviewed real snapshot to a public issue.
