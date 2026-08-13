[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactDirectory,

    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$ExpectedCommitSha,

    [switch]$RemoveToolchainFromPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (-not $Condition) {
        throw $Message
    }
}

function Read-AsciiString {
    param(
        [Parameter(Mandatory = $true)][System.IO.BinaryReader]$Reader,
        [Parameter(Mandatory = $true)][long]$Offset,
        [Parameter(Mandatory = $true)][long]$FileLength
    )
    Assert-Condition ($Offset -ge 0 -and $Offset -lt $FileLength) 'PE string offset is outside the file.'
    $Reader.BaseStream.Position = $Offset
    $bytes = [System.Collections.Generic.List[byte]]::new()
    while ($Reader.BaseStream.Position -lt $FileLength -and $bytes.Count -lt 4096) {
        $value = $Reader.ReadByte()
        if ($value -eq 0) {
            return [System.Text.Encoding]::ASCII.GetString($bytes.ToArray())
        }
        $bytes.Add($value)
    }
    throw 'PE string is unterminated or exceeds the inspection limit.'
}

function Read-PeMetadata {
    param([Parameter(Mandatory = $true)][string]$ExecutablePath)

    $stream = [System.IO.File]::Open(
        $ExecutablePath,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::Read
    )
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        $length = $stream.Length
        Assert-Condition ($length -ge 512) 'Executable is too small to be a valid PE file.'
        Assert-Condition ($reader.ReadUInt16() -eq 0x5a4d) 'Executable does not begin with the DOS MZ signature.'

        $stream.Position = 0x3c
        $peOffset = [long]$reader.ReadUInt32()
        Assert-Condition ($peOffset -ge 0x40 -and $peOffset + 24 -le $length) 'PE header offset is invalid.'
        $stream.Position = $peOffset
        Assert-Condition ($reader.ReadUInt32() -eq 0x00004550) 'Executable does not contain the PE signature.'

        $machine = $reader.ReadUInt16()
        $sectionCount = [int]$reader.ReadUInt16()
        $stream.Position = $peOffset + 20
        $optionalHeaderSize = [int]$reader.ReadUInt16()
        $optionalHeaderOffset = $peOffset + 24
        Assert-Condition ($optionalHeaderSize -ge 120) 'PE optional header is too small.'
        Assert-Condition ($optionalHeaderOffset + $optionalHeaderSize -le $length) 'PE optional header exceeds the file.'

        $stream.Position = $optionalHeaderOffset
        $optionalMagic = $reader.ReadUInt16()
        Assert-Condition ($optionalMagic -eq 0x020b) 'Executable is not a PE32+ image.'
        $stream.Position = $optionalHeaderOffset + 24
        $imageBase = $reader.ReadUInt64()
        $stream.Position = $optionalHeaderOffset + 68
        $subsystem = $reader.ReadUInt16()
        $stream.Position = $optionalHeaderOffset + 108
        $directoryCount = $reader.ReadUInt32()

        $sectionTableOffset = $optionalHeaderOffset + $optionalHeaderSize
        Assert-Condition ($sectionTableOffset + (40 * $sectionCount) -le $length) 'PE section table exceeds the file.'
        $sections = @()
        for ($sectionIndex = 0; $sectionIndex -lt $sectionCount; $sectionIndex++) {
            $stream.Position = $sectionTableOffset + (40 * $sectionIndex) + 8
            $virtualSize = $reader.ReadUInt32()
            $virtualAddress = $reader.ReadUInt32()
            $rawSize = $reader.ReadUInt32()
            $rawPointer = $reader.ReadUInt32()
            $sections += [pscustomobject]@{
                VirtualSize = [uint64]$virtualSize
                VirtualAddress = [uint64]$virtualAddress
                RawSize = [uint64]$rawSize
                RawPointer = [uint64]$rawPointer
            }
        }

        $rvaToOffset = {
            param([uint64]$Rva)
            foreach ($section in $sections) {
                $span = [Math]::Max($section.VirtualSize, $section.RawSize)
                if ($Rva -ge $section.VirtualAddress -and $Rva -lt $section.VirtualAddress + $span) {
                    $offset = $section.RawPointer + ($Rva - $section.VirtualAddress)
                    Assert-Condition ($offset -lt [uint64]$length) 'PE RVA maps outside the file.'
                    return [long]$offset
                }
            }
            throw "PE RVA 0x$($Rva.ToString('x')) does not map to a section."
        }

        $readDirectory = {
            param([int]$Index)
            if ($directoryCount -le [uint32]$Index) {
                return [pscustomobject]@{ Rva = [uint32]0; Size = [uint32]0 }
            }
            $entryOffset = $optionalHeaderOffset + 112 + (8 * $Index)
            Assert-Condition ($entryOffset + 8 -le $optionalHeaderOffset + $optionalHeaderSize) 'PE data directory exceeds the optional header.'
            $stream.Position = $entryOffset
            return [pscustomobject]@{ Rva = $reader.ReadUInt32(); Size = $reader.ReadUInt32() }
        }

        $imports = [System.Collections.Generic.List[string]]::new()
        $importDirectory = & $readDirectory 1
        if ($importDirectory.Rva -ne 0) {
            $descriptorOffset = & $rvaToOffset ([uint64]$importDirectory.Rva)
            for ($descriptorIndex = 0; $descriptorIndex -lt 4096; $descriptorIndex++) {
                $offset = $descriptorOffset + (20 * $descriptorIndex)
                Assert-Condition ($offset + 20 -le $length) 'PE import descriptor exceeds the file.'
                $stream.Position = $offset
                $originalFirstThunk = $reader.ReadUInt32()
                $timeDateStamp = $reader.ReadUInt32()
                $forwarderChain = $reader.ReadUInt32()
                $nameRva = $reader.ReadUInt32()
                $firstThunk = $reader.ReadUInt32()
                if (($originalFirstThunk -bor $timeDateStamp -bor $forwarderChain -bor $nameRva -bor $firstThunk) -eq 0) {
                    break
                }
                Assert-Condition ($nameRva -ne 0) 'PE import descriptor has no DLL name.'
                $imports.Add((Read-AsciiString -Reader $reader -Offset (& $rvaToOffset ([uint64]$nameRva)) -FileLength $length))
                if ($descriptorIndex -eq 4095) {
                    throw 'PE import descriptor count exceeds the inspection limit.'
                }
            }
        }

        $delayImports = [System.Collections.Generic.List[string]]::new()
        $delayDirectory = & $readDirectory 13
        if ($delayDirectory.Rva -ne 0) {
            $descriptorOffset = & $rvaToOffset ([uint64]$delayDirectory.Rva)
            for ($descriptorIndex = 0; $descriptorIndex -lt 4096; $descriptorIndex++) {
                $offset = $descriptorOffset + (32 * $descriptorIndex)
                Assert-Condition ($offset + 32 -le $length) 'PE delay-load descriptor exceeds the file.'
                $stream.Position = $offset
                $attributes = $reader.ReadUInt32()
                $nameValue = $reader.ReadUInt32()
                $moduleHandle = $reader.ReadUInt32()
                $delayIat = $reader.ReadUInt32()
                $delayInt = $reader.ReadUInt32()
                $boundIat = $reader.ReadUInt32()
                $unloadIat = $reader.ReadUInt32()
                $timestamp = $reader.ReadUInt32()
                if (($attributes -bor $nameValue -bor $moduleHandle -bor $delayIat -bor $delayInt -bor $boundIat -bor $unloadIat -bor $timestamp) -eq 0) {
                    break
                }
                Assert-Condition ($nameValue -ne 0) 'PE delay-load descriptor has no DLL name.'
                $nameRva = if (($attributes -band 1) -eq 1) {
                    [uint64]$nameValue
                } else {
                    Assert-Condition ([uint64]$nameValue -ge $imageBase) 'PE delay-load name address is below the image base.'
                    [uint64]$nameValue - $imageBase
                }
                $delayImports.Add((Read-AsciiString -Reader $reader -Offset (& $rvaToOffset $nameRva) -FileLength $length))
                if ($descriptorIndex -eq 4095) {
                    throw 'PE delay-load descriptor count exceeds the inspection limit.'
                }
            }
        }

        return [pscustomobject]@{
            Machine = $machine
            OptionalMagic = $optionalMagic
            Subsystem = $subsystem
            Imports = @($imports | Sort-Object -Unique)
            DelayImports = @($delayImports | Sort-Object -Unique)
        }
    } finally {
        $reader.Dispose()
        $stream.Dispose()
    }
}

