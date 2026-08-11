# ADR 0001: Rust shared core and CLI

- Status: Accepted
- Date: 2026-08-11

## Context

SystemDiff needs native Windows API access, deterministic domain/diff logic, a CLI-first MVP, and a future desktop client without duplicated behavior. The core processes privacy-sensitive, attacker-influenced system data and should remain useful offline.

## Decision

Use a Rust workspace for shared evidence, diff, rule, report, Windows integration, and CLI crates. Use feature-scoped typed `windows-rs` bindings for Win32/COM access. Keep the shared core platform-independent and keep all direct Windows access in `systemdiff-windows`.

The CLI is the first product surface. The desktop client will consume the same Rust boundaries rather than reimplementing collection or diff logic.

## Alternatives considered

- **C#/.NET with WPF or WinUI:** excellent Windows/COM ergonomics, but it would either make the product core Windows/UI-centric or require a second core for other clients. It remains a valid future reassessment if Rust Windows integration proves disproportionately costly.
- **Electron/Node core:** broad UI ecosystem but a larger runtime and weaker boundary between UI dependencies and forensic logic.
- **Mixed Rust library plus independent frontend/backend logic:** rejected because duplicated schema/diff/rule behavior would drift.

## Consequences

- Contributors need a Rust MSVC toolchain for product code.
- Win32/COM unsafe code receives extra review and deterministic safe wrappers.
- Cross-platform CI can test core/diff/report logic; Windows CI validates platform compilation.
- The project does not promise cross-platform collection. Platform independence is an internal testability boundary, not a product claim.
