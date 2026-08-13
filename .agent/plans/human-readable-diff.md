# Human-readable Diff and stranger-first product presentation

Status: In progress
Owner: Codex
Last updated: 2026-08-13

## Goal

Deliver GitHub issue [#7](https://github.com/XiaojuCH/SystemDiff/issues/7): turn the implemented Registry Run/RunOnce Snapshot -> Diff path into a calm, readable default CLI experience and make the repository front page demonstrate that real capability truthfully. Preserve exact evidence through an explicit technical text mode and the unchanged deterministic JSON mode.

## User-visible outcome

Users can run:

```text
systemdiff diff before.json after.json
systemdiff diff --technical before.json after.json
systemdiff diff --json before.json after.json
```

The default command identifies recognizable startup entries, what factually changed, decoded command/value data when available, and the Registry location. `--technical` exposes Collector identity/version, scope, canonical identity, native Registry evidence, hashes, decode status, and coverage diagnostics. `--json` retains the existing v1 wire contract byte-for-byte for an unchanged Diff value.

The English and Chinese READMEs lead with the long-term user problem, immediately disclose the current Registry-only preview scope, and show a static terminal visual derived from verified CLI output rather than an aspirational mock.

## Current architecture and context

- `main` at `0113b3e08811451bb580263b7b1c2db4fb7758b3` contains the merged Registry startup Collector, `snapshot` CLI, coverage-aware Diff, deterministic JSON, and a dual-gated real Windows E2E harness. Both required checks are green.
- `systemdiff-core` owns typed Registry startup artifacts, lossless Registry value names, decoded values, collector coverage, and scoped diagnostics.
- `systemdiff-diff` owns Added/Removed/Modified/Unchanged/Inconclusive classification and warnings. It intentionally omits Snapshot-only Collector version and diagnostic detail.
- `systemdiff-report` currently renders a terse ArtifactKey-oriented terminal list and generic pretty JSON.
- `systemdiff-cli` currently selects default terminal or `--json`; it already loads both validated Snapshots before diffing.
- Broad historical fixtures include service/task artifacts for schema and Diff coverage, but the truthful product demo uses the dedicated Registry-only fixtures.

The existing default renderer is unsuitable for ordinary users because it leads with opaque Collector/scope/canonical hashes, exposes no recognizable Registry name or command, provides no before/after value for modifications, and formats coverage as debug enums. Typed Registry fields can safely support factual descriptions of startup kind, hive/user scope, exact key location, lossless/display name, decoded value, and change kind. They do not support severity, maliciousness, signature, executable identity, or remediation claims.

## Constraints

- No Services or Scheduled Tasks Collector, session/baseline/history workflow, release packaging, schema version, rule/risk logic, signing, sanitizer, GUI, installer, updater, or package-manager work.
- Do not claim `suspicious`, `safe`, `unsigned`, severity, threat, or any other inference not present in evidence.
- Preserve no-false-removal semantics: only confirmed `Removed` is described as removed; `Inconclusive` remains explicitly uncertain.
- Do not weaken, transform, localize, or otherwise change the Diff JSON wire contract.
- Terminal output must remain readable without color and contain no ANSI escapes. Untrusted observed strings must not inject control sequences or new output lines.
- Technical output must derive Collector version and diagnostics from the supplied Snapshots, not infer them from an ID or hard-code Registry v1.
- Human output may omit technical evidence from the first layer, but `--technical` and `--json` must keep it inspectable.
- README claims must distinguish implemented Registry startup coverage from planned Services/Tasks and must not offer a nonexistent binary download.
- Real snapshots remain sensitive and unredacted; documentation must keep the public-sharing warning.

## Implementation steps

1. Add focused report tests first for Added Run, Added RunOnce, Modified decoded value, Removed, Inconclusive, undecoded data, invalid UTF-16 name, unnamed value, grouping, no changes, coverage warnings, plain redirected output, technical evidence, and unchanged JSON serialization.
2. Replace the default ArtifactKey list with a deterministic human renderer grouped by evidence category. Registry items use only typed evidence and calm factual wording. Unsupported artifact families receive a minimal typed fallback without pretending their Collectors are implemented.
3. Add a technical renderer that accepts the Diff plus both source Snapshots. Render exact Registry fields and locate Collector version/diagnostics from Snapshot coverage/observations. Escape observed control characters in both modes.
4. Add `diff --technical`, make it mutually exclusive with `--json`, keep the default route human, and retain `--include-unchanged` behavior.
5. Run the Registry-only synthetic fixture through all three modes. Save one representative default transcript as a narrow golden fixture and build a clearly labeled static SVG from that exact output.
6. Redesign the first screen of `README.md` and `README.zh-CN.md` around the real workflow, truthful visual, current/planned capability table, trust facts, and real CTAs. Keep factual parity and natural Chinese.
7. Update the reporting boundary documentation, demo reproduction notes, this plan, and `.agent/PROJECT_STATE.md` to the merged PR #6 and current productization phase.
8. Run formatting, Clippy, all workspace tests, CLI human/technical/JSON/collectors smoke tests, link/parity/stale-wording checks, and the existing dual-gated real Windows E2E only if it remains safely runnable without touching non-synthetic evidence.
9. Request an independent reviewer pass covering correctness, evidence completeness, terminal safety, JSON compatibility, stranger-first comprehension, privacy, and scope. Address all High/Medium findings and only actionable Low findings.
10. Inspect the final diff, commit, push `feat/human-readable-diff`, create a PR closing #7, and wait for both required GitHub checks. Do not merge.

## Affected files and modules

- `crates/systemdiff-report/src/lib.rs` and focused report tests/fixtures
- `crates/systemdiff-report/Cargo.toml` for the existing internal `systemdiff-core` workspace dependency needed by technical Snapshot context
- `crates/systemdiff-cli/src/main.rs` for `--technical` routing
- `README.md`, `README.zh-CN.md`
- a truthful asset and reproduction note under `docs/assets/`
- `docs/architecture.md` if needed to describe the three report modes
- `.agent/PROJECT_STATE.md`
- this ExecPlan

No Rust source outside report/CLI is expected to change. No schema, Collector, Windows adapter, Diff algorithm, rules, fixture Snapshot contract, CI behavior, or release configuration changes are expected.

## Test strategy

- Construct validated Registry-only fixture Diffs for each change kind and evidence edge case.
- Assert semantic sections and critical phrases rather than snapshotting every renderer permutation.
- Keep one short representative human transcript as a golden fixture; use the same transcript as the hero visual source.
- Assert human output contains recognizable value names/locations and no opaque SHA/ArtifactKey by default.
- Assert untrusted newline/escape/control characters are escaped and output contains no ANSI escape byte.
- Assert technical output contains change type, Collector ID/version, scope, artifact kind, canonical identity, hive/view/path, lossless name, native type, decode status, decoded before/after values, SHA-256, and matching scoped diagnostics.
- Assert `--technical` parses, `--json` parses, and the two flags conflict.
- Assert `write_json` output still equals `serde_json::to_string_pretty(diff) + "\n"` and existing deterministic Diff tests remain unchanged.
- Run the full workspace gates and three Registry-only CLI report modes. If the guarded E2E is rerun, require one synthetic Added change, zero Removed changes, and verified exact-data cleanup.

## Risks

- **False security interpretation:** readable prose may sound like a verdict. Limit copy to direct facts from typed evidence and test prohibited vocabulary.
- **Evidence loss in technical mode:** Diff alone lacks version/diagnostics. Pass both validated Snapshots and test lookup behavior rather than guessing.
- **Terminal injection:** Registry names/values are untrusted. Escape control characters while preserving readable Unicode and Windows path separators.
- **False removal under incomplete coverage:** keep Diff classification authoritative and never reinterpret Inconclusive as Removed.
- **README capability inflation:** use only Registry-only fixtures/output and clearly mark Services/Tasks planned.
- **Brittle presentation tests:** use semantic assertions plus one small representative golden, not a whole CLI corpus.
- **Cross-platform formatting drift:** avoid colors, terminal-width detection, locale-dependent output, and debug formatting.

## Rollback and compatibility

The change is additive at the CLI (`--technical`) and changes only the human terminal presentation of the unreleased development CLI. `--json` and both document schemas remain unchanged. Reverting the PR restores the earlier terminal renderer and README without affecting Snapshot/Diff compatibility. No migration framework or ADR is required.

## Progress

- [x] 2026-08-13: Synchronized clean `main`, confirmed local/remote SHA `0113b3e`, no open PR, and green latest CI.
- [x] 2026-08-13: Confirmed no equivalent Issue and created focused Issue #7.
- [x] 2026-08-13: Created `feat/human-readable-diff` from current `main`.
- [x] 2026-08-13: Read project state, architecture/data-format/Collector/threat/product docs, ADRs, implementation, tests, fixtures, and both READMEs.
- [x] 2026-08-13: Completed independent exploration and test-strategy synthesis; confirmed that technical version/diagnostic evidence requires source Snapshot context without changing Diff v1.
- [x] 2026-08-13: Implemented default human, explicit technical, and unchanged JSON routing with control/bidi escaping and focused behavior tests.
- [x] 2026-08-13: Produced a regression-tested Registry-only transcript and visually verified a static SVG rendered from that exact output; refreshed English/Chinese READMEs, architecture, and project state.
- [x] 2026-08-13: Ran full local gates and the dual-gated real Windows HKCU E2E; one synthetic Added change, zero Removed changes, both text renderers, exact-data cleanup, and Snapshot deletion were verified.
- [x] 2026-08-13: Addressed three Medium and one Low independent-review findings; final reviewer result is High 0 / Medium 0 / actionable Low 0.
- [ ] Commit, push, create the PR, and wait for both required GitHub checks.

## Discoveries

- Collector version and scoped diagnostic detail are Snapshot data and are not carried in `DiffDocument`. A complete technical text report needs source Snapshot context, while JSON output remains the Diff document only.
- Registry commands are only safely labelable as `Command` for decoded string/expand-string data. Numeric, multi-string, undecoded, or unsupported evidence must be described as a Registry value without guessing execution semantics.
- Registry evidence can contain terminal control characters; renderer escaping is a safety property, not cosmetic formatting.
- The existing Registry-only before/after fixtures are sufficient for a truthful product demo and avoid implying that Services or Scheduled Tasks collection exists.
- A Modified Registry artifact does not necessarily mean its decoded command text changed; human wording must compare the decoded strings before making that narrower claim.
- Exact technical evidence includes value boundaries, so `REG_MULTI_SZ` elements require count/index/quoted rendering rather than ambiguous delimiter joining.

## Decisions

- Keep plain output with no color dependency or ANSI behavior. Symbols and words carry all meaning.
- Keep `write_terminal` as the default human API for compatibility and add an explicit technical writer that receives both source Snapshots.
- Use typed, factual prose only; no rule engine or path/name heuristics.
- Represent an unnamed Registry value as `Default value (unnamed)` and an invalid UTF-16 name as an explicit undecodable-name label in human mode, with lossless UTF-16LE hex in technical mode.
- Use a static terminal-style SVG fallback, clearly labeled synthetic, because it can be reviewed and kept exactly synchronized with a verified transcript without faking an animation.

## Final validation

Local validation completed on Windows 2026-08-13:

- `cargo fmt --all --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace --all-targets`: 88 passed, 0 failed, 0 ignored.
- Registry-only human, technical, JSON, and `collectors` CLI smoke commands: exit 0. Human output reported one confirmed `SystemDiffSyntheticE2E` current-user Run addition; technical output retained Collector v1, scope, canonical identity, Registry evidence, SHA-256, and complete coverage; JSON remained the v1 Diff document.
- Existing broad-fixture human/JSON commands used by CI: exit 0.
- Dual-gated real Windows HKCU E2E: the randomized synthetic value was absent before, present after, classified as exactly one Added and zero Removed changes, matched by both human and technical renderers, removed only after exact type/data verification, and confirmed absent. Both temporary Snapshots were deleted.
- `git diff --check`, local Markdown links, English/Chinese factual-parity markers, stale/aspirational wording scan, secret/local-path scan, and SVG XML parsing passed.
- The SVG was rendered headlessly to PNG and inspected visually; the temporary PNG was removed and is not a repository artifact.
- Independent review initially found three Medium and one actionable Low issue. All were fixed with regressions; the focused final review reported High 0 / Medium 0 / actionable Low 0.

Commit/PR identifiers and required remote check results are intentionally recorded in the PR and maintainer report because a commit cannot contain its own final hash or future CI result.
