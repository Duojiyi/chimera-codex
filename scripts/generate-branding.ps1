#Requires -Version 5.1
<#
.SYNOPSIS
  Generate Rust/TS branding files from brand/product.toml.

.PARAMETER Check
  Regenerate into a temp directory and byte-compare against the working tree.
  Does not modify tracked generated files.

.PARAMETER SelfTest
  Run fail-closed macOS build-number progression tests without modifying files.
#>
[CmdletBinding()]
param(
    [switch]$Check,
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-RepoRoot {
    $scriptDir = Split-Path -Parent $PSCommandPath
    return (Resolve-Path (Join-Path $scriptDir '..')).Path
}

function Read-FlatToml {
    param([Parameter(Mandatory)][string]$Path)

    $map = [ordered]@{}
    foreach ($raw in Get-Content -LiteralPath $Path -Encoding UTF8) {
        $line = $raw.Trim()
        if ($line -eq '' -or $line.StartsWith('#')) { continue }
        if ($line -match '^\s*\[') {
            throw "Nested TOML tables are not supported in brand/product.toml: $line"
        }
        if ($line -notmatch '^\s*([A-Za-z0-9_]+)\s*=\s*(.+)\s*$') {
            throw "Unable to parse TOML line: $raw"
        }
        $key = $Matches[1]
        $value = $Matches[2].Trim()
        if ($value -match '^"(.*)"$') {
            $map[$key] = $Matches[1]
        }
        elseif ($value -match '^(true|false)$') {
            $map[$key] = [bool]::Parse($value)
        }
        elseif ($value -match '^-?\d+$') {
            $map[$key] = [int]$value
        }
        else {
            throw "Unsupported TOML value for key '$key': $value"
        }
    }
    return $map
}

function Require-Keys {
    param(
        [Parameter(Mandatory)]$Map,
        [Parameter(Mandatory)][string[]]$Keys
    )
    foreach ($key in $Keys) {
        if (-not $Map.Contains($key)) {
            throw "brand/product.toml missing required key: $key"
        }
    }
}

function Assert-NoPlaceholders {
    param([Parameter(Mandatory)]$Map)

    $forbidden = @('TBD', 'example', 'chimera-org', 'BigPizzaV3/CodexPlusPlus')
    $scanKeys = @(
        'display_silent_name', 'display_manager_name', 'publisher', 'repository',
        'latest_json_url', 'default_relay_base_url', 'default_relay_model',
        'artifact_prefix', 'website_url', 'api_key_url'
    )
    foreach ($key in $scanKeys) {
        $text = [string]$Map[$key]
        foreach ($token in $forbidden) {
            if ($text -like "*$token*") {
                throw "Placeholder or forbidden value in ${key}: contains '$token'"
            }
        }
    }

    if ($Map['repository'] -ne 'Duojiyi/chimera-codex') {
        throw "repository must be Duojiyi/chimera-codex, got: $($Map['repository'])"
    }

    $expectedLatest = "https://github.com/$($Map['repository'])/releases/latest/download/latest.json"
    if ($Map['latest_json_url'] -ne $expectedLatest) {
        throw "latest_json_url must be $expectedLatest"
    }

    if ([int]$Map['macos_build_number'] -lt 1) {
        throw 'macos_build_number must be a positive integer'
    }

    if (-not ([string]$Map['default_relay_base_url']).EndsWith('/v1')) {
        throw 'default_relay_base_url must end with /v1'
    }
}

function Get-WorkspaceCargoVersion {
    param([Parameter(Mandatory)][string]$Root)

    $cargoToml = Join-Path $Root 'Cargo.toml'
    $inWorkspacePackage = $false
    foreach ($raw in Get-Content -LiteralPath $cargoToml -Encoding UTF8) {
        $line = $raw.Trim()
        if ($line -eq '[workspace.package]') {
            $inWorkspacePackage = $true
            continue
        }
        if ($inWorkspacePackage -and $line.StartsWith('[')) {
            break
        }
        if ($inWorkspacePackage -and $line -match '^version\s*=\s*"(.*)"\s*$') {
            return $Matches[1]
        }
    }
    throw 'Unable to read [workspace.package].version from Cargo.toml'
}

function Get-JsonVersion {
    param([Parameter(Mandatory)][string]$Path)

    $json = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    if (-not $json.version) {
        throw "Missing version in $Path"
    }
    return [string]$json.version
}

function Assert-VersionSync {
    param([Parameter(Mandatory)][string]$Root)

    $cargoVersion = Get-WorkspaceCargoVersion -Root $Root
    $packageVersion = Get-JsonVersion -Path (Join-Path $Root 'apps\codex-plus-manager\package.json')
    $tauriVersion = Get-JsonVersion -Path (Join-Path $Root 'apps\codex-plus-manager\src-tauri\tauri.conf.json')

    if ($cargoVersion -ne $packageVersion) {
        throw "Version drift: Cargo.toml ($cargoVersion) != package.json ($packageVersion)"
    }
    if ($cargoVersion -ne $tauriVersion) {
        throw "Version drift: Cargo.toml ($cargoVersion) != tauri.conf.json ($tauriVersion)"
    }
    if ($cargoVersion -notmatch '^\d+\.\d+\.\d+-chimera\.\d+$') {
        throw "Cargo workspace version must match X.Y.Z-chimera.N, got: $cargoVersion"
    }
}

function Assert-TextContains {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string[]]$Expected
    )

    $text = [System.IO.File]::ReadAllText($Path)
    foreach ($value in $Expected) {
        if (-not $text.Contains($value)) {
            throw "Brand touchpoint drift in ${Path}: missing '$value'"
        }
    }
}

