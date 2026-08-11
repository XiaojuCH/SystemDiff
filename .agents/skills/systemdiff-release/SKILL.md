---
name: systemdiff-release
description: Validate and prepare a SystemDiff release candidate by checking repository state, versions, compatibility, full tests, dependencies, artifacts, installation documentation, release notes, privacy/security claims, and rollback steps. Use for release planning, release-candidate audits, notes, checklists, or artifact verification.
---

# SystemDiff release workflow

1. Read `AGENTS.md`, `.agent/PROJECT_STATE.md`, the roadmap, security policy, supported Collector documentation, and relevant release ExecPlan.
2. Verify the intended tag/version and a clean, understood tree. Confirm every package, schema, fixture, UI, installer, and documentation version that must agree.
3. Run full formatting, lint, test, build, dependency, and platform checks defined for the milestone. Record exact output and blockers.
4. Verify historical schema fixtures, upgrade/rollback behavior, Collector privilege claims, redaction/privacy language, and supported Windows matrix.
5. Inspect artifacts for expected names, architectures, licenses, checksums, signatures, provenance, and reproducibility information required by the release plan.
6. Re-test README installation/quick-start commands and confirm English/Chinese parity for material user-facing changes.
7. Inspect release notes for user-visible changes, compatibility, security/privacy impact, known limitations, contributors, and upgrade guidance.
8. Ask the project `reviewer` agent for an independent release-readiness pass.
9. Produce a release checklist with passed checks, evidence, blockers, residual risks, rollback path, and explicit maintainer actions.

Never tag, push, upload, sign with maintainer credentials, publish packages, create a public release, or merge without explicit maintainer authorization.
