# Portable Windows developer preview

Status: In progress
Owner: primary agent
Last updated: 2026-08-13

## Goal

Issue #9 removes the Rust/MSVC development-environment barrier for the current Registry-only product slice. The repository should produce a short-lived GitHub Actions artifact containing a release-mode Windows x64 CLI, a small end-user quick start, license, build provenance, and a checksum, then verify the uploaded bytes in a later clean job.

This is productization of the existing CLI, not an official SystemDiff release.

## User-visible outcome

A signed-in GitHub user can download the successful CI run artifact, verify `systemdiff-windows-x86_64.zip`, extract it, and run `systemdiff.exe` without Cargo or a Rust toolchain. The packaged executable supports the already implemented `--help`, `collectors`, `snapshot`, and Registry-only `diff` workflows. Documentation clearly labels the artifact as an unsigned, ephemeral Developer Preview rather than a release asset.

## Current architecture and context

- The workspace version remains `0.0.0`; `systemdiff-cli` declares the binary name `systemdiff`.
- The existing `rust` CI matrix provides the required `Rust (windows-latest)` and `Rust (ubuntu-latest)` correctness checks with repository contents read-only permission.
- The CLI has no runtime network feature, account, telemetry, installer, updater, persistence, or Registry write path. Real snapshot collection is query-only and does not require administrator elevation; denied coverage is reported.
- The current minimum collection platform is Windows 10 version 1709 or Windows Server 2016 version 1709. This preview is x86_64 only.
- A real default release build on 2026-08-13 produced an x64 CUI PE of 1,652,224 bytes. `dumpbin /DEPENDENTS` showed `VCRUNTIME140.dll` and UCRT API-set imports, and `mt.exe` confirmed the file had no manifest resource.
- A controlled release build with `-C target-feature=+crt-static` produced an x64 CUI PE of 1,751,040 bytes. Its dependency list contained only Windows system DLL/API-set imports (`advapi32`, `ntdll`, `kernel32`, and `api-ms-win-core-synch-l1-2-0`), removing the separate VC runtime dependency.
- GitHub Actions artifacts are run-scoped, expire, and are not GitHub Release assets. Browser download access requires GitHub sign-in; documentation must preserve that friction rather than call this a public release download.

## Constraints

- Do not create a GitHub Release, tag, semantic version, signing infrastructure, installer, package-manager entry, GUI, session/baseline workflow, or Collector.
- Preserve `permissions: contents: read`; do not use `pull_request_target`, secrets, or attacker-controlled GitHub context in shell commands or packaged metadata.
- Pull requests continue to run correctness checks but do not upload a trusted-looking developer-preview executable. Normal packaging is limited to upstream `main` pushes. The exact Issue #9 branch has a narrow pre-merge push exception and a distinct `-candidate` artifact name; a fork pull request cannot generate either upstream event.
- The archive is built from an explicit allowlist. It must not include PDBs, snapshots, credentials, runner paths, repository internals, or staging directories.
- The executable remains unsigned. Documentation may describe possible SmartScreen/reputation warnings, checksum verification, and source inspection, but must never recommend disabling or bypassing Windows security controls.
- Snapshots and every report mode remain unredacted and sensitive by default.
- Static CRT removes the observed VC redistributable imports; it does not make Windows system DLLs disappear and is not proof of compatibility on every clean machine.
- Do not claim reproducible builds. The actual guarantees are a locked Cargo graph, immutable Action pins, commit-linked metadata, exact artifact verification, and a SHA-256 checksum.

## Implementation steps

1. Add a minimal Windows MSVC executable manifest for the `systemdiff` binary with `requestedExecutionLevel="asInvoker"` and `uiAccess="false"`. Link it through a standard-library-only Cargo build script and verify the final PE resource; do not add a resource crate.
2. Add `packaging/windows/QUICKSTART.md` for non-Rust users. Cover preview status, x64/minimum OS, Registry-only scope, first commands, before/after workflow, privacy, unsigned status, no-admin expectation, and deletion-based uninstall.
3. Add a focused PowerShell packaging script that:
   - builds `systemdiff-cli` with `--locked --release --target x86_64-pc-windows-msvc`;
   - enables static MSVC CRT only for this portable build;
   - constructs an allowlisted staging root containing `systemdiff.exe`, `QUICKSTART.md`, `LICENSE`, `THIRD_PARTY_LICENSES.txt`, and `BUILD_INFO.txt`;
   - writes only non-sensitive commit/target/Rust/profile/CRT metadata;
   - creates `systemdiff-windows-x86_64.zip` and a standard external `SHA256SUMS` entry for that ZIP.
