# Roadmap

The roadmap is outcome-oriented. Dates are intentionally omitted until the project has measured delivery capacity.

## Bootstrap: maintainable foundation

- Establish product and security boundaries, ADRs, project memory, contributor workflows, and CI.
- Prove Rust workspace boundaries with deterministic synthetic fixtures.
- Keep all operating-system writes and remediation out of scope.

Exit condition: the repository is understandable, reviewable, and ready for the first collector issue without pretending the product already works.

Status: complete. The first Collector phase is now underway.

## v0.1: CLI evidence pipeline

User outcome:

```text
systemdiff snapshot -o before.json
systemdiff snapshot -o after.json
systemdiff diff before.json after.json
```

Required scope:

- Registry Run/RunOnce entries. **Implemented in the development CLI; pre-release validation continues.**
- Windows services configuration.
- Scheduled Tasks 2.0 configuration.
- Versioned snapshot and diff JSON.
- Human-readable terminal report.
- An unsigned, expiring Windows x64 CI Developer Preview that proves the existing CLI can run from a downloaded portable package without Cargo. **In progress; this is not an official release.**
- Independent collector failures and clear privilege/coverage reporting.
- Deterministic fixtures and snapshot-to-diff integration tests that do not need administrator privileges.

Explicitly excluded: whole-drive crawling, remediation, telemetry, cloud analysis, broad persistence coverage, and a large GUI.

## v0.2: polished desktop workflow

- Validate and adopt the Tauri 2 + React + TypeScript stack after a narrow security-focused spike.
- Guide users through snapshot A → change → snapshot B → explanation.
- Show plain-language summaries without hiding raw evidence.
- Ship maintained `en-US` and `zh-CN` locales.
- Establish Windows packaging and code-signing strategy before encouraging ordinary-user installation.

## v0.3: safe sharing and richer explanations

- Implement tested redaction/sanitization with explicit policy metadata.
- Add shareable report formats and issue-safe export guidance.
- Introduce a small, reviewable rule catalog with stable IDs and localized explanation keys.

## v0.4 and later

- Add collectors only when their evidence, stability, privilege, privacy, and test strategy are understood.
- Candidate work includes targeted filesystem analysis, Authenticode and executable metadata, firewall and network configuration, Defender exclusions, installed applications, browser extensions, and additional documented persistence mechanisms.
- Consider plugin or external collector contracts only after real in-tree collector diversity demonstrates a need.

## Adoption milestones

Product engineering precedes promotion:

1. v0.1 must solve one real install-and-compare workflow reliably.
2. A short hero demo should make the value obvious without security hype.
3. README installation and trust claims must match released artifacts.
4. Broader launch waits for a result worth sharing, not merely a completed scaffold.

The first CI Developer Preview intentionally stops short of a release: browser download requires GitHub sign-in, artifacts expire, and the executable is unsigned. A public alpha additionally requires clean-machine validation, a publisher-signing decision, stable version semantics, and a durable Release channel.
