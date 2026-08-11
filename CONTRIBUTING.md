# Contributing to SystemDiff

Thank you for helping build a trustworthy Windows change-auditing tool. Contributions in English or Chinese are welcome. Review is based on technical clarity and project fit, not the contributor's native language.

## Before starting

- Read `README.md`, `AGENTS.md`, `.agent/PROJECT_STATE.md`, and the relevant architecture/Collector documentation.
- Search existing issues and pull requests before opening a duplicate.
- Use an issue for substantial features, public schema changes, new Collectors, or work with meaningful privacy/privilege implications.
- Stop and ask before proposing remediation, evidence execution, credentials, persistence creation, evasion, exploitation, or another feature outside the defensive read-only boundary.

## Ways to contribute

You do not need to write Rust. Useful contributions include:

- documentation and examples;
- Simplified Chinese or future locale work;
- deterministic synthetic fixtures;
- official Windows API research and compatibility notes;
- privacy/redaction analysis;
- issue reproduction and sanitized diagnostics;
- rules and explanations once their stable authoring format exists;
- desktop UI work after the desktop ADR is accepted.

Never upload an unreviewed real snapshot, task XML, or log to a public issue.

## Development setup

On Windows, install Git and the stable Rust MSVC toolchain with `rustfmt` and `clippy`. The future Tauri app will additionally require the C++ desktop workload, WebView2, Node.js, and pnpm; do not install frontend dependencies for the CLI-only bootstrap.

```powershell
rustup default stable-msvc
rustup component add rustfmt clippy
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
```

Keep a root `Cargo.lock` once Cargo can generate it because the workspace ships binaries.

## Working agreement

Use GitHub Flow once a remote exists:

1. start from current `main`;
2. create a focused `feat/...`, `fix/...`, `docs/...`, `test/...`, `refactor/...`, or `chore/...` branch;
3. implement one coherent change with tests;
4. inspect the complete diff;
5. push and open a pull request;
6. address CI and review;
7. merge and delete the branch only with maintainer authorization.

Do not mix drive-by refactors into a feature. Conventional Commits are not required; write clear imperative commit subjects.

## Tests and evidence

- Add unit tests for domain, diff, rule, and serialization behavior.
- Add golden compatibility fixtures for released wire formats.
- Test deterministic ordering and duplicate identity rejection.
- Test permission denied, partial coverage, unavailable APIs, malformed input, and regression cases.
- Keep default tests read-only, synthetic, deterministic, and non-elevated.
- Never claim a command passed unless you ran it and observed success.

Collector changes must also follow [docs/contributing-collectors.md](docs/contributing-collectors.md).

## Documentation and localization

English is canonical for code, comments, commit messages, APIs, ADRs, and technical documentation. `README.md` and `README.zh-CN.md` are maintained peers; check translation parity after material user-facing changes. Do not create translated copies of every internal document without a sustainable need.

Machine-readable identifiers and rule/schema fields remain language-neutral. Do not hard-code future `en-US` or `zh-CN` user strings in core logic.

## AI-assisted contributions

AI tools are welcome, but the contributor remains responsible for correctness, license provenance, safety, tests, and reviewability. Inspect generated code and sources. Do not submit fabricated API behavior, test output, citations, or large unexplained rewrites.

## Pull requests

A pull request should include:

- the problem and user-visible outcome;
- linked issue when applicable;
- focused implementation summary;
- exact test commands/results;
- safety, privilege, privacy, and schema notes;
- documentation/translation updates;
- intentional follow-ups or limitations.

Maintainers may request an ExecPlan for complex changes. Pull requests are not merged automatically.

## Suggested labels

After the public repository exists, maintainers should establish: `good first issue`, `help wanted`, `collector`, `rule`, `ui`, `cli`, `documentation`, `windows-internals`, `security`, `privacy`, and `performance`. A `good first issue` must have clear scope, acceptance criteria, and validation steps.

## License

Unless explicitly stated otherwise, contributions are submitted under the repository's Apache-2.0 license. No CLA or DCO is required during bootstrap.