4. Add a focused PowerShell verifier that accepts explicit artifact and fixture paths, then:
   - enforces exact artifact and ZIP contents and verifies SHA-256;
   - checks PE x64/CUI identity, direct imported DLLs, no observed dynamic VC runtime imports, and no common direct network DLL imports;
   - extracts and parses the embedded manifest, requiring `asInvoker` and `uiAccess=false`;
   - reports the Authenticode state and requires the documented unsigned-preview state;
   - launches only the extracted artifact executable for `--help`, `collectors`, human/technical/JSON fixture diffs, and a real read-only Snapshot;
   - parses the Snapshot header and Collector presence, then deletes only its exact temporary files.
5. Extend CI after the existing matrix gate:
   - a Windows package job builds and verifies the preview, then uploads only the ZIP and checksum with immutable action pins and 14-day retention;
   - a later fresh Windows job downloads that workflow artifact, checks out fixtures, and runs the same artifact-only verifier without building through Cargo;
   - package/verify jobs run only for upstream pushes to `main` and the exact Issue #9 branch, never for a pull-request event; the feature-branch artifact is visibly labeled as a candidate.
6. Update English and Chinese READMEs, project state, roadmap, and the supply-chain portion of the threat model. Keep the hero focused on product value, maintain factual parity, and remove the stale source-build-only statement without implying an official release.
7. Run local validation, independent review, final-diff inspection, and documentation checks. Fix only material findings.
8. Commit, push `build/portable-developer-preview`, open a ready PR closing Issue #9, wait for every remote check, inspect the actual uploaded artifact metadata, download it back, and rerun verification. Do not merge.

## Affected files and modules

- `.agent/plans/portable-windows-developer-preview.md`
- `.agent/PROJECT_STATE.md`
- `.github/workflows/ci.yml`
- `crates/systemdiff-cli/build.rs`
- `crates/systemdiff-cli/systemdiff.manifest`
- `THIRD_PARTY_LICENSES.txt`
- `packaging/windows/QUICKSTART.md`
- `scripts/package-windows-preview.ps1`
- `scripts/verify-windows-preview.ps1`
- `README.md`
- `README.zh-CN.md`
- `docs/roadmap.md`
- `docs/threat-model.md`

No Rust source, public schema, fixture, Cargo dependency, Collector, or product runtime behavior should change.

## Test strategy

- Existing gates: `cargo fmt --all --check`, locked workspace Clippy with warnings denied, and locked workspace all-target tests.
- Build: a real locked `release` build for explicit `x86_64-pc-windows-msvc`, with the portable path's static CRT setting.
- Package integrity: exact allowlists at artifact and archive roots, normalized checksum format, checksum re-computation, no PDB or nested staging content.
- PE inspection: actual `dumpbin` headers/dependencies and `mt.exe` extraction of the final packaged executable; fail closed on wrong architecture, forbidden runtime imports, missing/wrong manifest, or unexpected signing state.
- Artifact-only CLI: packaged `--help`, `collectors`, human/technical/JSON Registry fixture diffs, and read-only `snapshot -o` with parsed v1 header and `windows.registry.startup` presence.
- CI chain: the smoke job downloads the artifact created by the preceding upload job on a fresh runner and never runs Cargo.
- Post-push: use GitHub API/CLI to confirm run, job results, artifact name/size/expiry; download remote artifact and repeat checksum/archive/binary/smoke verification locally.
- Manual: inspect README links, EN/ZH factual parity, stale source-build language, `git diff --check`, final tracked file list, and absence of machine paths/secrets.

## Risks

- A workflow artifact can be mistaken for a release. Mitigation: Developer Preview naming in the artifact, package, quick start, workflow jobs, and both READMEs; no tag/release/version bump.
- Fork code could publish an executable with a trusted-looking name. Mitigation: job-level same-repository guard; fork PRs run only the existing correctness matrix; no `pull_request_target` or privileged token.
- Dynamic CRT imports could make a supposedly portable executable fail on a fresh system. Mitigation: packaging-only static CRT plus inspection of the exact zipped executable; still retain a clean-machine manual-validation caveat.
- An implicit/default manifest could trigger Windows installer detection or obscure privilege intent. Mitigation: explicit embedded manifest verified from the packaged PE.
- Packaging could leak host/repository files. Mitigation: exact five-file ZIP allowlist and two-file outer artifact allowlist, verified before and after upload.
- CI can prove artifact-only execution on a hosted runner but not an arbitrary clean supported Windows installation. Mitigation: precise documentation and a deferred clean-machine release gate.
- Static CRT fixes require rebuilding to receive CRT security fixes. Mitigation: previews are short-lived CI artifacts; dependency/toolchain updates rebuild the executable. Revisit for a signed official release.

