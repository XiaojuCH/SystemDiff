# Bootstrap validation and stabilization

Status: Complete
Owner: primary agent
Last updated: 2026-08-11

## Goal

Turn the static bootstrap into a Rust workspace that has been compiled, linted, tested, and exercised through its existing CLI paths with the real stable MSVC toolchain.

## User-visible outcome

Contributors can clone the initial repository and run the documented Cargo checks and fixture-based CLI examples with results that match CI. The draft registry evidence model does not assume every value is a string, and Collector version mismatch behavior is documented as a current conservative default rather than a permanent limitation.

## Current architecture and context

The workspace contains platform-independent core, diff, risk, report, and CLI crates plus a Windows adapter boundary. No production Collector or `snapshot` command exists. Draft v1 fixtures exercise deterministic diffing. `.agent/PROJECT_STATE.md`, `docs/architecture.md`, `docs/data-format.md`, and ADRs 0002/0004 define the current invariants.

## Constraints

- Do not implement Collectors, snapshot acquisition, desktop code, rules, or release/GitHub operations.
- Use standard rustup with the stable MSVC toolchain; do not bypass UAC or other security controls.
- Keep schema changes minimal and pre-v0.1; preserve privacy and bounded evidence expectations.
- Do not claim a command passed unless it was actually run.

## Implementation steps

1. Install and record rustup, stable MSVC, Cargo, rustfmt, and Clippy versions.
2. Generate the lockfile and run formatting, Clippy, and all workspace tests; make only focused fixes.
3. Exercise the two fixture diff modes and Collector listing command.
4. Stabilize the draft registry value representation and clarify Collector version compatibility documentation.
5. Audit contributor placeholders, README claims, desktop status, CI parity, manifests, and generated files.
6. Request an independent read-only review and address only important findings.
7. Repeat the exact validation suite, inspect the final diff/status, and record observed results.

## Affected files and modules

- Rust workspace manifests, source, tests, and `Cargo.lock` as validation requires.
- Draft schema fixtures and focused architecture/data-format/ADR documentation.
- Community/bootstrap files only where placeholder or accuracy audits find a concrete problem.
- `.agent/PROJECT_STATE.md` after validation materially changes repository state.

## Test strategy

- Run `cargo fmt --all --check` and Clippy with warnings denied.
- Run every workspace target test using deterministic fixtures.
- Run all three existing CLI commands exactly as documented by the maintainer.
- Add/adjust serialization tests only where the registry representation changes.
- Confirm CI invokes the same local checks without administrator privileges.

## Risks

- Native toolchain installation may require user interaction or missing Visual C++ build tools.
- A draft schema that stores only decoded strings or unbounded raw bytes would constrain future Registry Collectors and create privacy/size problems.
- Broad cleanup could obscure bootstrap validation; final diff inspection limits scope.

## Rollback and compatibility

No released schema exists. Focused draft v1 fixture changes may be reverted before v0.1. No migration or cross-Collector-version compatibility framework will be introduced. Tool installation is user-scoped where rustup supports it and does not modify repository history.

## Progress

- [x] 2026-08-11: Read repository agent rules, state, relevant architecture, Collector, data-format, threat-model, and ADR documents.
- [x] 2026-08-11: Installed rustup 1.29.0 and selected stable `x86_64-pc-windows-msvc` with rustc/Cargo 1.97.1, rustfmt, and Clippy.
- [x] 2026-08-11: Generated `Cargo.lock`, resolved rustfmt-only bootstrap differences, passed Clippy and 21 tests after reviewer-driven regression coverage, and exercised all three CLI commands in the maintainer-requested and CI-locked forms.
- [x] 2026-08-11: Replaced the string-only/unbounded Registry value draft, clarified cross-Collector-version policy, removed inactive CODEOWNERS examples and unused manifest dependencies, and aligned CI/README state.
- [x] 2026-08-11: Closed both medium review findings, repeated required and `--locked` checks, and received a final review result of high 0 / medium 0 with initial-commit readiness `Ready`.

## Discoveries

- The starting environment has `winget` but no discoverable `rustup`, `rustc`, or `cargo`.
- The draft registry artifact currently stores an unbounded byte vector and only an optional decoded string.
- Visual C++ Build Tools were detected and Cargo successfully linked the MSVC workspace even though `cl.exe` was not initially on the interactive shell PATH.
- Independent review found that undecoded or truncated Registry values could collapse into equal artifacts without a full-content fingerprint, and that CI did not enforce `Cargo.lock`; both findings were addressed before final validation.

## Decisions

- Treat this as a schema-affecting stabilization task under the existing `systemdiff-feature` workflow, without expanding MVP behavior.
- Represent Registry decoding as a status-tagged result containing type-checked values. Preserve a complete-value SHA-256 for reliable comparison and keep raw evidence optional as a validated 4 KiB lowercase-hex prefix with capture/original sizes and truncation metadata.
- Keep unknown cross-Collector-version comparison strict in draft v1 while leaving room for explicitly verified compatible version pairs later; do not build a registry/migration framework now.
- Require a lowercase SHA-256 over complete native Registry value bytes, validate the optional 4 KiB raw prefix metadata, and run CI dependency-resolving commands with `--locked`.

## Final validation

- `cargo generate-lockfile`: exit 0; locked 28 packages compatible with Rust 1.97.1.
- `cargo fmt --all --check`: exit 0. The first pre-fix attempt found only rustfmt differences; `cargo fmt --all` corrected them before the clean run.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
- `cargo test --workspace --all-targets`: exit 0; 21 passed, 0 failed, 0 ignored.
- Both Clippy and test also passed with the CI `--locked` arguments.
- Human and JSON fixture diffs exited 0 and produced 2 Added, 1 Removed, 1 Modified, and 1 coverage-incomplete Inconclusive change.
- `systemdiff collectors` exited 0 and listed the three MVP descriptors as `Planned`.
- The same three CLI paths passed with CI's `--locked` arguments.
- Final independent review: high 0, medium 0; ready for an initial commit.