function Assert-TextNotContains {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string[]]$Forbidden
    )

    $text = [System.IO.File]::ReadAllText($Path)
    foreach ($value in $Forbidden) {
        if ($text.Contains($value)) {
            throw "Legacy brand touchpoint in ${Path}: found '$value'"
        }
    }
}

function Get-ActiveText {
    param([Parameter(Mandatory)][string]$Path)

    return ((Get-Content -LiteralPath $Path -Encoding UTF8 | Where-Object {
                -not $_.TrimStart().StartsWith('#')
            }) -join "`n")
}

function Assert-ActiveTextContains {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string[]]$Expected
    )

    $text = Get-ActiveText -Path $Path
    foreach ($value in $Expected) {
        if (-not $text.Contains($value)) {
            throw "Active brand touchpoint drift in ${Path}: missing '$value'"
        }
    }
}

function Assert-ActiveTextNotContains {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string[]]$Forbidden
    )

    $text = Get-ActiveText -Path $Path
    foreach ($value in $Forbidden) {
        if ($text.Contains($value)) {
            throw "Active legacy brand touchpoint in ${Path}: found '$value'"
        }
    }
}

function Assert-BrandTouchpoints {
    param(
        [Parameter(Mandatory)]$Map,
        [Parameter(Mandatory)][string]$Root
    )

    $windowsInstaller = Join-Path $Root 'scripts\installer\windows\CodexPlusPlus.nsi'
    Assert-TextContains -Path $windowsInstaller -Expected @(
        ('Name "' + [string]$Map['display_silent_name'] + '"'),
        ([string]$Map['artifact_prefix'] + '-${VERSION}-windows-x64-setup.exe'),
        ('"Publisher" "' + [string]$Map['publisher'] + '"')
    )

    $macosPackager = Join-Path $Root 'scripts\installer\macos\package-dmg.sh'
    Assert-TextContains -Path $macosPackager -Expected @(
        ([string]$Map['artifact_prefix'] + '-${VERSION}-macos-${ARCH}.dmg'),
        ('SILENT_APP_NAME="' + [string]$Map['display_silent_name'] + '"'),
        ('MANAGER_APP_NAME="' + [string]$Map['display_manager_name'] + '"')
    )

    $tauriPath = Join-Path $Root 'apps\codex-plus-manager\src-tauri\tauri.conf.json'
    $tauri = Get-Content -LiteralPath $tauriPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ([string]$tauri.productName -ne [string]$Map['display_manager_name']) {
        throw "Brand touchpoint drift in ${tauriPath}: productName"
    }

    $releaseWorkflow = Join-Path $Root '.github\workflows\release-assets.yml'
    Assert-ActiveTextContains -Path $releaseWorkflow -Expected @(
        ([string]$Map['artifact_prefix'] + '-${VERSION}-windows-x64-setup.exe'),
        ([string]$Map['artifact_prefix'] + '-${VERSION}-windows-x64.zip'),
        ([string]$Map['artifact_prefix'] + '-${VERSION}-macos-${{ matrix.arch }}.dmg'),
        ([string]$Map['artifact_prefix'] + '-${VERSION}-macos-${{ matrix.arch }}.zip'),
        ([string]$Map['artifact_prefix'] + '-*-windows-x64-setup.exe'),
        ([string]$Map['artifact_prefix'] + '-*-macos-${{ matrix.arch }}.dmg'),
        'REPO: ${{ github.repository }}'
    )
    Assert-ActiveTextNotContains -Path $releaseWorkflow -Forbidden @(
        'CodexPlusPlus-${VERSION}-',
        'CodexPlusPlus-${version}-',
        'CodexPlusPlus-$version-',
        'CodexPlusPlus-*'
    )

    $prWorkflow = Join-Path $Root '.github\workflows\pr-build.yml'
    Assert-ActiveTextContains -Path $prWorkflow -Expected @(
        ([string]$Map['artifact_prefix'] + '-$version-windows-x64-setup.exe'),
        ([string]$Map['artifact_prefix'] + '-$version-windows-x64.zip'),
        ([string]$Map['artifact_prefix'] + '-${VERSION}-macos-${{ matrix.arch }}.dmg'),
        ([string]$Map['artifact_prefix'] + '-${VERSION}-macos-${{ matrix.arch }}.zip'),
        ([string]$Map['artifact_prefix'] + '-*-windows-x64-setup.exe'),
        ([string]$Map['artifact_prefix'] + '-*-macos-${{ matrix.arch }}.dmg')
    )
    Assert-ActiveTextNotContains -Path $prWorkflow -Forbidden @(
        'CodexPlusPlus-${VERSION}-',
        'CodexPlusPlus-$version-',
        'CodexPlusPlus-*'
    )

    foreach ($readmeName in @('README.md', 'README_EN.md')) {
        $readmePath = Join-Path $Root $readmeName
        Assert-TextContains -Path $readmePath -Expected @(
            ([string]$Map['artifact_prefix'] + '-*-windows-x64-setup.exe'),
            ([string]$Map['artifact_prefix'] + '-*-macos-x64.dmg'),
            ([string]$Map['artifact_prefix'] + '-*-macos-arm64.dmg')
        )
        Assert-TextNotContains -Path $readmePath -Forbidden @(
            'CodexPlusPlus-*-windows-x64-setup.exe',
            'CodexPlusPlus-*-macos-x64.dmg',
            'CodexPlusPlus-*-macos-arm64.dmg'
        )
    }
}

