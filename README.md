# SystemDiff

[English](README.md) | [简体中文](README.zh-CN.md)

[![CI](https://github.com/XiaojuCH/SystemDiff/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/XiaojuCH/SystemDiff/actions/workflows/ci.yml)

**SystemDiff shows what changed on a Windows system, with plain-language explanations backed by inspectable evidence.**

> [!IMPORTANT]
> SystemDiff is pre-release software with no end-user distribution yet. The development CLI now captures the documented Windows Run/RunOnce startup locations through its first real read-only Collector and can compare two Snapshots. Services, Scheduled Tasks, explanations/rules, redaction, and the desktop app are not implemented.

## Why SystemDiff?

The canonical workflow is deliberately simple:

1. Take snapshot A.
2. Install, run, or change something.
3. Take snapshot B.
4. Compare the snapshots.
5. Understand exactly what changed.

Ordinary users should see a calm explanation:

```text
High attention

ExampleUpdater
  Added itself to startup
  Runs automatically when Windows starts
  File is not digitally signed
  Location: AppData
  Open technical details

Normal

ExampleApp settings
  Added application configuration
  Usually harmless
```

This output is illustrative, not a current detection claim. Current Registry diffs expose exact paths, value names/types, typed decode status, complete-value SHA-256, Collector/scope identity, and structured JSON. Service/task evidence, rules, and signature metadata remain planned.

SystemDiff does not equate unusual with malicious. Explanations sit on top of evidence; they never replace it.

## Trust model

- **Offline-first:** core scanning, diffing, and reporting stay local.
- **No account:** local use does not require registration.
- **No telemetry in the MVP:** system data is not uploaded by default.
- **Read-only:** SystemDiff observes and reports; it does not clean or remediate a system.
- **Graceful privileges:** inaccessible scopes are reported as partial or permission denied rather than hidden.
- **Evidence-first:** JSON formats are versioned and deterministic, and the GUI will not hide raw evidence.
- **Privacy-aware:** real snapshots may contain sensitive data and must not be shared until reviewed or sanitized.

See [product principles](docs/product-principles.md) and the [threat model](docs/threat-model.md).

## Current pre-v0.1 workflow

```powershell
systemdiff snapshot -o before.json

# Install or run the software you want to observe.

systemdiff snapshot -o after.json
systemdiff diff before.json after.json
```

This pipeline now works from a source build on supported Windows systems for Registry Run/RunOnce evidence only. Snapshot files are unredacted and may contain sensitive command strings and paths. v0.1 is not complete until all required Collectors and the full workflow are reliable.

The current minimum platform is Windows 10 version 1709 or Windows Server 2016 version 1709. ARM64 captures current-user shared Registry scopes, but v1 explicitly reports HKLM alternate-view coverage as unsupported until those view semantics can be represented and tested correctly.

The draft v0.1 comparison model assumes that both snapshots come from the same Windows installation and the same user/principal context. Cross-host and cross-user identity are intentionally out of scope.

## MVP scope

| Collector | v0.1 scope | Current status |
| --- | --- | --- |
| Registry startup | Documented Run/RunOnce locations and explicit Registry views | Implemented in the development CLI |
| Windows services | Stable Win32 service configuration; drivers excluded | Planned |
| Scheduled tasks | Task Scheduler 2.0 configuration with permission-aware coverage | Planned |

Whole-drive hashing, automatic remediation, telemetry, cloud analysis, and a large desktop UI are out of scope for v0.1.

## Developer quick start

Prerequisites on Windows:

- Git;
- stable Rust MSVC toolchain with `rustfmt` and `clippy`;
- Microsoft C++ Build Tools (Desktop development with C++);
- WebView2 only when the future Tauri desktop app is introduced.

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets

# Exercise the current deterministic diff/report path with synthetic fixtures.
cargo run --locked -p systemdiff-cli -- diff fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json
cargo run --locked -p systemdiff-cli -- diff --json fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json
cargo run --locked -p systemdiff-cli -- collectors
```

The workspace and the opt-in synthetic HKCU Registry E2E have been validated with a real stable Rust MSVC toolchain. The E2E harness is test-only, requires two explicit gates, refuses to overwrite an existing value, and is not run by default CI. See [.agent/PROJECT_STATE.md](.agent/PROJECT_STATE.md) for the exact validated state and remaining product limitations.

## Architecture

The Rust workspace keeps domain/schema, Windows access, deterministic diff, rules, reports, and CLI composition separate. The future Tauri desktop client will use the same Rust core. Tauri 2 + React + TypeScript is proposed, not yet accepted or generated.

Start with [architecture](docs/architecture.md), [data format](docs/data-format.md), [Collector notes](docs/collectors.md), and the [roadmap](docs/roadmap.md).

## Contributing

Contributions are welcome in English or Chinese. Useful work is not limited to Rust: documentation, translations, synthetic fixtures, Windows API research, privacy analysis, issue reproduction, and UI design all matter.

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [contributing a Collector](docs/contributing-collectors.md). Please never attach an unreviewed real snapshot or log to a public issue.

## Security and project boundary

SystemDiff is defensive auditing software. Credential dumping, token/cookie extraction, keylogging, persistence creation, AV/EDR bypass, stealth/C2, exploitation, and unauthorized-access tooling are outside the project boundary. See [SECURITY.md](SECURITY.md).

## License

Licensed under the [Apache License 2.0](LICENSE).
