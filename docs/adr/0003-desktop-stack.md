# ADR 0003: Tauri 2 with React and TypeScript for the desktop client

- Status: Accepted
- Date: 2026-08-11
- Accepted: 2026-08-20

## Context

SystemDiff needs a polished, localized evidence-exploration UI after the CLI pipeline proves the architecture. The UI must reuse Rust logic, remain reasonably small, and expose only narrow read-only commands across its trust boundary.

Tauri 2 uses a Rust core process and the operating-system WebView. On Windows it requires Microsoft C++ Build Tools and WebView2. React/TypeScript offers a broad contributor pool and is suitable for interactive diff inspection, but it also introduces a fast-moving Node dependency graph.

## Decision

Use Tauri 2 with React, TypeScript, and Vite for the first Windows desktop vertical slice. The completed narrow spike found no architectural blocker and established these required boundaries:

- typed, least-privilege IPC with no generic shell/filesystem/network capability;
- bundled local content and a restrictive content security policy;
- shared Rust core/report behavior rather than duplication;
- `en-US` and `zh-CN` localization boundaries;
- acceptable WebView2 and Windows packaging behavior;
- dependency license/security audit and maintainable lockfiles;
- testable UI behavior without elevated privileges.

Rust owns capture, Diff classification, coverage semantics, and a locale-neutral typed presentation model. React owns layout, interaction, local-time display, and `en-US` / `zh-CN` localization. The frontend does not receive Snapshot documents or reinterpret `Added`, `Removed`, `Modified`, or `Inconclusive` evidence.

The Tauri command surface is limited to the guided session lifecycle and takes no filesystem path, URL, shell command, or raw Snapshot JSON. Production uses bundled local assets, an explicit content security policy, one application window, and no shell, filesystem, HTTP, dialog, opener, updater, or network plugin. Synchronous Windows collection runs on a blocking worker so the WebView remains responsive.

The desktop Rust package has its own lockfile outside the root Cargo workspace. This keeps Tauri's platform dependency graph out of the existing CLI-only Developer Preview package and license verification. It still reuses SystemDiff crates through path dependencies and receives explicit CI validation.

Microsoft documents WebView2 support on Windows 10 SAC 1709 and later, but Tauri only describes it as normally preinstalled on Windows 10 1803 and later. The development app therefore requires an installed WebView2 Evergreen Runtime and does not establish a clean-machine 1709 distribution claim. Runtime bootstrap, installer behavior, and the missing-Runtime experience remain release work.

## Alternatives considered

- Tauri with a smaller frontend library or no framework.
- Native Rust UI libraries, considering accessibility and ecosystem maturity.
- .NET desktop UI calling a Rust library, considering packaging and interop cost.

## Consequences

- The WebView becomes an untrusted UI boundary; only the Rust core process accesses the OS.
- Frontend upgrades and supply-chain review become release work.
- Tauri and Node tooling are now part of desktop development and CI, with separate Cargo/npm lockfiles and dependency review.
- Windows desktop distribution depends on an appropriate WebView2 Runtime strategy. A no-bundle development executable is not a fully self-contained portable package.
- The desktop IPC presentation contract can evolve with the jointly shipped frontend/backend, but it does not replace or silently change the public Snapshot/Diff schema.
- A small React runtime is accepted for localization, interaction, and contributor accessibility; router, global state, design-system, and general i18n frameworks are not justified by the first three-state workflow.