function ConvertTo-ChimeraVersionParts {
    param([Parameter(Mandatory)][string]$Version)

    if ($Version -notmatch '^(?<major>0|[1-9]\d*)\.(?<minor>0|[1-9]\d*)\.(?<patch>0|[1-9]\d*)-chimera\.(?<revision>0|[1-9]\d*)$') {
        return $null
    }
    return @(
        $Matches['major'],
        $Matches['minor'],
        $Matches['patch'],
        $Matches['revision']
    )
}

function Compare-DecimalStrings {
    param(
        [Parameter(Mandatory)][string]$Left,
        [Parameter(Mandatory)][string]$Right
    )

    $normalizedLeft = $Left.TrimStart('0')
    $normalizedRight = $Right.TrimStart('0')
    if ($normalizedLeft.Length -eq 0) { $normalizedLeft = '0' }
    if ($normalizedRight.Length -eq 0) { $normalizedRight = '0' }
    if ($normalizedLeft.Length -lt $normalizedRight.Length) { return -1 }
    if ($normalizedLeft.Length -gt $normalizedRight.Length) { return 1 }
    return [Math]::Sign([string]::CompareOrdinal($normalizedLeft, $normalizedRight))
}

function ConvertFrom-ChimeraReleaseMetadata {
    param(
        [Parameter(Mandatory)][string]$Tag,
        [Parameter(Mandatory)][string]$ProductToml
    )

    if (-not $Tag.StartsWith('v')) {
        throw "Invalid Chimera Release tag: $Tag"
    }
    $version = $Tag.Substring(1)
    if ($null -eq (ConvertTo-ChimeraVersionParts -Version $version)) {
        throw "Invalid Chimera Release tag: $Tag"
    }
    $buildLines = @(
        $ProductToml -split "`n" |
            ForEach-Object { $_.Trim() } |
            Where-Object { $_ -match '^(?:macos_build_number|"macos_build_number"|''macos_build_number'')\s*=' }
    )
    if ($buildLines.Count -ne 1) {
        throw "Latest Chimera Release $Tag must contain exactly one macos_build_number"
    }
    if ($buildLines[0] -notmatch '^macos_build_number\s*=\s*(\d+)\s*$') {
        throw "Invalid macos_build_number in latest Chimera Release $Tag"
    }
    try {
        $build = [int]$Matches[1]
    }
    catch {
        throw "Invalid macos_build_number in latest Chimera Release ${Tag}: $($Matches[1])"
    }
    if ($build -lt 1) {
        throw "Invalid macos_build_number in latest Chimera Release ${Tag}: $build"
    }
    return [pscustomobject]@{
        Version = $version
        Build   = $build
    }
}

