# ADR 0003: Tauri 2 with React and TypeScript for the desktop client

- Status: Proposed
- Date: 2026-08-11

## Context

SystemDiff needs a polished, localized evidence-exploration UI after the CLI pipeline proves the architecture. The UI must reuse Rust logic, remain reasonably small, and expose only narrow read-only commands across its trust boundary.

Tauri 2 uses a Rust core process and the operating-system WebView. On Windows it requires Microsoft C++ Build Tools and WebView2. React/TypeScript offers a broad contributor pool and is suitable for interactive diff inspection, but it also introduces a fast-moving Node dependency graph.

## Proposed decision

After v0.1 CLI functionality, run a narrow spike using Tauri 2, React, TypeScript, and Vite. Accept the stack only if the spike verifies:

- typed, least-privilege IPC with no generic shell/filesystem/network capability;
- bundled local content and a restrictive content security policy;
- shared Rust core/report behavior rather than duplication;
- `en-US` and `zh-CN` localization boundaries;
- acceptable WebView2 and Windows packaging behavior;
- dependency license/security audit and maintainable lockfiles;
- testable UI behavior without elevated privileges.

## Alternatives to compare

- Tauri with a smaller frontend library or no framework.
- Native Rust UI libraries, considering accessibility and ecosystem maturity.
- .NET desktop UI calling a Rust library, considering packaging and interop cost.

## Consequences if accepted

- The WebView becomes an untrusted UI boundary; only the Rust core process accesses the OS.
- Frontend upgrades and supply-chain review become release work.
- Tauri and Node tooling remain absent from v0.1 CI and dependencies until the decision is accepted.
