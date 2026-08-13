[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,

    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$CommitSha
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Resolve-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [System.IO.Path]::GetFullPath($Path)
}

if ($env:OS -ne 'Windows_NT') {
    throw 'The portable Windows preview must be built on Windows.'
}

$repository = Resolve-FullPath -Path $RepositoryRoot
$output = Resolve-FullPath -Path $OutputDirectory
$cargoManifest = Join-Path $repository 'Cargo.toml'
$quickStart = Join-Path $repository 'packaging\windows\QUICKSTART.md'
$license = Join-Path $repository 'LICENSE'
$thirdPartyLicenses = Join-Path $repository 'THIRD_PARTY_LICENSES.txt'

foreach ($required in @($cargoManifest, $quickStart, $license, $thirdPartyLicenses)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required packaging input does not exist: $required"
    }
}

$metadata = & cargo metadata --locked --format-version 1 --manifest-path $cargoManifest |
    ConvertFrom-Json
if ($LASTEXITCODE -ne 0) {
    throw "Cargo metadata failed with exit code $LASTEXITCODE."
}
$workspaceMembers = @($metadata.workspace_members)
$lockedPackages = @(
    $metadata.packages |
        Where-Object { $workspaceMembers -notcontains $_.id } |
        ForEach-Object { "$($_.name) $($_.version)" } |
        Sort-Object -Unique
)
$licenseText = [System.IO.File]::ReadAllText($thirdPartyLicenses, [System.Text.Encoding]::UTF8)
$documentedPackages = @(
    [regex]::Matches($licenseText, '(?m)^- ([a-zA-Z0-9_-]+ [0-9][^\r\n ]*)$') |
        ForEach-Object { $_.Groups[1].Value } |
        Sort-Object -Unique
)
$missingLicenses = @($lockedPackages | Where-Object { $documentedPackages -notcontains $_ })
$staleLicenses = @($documentedPackages | Where-Object { $lockedPackages -notcontains $_ })
if ($missingLicenses.Count -ne 0 -or $staleLicenses.Count -ne 0) {
    throw "THIRD_PARTY_LICENSES.txt does not match Cargo.lock. Missing: $($missingLicenses -join ', '); stale: $($staleLicenses -join ', ')"
}

if (Test-Path -LiteralPath $output) {
    if ((Get-ChildItem -LiteralPath $output -Force | Measure-Object).Count -ne 0) {
        throw "Output directory must not already contain files: $output"
    }
} else {
    New-Item -ItemType Directory -Path $output | Out-Null
}

$targetTriple = 'x86_64-pc-windows-msvc'
$targetDirectory = Join-Path $repository 'target\portable-preview'
$executable = Join-Path $targetDirectory "$targetTriple\release\systemdiff.exe"
$rustFlagsName = 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS'
$previousRustFlags = [Environment]::GetEnvironmentVariable($rustFlagsName, 'Process')
$previousTargetDirectory = [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR', 'Process')

try {
    [Environment]::SetEnvironmentVariable(
        $rustFlagsName,
        '-C target-feature=+crt-static',
        'Process'
    )
    $env:CARGO_TARGET_DIR = $targetDirectory

    & cargo build `
        --locked `
        --release `
        --target $targetTriple `
        -p systemdiff-cli `
        --bin systemdiff `
        --manifest-path $cargoManifest
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo release build failed with exit code $LASTEXITCODE."
    }
} finally {
    [Environment]::SetEnvironmentVariable($rustFlagsName, $previousRustFlags, 'Process')
    [Environment]::SetEnvironmentVariable(
        'CARGO_TARGET_DIR',
        $previousTargetDirectory,
        'Process'
    )
}

if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Cargo did not produce the expected executable: $executable"
}

$rustcVersion = (& rustc --version).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($rustcVersion)) {
    throw 'Could not record the Rust compiler version.'
}

$stage = Join-Path ([System.IO.Path]::GetTempPath()) (
    'SystemDiff-package-' + [Guid]::NewGuid().ToString('N')
)
$zipPath = Join-Path $output 'systemdiff-windows-x86_64.zip'
$checksumPath = Join-Path $output 'SHA256SUMS'

try {
    New-Item -ItemType Directory -Path $stage | Out-Null
    Copy-Item -LiteralPath $executable -Destination (Join-Path $stage 'systemdiff.exe')
    Copy-Item -LiteralPath $quickStart -Destination (Join-Path $stage 'QUICKSTART.md')
    Copy-Item -LiteralPath $license -Destination (Join-Path $stage 'LICENSE')
    Copy-Item -LiteralPath $thirdPartyLicenses -Destination (Join-Path $stage 'THIRD_PARTY_LICENSES.txt')
    $executableHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $executable).Hash.ToLowerInvariant()

    $buildInfo = @(
        'SystemDiff Windows x64 Developer Preview'
        "Commit: $($CommitSha.ToLowerInvariant())"
        "Target: $targetTriple"
        'Cargo profile: release'
        'MSVC CRT linkage: static'
        "Rust compiler: $rustcVersion"
        "Executable SHA-256: $executableHash"
        'Official release: no'
        'Authenticode signed: no'
    ) -join "`n"
    [System.IO.File]::WriteAllText(
        (Join-Path $stage 'BUILD_INFO.txt'),
        $buildInfo + "`n",
        [System.Text.UTF8Encoding]::new($false)
    )

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $stage,
        $zipPath,
        [System.IO.Compression.CompressionLevel]::Optimal,
        $false
    )
} finally {
    if (Test-Path -LiteralPath $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
}

$zipHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText(
    $checksumPath,
    "$zipHash  systemdiff-windows-x86_64.zip`n",
    [System.Text.UTF8Encoding]::new($false)
)

$outerFiles = @(Get-ChildItem -LiteralPath $output -Force | Sort-Object Name)
if ($outerFiles.Count -ne 2 -or
    $outerFiles[0].PSIsContainer -or
    $outerFiles[1].PSIsContainer -or
    (($outerFiles[0].Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) -or
    (($outerFiles[1].Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) -or
    $outerFiles[0].Name -ne 'SHA256SUMS' -or
    $outerFiles[1].Name -ne 'systemdiff-windows-x86_64.zip') {
    throw 'Packaging output did not match the two-file allowlist.'
}

Write-Output "Created: $zipPath"
Write-Output "SHA-256: $zipHash"
Write-Output "ZIP bytes: $((Get-Item -LiteralPath $zipPath).Length)"
Write-Output "EXE bytes: $((Get-Item -LiteralPath $executable).Length)"
