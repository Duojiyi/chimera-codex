#Requires -Version 5.1
<#
.SYNOPSIS
  Fail the build if upstream promo / ad / placeholder residues remain in production paths.

.DESCRIPTION
  Scans production sources, root READMEs, packaging scripts, and workflows.
  Excludes .git, target, dist, node_modules, and historical design docs under docs/superpowers.
  Test fixtures that must keep old domains use scripts/verify-allowlist.txt (per-hit, not whole dirs).
#>
[CmdletBinding()]
param(
    [string]$AllowlistPath = ''
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

function Get-RelativePath {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$FullPath
    )
    $rootFull = [System.IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    $fileFull = [System.IO.Path]::GetFullPath($FullPath)
    if ($fileFull.StartsWith($rootFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        $rel = $fileFull.Substring($rootFull.Length).TrimStart('\', '/')
        return ($rel -replace '\\', '/')
    }
    return ($FullPath -replace '\\', '/')
}

function Test-IsExcludedPath {
    param(
        [Parameter(Mandatory)][string]$RelPath
    )
    $norm = $RelPath -replace '\\', '/'
    $excludeDirs = @(
        '.git', 'target', 'dist', 'node_modules', '.cargo',
        'docs/superpowers', 'docs/images'
    )
    foreach ($ex in $excludeDirs) {
        if ($norm -eq $ex -or $norm.StartsWith("$ex/")) { return $true }
    }
    # Nested build / generated outputs
    if ($norm -match '(^|/)(dist|target|node_modules|gen)(/|$)') { return $true }
    if ($norm -match '(^|/)package-lock\.json$') { return $true }
    if ($norm -match '(^|/)Cargo\.lock$') { return $true }
    if ($norm -match '\.(png|jpg|jpeg|gif|webp|ico|svg|woff2?|ttf|eot|pdb|exe|dll|dylib|so)$') { return $true }
    return $false
}

function Test-IsProductionPath {
    param([Parameter(Mandatory)][string]$RelPath)
    $norm = $RelPath -replace '\\', '/'
    if ($norm -match '(^|/)tests?/') { return $false }
    if ($norm -match '(^|/)__tests__/') { return $false }
    if ($norm -match '\.(test|spec)\.(ts|tsx|js|jsx|rs)$') { return $false }
    return $true
}

function Test-ContainsExact {
    param(
        [Parameter(Mandatory)][AllowEmptyString()][string]$Haystack,
        [Parameter(Mandatory)][string]$Needle
    )
    return $Haystack.Contains($Needle)
}

function Read-Allowlist {
    param([Parameter(Mandatory)][string]$Path)

    $entries = New-Object System.Collections.Generic.List[object]
    if (-not (Test-Path -LiteralPath $Path)) {
        return $entries
    }
    $lineNo = 0
    foreach ($raw in Get-Content -LiteralPath $Path -Encoding UTF8) {
        $lineNo++
        $line = $raw.Trim()
        if ($line -eq '' -or $line.StartsWith('#')) { continue }
        # format: relative/path:pattern:reason
        $parts = $line.Split(':', 3)
        if ($parts.Count -lt 3) {
            throw "Invalid allowlist line ${lineNo}: expected path:pattern:reason"
        }
        $entries.Add([pscustomobject]@{
            Path    = ($parts[0] -replace '\\', '/')
            Pattern = $parts[1]
            Reason  = $parts[2]
            Raw     = $line
        }) | Out-Null
    }
    return $entries
}

function Test-Allowlisted {
    param(
        [Parameter(Mandatory)][string]$RelPath,
        [Parameter(Mandatory)][string]$Pattern,
        $Allowlist,
        [string]$LineText = ''
    )
    if ($null -eq $Allowlist -or $Allowlist.Count -eq 0) { return $false }
    $norm = $RelPath -replace '\\', '/'
    foreach ($entry in $Allowlist) {
        $pathOk = ($entry.Path -eq '*' -or $norm -eq $entry.Path -or $norm.EndsWith('/' + $entry.Path))
        if (-not $pathOk) { continue }
        if ($entry.Pattern -eq '*' -or $Pattern -eq $entry.Pattern -or (Test-ContainsExact -Haystack $Pattern -Needle $entry.Pattern)) {
            return $true
        }
        if ($LineText -ne '' -and (Test-ContainsExact -Haystack $LineText -Needle $entry.Pattern)) {
            return $true
        }
    }
    return $false
}

$root = Get-RepoRoot
if ([string]::IsNullOrWhiteSpace($AllowlistPath)) {
    $AllowlistPath = Join-Path $root 'scripts/verify-allowlist.txt'
}
$allowlist = Read-Allowlist -Path $AllowlistPath
$brandPath = Join-Path $root 'brand/product.toml'
$brand = Read-FlatToml -Path $brandPath
$artifactPrefix = [string]$brand['artifact_prefix']
$repository = [string]$brand['repository']
$publisher = [string]$brand['publisher']
$latestUrl = [string]$brand['latest_json_url']

$failures = New-Object System.Collections.Generic.List[string]

function Add-Failure {
    param([Parameter(Mandatory)][string]$Message)
    $script:failures.Add($Message) | Out-Null
}

# --- Version consistency ---
function Read-JsonVersion {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$KeyPath)
    $json = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    $cur = $json
    foreach ($part in $KeyPath.Split('.')) {
        $cur = $cur.$part
    }
    return [string]$cur
}

$cargoToml = Get-Content -LiteralPath (Join-Path $root 'Cargo.toml') -Raw -Encoding UTF8
if ($cargoToml -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    Add-Failure "Cargo.toml workspace.package version not found"
    $cargoVersion = ''
}
else {
    $cargoVersion = $Matches[1]
}
$pkgVersion = Read-JsonVersion -Path (Join-Path $root 'apps/codex-plus-manager/package.json') -KeyPath 'version'
$tauriVersion = Read-JsonVersion -Path (Join-Path $root 'apps/codex-plus-manager/src-tauri/tauri.conf.json') -KeyPath 'version'
if ($cargoVersion -and ($pkgVersion -ne $cargoVersion -or $tauriVersion -ne $cargoVersion)) {
    Add-Failure "Version mismatch: Cargo=$cargoVersion package.json=$pkgVersion tauri.conf.json=$tauriVersion"
}

# --- Brand placeholder scan on product.toml ---
$forbiddenBrandTokens = @('TBD', 'example owner', 'example/', 'chimera-org/chimera-codex', 'BigPizzaV3/CodexPlusPlus')
    foreach ($key in @('repository', 'latest_json_url', 'publisher', 'artifact_prefix', 'default_relay_base_url', 'website_url', 'api_key_url')) {
    $text = [string]$brand[$key]
    foreach ($token in $forbiddenBrandTokens) {
        if (Test-ContainsExact -Haystack $text -Needle $token) {
            Add-Failure "brand/product.toml ${key} contains forbidden token '$token'"
        }
    }
}
if (-not (Test-ContainsExact -Haystack $latestUrl -Needle $repository)) {
    Add-Failure "latest_json_url does not contain repository '$repository'"
}
if ($artifactPrefix -ne 'ChimeraCodex') {
    Add-Failure "artifact_prefix must be ChimeraCodex (got '$artifactPrefix')"
}

# --- File content patterns ---
$promoPatterns = @(
    [pscustomobject]@{ Id = 'Ad-List'; Pattern = 'BigPizzaV3/Ad-List'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'ScriptMarket'; Pattern = 'BigPizzaV3/CodexPlusPlusScriptMarket'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'jojocode'; Pattern = 'jojocode.com'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'chimera-org'; Pattern = 'chimera-org/chimera-codex'; ProductionOnly = $false },
    [pscustomobject]@{ Id = 'example-owner'; Pattern = 'example owner'; ProductionOnly = $false },
    [pscustomobject]@{ Id = 'sponsor-inject'; Pattern = '__CODEX_PLUS_SPONSOR_IMAGES__'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'manager-en'; Pattern = 'Codex++ Manager'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'manager-zh'; Pattern = 'Codex++ 管理工具'; ProductionOnly = $true }
)

$scanRoots = @(
    'README.md',
    'README_EN.md',
    'brand',
    'apps',
    'crates',
    'assets',
    'scripts',
    '.github'
)

$files = New-Object System.Collections.Generic.List[string]
foreach ($item in $scanRoots) {
    $full = Join-Path $root $item
    if (Test-Path -LiteralPath $full -PathType Leaf) {
        $files.Add($full) | Out-Null
    }
    elseif (Test-Path -LiteralPath $full -PathType Container) {
        Get-ChildItem -LiteralPath $full -Recurse -File -ErrorAction SilentlyContinue |
            ForEach-Object { $files.Add($_.FullName) | Out-Null }
    }
}

foreach ($file in $files) {
    $rel = Get-RelativePath -Root $root -FullPath $file
    if (Test-IsExcludedPath -RelPath $rel) { continue }

    # Skip the scanner / allowlist themselves
    if ($rel -eq 'scripts/verify-no-upstream-ads.ps1') { continue }
    if ($rel -eq 'scripts/verify-allowlist.txt') { continue }

    $ext = [System.IO.Path]::GetExtension($file).ToLowerInvariant()
    $textExts = @(
        '.rs', '.ts', '.tsx', '.js', '.jsx', '.json', '.toml', '.md', '.ps1', '.sh',
        '.yml', '.yaml', '.nsi', '.css', '.html', '.txt', '.svg'
    )
    if ($textExts -notcontains $ext -and $rel -notmatch '^README') { continue }

    $content = Get-Content -LiteralPath $file -Raw -Encoding UTF8 -ErrorAction SilentlyContinue
    if ($null -eq $content) { continue }

    $isProd = Test-IsProductionPath -RelPath $rel

    foreach ($rule in $promoPatterns) {
        if ($rule.ProductionOnly -and -not $isProd) { continue }
        if (-not (Test-ContainsExact -Haystack $content -Needle $rule.Pattern)) { continue }

        # Line-level reporting
        $lineNum = 0
        foreach ($line in ($content -split "`n")) {
            $lineNum++
            if (-not (Test-ContainsExact -Haystack $line -Needle $rule.Pattern)) { continue }
            if (Test-Allowlisted -RelPath $rel -Pattern $rule.Pattern -Allowlist $allowlist -LineText $line) {
                continue
            }
            Add-Failure ("{0}:{1}: forbidden '{2}'" -f $rel, $lineNum, $rule.Pattern)
        }
    }

    # append_builtin_sponsors still called (not merely mentioned in comments/tests asserting absence)
    if ($isProd -and $rel -match '\.rs$' -and $content -match 'append_builtin_sponsors\s*\(') {
        if (-not (Test-Allowlisted -RelPath $rel -Pattern 'append_builtin_sponsors' -Allowlist $allowlist)) {
            Add-Failure "${rel}: append_builtin_sponsors(...) call must not remain in production"
        }
    }

    # update.rs must not hardcode upstream release URL
    if ($rel -eq 'crates/codex-plus-core/src/update.rs' -and (Test-ContainsExact -Haystack $content -Needle 'BigPizzaV3/CodexPlusPlus')) {
        if (-not (Test-Allowlisted -RelPath $rel -Pattern 'BigPizzaV3/CodexPlusPlus' -Allowlist $allowlist)) {
            Add-Failure "${rel}: must not contain BigPizzaV3/CodexPlusPlus"
        }
    }
}

# --- Packaging / workflow brand + artifact prefix ---
$packagingFiles = @(
    'scripts/installer/windows/CodexPlusPlus.nsi',
    'scripts/installer/macos/package-dmg.sh',
    '.github/workflows/release-assets.yml',
    '.github/workflows/pr-build.yml'
)

foreach ($rel in $packagingFiles) {
    $full = Join-Path $root ($rel -replace '/', [System.IO.Path]::DirectorySeparatorChar)
    if (-not (Test-Path -LiteralPath $full)) { continue }
    $content = Get-Content -LiteralPath $full -Raw -Encoding UTF8

    # Artifact prefix: production release names should use ChimeraCodex, not CodexPlusPlus-
    if ($content -match 'CodexPlusPlus-\$\{?VERSION\}?|CodexPlusPlus-\$version|CodexPlusPlus-.*-windows|CodexPlusPlus-.*-macos') {
        if (-not (Test-Allowlisted -RelPath $rel -Pattern 'CodexPlusPlus-' -Allowlist $allowlist -LineText $content)) {
            Add-Failure "${rel}: artifact names still use CodexPlusPlus- prefix; expected '$artifactPrefix-'"
        }
    }

    # Publisher / display brand drift vs product.toml (NSIS Publisher)
    if ($rel -like '*.nsi' -and $content -match 'Publisher"\s+"BigPizzaV3"') {
        if (-not (Test-Allowlisted -RelPath $rel -Pattern 'Publisher" "BigPizzaV3"' -Allowlist $allowlist)) {
            Add-Failure "${rel}: Publisher BigPizzaV3 does not match brand publisher '$publisher'"
        }
    }
}

# --- Summary ---
if ($failures.Count -gt 0) {
    Write-Host "verify-no-upstream-ads: FAILED ($($failures.Count) finding(s))" -ForegroundColor Red
    foreach ($f in $failures) {
        Write-Host "  - $f" -ForegroundColor Red
    }
    exit 1
}

Write-Host "verify-no-upstream-ads: OK" -ForegroundColor Green
exit 0
