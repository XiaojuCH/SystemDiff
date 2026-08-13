# SystemDiff

[English](README.md) | [简体中文](README.zh-CN.md)

[![CI](https://github.com/XiaojuCH/SystemDiff/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/XiaojuCH/SystemDiff/actions/workflows/ci.yml)

## See what apps, installers, and scripts change on Windows.

**Offline-first · Read-only · No account · No telemetry**

SystemDiff takes a before Snapshot and an after Snapshot, then explains the evidence that changed. It is for questions like: “I installed this program—what startup entries or Windows services did it change?”

> [!IMPORTANT]
> SystemDiff is pre-release software. Today it captures and compares the documented Windows Registry Run/RunOnce startup locations and Windows service configuration visible to the current token. An unsigned, short-lived Windows x64 Developer Preview is available from eligible CI runs, but there is no official binary Release. Scheduled Tasks, rules, redaction, releases, and the desktop app are not implemented.

[Try the sample](#try-the-registry-demo) · [Developer Preview builds](#developer-preview-builds) · [Build from source](#build-from-source) · [Inspect the data format](docs/data-format.md)

![SystemDiff showing one synthetic Registry startup entry added](docs/assets/registry-startup-demo.svg)

_Verified output from the committed synthetic Registry-only fixtures. No real host data is shown._

## Available today

| Capability | Status |
| --- | --- |
| Capture current-user and local-machine Run/RunOnce evidence | Implemented on supported Windows systems |
| Human-readable, technical, and deterministic JSON Diff output | Implemented |
| Coverage-aware comparison that does not turn missing evidence into a false removal | Implemented |
| Capture current-token-visible Windows service configuration (drivers excluded) | Implemented with conservative partial coverage |
| Scheduled Tasks Collector | Planned; not implemented |
| Rules, signatures, risk classification, and redacted sharing | Planned; not implemented |

SystemDiff reports facts such as “Added to current-user startup.” It does not currently decide whether an entry is malicious, safe, signed, or worthy of removal.

## Developer Preview builds

Successful `main` CI runs attach `systemdiff-windows-x86_64-developer-preview` for 14 days. This is an ephemeral GitHub Actions artifact, not a GitHub Release or a supported version. To get it:

1. sign in to GitHub and open a successful [CI workflow run](https://github.com/XiaojuCH/SystemDiff/actions/workflows/ci.yml);
2. find **Artifacts** at the bottom of the run and download `systemdiff-windows-x86_64-developer-preview`;
3. extract GitHub's outer download, verify `systemdiff-windows-x86_64.zip` against the adjacent `SHA256SUMS`, then extract the portable ZIP;
4. read `QUICKSTART.md` and run `.\systemdiff.exe --help`.

The x64 executable is built in Cargo's `release` profile. CI checks its PE architecture and imports, verifies an embedded `asInvoker` / `uiAccess=false` manifest, and runs the downloaded artifact without Cargo. The current portable build statically links the MSVC CRT, so inspection shows no dynamic VC/UCRT runtime import; ordinary Windows system DLLs remain dependencies. Clean-machine validation on every supported Windows baseline is still required before an official alpha.

The preview is not Authenticode-signed. Windows may show a SmartScreen or reputation warning. Verify the checksum and public source; SystemDiff does not ask users to disable or bypass Windows security controls. Browser artifact downloads require GitHub sign-in and expire, so this is intentionally not presented as the final public download experience.

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

The default output uses no color or ANSI formatting, so meaning is preserved when redirected or piped. `--technical` exposes Collector version, scope, canonical identity, Registry and service configuration evidence, raw numeric values, and coverage diagnostics. `--json` preserves the language-neutral Diff schema.

## Capture a real before/after pair

```powershell
systemdiff snapshot -o before.json

# Install or run the software you want to observe.

systemdiff snapshot -o after.json
systemdiff diff before.json after.json
```

This workflow currently covers Registry Run/RunOnce evidence and Windows service configuration. Service visibility depends on the current token and object ACLs, so Services v1 conservatively marks its scope partial: a missing service becomes Inconclusive rather than a confirmed removal. Compare Snapshots from the same Windows installation and the same user/principal context. Snapshots and every Diff/report mode are unredacted: human text, technical text, and JSON can contain service accounts, paths and arguments, descriptions, command strings, usernames, hashes, and other host details. Review every report before sharing, and never attach unreviewed real evidence to a public Issue.

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

There is no official binary Release yet. The CI Developer Preview above is unsigned and temporary. The existing synthetic HKCU write-based E2E harness is test-only, requires two explicit gates, refuses to overwrite an existing value, performs exact-data guarded cleanup, and is not run by default CI.

## Architecture and roadmap

The Rust workspace separates versioned domain data, Windows API access, deterministic Diff, rules, reporting, and CLI composition. The future desktop client is proposed to reuse the same core; no Tauri application has been generated.

Registry startup and Windows Services are the first two completed vertical slices, not the finished v0.1. See the [Collector notes](docs/collectors.md) and [roadmap](docs/roadmap.md) for current boundaries.

## Contributing

Contributions are welcome in English or Chinese. Useful work is not limited to Rust: documentation, translations, synthetic fixtures, Windows API research, privacy analysis, issue reproduction, and UI design all matter.

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [contributing a Collector](docs/contributing-collectors.md).

## Security and project boundary

SystemDiff is defensive auditing software. Credential dumping, token/cookie extraction, keylogging, persistence creation, AV/EDR bypass, stealth/C2, exploitation, and unauthorized-access tooling are outside the project boundary. Report vulnerabilities through [GitHub Private Vulnerability Reporting](https://github.com/XiaojuCH/SystemDiff/security/advisories/new); see [SECURITY.md](SECURITY.md).

## License

SystemDiff is licensed under the [Apache License 2.0](LICENSE). The portable binary's dependency notices are listed in [THIRD_PARTY_LICENSES.txt](THIRD_PARTY_LICENSES.txt).