function Invoke-BrandMetadataGit {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][ValidateSet('list-tags', 'show-product')][string]$Operation,
        [AllowNull()][string]$Reference
    )

    Push-Location $Root
    try {
        if ($Operation -eq 'list-tags') {
            $output = @(git tag -l 'v*-chimera.*' --sort=-v:refname 2>$null)
        }
        else {
            $output = @(git show "${Reference}:brand/product.toml" 2>$null)
        }
        return [pscustomobject]@{
            ExitCode = $LASTEXITCODE
            Output   = $output
        }
    }
    finally {
        Pop-Location
    }
}

function Get-LatestChimeraReleaseMetadata {
    param(
        [Parameter(Mandatory)][string]$Root,
        [AllowNull()][scriptblock]$GitRunner
    )

    if ($null -eq $GitRunner) {
        $GitRunner = ${function:Invoke-BrandMetadataGit}
    }
    $tagResult = & $GitRunner $Root 'list-tags' $null
    if ($tagResult.ExitCode -ne 0) {
        throw 'Unable to enumerate Chimera Release tags'
    }
    $tags = @($tagResult.Output | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ })
    if ($tags.Count -eq 0) {
        return $null
    }
    $tag = $tags[0]
    $productResult = & $GitRunner $Root 'show-product' $tag
    if ($productResult.ExitCode -ne 0) {
        throw "Unable to read brand/product.toml from latest Chimera Release $tag"
    }
    return ConvertFrom-ChimeraReleaseMetadata `
        -Tag $tag `
        -ProductToml (@($productResult.Output) -join "`n")
}

function Test-MacosBuildNumberProgress {
    param(
        [Parameter(Mandatory)][string]$CurrentVersion,
        [Parameter(Mandatory)][int]$CurrentBuild,
        [AllowNull()][string]$PreviousVersion,
        [AllowNull()][object]$PreviousBuild
    )

    if ($CurrentBuild -lt 1) { return $false }
    if ([string]::IsNullOrWhiteSpace($PreviousVersion) -or $null -eq $PreviousBuild) {
        return $true
    }
    $currentParts = ConvertTo-ChimeraVersionParts -Version $CurrentVersion
    $previousParts = ConvertTo-ChimeraVersionParts -Version $PreviousVersion
    if ($null -eq $currentParts -or $null -eq $previousParts) {
        return $false
    }
    for ($i = 0; $i -lt $currentParts.Count; $i++) {
        $comparison = Compare-DecimalStrings -Left $currentParts[$i] -Right $previousParts[$i]
        if ($comparison -lt 0) { return $false }
        if ($comparison -gt 0) { return $CurrentBuild -gt [int]$PreviousBuild }
    }
    if ($CurrentVersion -eq $PreviousVersion) {
        return $CurrentBuild -eq [int]$PreviousBuild
    }
    return $false
}

function Assert-MacosBuildNumberProgress {
    param(
        [Parameter(Mandatory)]$Map,
        [Parameter(Mandatory)][string]$Root
    )

    $current = [int]$Map['macos_build_number']
    if ($current -lt 1) {
        throw 'macos_build_number must be a positive integer'
    }

    $currentVersion = Get-WorkspaceCargoVersion -Root $Root
    $previous = Get-LatestChimeraReleaseMetadata -Root $Root
    if ($null -eq $previous) { return }
    if (-not (Test-MacosBuildNumberProgress `
            -CurrentVersion $currentVersion `
            -CurrentBuild $current `
            -PreviousVersion $previous.Version `
            -PreviousBuild $previous.Build)) {
        if ($currentVersion -eq $previous.Version) {
            throw "macos_build_number ($current) must equal released $($previous.Version) value ($($previous.Build))"
        }
        throw "macos_build_number ($current) must be greater than previous release $($previous.Version) value ($($previous.Build))"
    }
}

