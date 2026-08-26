# SystemDiff desktop development

The desktop app is the first guided, ordinary-user interface for SystemDiff. It provides a local three-stage workflow—start capture, make a Windows change, finish and compare—without exposing Snapshot JSON or asking for file paths.

This is currently a **developer preview built from source**, not an official release, signed installer, or replacement for the existing CLI artifact. It has been designed for Windows 10 version 1709 and later, but a clean Windows 10 1709 machine may not already have the required Microsoft WebView2 Evergreen Runtime. The first validated development environment and remaining baseline work are tracked in the repository project state.

## Boundaries

- Rust calls the existing Registry startup and Windows Services collectors, coverage-aware Diff, and report presentation APIs in-process. The app never shells out to the CLI.
- React handles layout, interaction, local time, and `en-US` / `zh-CN` text. It does not classify evidence.
- Production behavior is read-only, local, non-elevated, and without telemetry or network access.
- Tauri exposes only the guided session commands. The frontend cannot provide arbitrary filesystem paths, URLs, shell commands, or Snapshot JSON.
- Unredacted before/after evidence is stored only in a collision-safe application-local session directory and removed after compare, cancel, or a stable normal exit. Cleanup failures are visible in ordinary Results, while in-flight exit/crash recovery considers only verified SystemDiff-owned stale sessions. This local storage is not encryption.

## Prerequisites

- Rust stable with the MSVC toolchain, `rustfmt`, and Clippy
- Node.js 22 and npm
- Microsoft Visual Studio C++ Build Tools and a Windows SDK
- Microsoft WebView2 Evergreen Runtime

Use the project-local Tauri CLI; a global `cargo-tauri` install is not required.

## Run the desktop app

```powershell
npm ci --prefix apps/desktop
npm --prefix apps/desktop run tauri -- dev
```

The hot-reload development process is launched from a terminal for developer diagnostics, but the product workflow itself does not require a console, PowerShell, JSON, or file picker.

## Validate

```powershell
npm run check --prefix apps/desktop
npm run lint --prefix apps/desktop
npm run build --prefix apps/desktop

cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --all --check
cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets

npm run tauri:build --prefix apps/desktop
```

The last command produces an unsigned development executable under `apps/desktop/src-tauri/target/release/`. It depends on the installed WebView2 Runtime and is not an installer or an official downloadable release.

## Test-only Registry dogfood

The production desktop binary contains no Registry write command. Maintainers can separately run the explicitly gated mutation-only harness in `scripts/set-desktop-dogfood-registry-value.ps1` between **Start capture** and **Finish & Compare**. The harness refuses an existing value and only removes the exact known synthetic type and data.

Do not run write-enabled dogfood on a machine or account where you are not authorized to make the test change.