## Rollback and compatibility

The change is isolated to build metadata, scripts, CI, and documentation. Reverting it removes Developer Preview artifact production without affecting source builds, Snapshot/Diff schemas, fixtures, CLI arguments, or runtime evidence. Existing required check names remain unchanged. The explicit `asInvoker` manifest makes the intended existing privilege model reliable but does not add an elevation path.

## Progress

- [x] 2026-08-13: synchronized clean `main` at `b9c6a981ef7f45c2f4645066a050f4e97b5a9863`; latest `main` CI was green.
- [x] 2026-08-13: created Issue #9 and branch `build/portable-developer-preview`.
- [x] 2026-08-13: built and inspected default and static-CRT release executables; recorded exact sizes, imports, signature state, and missing manifest.
- [x] 2026-08-13: implemented packaging, binary verification, upstream-push-only CI upload/download verification, and English/Chinese end-user documentation.
- [x] 2026-08-13: local locked format, Clippy, and 88-test workspace gates passed; the packaged artifact-only smoke passed with the toolchain removed from `PATH`.
- [x] 2026-08-13: independent review found no High issues and five Medium/one Low across publication scope, upload allowlisting, timeouts, manifest namespace validation, and import validation; all were addressed and the focused package verifier reran successfully.
- [ ] Open PR, wait for remote CI, and verify the downloaded artifact.

## Discoveries

- The default Rust MSVC release executable is not sufficient evidence for “no VC++ Redistributable”: it directly imports `VCRUNTIME140.dll` and UCRT API-set libraries on the actual maintainer machine.
- Static CRT is a small measured size tradeoff here: +98,816 bytes (about 6%) before adding the manifest, while removing the observed redistributable imports.
- The default executable contains no resource section, so privilege intent is not currently explicit or inspectable. A manifest is a correctness change to build metadata, not packaging decoration.
- A ZIP cannot contain a checksum of itself. `SHA256SUMS` therefore belongs beside the ZIP in the outer Actions artifact; the inner ZIP remains the direct download/extract/run payload.
- The final local manifest-bearing portable executable is 1,752,064 bytes. After adding required third-party license notices, its latest ZIP is 705,116 bytes with SHA-256 `0bb679fdb31deb28c14811350b5bb93ac31c658cd3f03b6d8380a9af15ac6459`; these local working-tree bytes can differ across package runs and from the later commit-built remote artifact because reproducible ZIP bytes are not claimed.

## Decisions

- Keep Cargo version `0.0.0`. A CI Developer Preview is not a semantic release promise.
- Use static CRT only in the portable packaging command, leaving normal development and existing CI compilation behavior unchanged.
- Add a standard-library-only `build.rs` plus source manifest rather than a new resource dependency.
- Package exactly five inner files: executable, quick start, project license, third-party license notices, and concise build info. Upload exactly the ZIP and its checksum.
- Defer artifact attestations. They require additional `id-token`/attestation write permissions and workflow complexity that is disproportionate for an unsigned, expiring CI preview; checksums and exact download-back verification are the focused controls here.
- Use immutable SHA pins for official upload/download Artifact actions. Keep global workflow permissions at `contents: read`.
- Do not call the artifact a download or release. Document browser sign-in, run-scoped retention, and the eventual need for a signed Release asset.

## Final validation

Local validation completed on 2026-08-13:

- `cargo build --locked --release -p systemdiff-cli`: passed; produced `systemdiff.exe` with the explicit `asInvoker`, `uiAccess=false` manifest.
- `cargo fmt --all --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace --all-targets`: passed, 88 tests and no failures.
- `scripts/package-windows-preview.ps1`: passed with explicit x64 target, release profile, and packaging-only static CRT.
- `scripts/verify-windows-preview.ps1 -RemoveToolchainFromPath`: passed checksum, exact contents, PE parser, normal/delay import, manifest, unsigned-state, `--help`, `collectors`, human/technical/JSON Diff, and real read-only Snapshot checks.
- Independent `dumpbin`: confirmed AMD64 CUI, a manifest resource, no delay-import directory, and only `advapi32.dll`, `ntdll.dll`, `kernel32.dll`, and `api-ms-win-core-synch-l1-2-0.dll` direct dependencies. A binary string search found no local workspace path.
- `git diff --check`, PowerShell syntax parsing, local Markdown link targets, stale wording, and machine-path/secret scans: passed.

Independent review, commit-linked repackage evidence, remote jobs/artifact metadata, and remote download-back verification remain pending.