function Find-ManifestTool {
    $fromPath = Get-Command mt.exe -ErrorAction SilentlyContinue
    if ($fromPath) {
        return $fromPath.Source
    }

    $kitsRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    if (-not (Test-Path -LiteralPath $kitsRoot -PathType Container)) {
        throw 'Could not locate the Windows SDK manifest tool (mt.exe).'
    }
    $candidate = Get-ChildItem -LiteralPath $kitsRoot -Filter mt.exe -Recurse -File |
        Where-Object { $_.FullName -match '\\x64\\mt\.exe$' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1
    if (-not $candidate) {
        throw 'Could not locate an x64 Windows SDK manifest tool (mt.exe).'
    }
    return $candidate.FullName
}

function Invoke-PreviewCommand {
    param(
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )
    $lines = @(& $Executable @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "Packaged command failed ($($Arguments -join ' ')): $($lines -join [Environment]::NewLine)"
    }
    return $lines -join "`n"
}

$artifact = [System.IO.Path]::GetFullPath($ArtifactDirectory)
$repository = [System.IO.Path]::GetFullPath($RepositoryRoot)
$fixtures = Join-Path $repository 'fixtures'
Assert-Condition (Test-Path -LiteralPath $artifact -PathType Container) 'Artifact directory does not exist.'
Assert-Condition (Test-Path -LiteralPath $repository -PathType Container) 'Repository directory does not exist.'
Assert-Condition (Test-Path -LiteralPath $fixtures -PathType Container) 'Repository fixture directory does not exist.'

$outerEntries = @(Get-ChildItem -LiteralPath $artifact -Force | Sort-Object Name)
Assert-Condition ($outerEntries.Count -eq 2) 'Artifact directory must contain exactly two entries.'
Assert-Condition (-not $outerEntries[0].PSIsContainer -and -not $outerEntries[1].PSIsContainer) 'Artifact directory must not contain directories.'
Assert-Condition ((($outerEntries[0].Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0) -and (($outerEntries[1].Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0)) 'Artifact directory must not contain reparse points.'
$outerNames = @($outerEntries | ForEach-Object Name)
Assert-Condition ($outerNames[0] -ceq 'SHA256SUMS') 'Artifact is missing the exact SHA256SUMS filename.'
Assert-Condition ($outerNames[1] -ceq 'systemdiff-windows-x86_64.zip') 'Artifact is missing the expected ZIP filename.'

$zipPath = Join-Path $artifact 'systemdiff-windows-x86_64.zip'
$checksumPath = Join-Path $artifact 'SHA256SUMS'
$checksumText = [System.IO.File]::ReadAllText($checksumPath, [System.Text.Encoding]::UTF8)
$checksumMatch = [regex]::Match(
    $checksumText,
    '\A([0-9a-f]{64})  systemdiff-windows-x86_64\.zip\n\z',
    [System.Text.RegularExpressions.RegexOptions]::CultureInvariant
)
Assert-Condition $checksumMatch.Success 'SHA256SUMS must contain one lowercase SHA-256 line with LF termination.'
$expectedZipHash = $checksumMatch.Groups[1].Value
$actualZipHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath).Hash.ToLowerInvariant()
Assert-Condition ($actualZipHash -ceq $expectedZipHash) 'Portable ZIP SHA-256 does not match SHA256SUMS.'

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
try {
    $entryNames = @($archive.Entries | ForEach-Object FullName | Sort-Object)
    $expectedEntries = @('BUILD_INFO.txt', 'LICENSE', 'QUICKSTART.md', 'systemdiff.exe', 'THIRD_PARTY_LICENSES.txt') | Sort-Object
    Assert-Condition ($entryNames.Count -eq $expectedEntries.Count) 'Portable ZIP contains an unexpected number of entries.'
    for ($index = 0; $index -lt $expectedEntries.Count; $index++) {
        Assert-Condition ($entryNames[$index] -ceq $expectedEntries[$index]) "Unexpected ZIP entry: $($entryNames[$index])"
    }
    $caseInsensitiveNames = @($entryNames | ForEach-Object { $_.ToLowerInvariant() } | Sort-Object -Unique)
    Assert-Condition ($caseInsensitiveNames.Count -eq $entryNames.Count) 'Portable ZIP contains case-insensitive duplicate paths.'
} finally {
    $archive.Dispose()
}

$verificationRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
    'SystemDiff-preview-verification-' + [Guid]::NewGuid().ToString('N')
)
$previousPath = $env:PATH
$extractDirectory = Join-Path $verificationRoot 'package'
$snapshotPath = Join-Path $verificationRoot 'snapshot.json'
$manifestPath = Join-Path $verificationRoot 'systemdiff.manifest.xml'

try {
    New-Item -ItemType Directory -Path $verificationRoot | Out-Null
    [System.IO.Compression.ZipFile]::ExtractToDirectory($zipPath, $extractDirectory)
    $executable = Join-Path $extractDirectory 'systemdiff.exe'
    Assert-Condition (Test-Path -LiteralPath $executable -PathType Leaf) 'Extracted executable is missing.'
    Assert-Condition ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $extractDirectory 'QUICKSTART.md')).Hash -ceq (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $repository 'packaging\windows\QUICKSTART.md')).Hash) 'Packaged QUICKSTART.md does not match the reviewed repository file.'
    Assert-Condition ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $extractDirectory 'LICENSE')).Hash -ceq (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $repository 'LICENSE')).Hash) 'Packaged LICENSE does not match the reviewed repository file.'
    Assert-Condition ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $extractDirectory 'THIRD_PARTY_LICENSES.txt')).Hash -ceq (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $repository 'THIRD_PARTY_LICENSES.txt')).Hash) 'Packaged third-party licenses do not match the reviewed repository file.'

    $executableHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $executable).Hash.ToLowerInvariant()
    $buildInfo = [System.IO.File]::ReadAllText((Join-Path $extractDirectory 'BUILD_INFO.txt'), [System.Text.Encoding]::UTF8)
    Assert-Condition ($buildInfo -match "(?m)^Commit: $([regex]::Escape($ExpectedCommitSha.ToLowerInvariant()))$") 'BUILD_INFO.txt does not identify the expected commit.'
    Assert-Condition ($buildInfo -match '(?m)^Target: x86_64-pc-windows-msvc$') 'BUILD_INFO.txt does not identify the expected target.'
    Assert-Condition ($buildInfo -match '(?m)^Cargo profile: release$') 'BUILD_INFO.txt does not identify the release profile.'
    Assert-Condition ($buildInfo -match '(?m)^MSVC CRT linkage: static$') 'BUILD_INFO.txt does not identify static CRT linkage.'
    Assert-Condition ($buildInfo -match "(?m)^Executable SHA-256: $executableHash$") 'BUILD_INFO.txt executable hash does not match the packaged executable.'
    Assert-Condition ($buildInfo -match '(?m)^Official release: no$') 'BUILD_INFO.txt must state that this is not an official release.'
    Assert-Condition ($buildInfo -match '(?m)^Authenticode signed: no$') 'BUILD_INFO.txt must state the expected unsigned status.'

    $pe = Read-PeMetadata -ExecutablePath $executable
    Assert-Condition ($pe.Machine -eq 0x8664) 'Packaged executable is not AMD64/x86_64.'
    Assert-Condition ($pe.OptionalMagic -eq 0x020b) 'Packaged executable is not PE32+.'
    Assert-Condition ($pe.Subsystem -eq 3) 'Packaged executable is not a Windows console application.'

    foreach ($import in @($pe.Imports + $pe.DelayImports)) {
        Assert-Condition ($import.IndexOfAny([char[]]@('/', '\')) -lt 0) "PE import name contains a path separator: $import"
    }
    Assert-Condition ($pe.DelayImports.Count -eq 0) 'Packaged executable has an unexpected delay-loaded DLL.'
    $normalizedImports = @($pe.Imports | ForEach-Object { $_.ToLowerInvariant() } | Sort-Object -Unique)
    $expectedImports = @(
        'advapi32.dll'
        'api-ms-win-core-synch-l1-2-0.dll'
        'kernel32.dll'
        'ntdll.dll'
    )
    Assert-Condition ($normalizedImports.Count -eq $expectedImports.Count) "Packaged executable import set changed: $($normalizedImports -join ', ')"
    for ($importIndex = 0; $importIndex -lt $expectedImports.Count; $importIndex++) {
        Assert-Condition ($normalizedImports[$importIndex] -ceq $expectedImports[$importIndex]) "Packaged executable import set changed: $($normalizedImports -join ', ')"
    }

    $manifestTool = Find-ManifestTool
    & $manifestTool "-inputresource:$executable;#1" "-out:$manifestPath"
    if ($LASTEXITCODE -ne 0) {
        throw "mt.exe could not extract RT_MANIFEST #1 (exit $LASTEXITCODE)."
    }
    [xml]$manifest = Get-Content -Raw -LiteralPath $manifestPath
    $namespaceManager = [System.Xml.XmlNamespaceManager]::new($manifest.NameTable)
    $namespaceManager.AddNamespace('asmv1', 'urn:schemas-microsoft-com:asm.v1')
    $namespaceManager.AddNamespace('asmv3', 'urn:schemas-microsoft-com:asm.v3')
    $executionLevels = @($manifest.SelectNodes('/asmv1:assembly/asmv3:trustInfo/asmv3:security/asmv3:requestedPrivileges/asmv3:requestedExecutionLevel', $namespaceManager))
    $allExecutionLevelNames = @($manifest.SelectNodes("//*[local-name()='requestedExecutionLevel']"))
    Assert-Condition ($executionLevels.Count -eq 1 -and $allExecutionLevelNames.Count -eq 1) 'Manifest must contain exactly one correctly namespaced requestedExecutionLevel.'
    Assert-Condition ($executionLevels[0].level -ceq 'asInvoker') 'Manifest must request asInvoker.'
    Assert-Condition ($executionLevels[0].uiAccess -ceq 'false') 'Manifest must set uiAccess=false.'
    Assert-Condition (@($manifest.SelectNodes("//*[local-name()='autoElevate']")).Count -eq 0) 'Manifest must not request autoElevate.'

    $signature = Get-AuthenticodeSignature -LiteralPath $executable
    Assert-Condition ($signature.Status -eq [System.Management.Automation.SignatureStatus]::NotSigned) 'Developer Preview executable must match its documented unsigned state.'

    if ($RemoveToolchainFromPath) {
        $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
        foreach ($tool in @('cargo.exe', 'rustc.exe', 'link.exe', 'dumpbin.exe', 'mt.exe')) {
            Assert-Condition (-not (Get-Command $tool -ErrorAction SilentlyContinue)) "Toolchain command remains on the smoke-test PATH: $tool"
        }
    }

    $help = Invoke-PreviewCommand -Executable $executable -Arguments @('--help')
    foreach ($command in @('snapshot', 'diff', 'collectors')) {
        Assert-Condition ($help.IndexOf($command, [System.StringComparison]::Ordinal) -ge 0) "Help output does not mention $command."
    }

    $collectors = Invoke-PreviewCommand -Executable $executable -Arguments @('collectors')
    Assert-Condition ($collectors.IndexOf('windows.registry.startup v1: Implemented', [System.StringComparison]::Ordinal) -ge 0) 'Collector output does not report the Registry startup Collector as implemented.'
    Assert-Condition ($collectors.IndexOf('windows.services v1: Implemented', [System.StringComparison]::Ordinal) -ge 0) 'Collector output does not report the Windows Services Collector as implemented.'

    $before = Join-Path $fixtures 'snapshots\registry-before-v1.json'
    $after = Join-Path $fixtures 'snapshots\registry-after-v1.json'
    Assert-Condition (Test-Path -LiteralPath $before -PathType Leaf) 'Registry before fixture is missing.'
    Assert-Condition (Test-Path -LiteralPath $after -PathType Leaf) 'Registry after fixture is missing.'

    $humanDiff = Invoke-PreviewCommand -Executable $executable -Arguments @('diff', $before, $after)
    Assert-Condition ($humanDiff.IndexOf('1 confirmed change', [System.StringComparison]::Ordinal) -ge 0) 'Human Diff did not report exactly one confirmed change.'
    Assert-Condition ($humanDiff.IndexOf('SystemDiffSyntheticE2E', [System.StringComparison]::Ordinal) -ge 0) 'Human Diff did not display the synthetic Registry value.'

    $technicalDiff = Invoke-PreviewCommand -Executable $executable -Arguments @('diff', '--technical', $before, $after)
    Assert-Condition ($technicalDiff.IndexOf('windows.registry.startup', [System.StringComparison]::Ordinal) -ge 0) 'Technical Diff did not expose the Collector identity.'
    Assert-Condition ($technicalDiff.IndexOf('SHA-256:', [System.StringComparison]::Ordinal) -ge 0) 'Technical Diff did not expose the evidence hash.'

    $jsonDiff = Invoke-PreviewCommand -Executable $executable -Arguments @('diff', '--json', $before, $after)
    $diffDocument = $jsonDiff | ConvertFrom-Json
    Assert-Condition ($diffDocument.document_type -ceq 'systemdiff.diff') 'JSON Diff document type is not canonical.'
    Assert-Condition ($diffDocument.schema_version -eq 1) 'JSON Diff schema version is not 1.'
    Assert-Condition (@($diffDocument.changes).Count -eq 1) 'JSON Diff did not contain exactly one change.'
    Assert-Condition ($diffDocument.changes[0].change.change -ceq 'added') 'JSON Diff did not classify the synthetic change as Added.'

    $null = Invoke-PreviewCommand -Executable $executable -Arguments @('snapshot', '-o', $snapshotPath)
    Assert-Condition (Test-Path -LiteralPath $snapshotPath -PathType Leaf) 'Packaged executable did not create a Snapshot.'
    $snapshotText = [System.IO.File]::ReadAllText($snapshotPath, [System.Text.Encoding]::UTF8)
    $snapshotDocument = $snapshotText | ConvertFrom-Json
    Assert-Condition ($snapshotDocument.document_type -ceq 'systemdiff.snapshot') 'Snapshot document type is not canonical.'
    Assert-Condition ($snapshotDocument.schema_version -eq 1) 'Snapshot schema version is not 1.'
    Assert-Condition (@($snapshotDocument.enabled_collectors).Count -eq 2) 'Snapshot does not enable exactly both implemented Collectors.'
    Assert-Condition (@($snapshotDocument.enabled_collectors | Where-Object { $_ -ceq 'windows.registry.startup' }).Count -eq 1) 'Snapshot does not enable the Registry startup Collector.'
    Assert-Condition (@($snapshotDocument.enabled_collectors | Where-Object { $_ -ceq 'windows.services' }).Count -eq 1) 'Snapshot does not enable the Windows Services Collector.'
    $registryRun = @($snapshotDocument.collectors | Where-Object { $_.id -ceq 'windows.registry.startup' })
    $servicesRun = @($snapshotDocument.collectors | Where-Object { $_.id -ceq 'windows.services' })
    Assert-Condition ($registryRun.Count -eq 1) 'Snapshot does not contain exactly one Registry startup Collector run.'
    Assert-Condition ($servicesRun.Count -eq 1) 'Snapshot does not contain exactly one Windows Services Collector run.'
    Assert-Condition ($servicesRun[0].status -ceq 'partial') 'Services Collector did not report conservative partial coverage.'
    Assert-Condition (@($servicesRun[0].coverage | Where-Object { $_.scope_id -ceq 'current_token.win32' -and $_.status -ceq 'partial' }).Count -eq 1) 'Snapshot does not report the Services current-token partial scope.'
    $null = Invoke-PreviewCommand -Executable $executable -Arguments @('diff', '--json', $snapshotPath, $snapshotPath)

    Write-Output 'Artifact-only smoke: --help passed'
    Write-Output 'Artifact-only smoke: collectors passed'
    Write-Output 'Artifact-only smoke: human Diff passed'
    Write-Output 'Artifact-only smoke: technical Diff passed'
    Write-Output 'Artifact-only smoke: JSON Diff passed'
    Write-Output 'Artifact-only smoke: read-only Snapshot passed'
    Write-Output "PE machine: AMD64 (0x$($pe.Machine.ToString('x4')))"
    Write-Output "PE imports: $($pe.Imports -join ', ')"
    Write-Output "PE delay imports: $($pe.DelayImports -join ', ')"
    Write-Output 'Manifest: asInvoker, uiAccess=false'
    Write-Output "Authenticode: $($signature.Status)"
    Write-Output "EXE SHA-256: $executableHash"
    Write-Output "EXE bytes: $((Get-Item -LiteralPath $executable).Length)"
    Write-Output "ZIP bytes: $((Get-Item -LiteralPath $zipPath).Length)"
    Write-Output "ZIP SHA-256: $actualZipHash"
    Write-Output "Artifact contents: $($outerNames -join ', ')"
    Write-Output "ZIP contents: $($entryNames -join ', ')"
    Write-Output "Build commit: $($ExpectedCommitSha.ToLowerInvariant())"
} finally {
    $env:PATH = $previousPath
    if (Test-Path -LiteralPath $verificationRoot) {
        Remove-Item -LiteralPath $verificationRoot -Recurse -Force
    }
}