function Escape-RustString([string]$Value) {
    return ($Value -replace '\\', '\\' -replace '"', '\"')
}

function Escape-TsString([string]$Value) {
    return ($Value -replace '\\', '\\' -replace "'", "\'")
}

function New-RustBranding {
    param([Parameter(Mandatory)]$Map)

    $lines = @(
        '// @generated by scripts/generate-branding.ps1 — DO NOT EDIT BY HAND'
        '#![allow(dead_code)]'
        ''
        "pub const DISPLAY_SILENT_NAME: &str = `"$(Escape-RustString $Map['display_silent_name'])`";"
        "pub const DISPLAY_MANAGER_NAME: &str = `"$(Escape-RustString $Map['display_manager_name'])`";"
        "pub const PUBLISHER: &str = `"$(Escape-RustString $Map['publisher'])`";"
        "pub const REPOSITORY: &str = `"$(Escape-RustString $Map['repository'])`";"
        'pub const LATEST_JSON_URL: &str ='
        "    `"$(Escape-RustString $Map['latest_json_url'])`";"
        "pub const DEFAULT_RELAY_BASE_URL: &str = `"$(Escape-RustString $Map['default_relay_base_url'])`";"
        "pub const DEFAULT_RELAY_MODEL: &str = `"$(Escape-RustString $Map['default_relay_model'])`";"
        "pub const ARTIFACT_PREFIX: &str = `"$(Escape-RustString $Map['artifact_prefix'])`";"
        "pub const MACOS_BUILD_NUMBER: u32 = $([int]$Map['macos_build_number']);"
        "pub const WEBSITE_URL: &str = `"$(Escape-RustString $Map['website_url'])`";"
        "pub const API_KEY_URL: &str = `"$(Escape-RustString $Map['api_key_url'])`";"
        ''
    )
    return ($lines -join "`n")
}

function New-TsBranding {
    param([Parameter(Mandatory)]$Map)

    $lines = @(
        '// @generated by scripts/generate-branding.ps1 — DO NOT EDIT BY HAND'
        ''
        "export const DISPLAY_SILENT_NAME = '$(Escape-TsString $Map['display_silent_name'])';"
        "export const DISPLAY_MANAGER_NAME = '$(Escape-TsString $Map['display_manager_name'])';"
        "export const PUBLISHER = '$(Escape-TsString $Map['publisher'])';"
        "export const REPOSITORY = '$(Escape-TsString $Map['repository'])';"
        "export const LATEST_JSON_URL = '$(Escape-TsString $Map['latest_json_url'])';"
        "export const DEFAULT_RELAY_BASE_URL = '$(Escape-TsString $Map['default_relay_base_url'])';"
        "export const DEFAULT_RELAY_MODEL = '$(Escape-TsString $Map['default_relay_model'])';"
        "export const ARTIFACT_PREFIX = '$(Escape-TsString $Map['artifact_prefix'])';"
        "export const MACOS_BUILD_NUMBER = $([int]$Map['macos_build_number']);"
        "export const WEBSITE_URL = '$(Escape-TsString $Map['website_url'])';"
        "export const API_KEY_URL = '$(Escape-TsString $Map['api_key_url'])';"
        ''
    )
    return ($lines -join "`n")
}

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Content
    )
    $dir = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $utf8 = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($Path, $Content, $utf8)
}

function Compare-FilesExact {
    param(
        [Parameter(Mandatory)][string]$ExpectedPath,
        [Parameter(Mandatory)][string]$ActualPath
    )
    if (-not (Test-Path -LiteralPath $ActualPath)) {
        throw "Missing generated file in working tree: $ActualPath"
    }
    # Normalize newlines so Windows autocrlf checkouts do not false-fail -Check.
    $expected = ([System.IO.File]::ReadAllText($ExpectedPath) -replace "`r`n", "`n" -replace "`r", "`n")
    $actual = ([System.IO.File]::ReadAllText($ActualPath) -replace "`r`n", "`n" -replace "`r", "`n")
    if ($expected -cne $actual) {
        throw "Generated drift: $ActualPath"
    }
}

