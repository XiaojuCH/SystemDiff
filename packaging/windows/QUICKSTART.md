# SystemDiff Windows x64 Developer Preview

This is an **unsigned, pre-release Developer Preview**, not an official SystemDiff release. It currently observes only the documented Windows Registry `Run` and `RunOnce` startup locations. Windows Services and Scheduled Tasks are not implemented.

SystemDiff runs locally, requires no account, includes no telemetry, and its product behavior is read-only. This package targets Windows x64. The current minimum collection platform is Windows 10 version 1709 or Windows Server 2016 version 1709.

## First run

Open PowerShell in the extracted directory:

```powershell
.\systemdiff.exe --help
.\systemdiff.exe collectors
```

Administrator privileges are not required. If Windows limits access to a Registry scope, SystemDiff reports the coverage gap instead of silently treating missing evidence as a removal.

## Compare before and after

```powershell
.\systemdiff.exe snapshot -o before.json

# Install or run the software you want to observe.

.\systemdiff.exe snapshot -o after.json
.\systemdiff.exe diff before.json after.json
```

Use `diff --technical` for exact text evidence or `diff --json` for the versioned machine-readable document. Compare Snapshots from the same Windows installation and the same user/principal context.

## Privacy

Snapshots and all report modes are unredacted. They may contain command strings, usernames in paths, hashes, and other host details. Review every file before sharing it, and never attach an unreviewed real Snapshot or report to a public Issue.

## Trust and removal

This preview is not Authenticode-signed, so Windows may show a SmartScreen or reputation warning. Verify the ZIP SHA-256 against the adjacent `SHA256SUMS` file and inspect the public source. SystemDiff does not ask you to disable or bypass Windows security controls.

The portable preview installs no service, driver, Scheduled Task, updater, or `PATH` entry. Delete the extracted directory to remove the program. Snapshot and report files you created elsewhere are not deleted automatically.
