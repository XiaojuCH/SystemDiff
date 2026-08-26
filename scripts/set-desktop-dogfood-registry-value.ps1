[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Add', 'Remove')]
    [string]$Action,

    [Parameter(Mandatory = $true)]
    [switch]$ConfirmSyntheticRegistryTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($env:SYSTEMDIFF_RUN_DESKTOP_DOGFOOD -ne '1' -or -not $ConfirmSyntheticRegistryTest) {
    throw 'Refusing to write test Registry evidence. Set SYSTEMDIFF_RUN_DESKTOP_DOGFOOD=1 and pass -ConfirmSyntheticRegistryTest.'
}

# This mutation-only harness is deliberately separate from the production
# desktop executable. It never writes outside the current user's existing Run
# key and cleanup is allowed only when the exact known type and data still match.
$keyPath = 'Software\Microsoft\Windows\CurrentVersion\Run'
$valueName = 'SystemDiffDogfood'
$expectedData = 'cmd.exe /d /c exit 0'

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

function Assert-ExactSyntheticValue {
    param(
        [Parameter(Mandatory = $true)]
        [Microsoft.Win32.RegistryKey]$Key,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $actualName = Get-MatchingValueName -Key $Key -Name $Name
    if ($null -eq $actualName) {
        throw "Synthetic Registry value '$Name' is absent."
    }
    $actualKind = $Key.GetValueKind($actualName)
    $actualData = $Key.GetValue(
        $actualName,
        $null,
        [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
    )
    if ($actualKind -ne [Microsoft.Win32.RegistryValueKind]::String -or $actualData -cne $expectedData) {
        throw "Refusing operation: '$actualName' does not have the exact known synthetic type and data."
    }
    return $actualName
}

$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($keyPath, $true)
if ($null -eq $key) {
    throw 'The existing HKCU Run key could not be opened; the harness will not create it.'
}

try {
    if ($Action -eq 'Add') {
        if ($null -ne (Get-MatchingValueName -Key $key -Name $valueName)) {
            throw "Refusing to overwrite existing Registry value '$valueName'."
        }

        $writeAttempted = $false
        try {
            # Recheck immediately before the non-atomic Registry value write.
            if ($null -ne (Get-MatchingValueName -Key $key -Name $valueName)) {
                throw "Registry value '$valueName' appeared after preflight; refusing to overwrite it."
            }
            $writeAttempted = $true
            $key.SetValue($valueName, $expectedData, [Microsoft.Win32.RegistryValueKind]::String)
            $actualName = Assert-ExactSyntheticValue -Key $key -Name $valueName
            Write-Output "Added exact synthetic HKCU Run value: $actualName"
            Write-Output "Synthetic data: $expectedData"
        }
        catch {
            if ($writeAttempted) {
                $actualName = Get-MatchingValueName -Key $key -Name $valueName
                if ($null -ne $actualName) {
                    $actualKind = $key.GetValueKind($actualName)
                    $actualData = $key.GetValue(
                        $actualName,
                        $null,
                        [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
                    )
                    if ($actualKind -eq [Microsoft.Win32.RegistryValueKind]::String -and $actualData -ceq $expectedData) {
                        $key.DeleteValue($actualName, $true)
                    }
                }
            }
            throw
        }
    }
    else {
        $actualName = Get-MatchingValueName -Key $key -Name $valueName
        if ($null -eq $actualName) {
            Write-Output 'Cleanup: synthetic Registry value is already absent'
            return
        }
        $exactName = Assert-ExactSyntheticValue -Key $key -Name $valueName
        $key.DeleteValue($exactName, $true)
        if ($null -ne (Get-MatchingValueName -Key $key -Name $valueName)) {
            throw 'Cleanup failed: the synthetic Registry value still exists.'
        }
        Write-Output 'Cleanup: exact-data guarded deletion verified'
    }
}
finally {
    $key.Dispose()
}