if ($SelfTest) {
    $cases = @(
        [pscustomobject]@{ Name = 'released baseline'; CurrentVersion = '1.2.35-chimera.4'; CurrentBuild = 6; PreviousVersion = '1.2.35-chimera.4'; PreviousBuild = 6; Expected = $true },
        [pscustomobject]@{ Name = 'new version increments build'; CurrentVersion = '1.2.36-chimera.1'; CurrentBuild = 7; PreviousVersion = '1.2.35-chimera.4'; PreviousBuild = 6; Expected = $true },
        [pscustomobject]@{ Name = 'new version reuses build'; CurrentVersion = '1.2.36-chimera.1'; CurrentBuild = 6; PreviousVersion = '1.2.35-chimera.4'; PreviousBuild = 6; Expected = $false },
        [pscustomobject]@{ Name = 'released version changes build'; CurrentVersion = '1.2.35-chimera.4'; CurrentBuild = 7; PreviousVersion = '1.2.35-chimera.4'; PreviousBuild = 6; Expected = $false },
        [pscustomobject]@{ Name = 'released version regresses build'; CurrentVersion = '1.2.35-chimera.4'; CurrentBuild = 5; PreviousVersion = '1.2.35-chimera.4'; PreviousBuild = 6; Expected = $false },
        [pscustomobject]@{ Name = 'older upstream version with higher build'; CurrentVersion = '1.2.34-chimera.99'; CurrentBuild = 7; PreviousVersion = '1.2.35-chimera.4'; PreviousBuild = 6; Expected = $false },
        [pscustomobject]@{ Name = 'older Chimera revision with higher build'; CurrentVersion = '1.2.35-chimera.3'; CurrentBuild = 7; PreviousVersion = '1.2.35-chimera.4'; PreviousBuild = 6; Expected = $false },
        [pscustomobject]@{ Name = 'invalid current version'; CurrentVersion = 'not-a-version'; CurrentBuild = 7; PreviousVersion = '1.2.35-chimera.4'; PreviousBuild = 6; Expected = $false },
        [pscustomobject]@{ Name = 'first release positive build'; CurrentVersion = '1.0.0-chimera.1'; CurrentBuild = 1; PreviousVersion = $null; PreviousBuild = $null; Expected = $true },
        [pscustomobject]@{ Name = 'non-positive build'; CurrentVersion = '1.0.0-chimera.1'; CurrentBuild = 0; PreviousVersion = $null; PreviousBuild = $null; Expected = $false }
    )
    foreach ($case in $cases) {
        $actual = Test-MacosBuildNumberProgress `
            -CurrentVersion $case.CurrentVersion `
            -CurrentBuild $case.CurrentBuild `
            -PreviousVersion $case.PreviousVersion `
            -PreviousBuild $case.PreviousBuild
        if ($actual -ne $case.Expected) {
            throw "macOS build-number self-test '$($case.Name)' expected $($case.Expected), got $actual"
        }
    }

    $metadata = ConvertFrom-ChimeraReleaseMetadata `
        -Tag 'v1.2.35-chimera.4' `
        -ProductToml "display_silent_name = `"Chimera++`"`nmacos_build_number = 6`n"
    if ($metadata.Version -ne '1.2.35-chimera.4' -or $metadata.Build -ne 6) {
        throw 'macOS release metadata self-test failed to parse a valid latest tag'
    }
    foreach ($invalid in @(
            [pscustomobject]@{ Name = 'invalid tag'; Tag = 'v1.2.35'; ProductToml = 'macos_build_number = 6' },
            [pscustomobject]@{ Name = 'missing build'; Tag = 'v1.2.35-chimera.4'; ProductToml = 'display_silent_name = "Chimera++"' },
            [pscustomobject]@{ Name = 'duplicate build'; Tag = 'v1.2.35-chimera.4'; ProductToml = "macos_build_number = 6`nmacos_build_number = 7" },
            [pscustomobject]@{ Name = 'mixed valid and invalid duplicate build'; Tag = 'v1.2.35-chimera.4'; ProductToml = "macos_build_number = 6`nmacos_build_number = `"invalid`"" }
        )) {
        $rejected = $false
        try {
            ConvertFrom-ChimeraReleaseMetadata -Tag $invalid.Tag -ProductToml $invalid.ProductToml | Out-Null
        }
        catch {
            $rejected = $true
        }
        if (-not $rejected) {
            throw "macOS release metadata self-test '$($invalid.Name)' must fail closed"
        }
    }

    $gitCalls = [System.Collections.Generic.List[string]]::new()
    $validGitRunner = {
        param($Root, $Operation, $Reference)
        $gitCalls.Add("${Operation}|${Reference}")
        if ($Operation -eq 'list-tags') {
            return [pscustomobject]@{ ExitCode = 0; Output = @('v1.2.35-chimera.4', 'v1.2.35-chimera.3') }
        }
        return [pscustomobject]@{ ExitCode = 0; Output = @('macos_build_number = 6') }
    }.GetNewClosure()
    $discovered = Get-LatestChimeraReleaseMetadata -Root '.' -GitRunner $validGitRunner
    if ($discovered.Version -ne '1.2.35-chimera.4' -or $discovered.Build -ne 6) {
        throw 'latest Chimera Release discovery self-test returned incorrect metadata'
    }
    if (($gitCalls -join ',') -ne 'list-tags|,show-product|v1.2.35-chimera.4') {
        throw "latest Chimera Release discovery must read only the newest tag, got: $($gitCalls -join ',')"
    }

    foreach ($failure in @('list-tags', 'show-product')) {
        $failingGitRunner = {
            param($Root, $Operation, $Reference)
            if ($Operation -eq $failure) {
                return [pscustomobject]@{ ExitCode = 1; Output = @() }
            }
            if ($Operation -eq 'list-tags') {
                return [pscustomobject]@{ ExitCode = 0; Output = @('v1.2.35-chimera.4', 'v1.2.35-chimera.3') }
            }
            return [pscustomobject]@{ ExitCode = 0; Output = @('macos_build_number = 6') }
        }.GetNewClosure()
        $rejected = $false
        try {
            Get-LatestChimeraReleaseMetadata -Root '.' -GitRunner $failingGitRunner | Out-Null
        }
        catch {
            $rejected = $true
        }
        if (-not $rejected) {
            throw "latest Chimera Release discovery '$failure' failure must fail closed"
        }
    }
    Write-Host 'generate-branding self-test: PASS'
    exit 0
}

$root = Get-RepoRoot
$tomlPath = Join-Path $root 'brand\product.toml'
if (-not (Test-Path -LiteralPath $tomlPath)) {
    throw "Missing $tomlPath"
}

$map = Read-FlatToml -Path $tomlPath
Require-Keys -Map $map -Keys @(
    'display_silent_name', 'display_manager_name', 'publisher', 'repository',
    'latest_json_url', 'default_relay_base_url', 'default_relay_model',
    'artifact_prefix', 'macos_build_number', 'website_url', 'api_key_url'
)
Assert-NoPlaceholders -Map $map
Assert-VersionSync -Root $root
Assert-BrandTouchpoints -Map $map -Root $root
Assert-MacosBuildNumberProgress -Map $map -Root $root

$rustContent = New-RustBranding -Map $map
$tsContent = New-TsBranding -Map $map

$rustRel = 'crates\codex-plus-core\src\branding.rs'
$tsRel = 'apps\codex-plus-manager\src\branding.generated.ts'
$rustPath = Join-Path $root $rustRel
$tsPath = Join-Path $root $tsRel

if ($Check) {
    $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("chimera-branding-check-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
    try {
        $tempRust = Join-Path $tempRoot 'branding.rs'
        $tempTs = Join-Path $tempRoot 'branding.generated.ts'
        Write-Utf8NoBom -Path $tempRust -Content $rustContent
        Write-Utf8NoBom -Path $tempTs -Content $tsContent
        Compare-FilesExact -ExpectedPath $tempRust -ActualPath $rustPath
        Compare-FilesExact -ExpectedPath $tempTs -ActualPath $tsPath
        Write-Host 'generate-branding -Check: PASS'
    }
    finally {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
else {
    Write-Utf8NoBom -Path $rustPath -Content $rustContent
    Write-Utf8NoBom -Path $tsPath -Content $tsContent
    Write-Host "Wrote $rustRel"
    Write-Host "Wrote $tsRel"
}
