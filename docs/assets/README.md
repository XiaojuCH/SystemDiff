# Demo assets

`registry-startup-demo.svg` is a static rendering of the exact human-readable output stored in `fixtures/reports/registry-added-human.txt`. The transcript is generated from the two synthetic Registry-only Snapshot fixtures and is enforced by a report regression test.

Reproduce the source output from the repository root:

```powershell
cargo run --locked --quiet -p systemdiff-cli -- diff fixtures/snapshots/registry-before-v1.json fixtures/snapshots/registry-after-v1.json
```

The fixtures contain one clearly synthetic `HKCU\\...\\Run` value. They contain no real host data and do not claim Services, Scheduled Tasks, signatures, rules, or risk classification. If renderer wording changes, update the transcript first from verified output, run the report tests, and then update the SVG to match it exactly.

An eventual animated recording should use the same guarded synthetic workflow: capture before, add the unique test-only HKCU value through the existing dual-gated harness, capture after, show the human Diff, verify exact-data cleanup, and retain no real Snapshot.
