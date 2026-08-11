---
name: systemdiff-pr
description: Prepare or review a SystemDiff pull request with focused diff inspection, issue linkage, checks, independent review, schema/safety/privacy analysis, tests, documentation, and a concise PR description. Use when the maintainer asks to prepare, review, update, or assess a PR or branch for merge readiness.
---

# SystemDiff pull request workflow

1. Read `AGENTS.md` and `.agent/PROJECT_STATE.md`.
2. Inspect repository, branch, remote, working-tree state, and the complete diff against the intended base before any GitHub write.
3. Verify the linked issue and acceptance criteria when applicable. Do not create duplicate issues or PRs.
4. Separate the coherent change from unrelated edits, generated noise, secrets, and accidental schema or lockfile changes.
5. Run the required checks and targeted tests. Report exact commands and results, including anything unavailable.
6. Review safety, privacy, privilege, Collector coverage, deterministic output, public schema compatibility, dependencies, docs, and contributor impact.
7. Ask the project `reviewer` agent for an independent review and address material findings.
8. Ensure behavior changes have tests and public changes have the necessary English-first documentation plus README translation parity where relevant.
9. Produce a concise PR description with summary, linked issue, test plan/results, safety/privacy/schema notes, screenshots only when useful, and intentional follow-ups.

Never merge, push, force-push, close, publish, delete a branch, or perform another GitHub write unless the maintainer explicitly requests that action.
