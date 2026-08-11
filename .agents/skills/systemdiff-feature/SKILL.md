---
name: systemdiff-feature
description: Implement a non-trivial SystemDiff feature or significant behavior change with architecture discovery, an ExecPlan when warranted, tests, validation, independent review, and durable documentation. Use for collectors, diff or schema behavior, rules, reports, CLI/desktop features, cross-crate refactors, migrations, or other work that is larger than a small self-contained fix.
---

# SystemDiff feature workflow

1. Read `AGENTS.md` and `.agent/PROJECT_STATE.md`.
2. Read the relevant architecture, data-format, Collector, threat-model, and ADR documents.
3. Explore the existing implementation, tests, fixtures, and execution paths before editing.
4. Decide whether the work requires an ExecPlan under `.agent/PLANS.md`. Create or update one before complex implementation.
5. Define observable behavior, compatibility, privilege, privacy, and failure semantics. Identify tests first.
6. Implement the smallest coherent change. Keep Windows access, core logic, rules, reporting, and UI in their documented boundaries.
7. Add deterministic unit/fixture/integration coverage, including permission or partial outcomes where applicable.
8. Run the relevant format, lint, test, and build checks. Record exact results; never infer success.
9. Ask the project `reviewer` agent for an independent pass on substantial changes. Resolve important findings or document a deliberate decision.
10. Update public docs, ADRs, data-format fixtures, and `.agent/PROJECT_STATE.md` only when state or behavior materially changes.
11. Inspect the final Git diff for unrelated changes and summarize behavior, compatibility, validation, and intentional omissions.

Stop and ask the maintainer if the feature adds remediation/write behavior, executes evidence, crosses the documented defensive boundary, or requires a public schema break without an agreed migration.
