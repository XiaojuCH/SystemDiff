# SystemDiff

[English](README.md) | [简体中文](README.zh-CN.md)

[![CI](https://github.com/XiaojuCH/SystemDiff/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/XiaojuCH/SystemDiff/actions/workflows/ci.yml)

## See what apps, installers, and scripts change on Windows.

**Offline-first · Read-only · No account · No telemetry**

SystemDiff takes a before Snapshot and an after Snapshot, then explains the evidence that changed. It is for questions like: “I installed this program—what did it add to startup?”

> [!IMPORTANT]
> SystemDiff is pre-release, source-build-only software. Today it captures and compares the documented Windows Registry Run/RunOnce startup locations. Services, Scheduled Tasks, rules, redaction, releases, and the desktop app are not implemented.

[Try the sample](#try-the-registry-demo) · [Build from source](#build-from-source) · [Inspect the data format](docs/data-format.md)

![SystemDiff showing one synthetic Registry startup entry added](docs/assets/registry-startup-demo.svg)

_Verified output from the committed synthetic Registry-only fixtures. No real host data is shown._

## Available today

| Capability | Status |
| --- | --- |
| Capture current-user and local-machine Run/RunOnce evidence | Implemented on supported Windows systems |
| Human-readable, technical, and deterministic JSON Diff output | Implemented |
| Coverage-aware comparison that does not turn missing evidence into a false removal | Implemented |
| Windows Services Collector | Planned next Collector; not implemented |
| Scheduled Tasks Collector | Planned; not implemented |
| Rules, signatures, risk classification, and redacted sharing | Planned; not implemented |

SystemDiff reports facts such as “Added to current-user startup.” It does not currently decide whether an entry is malicious, safe, signed, or worthy of removal.

## Try the Registry demo

With a stable Rust MSVC toolchain installed:

```powershell
cargo run --locked --quiet -p systemdiff-cli -- diff fixtures/snapshots/registry-before-v1.json fixtures/snapshots/registry-after-v1.json
```

The sample contains one synthetic `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` addition. It is the same output shown above and is fixed by a regression test.

The three report modes serve different needs:

```powershell
# Calm, readable summary
systemdiff diff before.json after.json

# Exact text evidence for power users and debugging
systemdiff diff --technical before.json after.json

# Versioned, deterministic machine-readable document
systemdiff diff --json before.json after.json
```

The default output uses no color or ANSI formatting, so meaning is preserved when redirected or piped. `--technical` exposes Collector version, scope, canonical identity, Registry hive/view/path, lossless value name, native type, decode status, values, SHA-256, and coverage diagnostics. `--json` preserves the language-neutral Diff schema.

## Capture a real before/after pair

```powershell
systemdiff snapshot -o before.json

# Install or run the software you want to observe.

systemdiff snapshot -o after.json
systemdiff diff before.json after.json
```

This workflow is currently limited to Registry Run/RunOnce evidence. Compare Snapshots from the same Windows installation and the same user/principal context. Snapshots and every Diff/report mode are unredacted: human text, technical text, and JSON can all contain sensitive command strings, usernames in paths, hashes, raw evidence, and other host details. Review every report before sharing, and never attach unreviewed real evidence to a public Issue.

Current minimum collection platform: Windows 10 version 1709 or Windows Server 2016 version 1709. ARM64 captures current-user shared Registry scopes, but Collector v1 reports HKLM alternate-view coverage as unsupported until those views can be represented and tested correctly.

## Why trust the design?

- **Offline-first:** scanning, diffing, and reporting happen locally.
- **Read-only product behavior:** SystemDiff observes and reports; it does not clean, remediate, execute evidence, or change startup configuration.
- **Coverage is evidence:** permission and collection gaps are explicit. Incomplete scope coverage produces an Inconclusive result rather than a false Removed result.
- **Evidence remains inspectable:** plain-language output is layered over technical text and versioned JSON.
- **No account or telemetry:** the current product has no upload path, network client, or usage tracking.

See the [product principles](docs/product-principles.md), [architecture](docs/architecture.md), [data format](docs/data-format.md), and [threat model](docs/threat-model.md).

## Build from source

Prerequisites on Windows:

- Git;
- stable Rust MSVC toolchain with `rustfmt` and `clippy`;
- Microsoft C++ Build Tools (Desktop development with C++).

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets

cargo run --locked -p systemdiff-cli -- collectors
```

There is no official binary release yet. The existing synthetic HKCU write-based E2E harness is test-only, requires two explicit gates, refuses to overwrite an existing value, performs exact-data guarded cleanup, and is not run by default CI.

## Architecture and roadmap

The Rust workspace separates versioned domain data, Windows API access, deterministic Diff, rules, reporting, and CLI composition. The future desktop client is proposed to reuse the same core; no Tauri application has been generated.

Registry startup is the first completed vertical slice, not the finished v0.1. See the [Collector notes](docs/collectors.md) and [roadmap](docs/roadmap.md) for current boundaries.

## Contributing

Contributions are welcome in English or Chinese. Useful work is not limited to Rust: documentation, translations, synthetic fixtures, Windows API research, privacy analysis, issue reproduction, and UI design all matter.

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [contributing a Collector](docs/contributing-collectors.md).

## Security and project boundary

SystemDiff is defensive auditing software. Credential dumping, token/cookie extraction, keylogging, persistence creation, AV/EDR bypass, stealth/C2, exploitation, and unauthorized-access tooling are outside the project boundary. Report vulnerabilities through [GitHub Private Vulnerability Reporting](https://github.com/XiaojuCH/SystemDiff/security/advisories/new); see [SECURITY.md](SECURITY.md).

## License

Licensed under the [Apache License 2.0](LICENSE).
