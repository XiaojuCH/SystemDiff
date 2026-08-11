[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [switch]$ConfirmSyntheticRegistryTest,

    [switch]$RecoveryOnly,

    [string]$SyntheticValueName
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($env:SYSTEMDIFF_RUN_SYNTHETIC_E2E -ne '1' -or -not $ConfirmSyntheticRegistryTest) {
    throw 'Refusing to write test Registry evidence. Set SYSTEMDIFF_RUN_SYNTHETIC_E2E=1 and pass -ConfirmSyntheticRegistryTest.'
}

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
$cargo = if ($null -ne $cargoCommand) {
    $cargoCommand.Source
}
else {
    Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
}
if (-not (Test-Path -LiteralPath $cargo -PathType Leaf)) {
    throw "Cargo was not found at '$cargo'."
}

$keyPath = 'Software\Microsoft\Windows\CurrentVersion\Run'
$syntheticNamePattern = '^SystemDiffSyntheticE2E-[0-9a-f]{32}$'
if ($RecoveryOnly -and [String]::IsNullOrWhiteSpace($SyntheticValueName)) {
    throw 'Recovery requires -SyntheticValueName with the exact name printed by the original run.'
}
$valueName = if ([String]::IsNullOrWhiteSpace($SyntheticValueName)) {
    'SystemDiffSyntheticE2E-' + [Guid]::NewGuid().ToString('N')
}
else {
    $SyntheticValueName
}
if ($valueName -cnotmatch $syntheticNamePattern) {
    throw "Synthetic value name '$valueName' does not match the guarded SystemDiff E2E format."
}
$expectedData = '"' + (Join-Path $env:SystemRoot 'System32\cmd.exe') + '" /d /c exit 0'
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("SystemDiffRegistryE2E-{0}" -f [Guid]::NewGuid().ToString('N'))
$beforePath = Join-Path $temporaryRoot 'before.json'
$afterPath = Join-Path $temporaryRoot 'after.json'
$cleanupEligible = $false
$cleanupVerified = $false
$temporaryCleanupVerified = $false

Write-Output "Synthetic value name: $valueName"
Write-Warning 'Snapshot files contain privacy-sensitive local evidence. This harness deletes them after validation; do not upload recovered files without review and sanitization.'

function Get-MatchingValueName {
    param(
        [Parameter(Mandatory = $true)]
        [Microsoft.Win32.RegistryKey]$Key,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    foreach ($candidate in $Key.GetValueNames()) {
        if ([StringComparer]::OrdinalIgnoreCase.Equals($candidate, $Name)) {
            return $candidate
        }
    }
    return $null
}

function Invoke-SystemDiff {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Executable,
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $output = & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "SystemDiff exited with code $LASTEXITCODE while running: $($Arguments -join ' ')"
    }
    return $output
}

if ($RecoveryOnly) {
    $recoveryKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($keyPath, $true)
    if ($null -eq $recoveryKey) {
        throw 'Recovery failed: the HKCU Run key could not be opened.'
    }
    try {
        $actualName = Get-MatchingValueName -Key $recoveryKey -Name $valueName
        if ($null -eq $actualName) {
            Write-Output 'Recovery: synthetic Registry value already absent'
            return
        }
        $actualKind = $recoveryKey.GetValueKind($actualName)
        $actualData = $recoveryKey.GetValue($actualName, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        if ($actualKind -ne [Microsoft.Win32.RegistryValueKind]::String -or $actualData -cne $expectedData) {
            throw "Recovery refused: '$actualName' does not have the exact known synthetic type and data."
        }
        $recoveryKey.DeleteValue($actualName, $true)
        if ($null -ne (Get-MatchingValueName -Key $recoveryKey -Name $valueName)) {
            throw 'Recovery failed: the synthetic Registry value still exists.'
        }
        Write-Output 'Recovery: exact-data guarded deletion verified'
        return
    }
    finally {
        $recoveryKey.Dispose()
    }
}

New-Item -ItemType Directory -Path $temporaryRoot -ErrorAction Stop | Out-Null

try {
    $preflightKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($keyPath, $false)
    if ($null -eq $preflightKey) {
        throw "The HKCU Run key does not exist; the harness will not create it."
    }
    try {
        if ($null -ne (Get-MatchingValueName -Key $preflightKey -Name $valueName)) {
            throw "Refusing to overwrite existing Registry value '$valueName'."
        }
    }
    finally {
        $preflightKey.Dispose()
    }

    Push-Location $repositoryRoot
    try {
        & $cargo build --locked -p systemdiff-cli
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo build failed with code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }

    $systemdiff = Join-Path $repositoryRoot 'target\debug\systemdiff.exe'
    if (-not (Test-Path -LiteralPath $systemdiff -PathType Leaf)) {
        throw "Built SystemDiff executable was not found at '$systemdiff'."
    }

    Invoke-SystemDiff -Executable $systemdiff -Arguments @('snapshot', '-o', $beforePath) | Out-Null

    $writeKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($keyPath, $true)
    if ($null -eq $writeKey) {
        throw 'The HKCU Run key could not be opened for the explicitly gated test mutation.'
    }
    try {
        if ($null -ne (Get-MatchingValueName -Key $writeKey -Name $valueName)) {
            throw "Registry value '$valueName' appeared after preflight; refusing to overwrite it."
        }
        $cleanupEligible = $true
        $writeKey.SetValue($valueName, $expectedData, [Microsoft.Win32.RegistryValueKind]::String)
        $actualName = Get-MatchingValueName -Key $writeKey -Name $valueName
        if ($null -eq $actualName) {
            throw 'Synthetic Registry value was not observable immediately after the test write.'
        }
        $actualKind = $writeKey.GetValueKind($actualName)
        $actualData = $writeKey.GetValue($actualName, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        if ($actualKind -ne [Microsoft.Win32.RegistryValueKind]::String -or $actualData -cne $expectedData) {
            throw 'Synthetic Registry value did not round-trip with the exact expected type and data.'
        }
    }
    finally {
        $writeKey.Dispose()
    }

    Invoke-SystemDiff -Executable $systemdiff -Arguments @('snapshot', '-o', $afterPath) | Out-Null
    $diffText = (Invoke-SystemDiff -Executable $systemdiff -Arguments @('diff', '--json', $beforePath, $afterPath)) -join [Environment]::NewLine
    $diff = $diffText | ConvertFrom-Json
    $changes = @($diff.changes)
    $added = @($changes | Where-Object { $_.change.change -eq 'added' })
    $removed = @($changes | Where-Object { $_.change.change -eq 'removed' })

    if ($changes.Count -ne 1 -or $added.Count -ne 1 -or $removed.Count -ne 0) {
        throw "Expected exactly one Added change and no other changes; observed $($changes.Count) total, $($added.Count) Added, and $($removed.Count) Removed."
    }

    $change = $added[0]
    $evidence = $change.change.after.evidence
    if (
        $change.key.collector_id -ne 'windows.registry.startup' -or
        $change.key.scope_id -ne 'current_user.shared.run' -or
        $change.change.after.kind -ne 'registry_startup' -or
        $evidence.hive -ne 'current_user' -or
        $evidence.registry_view -ne 'shared' -or
        $evidence.startup_kind -ne 'run' -or
        $evidence.value_name.encoding -ne 'decoded' -or
        $evidence.value_name.value -cne $valueName
    ) {
        throw 'The Added change did not match the exact synthetic HKCU Shared Run evidence identity.'
    }

    Write-Output 'Before Snapshot: synthetic value absent'
    Write-Output 'Synthetic mutation: exact HKCU Shared Run REG_SZ established'
    Write-Output 'After Snapshot: synthetic value present'
    Write-Output 'Diff: exactly 1 Added, 0 Removed, expected identity matched'
}
finally {
    $cleanupFailure = $null
    if ($cleanupEligible) {
        try {
            $cleanupKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($keyPath, $true)
            if ($null -eq $cleanupKey) {
                throw 'Cleanup failed: the HKCU Run key could not be opened.'
            }
            try {
                $actualName = Get-MatchingValueName -Key $cleanupKey -Name $valueName
                if ($null -eq $actualName) {
                    $cleanupVerified = $true
                }
                else {
                    $actualKind = $cleanupKey.GetValueKind($actualName)
                    $actualData = $cleanupKey.GetValue($actualName, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
                    if ($actualKind -ne [Microsoft.Win32.RegistryValueKind]::String -or $actualData -cne $expectedData) {
                        throw "Cleanup refused: '$actualName' no longer has the exact synthetic type and data."
                    }
                    $cleanupKey.DeleteValue($actualName, $true)
                    $cleanupVerified = $null -eq (Get-MatchingValueName -Key $cleanupKey -Name $valueName)
                    if (-not $cleanupVerified) {
                        throw 'Cleanup failed: the synthetic Registry value still exists.'
                    }
                }
            }
            finally {
                $cleanupKey.Dispose()
            }
        }
        catch {
            $cleanupFailure = $_.Exception.Message
        }
    }

    try {
        $resolvedTemporaryRoot = [System.IO.Path]::GetFullPath($temporaryRoot)
        $expectedTemporaryPrefix = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
        if (-not $resolvedTemporaryRoot.StartsWith($expectedTemporaryPrefix, [StringComparison]::OrdinalIgnoreCase) -or
            -not ([System.IO.Path]::GetFileName($resolvedTemporaryRoot)).StartsWith('SystemDiffRegistryE2E-', [StringComparison]::Ordinal)) {
            throw "Refusing to remove unexpected temporary path '$resolvedTemporaryRoot'."
        }
        if ([System.IO.Directory]::Exists($resolvedTemporaryRoot)) {
            $directoryAttributes = [System.IO.File]::GetAttributes($resolvedTemporaryRoot)
            if (($directoryAttributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "Refusing to remove reparse-point temporary directory '$resolvedTemporaryRoot'."
            }
            $allowedFiles = @(
                [System.IO.Path]::GetFullPath($beforePath),
                [System.IO.Path]::GetFullPath($afterPath)
            )
            foreach ($entry in [System.IO.Directory]::EnumerateFileSystemEntries($resolvedTemporaryRoot)) {
                $resolvedEntry = [System.IO.Path]::GetFullPath($entry)
                if ($allowedFiles -notcontains $resolvedEntry) {
                    throw "Refusing to remove unexpected temporary entry '$resolvedEntry'."
                }
                $entryAttributes = [System.IO.File]::GetAttributes($resolvedEntry)
                if (($entryAttributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
                    ($entryAttributes -band [System.IO.FileAttributes]::Directory) -ne 0) {
                    throw "Refusing to remove unexpected temporary entry type '$resolvedEntry'."
                }
            }
            foreach ($knownFile in $allowedFiles) {
                if ([System.IO.File]::Exists($knownFile)) {
                    [System.IO.File]::Delete($knownFile)
                }
            }
            [System.IO.Directory]::Delete($resolvedTemporaryRoot, $false)
        }
        $temporaryCleanupVerified = -not [System.IO.Directory]::Exists($resolvedTemporaryRoot)
        if (-not $temporaryCleanupVerified) {
            throw 'Temporary Snapshot directory still exists after cleanup.'
        }
    }
    catch {
        $temporaryFailure = $_.Exception.Message
        $cleanupFailure = if ($null -eq $cleanupFailure) {
            $temporaryFailure
        }
        else {
            "$cleanupFailure Temporary-file cleanup also failed: $temporaryFailure"
        }
    }
    if ($null -ne $cleanupFailure) {
        throw $cleanupFailure
    }
}

if (-not $cleanupVerified) {
    throw 'Synthetic Registry cleanup was not verified.'
}
Write-Output 'Cleanup: exact-data guarded deletion verified'
if (-not $temporaryCleanupVerified) {
    throw 'Temporary Snapshot cleanup was not verified.'
}
Write-Output 'Temporary Snapshots: deletion verified'
