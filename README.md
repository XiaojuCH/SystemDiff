# SystemDiff

[English](README.md) | [简体中文](README.zh-CN.md)

**SystemDiff shows what changed on a Windows system, with plain-language explanations backed by inspectable evidence.**

> [!IMPORTANT]
> SystemDiff is in repository bootstrap. There is no end-user release and no operating-system Collector is implemented yet. The current code proves the draft schema, deterministic diff, report, and contribution boundaries; it is not an effective system scanner today.

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

This output is illustrative, not a current detection claim. Advanced users will be able to inspect exact registry paths, service/task configuration, raw before/after values, Collector and rule IDs, structured JSON, hashes, and signature metadata when supported.

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

## Planned v0.1 workflow

```powershell
systemdiff snapshot -o before.json

# Install or run the software you want to observe.

systemdiff snapshot -o after.json
systemdiff diff before.json after.json
```

The `snapshot` command is intentionally not implemented in the bootstrap. v0.1 will be complete only when this pipeline works reliably.

## MVP scope

| Collector | v0.1 scope | Current status |
| --- | --- | --- |
| Registry startup | Documented Run/RunOnce locations and correct Registry views | Planned |
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

The bootstrap workspace has been validated with a real stable Rust MSVC toolchain. See [.agent/PROJECT_STATE.md](.agent/PROJECT_STATE.md) for the exact validated state and remaining product limitations.

## Architecture

The Rust workspace keeps domain/schema, Windows access, deterministic diff, rules, reports, and CLI composition separate. The future Tauri desktop client will use the same Rust core. Tauri 2 + React + TypeScript is proposed, not yet accepted or generated.

Start with [architecture](docs/architecture.md), [data format](docs/data-format.md), [Collector notes](docs/collectors.md), and the [roadmap](docs/roadmap.md).

## Contributing

Contributions are welcome in English or Chinese. Useful work is not limited to Rust: documentation, translations, synthetic fixtures, Windows API research, privacy analysis, issue reproduction, and UI design all matter.

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [contributing a Collector](docs/contributing-collectors.md). Please never attach an unreviewed real snapshot or log to a public issue.

## Security and project boundary

SystemDiff is defensive auditing software. Credential dumping, token/cookie extraction, keylogging, persistence creation, AV/EDR bypass, stealth/C2, exploitation, and unauthorized-access tooling are outside the project boundary. See [SECURITY.md](SECURITY.md).

## License

Licensed under the [Apache License 2.0](LICENSE). The maintainer should confirm this governance choice before public launch.
