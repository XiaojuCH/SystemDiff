# SystemDiff desktop application

The desktop application is intentionally not scaffolded during the CLI bootstrap.

[ADR 0003](../../docs/adr/0003-desktop-stack.md) proposes Tauri 2 with React and TypeScript for v0.2, subject to a narrow post-v0.1 spike covering least-privilege IPC, bundled content/CSP, `en-US` and `zh-CN` localization, WebView2 behavior, packaging, tests, and dependency audit.

Do not generate a frontend package or add Node/Tauri CI until that ADR is accepted. The desktop client must reuse the Rust core and must not call Windows APIs directly or hide underlying evidence.
