#Requires -Version 5.1
<#
.SYNOPSIS
  Fail the build if upstream promo / ad / placeholder residues remain in production paths.

.DESCRIPTION
  Scans production sources, root READMEs, packaging scripts, and workflows.
  Excludes .git, target, dist, node_modules, and historical design docs under docs/superpowers.
  Test fixtures that must keep old domains use scripts/verify-allowlist.txt.
  Every exception binds an exact root-relative path, scanner pattern, and complete trimmed line.
#>
[CmdletBinding()]
param(
    [string]$AllowlistPath = '',
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

function Get-AllowedDocsImagePaths {
    return @(
        'backend-status-indicator.png',
        'codex-plus-plus.ico',
        'codex-plus-plus.png',
        'issue-1312-product-design-working.png',
        'macos-damaged-warning.png',
        'model-suffix-flow.png',
        'pain-no-delete-button.png',
        'pain-plugin-disabled.png',
        'service-tier-composer-badge.png',
        'service-tier-settings.png',
        'settings-panel.png',
        'solution-plugin-and-delete.png'
    )
}

function Get-UnapprovedDocsImagePaths {
    param([string[]]$RelativePaths = @())

    $allowed = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($path in @(Get-AllowedDocsImagePaths)) {
        if (-not $allowed.Add($path)) {
            throw "Duplicate docs/images allowlist entry: $path"
        }
    }

    $unapproved = New-Object System.Collections.Generic.List[string]
    foreach ($path in $RelativePaths) {
        $normalized = ([string]$path) -replace '\\', '/'
        if (-not $allowed.Contains($normalized)) {
            $unapproved.Add($normalized) | Out-Null
        }
    }
    return $unapproved
}

function Get-AllowedAssetImagePaths {
    return @(
        'codex-plus-plus.ico',
        'codex-plus-plus.png'
    )
}

function Get-UnapprovedAssetImagePaths {
    param([string[]]$RelativePaths = @())

    $allowed = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($path in @(Get-AllowedAssetImagePaths)) {
        if (-not $allowed.Add($path)) {
            throw "Duplicate assets/images allowlist entry: $path"
        }
    }

    $unapproved = New-Object System.Collections.Generic.List[string]
    foreach ($path in $RelativePaths) {
        $normalized = ([string]$path) -replace '\\', '/'
        if (-not $allowed.Contains($normalized)) {
            $unapproved.Add($normalized) | Out-Null
        }
    }
    return $unapproved
}

function Test-ContainsExact {
    param(
        [Parameter(Mandatory)][AllowEmptyString()][string]$Haystack,
        [Parameter(Mandatory)][string]$Needle
    )
    return $Haystack.Contains($Needle)
}

function Convert-AllowlistLine {
    param(
        [Parameter(Mandatory)][string]$RawLine,
        [Parameter(Mandatory)][string]$AllowlistPath,
        [Parameter(Mandatory)][int]$LineNo
    )

    try {
        $parsed = $RawLine | ConvertFrom-Json -ErrorAction Stop
    }
    catch {
        throw "${AllowlistPath}:${LineNo}: expected one JSON object per line ($($_.Exception.Message))"
    }
    if ($null -eq $parsed -or $parsed -is [System.Array]) {
        throw "${AllowlistPath}:${LineNo}: allowlist entry must be one JSON object"
    }

    $allowedFields = @('path', 'pattern', 'lineNumber', 'line', 'reason')
    foreach ($property in $parsed.PSObject.Properties) {
        if ($allowedFields -cnotcontains $property.Name) {
            throw "${AllowlistPath}:${LineNo}: unknown field '$($property.Name)'"
        }
    }
    foreach ($field in @('path', 'pattern', 'line', 'reason')) {
        $property = $parsed.PSObject.Properties[$field]
        if ($null -eq $property -or -not ($property.Value -is [string])) {
            throw "${AllowlistPath}:${LineNo}: '$field' must be a string"
        }
        $value = [string]$property.Value
        if ([string]::IsNullOrWhiteSpace($value) -or $value -cne $value.Trim()) {
            throw "${AllowlistPath}:${LineNo}: '$field' must be a non-empty trimmed string"
        }
        if ($value.Contains("`r") -or $value.Contains("`n")) {
            throw "${AllowlistPath}:${LineNo}: '$field' must not contain CR/LF"
        }
    }

    $lineNumberProperty = $parsed.PSObject.Properties['lineNumber']
    if ($null -eq $lineNumberProperty -or
        -not (($lineNumberProperty.Value -is [int]) -or ($lineNumberProperty.Value -is [long]))) {
        throw "${AllowlistPath}:${LineNo}: 'lineNumber' must be an integer"
    }
    $entryLineNumber = [long]$lineNumberProperty.Value
    if ($entryLineNumber -lt 1 -or $entryLineNumber -gt [int]::MaxValue) {
        throw "${AllowlistPath}:${LineNo}: 'lineNumber' must be between 1 and $([int]::MaxValue)"
    }

    $entryPath = [string]$parsed.path
    if ($entryPath.Contains('\') -or
        $entryPath -match '(^/|^[A-Za-z]:|(^|/)\.\.?(/|$)|[*?\[])') {
        throw "${AllowlistPath}:${LineNo}: path must be an exact root-relative file path using '/'"
    }

    return [pscustomobject]@{
        Path       = $entryPath
        Pattern    = [string]$parsed.pattern
        LineNumber = [int]$entryLineNumber
        Line       = [string]$parsed.line
        Reason     = [string]$parsed.reason
        SourceLine = $LineNo
        Used       = $false
        Raw        = $RawLine
    }
}

function Read-Allowlist {
    param([Parameter(Mandatory)][string]$Path)

    $entries = New-Object System.Collections.Generic.List[object]
    $seen = New-Object System.Collections.Generic.HashSet[string]
    if (-not (Test-Path -LiteralPath $Path)) {
        return $entries
    }
    $lineNo = 0
    foreach ($raw in Get-Content -LiteralPath $Path -Encoding UTF8) {
        $lineNo++
        $line = $raw.Trim()
        if ($line -eq '' -or $line.StartsWith('#')) { continue }
        $entry = Convert-AllowlistLine -RawLine $line -AllowlistPath $Path -LineNo $lineNo
        $identity = "$($entry.Path)`0$($entry.Pattern)`0$($entry.LineNumber)`0$($entry.Line)"
        if (-not $seen.Add($identity)) {
            throw "${Path}:${lineNo}: duplicate exact exception"
        }
        $entries.Add($entry) | Out-Null
    }
    return $entries
}

function Test-Allowlisted {
    param(
        [Parameter(Mandatory)][string]$RelPath,
        [Parameter(Mandatory)][string]$Pattern,
        $Allowlist,
        [Parameter(Mandatory)][int]$LineNumber,
        [string]$LineText = ''
    )
    if ($null -eq $Allowlist -or $Allowlist.Count -eq 0) { return $false }
    $norm = $RelPath -replace '\\', '/'
    if ([string]::IsNullOrWhiteSpace($LineText)) { return $false }
    $candidateLine = $LineText.Trim()
    foreach ($entry in $Allowlist) {
        if ([bool]$entry.Used) { continue }
        if (-not [string]::Equals($norm, [string]$entry.Path, [System.StringComparison]::Ordinal)) {
            continue
        }
        if (-not [string]::Equals($Pattern, [string]$entry.Pattern, [System.StringComparison]::Ordinal)) {
            continue
        }
        if ($LineNumber -ne [int]$entry.LineNumber) { continue }
        if ([string]::Equals($candidateLine, [string]$entry.Line, [System.StringComparison]::Ordinal)) {
            $entry.Used = $true
            return $true
        }
    }
    return $false
}

function Assert-Throws {
    param(
        [Parameter(Mandatory)][scriptblock]$Action,
        [Parameter(Mandatory)][string]$Message
    )
    try {
        & $Action
    }
    catch {
        return
    }
    throw $Message
}

function Assert-AllowlistMatcherContract {
    $entries = @(
        [pscustomobject]@{
            Path = 'README.md'
            Pattern = 'legacy.example'
            LineNumber = 42
            Line = 'Legacy migration source: legacy.example'
            Reason = 'self-test'
            Used = $false
        }
    )
    if (-not (Test-Allowlisted -RelPath 'README.md' -Pattern 'legacy.example' -Allowlist $entries -LineNumber 42 -LineText 'Legacy migration source: legacy.example')) {
        throw 'Allowlist self-test failed: exact path/pattern/line must match'
    }
    if (Test-Allowlisted -RelPath 'README.md' -Pattern 'legacy.example' -Allowlist $entries -LineNumber 42 -LineText 'Legacy migration source: legacy.example') {
        throw 'Allowlist self-test failed: one exception matched more than once'
    }
    $entries[0].Used = $false
    if (Test-Allowlisted -RelPath 'nested/README.md' -Pattern 'legacy.example' -Allowlist $entries -LineNumber 42 -LineText 'Legacy migration source: legacy.example') {
        throw 'Allowlist self-test failed: nested same-name path bypassed exact matching'
    }
    if (Test-Allowlisted -RelPath 'README.md' -Pattern 'legacy.example' -Allowlist $entries -LineNumber 42 -LineText 'Unexpected promotion: legacy.example') {
        throw 'Allowlist self-test failed: wrong line context bypassed exact matching'
    }
    if (Test-Allowlisted -RelPath 'README.md' -Pattern 'legacy.example' -Allowlist $entries -LineNumber 43 -LineText 'Legacy migration source: legacy.example') {
        throw 'Allowlist self-test failed: relocated line bypassed exact matching'
    }
    if (Test-Allowlisted -RelPath 'README.md' -Pattern 'prefix legacy.example suffix' -Allowlist $entries -LineNumber 42 -LineText 'Legacy migration source: legacy.example') {
        throw 'Allowlist self-test failed: substring pattern bypassed exact matching'
    }

    $validJson = '{"path":"README.md","pattern":"legacy.example","lineNumber":42,"line":"Legacy migration source: legacy.example","reason":"self-test"}'
    $parsed = Convert-AllowlistLine -RawLine $validJson -AllowlistPath 'self-test.jsonl' -LineNo 1
    if ($parsed.Path -ne 'README.md' -or $parsed.LineNumber -ne 42) {
        throw 'Allowlist self-test failed: valid strict JSON entry did not parse'
    }
    Assert-Throws -Message 'Allowlist self-test failed: legacy format was accepted' -Action {
        Convert-AllowlistLine -RawLine 'README.md:legacy.example:old format' -AllowlistPath 'self-test.jsonl' -LineNo 2
    }
    Assert-Throws -Message 'Allowlist self-test failed: non-string field was accepted' -Action {
        Convert-AllowlistLine -RawLine '{"path":"README.md","pattern":7,"lineNumber":42,"line":"x","reason":"x"}' -AllowlistPath 'self-test.jsonl' -LineNo 3
    }
    Assert-Throws -Message 'Allowlist self-test failed: multiline exception was accepted' -Action {
        Convert-AllowlistLine -RawLine '{"path":"README.md","pattern":"legacy.example","lineNumber":42,"line":"first\nsecond","reason":"x"}' -AllowlistPath 'self-test.jsonl' -LineNo 4
    }
    Assert-Throws -Message 'Allowlist self-test failed: wildcard path was accepted' -Action {
        Convert-AllowlistLine -RawLine '{"path":"*/README.md","pattern":"legacy.example","lineNumber":42,"line":"x","reason":"x"}' -AllowlistPath 'self-test.jsonl' -LineNo 5
    }
}

function Assert-DocsImageGateContract {
    $legalProductImages = @(Get-AllowedDocsImagePaths)
    $unexpectedLegal = @(Get-UnapprovedDocsImagePaths -RelativePaths $legalProductImages)
    if ($unexpectedLegal.Count -ne 0) {
        throw "docs/images self-test failed: legal product images were rejected: $($unexpectedLegal -join ', ')"
    }

    $renamedPromotionFixtures = @(
        'renamed-sponsor.png',
        'renamed-community-qr.jpg',
        'renamed-promo-banner.webp',
        'nested/renamed-promo.gif',
        'Settings-panel.png'
    )
    $rejected = @(Get-UnapprovedDocsImagePaths -RelativePaths $renamedPromotionFixtures)
    if ($rejected.Count -ne $renamedPromotionFixtures.Count) {
        throw 'docs/images self-test failed: a new, renamed, nested, or case-changed image bypassed the exact allowlist'
    }
}

function Assert-AssetImageGateContract {
    $legalProductImages = @(Get-AllowedAssetImagePaths)
    $unexpectedLegal = @(Get-UnapprovedAssetImagePaths -RelativePaths $legalProductImages)
    if ($unexpectedLegal.Count -ne 0) {
        throw "assets/images self-test failed: legal product images were rejected: $($unexpectedLegal -join ', ')"
    }

    $promotionFixtures = @(
        'renamed-sponsor.jpg',
        'community-qr.png',
        'nested/payment-code.webp'
    )
    $rejected = @(Get-UnapprovedAssetImagePaths -RelativePaths $promotionFixtures)
    if ($rejected.Count -ne $promotionFixtures.Count) {
        throw 'assets/images self-test failed: an unapproved image bypassed the exact allowlist'
    }
}

function Get-CustomerSurfacePatterns {
    return @(
        [pscustomobject]@{ Id = 'customer-github'; Pattern = 'github'; ProductionOnly = $true; CustomerUiOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'customer-repository-identifier'; Pattern = 'REPOSITORY'; ProductionOnly = $true; CustomerConsumerOnly = $true },
        [pscustomobject]@{ Id = 'customer-latest-url-identifier'; Pattern = 'LATEST_JSON_URL'; ProductionOnly = $true; CustomerConsumerOnly = $true },
        [pscustomobject]@{ Id = 'customer-dynamic-homepage'; Pattern = 'script.homepage'; ProductionOnly = $true; CustomerConsumerOnly = $true },
        [pscustomobject]@{ Id = 'recommendation-command'; Pattern = 'pub async fn load_ads()'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'recommendation-route'; Pattern = '"/ads" => ctx.runtime.ads().await'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'recommendation-module'; Pattern = 'pub mod ads;'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'recommendation-payload'; Pattern = 'pub struct AdsPayload'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'ads-enabled-setting'; Pattern = 'ads_enabled'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'ads-enabled-constant'; Pattern = 'ADS_ENABLED'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'ads-list-constant'; Pattern = 'DEFAULT_AD_LIST_URLS'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'sponsor-runtime'; Pattern = 'matches!(ad_type, Some("sponsor" | "normal"))'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'github-ui-update'; Pattern = '"GitHub Release 更新"'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'github-ui-check'; Pattern = '"GitHub Release 检查"'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'github-ui-manifest'; Pattern = '"从 GitHub 静态清单加载"'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'github-ui-unchecked'; Pattern = '"尚未检查 GitHub Release'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'github-ui-fetching'; Pattern = '"正在获取 GitHub Release 信息'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'github-ui-summary'; Pattern = '"版本信息、项目链接、GitHub Release'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'github-ui-project-home'; Pattern = '"打开项目主页"'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'github-ui-about'; Pattern = '"打开关于"'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'github-ui-report'; Pattern = '"反馈问题"'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'community-discord'; Pattern = 'discord.gg/'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'community-telegram'; Pattern = 't.me/'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'community-zh'; Pattern = '交流群'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'community-qq'; Pattern = 'QQ群'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'community-wechat'; Pattern = '微信群'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'repository-global'; Pattern = '__CODEX_PLUS_REPOSITORY__'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'issue-hook'; Pattern = 'data-codex-plus-issue'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'dynamic-homepage'; Pattern = 'actions.openExternalUrl(script.homepage)'; ProductionOnly = $true },
        [pscustomobject]@{ Id = 'chatgpt-icon-en'; Pattern = 'ChatGPT icon'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'openai-icon-en'; Pattern = 'OpenAI icon'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'chatgpt-icon-zh'; Pattern = 'ChatGPT 图标'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'openai-icon-zh'; Pattern = 'OpenAI 图标'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'store-chatgpt-icon'; Pattern = 'Microsoft Store ChatGPT icon'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'store-chatgpt-icon-zh'; Pattern = '微软应用商店 ChatGPT 图标'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'chatgpt-logo-en'; Pattern = 'ChatGPT logo'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'openai-logo-en'; Pattern = 'OpenAI logo'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'chatgpt-badge-zh'; Pattern = 'ChatGPT 徽标'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'openai-badge-zh'; Pattern = 'OpenAI 徽标'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'chatgpt-mark-zh'; Pattern = 'ChatGPT 标志'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'openai-mark-zh'; Pattern = 'OpenAI 标志'; ProductionOnly = $true; CaseInsensitive = $true },
        [pscustomobject]@{ Id = 'store-chatgpt-reference'; Pattern = 'Microsoft Store ChatGPT'; ProductionOnly = $true; CaseInsensitive = $true }
    )
}

function Test-IsCustomerUiPath {
    param([Parameter(Mandatory)][string]$RelPath)

    return $RelPath.Equals('apps/codex-plus-manager/index.html', [StringComparison]::OrdinalIgnoreCase) -or
        $RelPath.StartsWith('apps/codex-plus-manager/src/', [StringComparison]::OrdinalIgnoreCase) -or
        $RelPath.StartsWith('assets/inject/', [StringComparison]::OrdinalIgnoreCase)
}

function Test-IsCustomerUiConsumerPath {
    param([Parameter(Mandatory)][string]$RelPath)

    return (Test-IsCustomerUiPath -RelPath $RelPath) -and
        -not $RelPath.Equals('apps/codex-plus-manager/src/branding.generated.ts', [StringComparison]::OrdinalIgnoreCase)
}

function Test-RuleContains {
    param(
        [AllowEmptyString()][string]$Haystack,
        [Parameter(Mandatory)]$Rule
    )

    if ($Rule.PSObject.Properties.Name -contains 'CaseInsensitive' -and $Rule.CaseInsensitive) {
        return $Haystack.IndexOf([string]$Rule.Pattern, [StringComparison]::OrdinalIgnoreCase) -ge 0
    }
    return Test-ContainsExact -Haystack $Haystack -Needle ([string]$Rule.Pattern)
}

function Assert-CustomerSurfaceGateContract {
    $patterns = @(Get-CustomerSurfacePatterns)
    $fixtures = @(
        [pscustomobject]@{ Name = 'recommendation'; Text = 'pub async fn load_ads()' },
        [pscustomobject]@{ Name = 'sponsor'; Text = 'matches!(ad_type, Some("sponsor" | "normal"))' },
        [pscustomobject]@{ Name = 'community'; Text = '加入客户交流群' },
        [pscustomobject]@{ Name = 'GitHub UI'; Text = '"GitHub Release 更新"' },
        [pscustomobject]@{ Name = 'generic GitHub URL'; Text = 'HTTPS://GITHUB.COM/example/customer-link' },
        [pscustomobject]@{ Name = 'third-party icon'; Text = 'Microsoft Store ChatGPT icon' },
        [pscustomobject]@{ Name = 'third-party logo'; Text = 'chatgpt logo' },
        [pscustomobject]@{ Name = 'third-party badge'; Text = 'ChatGPT 徽标' },
        [pscustomobject]@{ Name = 'generated URL consumer'; Text = 'LATEST_JSON_URL' },
        [pscustomobject]@{ Name = 'dynamic homepage sink'; Text = 'href={script.homepage}' }
    )
    if (-not (Test-IsCustomerUiPath -RelPath 'apps/codex-plus-manager/index.html')) {
        throw 'customer surface self-test failed: manager index.html is not classified as customer UI'
    }
    foreach ($fixture in $fixtures) {
        $matched = @($patterns | Where-Object { Test-RuleContains -Haystack $fixture.Text -Rule $_ })
        if ($matched.Count -eq 0) {
            throw "customer surface self-test failed: $($fixture.Name) fixture bypassed all production patterns"
        }
    }
}

Assert-AllowlistMatcherContract
Assert-DocsImageGateContract
Assert-AssetImageGateContract
Assert-CustomerSurfaceGateContract
if ($SelfTest) {
    Write-Host 'verify-no-upstream-ads allowlist self-test: OK' -ForegroundColor Green
    Write-Host 'docs/images fail-closed fixtures: OK' -ForegroundColor Green
    Write-Host 'assets/images fail-closed fixtures: OK' -ForegroundColor Green
    Write-Host 'customer surface fail-closed fixtures: OK' -ForegroundColor Green
    exit 0
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
if ($artifactPrefix -ne 'ChimeraPlusPlus') {
    Add-Failure "artifact_prefix must be ChimeraPlusPlus (got '$artifactPrefix')"
}

# --- File content patterns ---
$docsImages = Join-Path $root 'docs\images'
$docsImagePaths = New-Object System.Collections.Generic.List[string]
if (Test-Path -LiteralPath $docsImages -PathType Container) {
    Get-ChildItem -LiteralPath $docsImages -Recurse -File -ErrorAction SilentlyContinue |
        ForEach-Object {
            $relativeImagePath = Get-RelativePath -Root $docsImages -FullPath $_.FullName
            $docsImagePaths.Add($relativeImagePath) | Out-Null
        }
}
foreach ($unapprovedImage in @(Get-UnapprovedDocsImagePaths -RelativePaths $docsImagePaths)) {
    Add-Failure "unapproved docs image asset: docs/images/$unapprovedImage"
}

$assetImages = Join-Path $root 'assets\images'
$assetImagePaths = New-Object System.Collections.Generic.List[string]
if (Test-Path -LiteralPath $assetImages -PathType Container) {
    Get-ChildItem -LiteralPath $assetImages -Recurse -File -ErrorAction SilentlyContinue |
        ForEach-Object {
            $relativeImagePath = Get-RelativePath -Root $assetImages -FullPath $_.FullName
            $assetImagePaths.Add($relativeImagePath) | Out-Null
        }
}
foreach ($unapprovedImage in @(Get-UnapprovedAssetImagePaths -RelativePaths $assetImagePaths)) {
    Add-Failure "unapproved product image asset: assets/images/$unapprovedImage"
}

foreach ($relPath in @(
    'apps/codex-plus-manager/src/App.tsx',
    'apps/codex-plus-manager/src/i18n-en.ts',
    'apps/codex-plus-mobile-relay/src/main.rs',
    'crates/codex-plus-core/src/launcher.rs',
    'crates/codex-plus-core/src/plugin_marketplace.rs',
    'crates/codex-plus-core/src/stepwise.rs',
    'crates/codex-plus-core/src/update.rs',
    'crates/codex-plus-core/src/watcher.rs',
    'crates/codex-plus-core/src/windows_integration.rs',
    'crates/codex-plus-core/src/zed_remote.rs'
)) {
    $fullPath = Join-Path $root $relPath
    $content = Get-Content -LiteralPath $fullPath -Raw -Encoding UTF8
    if (Test-ContainsExact -Haystack $content -Needle 'Codex++') {
        Add-Failure "$relPath contains legacy user-visible product name 'Codex++'"
    }
}

$publicBrandFiles = @('.github/ISSUE_TEMPLATE/bug_report.yml')
foreach ($relPath in $publicBrandFiles) {
    $fullPath = Join-Path $root $relPath
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) { continue }
    $publicContent = Get-Content -LiteralPath $fullPath -Raw -Encoding UTF8
    if (Test-ContainsExact -Haystack $publicContent -Needle 'Codex++') {
        Add-Failure "$relPath contains legacy public product name 'Codex++'"
    }
}

$promoPatterns = @(
    [pscustomobject]@{ Id = 'Ad-List'; Pattern = 'BigPizzaV3/Ad-List'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'ScriptMarket'; Pattern = 'BigPizzaV3/CodexPlusPlusScriptMarket'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'jojocode'; Pattern = 'jojocode.com'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'chimera-org'; Pattern = 'chimera-org/chimera-codex'; ProductionOnly = $false },
    [pscustomobject]@{ Id = 'example-owner'; Pattern = 'example owner'; ProductionOnly = $false },
    [pscustomobject]@{ Id = 'sponsor-inject'; Pattern = '__CODEX_PLUS_SPONSOR_IMAGES__'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'manager-en'; Pattern = 'Codex++ Manager'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'manager-zh'; Pattern = 'Codex++ 管理工具'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'upstream-discord'; Pattern = 'discord.gg/y96kX7A76v'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'upstream-telegram'; Pattern = 't.me/CodexPlusPlus'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'upstream-issues'; Pattern = 'BigPizzaV3/CodexPlusPlus/issues'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'upstream-discussions'; Pattern = 'BigPizzaV3/CodexPlusPlus/discussions'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'sponsor-binary'; Pattern = 'include_bytes!("../../../docs/images/sponsor-'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'sponsor-dead-ui'; Pattern = 'renderCodexPlusAds'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'ads-dead-route'; Pattern = 'codexPlusAdsUrl'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'injected-old-brand'; Pattern = 'Codex++ ${codexPlusVersion}'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'legacy-client-user-agent'; Pattern = 'CodexPlusPlus/'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'legacy-default-user-agent'; Pattern = 'proxied_client("CodexPlusPlus")'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'legacy-provider-sync-writer'; Pattern = '"managedBy": "Codex++ provider sync"'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'upstream-cargo-metadata'; Pattern = 'repository = "https://github.com/BigPizzaV3/CodexPlusPlus"'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'upstream-contributing-clone'; Pattern = 'git clone https://github.com/BigPizzaV3/CodexPlusPlus.git'; ProductionOnly = $true },
    [pscustomobject]@{ Id = 'unsafe-bootstrap-pipe'; Pattern = 'curl --proto ''=https'' --tlsv1.2 -sSf https://sh.rustup.rs | sh'; ProductionOnly = $true }
)
$promoPatterns += @(Get-CustomerSurfacePatterns)

$scanRoots = @(
    'README.md',
    'README_EN.md',
    'Cargo.toml',
    'CONTRIBUTING.md',
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
        if ($rule.PSObject.Properties.Name -contains 'CustomerUiOnly' -and
            $rule.CustomerUiOnly -and
            -not (Test-IsCustomerUiPath -RelPath $rel)) { continue }
        if ($rule.PSObject.Properties.Name -contains 'CustomerConsumerOnly' -and
            $rule.CustomerConsumerOnly -and
            -not (Test-IsCustomerUiConsumerPath -RelPath $rel)) { continue }
        if (-not (Test-RuleContains -Haystack $content -Rule $rule)) { continue }

        # Line-level reporting
        $lineNum = 0
        foreach ($line in ($content -split "`n")) {
            $lineNum++
            if (-not (Test-RuleContains -Haystack $line -Rule $rule)) { continue }
            if (Test-Allowlisted -RelPath $rel -Pattern $rule.Pattern -Allowlist $allowlist -LineNumber $lineNum -LineText $line) {
                continue
            }
            Add-Failure ("{0}:{1}: forbidden '{2}'" -f $rel, $lineNum, $rule.Pattern)
        }
    }

    # append_builtin_sponsors still called (not merely mentioned in comments/tests asserting absence)
    if ($isProd -and $rel -match '\.rs$') {
        $lineNum = 0
        foreach ($line in ($content -split "`n")) {
            $lineNum++
            if ($line -notmatch 'append_builtin_sponsors\s*\(') { continue }
            if (Test-Allowlisted -RelPath $rel -Pattern 'append_builtin_sponsors' -Allowlist $allowlist -LineNumber $lineNum -LineText $line) {
                continue
            }
            Add-Failure "${rel}:${lineNum}: append_builtin_sponsors(...) call must not remain in production"
        }
    }

    # update.rs must not hardcode upstream release URL
    if ($rel -eq 'crates/codex-plus-core/src/update.rs') {
        $lineNum = 0
        foreach ($line in ($content -split "`n")) {
            $lineNum++
            if (-not (Test-ContainsExact -Haystack $line -Needle 'BigPizzaV3/CodexPlusPlus')) { continue }
            if (Test-Allowlisted -RelPath $rel -Pattern 'BigPizzaV3/CodexPlusPlus' -Allowlist $allowlist -LineNumber $lineNum -LineText $line) {
                continue
            }
            Add-Failure "${rel}:${lineNum}: must not contain BigPizzaV3/CodexPlusPlus"
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
    $lineNum = 0
    foreach ($line in ($content -split "`n")) {
        $lineNum++

        # Artifact prefix: production release names should use ChimeraPlusPlus, not CodexPlusPlus-
        if ($line -match 'CodexPlusPlus-\$\{?VERSION\}?|CodexPlusPlus-\$version|CodexPlusPlus-.*-windows|CodexPlusPlus-.*-macos') {
            if (-not (Test-Allowlisted -RelPath $rel -Pattern 'CodexPlusPlus-' -Allowlist $allowlist -LineNumber $lineNum -LineText $line)) {
                Add-Failure "${rel}:${lineNum}: artifact names still use CodexPlusPlus- prefix; expected '$artifactPrefix-'"
            }
        }

        # Publisher / display brand drift vs product.toml (NSIS Publisher)
        if ($rel -like '*.nsi' -and $line -match 'Publisher"\s+"BigPizzaV3"') {
            if (-not (Test-Allowlisted -RelPath $rel -Pattern 'Publisher" "BigPizzaV3"' -Allowlist $allowlist -LineNumber $lineNum -LineText $line)) {
                Add-Failure "${rel}:${lineNum}: Publisher BigPizzaV3 does not match brand publisher '$publisher'"
            }
        }
    }
}

foreach ($entry in $allowlist) {
    if (-not [bool]$entry.Used) {
        Add-Failure ("unused allowlist entry {0}:{1} for {2}:{3} ('{4}')" -f
            $AllowlistPath, $entry.SourceLine, $entry.Path, $entry.LineNumber, $entry.Pattern)
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
