# ExecPlans

An ExecPlan is a living implementation document for work that is too large or risky to coordinate from a short issue description. Create one for a complex feature, significant refactor, schema migration, multi-hour task, or work spanning several architectural boundaries.

Store active plans at `.agent/plans/<short-name>.md`. Do not create a plan for a small, self-contained edit. Update the plan while implementing; it is not a prediction written once and forgotten.

## Required format

```markdown
# <Plan title>

Status: Draft | In progress | Blocked | Complete
Owner: <person or agent>
Last updated: YYYY-MM-DD

## Goal
What problem is being solved and why now?

## User-visible outcome
What will a user or contributor be able to do when this is complete?

## Current architecture and context
Relevant behavior, modules, entry points, invariants, and evidence links.

## Constraints
Safety, privacy, compatibility, performance, privilege, and scope limits.

## Implementation steps
Ordered, independently verifiable milestones. Include commands where useful.

## Affected files and modules
Expected change surface and ownership boundaries.

## Test strategy
Unit, fixture, integration, Windows-specific, failure, permission, and regression coverage.

## Risks
Likely failure modes and how they will be detected or limited.

## Rollback and compatibility
How to revert safely; schema/data/API compatibility implications.

## Progress
- [ ] Timestamped, concrete progress entries.

## Discoveries
Facts learned during implementation that change or sharpen the plan.

## Decisions
Material choices and rationale. Link an ADR when the decision is durable.

## Final validation
Exact commands and manual checks, with observed results. Never infer success.
```

## Plan discipline

- Keep the goal stable. If the goal changes materially, split or replace the plan explicitly.
- Record discoveries and decisions close to when they happen.
- Mark blocked steps with the actual blocker and safe work already attempted.
- Do not use a plan as a substitute for tests, ADRs, issues, or current project state.
- At completion, ensure the plan matches what was actually delivered and link follow-up issues for intentionally deferred work.
