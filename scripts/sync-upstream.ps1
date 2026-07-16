#Requires -Version 5.1
<#
.SYNOPSIS
  Prepare an isolated sync/upstream-vX.Y.Z branch from the latest formal upstream Release.

.DESCRIPTION
  Validates remotes and repo state, discovers the latest non-draft/non-prerelease upstream
  Release, merges that tag onto a sync branch in an isolated worktree, sets
  X.Y.Z-chimera.1, runs branding + gate checks, and commits on the sync branch.

  Does NOT modify main, push, open PRs/Issues, or create GitHub Releases.
  The companion workflow (.github/workflows/sync-upstream.yml) handles push / PR / Issue.

.PARAMETER DryRun
  Read-only plan: remotes, status, ls-remote / GitHub API. No fetch, worktree, merge, or writes.

.PARAMETER SkipGates
  Skip branding/ads/cargo gates after a successful merge (CI may rely on PR checks). Default off.

.PARAMETER ResultPath
  Optional path to write a machine-readable result JSON for the workflow.

Exit codes:
  0  no change needed, or sync branch prepared successfully (DryRun plan OK)
  2  merge conflict (merge aborted)
  3  gate failure
  4  configuration / permission / precondition error
#>
[CmdletBinding()]
param(
    [switch]$DryRun,
    [switch]$SkipGates,
    [string]$ResultPath = '',
    [switch]$SkipMain,
    [string]$UpstreamTag = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:OriginUrlExact = 'https://github.com/Duojiyi/chimera-codex.git'
$script:UpstreamUrlExact = 'https://github.com/BigPizzaV3/CodexPlusPlus.git'
$script:UpstreamOwnerRepo = 'BigPizzaV3/CodexPlusPlus'
$script:OriginOwnerRepo = 'Duojiyi/chimera-codex'
$script:ExitCode = 0
$script:Result = [ordered]@{
    mode            = $(if ($DryRun) { 'dry-run' } else { 'apply' })
    action          = 'none'
    upstream_tag    = $null
    upstream_sha    = $null
    sync_branch     = $null
    chimera_version = $null
    gated_sha       = $null
    conflict_files  = @()
    message         = ''
}

function Get-RepoRoot {
    $scriptDir = Split-Path -Parent $PSCommandPath
    return (Resolve-Path (Join-Path $scriptDir '..')).Path
}

function Write-Info([string]$Message) {
    Write-Host $Message
}

function Write-Err([string]$Message) {
    Write-Host "ERROR: $Message" -ForegroundColor Red
}

function Set-ResultAndExit {
    param(
        [Parameter(Mandatory)][int]$Code,
        [Parameter(Mandatory)][string]$Message,
        [string]$Action = 'none'
    )
    $script:ExitCode = $Code
    $script:Result.action = $Action
    $script:Result.message = $Message
    Write-SyncResult
    if ($Code -eq 0) {
        Write-Info $Message
    }
    else {
        Write-Err $Message
    }
    exit $Code
}

function Write-SyncResult {
    # DryRun never writes unless the caller explicitly asked for an out-of-tree ResultPath.
    if ([string]::IsNullOrWhiteSpace($ResultPath)) { return }
    if ($DryRun -and $script:RepoRoot) {
        $full = [System.IO.Path]::GetFullPath($ResultPath)
        $repoFull = [System.IO.Path]::GetFullPath($script:RepoRoot)
        if ($full.StartsWith($repoFull, [System.StringComparison]::OrdinalIgnoreCase)) {
            Write-Info "DryRun: refusing to write ResultPath inside repo ($ResultPath)"
            return
        }
    }
    $dir = Split-Path -Parent $ResultPath
    if ($dir -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    ($script:Result | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $ResultPath -Encoding UTF8
}

function Invoke-Git {
    param(
        [Parameter(Mandatory)][string[]]$Args,
        [string]$WorkDir = ''
    )
    $argList = $Args
    if ($WorkDir) {
        $argList = @('-C', $WorkDir) + $Args
    }
    $stderrPath = [System.IO.Path]::GetTempFileName()
    try {
        $stdout = @(& git @argList 2> $stderrPath)
        $code = $LASTEXITCODE
        $stderr = @()
        if ((Get-Item -LiteralPath $stderrPath).Length -gt 0) {
            $stderr = @(Get-Content -LiteralPath $stderrPath -Encoding UTF8)
        }
        $stdoutLines = @($stdout | ForEach-Object { "$_" })
        $stderrLines = @($stderr | ForEach-Object { "$_" })
        $text = (@($stdoutLines) + @($stderrLines)) -join "`n"
        return [pscustomobject]@{
            Code        = $code
            Text        = $text
            Lines       = $stdoutLines
            StdoutLines = $stdoutLines
            StderrLines = $stderrLines
        }
    }
    finally {
        if (Test-Path -LiteralPath $stderrPath -PathType Leaf) {
            Remove-Item -LiteralPath $stderrPath -Force
        }
    }
}

function Get-IdentifiedGitArgs {
    param([Parameter(Mandatory)][string[]]$Arguments)

    return @(
        '-c', 'user.name=github-actions[bot]',
        '-c', 'user.email=41898282+github-actions[bot]@users.noreply.github.com'
    ) + $Arguments
}

function Get-UpstreamBaselineAncestryDisposition {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$BaselineTag,
        [string]$CandidateRef = 'HEAD'
    )

    $probe = Invoke-Git -WorkDir $Root -Args @(
        'merge-base', '--is-ancestor', $BaselineTag, $CandidateRef
    )
    if ($probe.Code -eq 0) { return 'present' }
    if ($probe.Code -eq 1) { return 'stitch' }
    throw "unable to inspect upstream baseline ancestry for ${BaselineTag}: $($probe.Text)"
}

function Ensure-UpstreamBaselineAncestry {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$BaselineTag
    )

    $disposition = Get-UpstreamBaselineAncestryDisposition `
        -Root $Root `
        -BaselineTag $BaselineTag
    if ($disposition -eq 'present') {
        Write-Info "Upstream baseline ancestry already contains $BaselineTag"
        return
    }

    Write-Info "Recording squash-merged upstream baseline ancestry for $BaselineTag"
    $mergeArgs = Get-IdentifiedGitArgs -Arguments @(
        'merge', '--no-ff', '--no-edit', '-s', 'ours', $BaselineTag
    )
    $merge = Invoke-Git -WorkDir $Root -Args $mergeArgs
    if ($merge.Code -ne 0) {
        throw "failed to record upstream baseline ancestry for ${BaselineTag}: $($merge.Text)"
    }
}

function Get-MergeFailureDisposition {
    param(
        [Parameter(Mandatory)]$MergeResult,
        [Parameter(Mandatory)]$ConflictResult
    )

    $files = @()
    if ($ConflictResult.Code -eq 0) {
        $files = @($ConflictResult.StdoutLines | Where-Object { $_ -match '\S' })
    }
    if ($files.Count -gt 0) {
        return [pscustomobject]@{
            Kind    = 'conflict'
            Files   = $files
            Message = "Merge conflict in: $($files -join ', ')"
            ExitCode = 2
            Action   = 'conflict'
            ShouldAbort = $true
        }
    }

    $message = "git merge failed (exit $($MergeResult.Code)): $($MergeResult.Text)"
    if ($ConflictResult.Code -ne 0) {
        $message += " | unable to inspect unmerged paths (exit $($ConflictResult.Code)): $($ConflictResult.Text)"
    }
    else {
        $message += ' | no unmerged paths were recorded'
    }
    return [pscustomobject]@{
        Kind    = 'error'
        Files   = @()
        Message = $message
        ExitCode = 4
        Action   = 'error'
        ShouldAbort = $false
    }
}

function Require-GitOk {
    param(
        [Parameter(Mandatory)]$Result,
        [Parameter(Mandatory)][string]$Context
    )
    if ($Result.Code -ne 0) {
        Set-ResultAndExit -Code 4 -Message "$Context failed (exit $($Result.Code)): $($Result.Text)" -Action 'error'
    }
}

function Get-RemoteUrl {
    param(
        [Parameter(Mandatory)][string]$Name,
        [switch]$Push
    )
    $gitArgs = @('remote', 'get-url')
    if ($Push) { $gitArgs += '--push' }
    $gitArgs += $Name
    $r = Invoke-Git -Args $gitArgs
    if ($r.Code -ne 0) { return $null }
    return $r.Text.Trim()
}

function Test-CleanWorktree {
    $r = Invoke-Git -Args @('status', '--porcelain')
    Require-GitOk -Result $r -Context 'git status'
    return [string]::IsNullOrWhiteSpace($r.Text)
}

function Test-InProgressGitOp([string]$Root) {
    $gitDir = Join-Path $Root '.git'
    if (Test-Path -LiteralPath (Join-Path $gitDir 'MERGE_HEAD')) { return 'merge' }
    if (Test-Path -LiteralPath (Join-Path $gitDir 'rebase-merge')) { return 'rebase' }
    if (Test-Path -LiteralPath (Join-Path $gitDir 'rebase-apply')) { return 'rebase' }
    if (Test-Path -LiteralPath (Join-Path $gitDir 'CHERRY_PICK_HEAD')) { return 'cherry-pick' }
    if (Test-Path -LiteralPath (Join-Path $gitDir 'REVERT_HEAD')) { return 'revert' }
    return $null
}

function Assert-Remotes {
    $origin = Get-RemoteUrl -Name 'origin'
    $upstream = Get-RemoteUrl -Name 'upstream'
    $upstreamPush = Get-RemoteUrl -Name 'upstream' -Push

    if ($origin -ne $script:OriginUrlExact) {
        Set-ResultAndExit -Code 4 -Message "origin must be $($script:OriginUrlExact), got: $origin" -Action 'error'
    }
    if ($upstream -ne $script:UpstreamUrlExact) {
        Set-ResultAndExit -Code 4 -Message "upstream must be $($script:UpstreamUrlExact), got: $upstream" -Action 'error'
    }
    if ($upstreamPush -eq $script:UpstreamUrlExact -or $upstreamPush -match 'BigPizzaV3/CodexPlusPlus') {
        Set-ResultAndExit -Code 4 -Message "upstream push URL must be blocked, got: $upstreamPush" -Action 'error'
    }

    return [pscustomobject]@{
        Origin        = $origin
        Upstream      = $upstream
        UpstreamPush  = $upstreamPush
    }
}

function Get-FormalSemVerParts {
    param([Parameter(Mandatory)][string]$Version)

    $normalized = $Version.Trim()
    if ($normalized.StartsWith('v') -or $normalized.StartsWith('V')) {
        $normalized = $normalized.Substring(1)
    }
    if ($normalized -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
        throw "Version is not formal SemVer X.Y.Z: $Version"
    }
    return @($Matches[1], $Matches[2], $Matches[3])
}

function Compare-FormalSemVer {
    param(
        [Parameter(Mandatory)][string]$Left,
        [Parameter(Mandatory)][string]$Right
    )

    $leftParts = @(Get-FormalSemVerParts -Version $Left)
    $rightParts = @(Get-FormalSemVerParts -Version $Right)
    for ($i = 0; $i -lt 3; $i++) {
        if ($leftParts[$i].Length -lt $rightParts[$i].Length) { return -1 }
        if ($leftParts[$i].Length -gt $rightParts[$i].Length) { return 1 }
        $comparison = [string]::CompareOrdinal($leftParts[$i], $rightParts[$i])
        if ($comparison -lt 0) { return -1 }
        if ($comparison -gt 0) { return 1 }
    }
    return 0
}

function Get-FormalBaselineVersion {
    param([Parameter(Mandatory)][string]$WorkspaceVersion)

    if ($WorkspaceVersion -notmatch '^((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))(?:-chimera\.\d+)?$') {
        throw "Workspace version does not contain a formal X.Y.Z baseline: $WorkspaceVersion"
    }
    return $Matches[1]
}

function Get-UpstreamVersionDisposition {
    param(
        [Parameter(Mandatory)][string]$CandidateVersion,
        [Parameter(Mandatory)][string]$BaselineVersion
    )

    $comparison = Compare-FormalSemVer -Left $CandidateVersion -Right $BaselineVersion
    if ($comparison -lt 0) { return 'regression' }
    if ($comparison -eq 0) { return 'duplicate' }
    return 'advance'
}

function Select-LatestFormalRelease {
    param([Parameter(Mandatory)][object[]]$Releases)

    $best = $null
    $bestVersion = $null
    foreach ($rel in $Releases) {
        if ($null -eq $rel -or $rel.draft -eq $true -or $rel.prerelease -eq $true) {
            continue
        }
        $tag = [string]$rel.tag_name
        if ($tag -notmatch '^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
            continue
        }
        $version = $tag -replace '^[vV]', ''
        if ($null -eq $best -or (Compare-FormalSemVer -Left $version -Right $bestVersion) -gt 0) {
            $bestVersion = $version
            $best = [pscustomobject]@{
                Tag         = $tag
                Name        = [string]$rel.name
                Targetish   = [string]$rel.target_commitish
                HtmlUrl     = [string]$rel.html_url
                PublishedAt = [string]$rel.published_at
            }
        }
    }
    return $best
}

function Get-LatestFormalUpstreamRelease {
    param(
        [string]$RequestedTag = '',
        [scriptblock]$RequestPage = $null,
        [scriptblock]$Fail = $null
    )

    if ($null -eq $Fail) {
        $Fail = {
            param([int]$Code, [string]$Message, [string]$Action)
            Set-ResultAndExit -Code $Code -Message $Message -Action $Action
        }
    }

    $headers = @{
        'Accept'               = 'application/vnd.github+json'
        'User-Agent'           = 'chimera-codex-sync-upstream'
        'X-GitHub-Api-Version' = '2022-11-28'
    }
    $token = $env:CHIMERA_AUTOMATION_TOKEN
    if (-not $token) { $token = $env:GH_TOKEN }
    if (-not $token) { $token = $env:GITHUB_TOKEN }
    if ($token) {
        $headers['Authorization'] = "Bearer $token"
    }

    if ($null -eq $RequestPage) {
        $RequestPage = {
            param([string]$Uri, [hashtable]$Headers)
            Invoke-RestMethod -Uri $Uri -Headers $Headers -Method Get
        }
    }

    $allReleases = New-Object 'System.Collections.Generic.List[object]'
    $page = 1
    while ($true) {
        $uri = "https://api.github.com/repos/$($script:UpstreamOwnerRepo)/releases?per_page=100&page=$page"
        try {
            # Invoke-RestMethod emits a JSON array as one pipeline object. Assign first,
            # then array-expand the value so both live API responses and fixtures agree.
            $pageResponse = & $RequestPage -Uri $uri -Headers $headers
            $pageReleases = @($pageResponse)
        }
        catch {
            Set-ResultAndExit -Code 4 -Message "Failed to query upstream releases page ${page}: $_" -Action 'error'
        }
        foreach ($release in $pageReleases) {
            if ($null -ne $release) {
                $allReleases.Add($release)
            }
        }
        if ($pageReleases.Count -lt 100) { break }
        if ($page -ge 100) {
            Set-ResultAndExit -Code 4 -Message 'Upstream release pagination exceeded 100 pages' -Action 'error'
        }
        $page++
    }

    $candidateReleases = $allReleases.ToArray()
    if (-not [string]::IsNullOrWhiteSpace($RequestedTag)) {
        $candidateReleases = @($candidateReleases | Where-Object {
                [string]$_.tag_name -ceq $RequestedTag
            })
    }
    $selected = if ($candidateReleases.Count -gt 0) {
        Select-LatestFormalRelease -Releases $candidateReleases
    }
    else {
        $null
    }
    if ($null -ne $selected) { return $selected }
    if (-not [string]::IsNullOrWhiteSpace($RequestedTag)) {
        & $Fail 4 "Requested tag is not a formal upstream Release: $RequestedTag" 'error'
        return $null
    }
    & $Fail 4 'No formal (non-draft/non-prerelease) upstream Release found' 'error'
    return $null
}

function Normalize-UpstreamVersion([string]$Tag) {
    try {
        return (@(Get-FormalSemVerParts -Version $Tag) -join '.')
    }
    catch {
        Set-ResultAndExit -Code 4 -Message "Upstream tag is not X.Y.Z: $Tag" -Action 'error'
    }
}

function Get-LsRemoteRef {
    param(
        [Parameter(Mandatory)][string]$Remote,
        [Parameter(Mandatory)][string]$Ref
    )
    $r = Invoke-Git -Args @('ls-remote', $Remote, $Ref)
    if ($r.Code -ne 0) {
        return $null
    }
    $line = ($r.Lines | Where-Object { $_ -match '\S' } | Select-Object -First 1)
    if (-not $line) { return $null }
    if ($line -match '^([0-9a-f]{40})\s+') {
        return $Matches[1]
    }
    return $null
}

function Get-WorkspaceCargoVersion {
    param([Parameter(Mandatory)][string]$Root)
    $cargoToml = Join-Path $Root 'Cargo.toml'
    $inWorkspacePackage = $false
    foreach ($raw in Get-Content -LiteralPath $cargoToml -Encoding UTF8) {
        $line = $raw.Trim()
        if ($line -eq '[workspace.package]') { $inWorkspacePackage = $true; continue }
        if ($inWorkspacePackage -and $line.StartsWith('[')) { break }
        if ($inWorkspacePackage -and $line -match '^version\s*=\s*"(.*)"\s*$') {
            return $Matches[1]
        }
    }
    return $null
}

function Test-IdempotentAlreadySynced {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][string]$SyncBranch
    )
    $chimeraTag = "v$Version-chimera.1"
    $tagSha = Get-LsRemoteRef -Remote 'origin' -Ref "refs/tags/$chimeraTag"
    if ($tagSha) {
        return [pscustomobject]@{
            Synced  = $true
            Resume  = $false
            Reason  = "origin already has tag $chimeraTag ($tagSha)"
        }
    }
    $branchSha = Get-LsRemoteRef -Remote 'origin' -Ref "refs/heads/$SyncBranch"
    if ($branchSha) {
        return [pscustomobject]@{
            Synced  = $false
            Resume  = $true
            Reason  = "origin already has branch $SyncBranch ($branchSha)"
        }
    }
    $localVer = Get-WorkspaceCargoVersion -Root $Root
    if ($localVer -and $localVer -match "^$([regex]::Escape($Version))-chimera\.\d+$") {
        return [pscustomobject]@{
            Synced  = $true
            Resume  = $false
            Reason  = "workspace already at upstream $Version ($localVer)"
        }
    }
    return [pscustomobject]@{ Synced = $false; Resume = $false; Reason = '' }
}

function Set-WorkspaceChimeraVersion {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$Version
    )

    $cargoPath = Join-Path $Root 'Cargo.toml'
    $lines = Get-Content -LiteralPath $cargoPath -Encoding UTF8
    $inWp = $false
    $replaced = $false
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        if ($line.Trim() -eq '[workspace.package]') { $inWp = $true; continue }
        if ($inWp -and $line.Trim().StartsWith('[')) { break }
        if ($inWp -and $line -match '^version\s*=\s*"') {
            $lines[$i] = "version = `"$Version`""
            $replaced = $true
            break
        }
    }
    if (-not $replaced) {
        throw 'Unable to update [workspace.package].version in Cargo.toml'
    }
    [System.IO.File]::WriteAllText($cargoPath, (($lines -join "`n") + "`n"))

    foreach ($rel in @(
            'apps\codex-plus-manager\package.json',
            'apps\codex-plus-manager\src-tauri\tauri.conf.json'
        )) {
        $path = Join-Path $Root $rel
        $raw = Get-Content -LiteralPath $path -Raw -Encoding UTF8
        $updated = [regex]::Replace($raw, '("version"\s*:\s*")[^"]*(")', { param($m) $m.Groups[1].Value + $Version + $m.Groups[2].Value }, 1)
        if ($updated -eq $raw) {
            throw "Unable to update version in $rel"
        }
        [System.IO.File]::WriteAllText($path, $updated)
    }

    $tomlPath = Join-Path $Root 'brand\product.toml'
    $tomlLines = Get-Content -LiteralPath $tomlPath -Encoding UTF8
    $build = 1
    foreach ($line in $tomlLines) {
        if ($line -match '^macos_build_number\s*=\s*(\d+)\s*$') {
            $build = [int]$Matches[1] + 1
            break
        }
    }
    $newToml = foreach ($line in $tomlLines) {
        if ($line -match '^macos_build_number\s*=') {
            "macos_build_number = $build"
        }
        else {
            $line
        }
    }
    [System.IO.File]::WriteAllText($tomlPath, (($newToml -join "`n") + "`n"))
}

function Set-PackageLockVersion {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$Version
    )

    $path = Join-Path $Root 'apps\codex-plus-manager\package-lock.json'
    $raw = Get-Content -LiteralPath $path -Raw -Encoding UTF8
    $json = $raw | ConvertFrom-Json
    $rootPackageProperty = $json.packages.PSObject.Properties['']
    if (-not $json.version -or $null -eq $rootPackageProperty -or -not $rootPackageProperty.Value.version) {
        throw 'package-lock.json is missing top-level or root-package version'
    }

    $topVersion = [regex]::new('(?m)^(  "version"\s*:\s*")[^"]+(",\s*)$')
    if ($topVersion.Matches($raw).Count -ne 1) {
        throw 'Unable to locate unique top-level package-lock.json version'
    }
    $updated = $topVersion.Replace(
        $raw,
        { param($match) $match.Groups[1].Value + $Version + $match.Groups[2].Value },
        1
    )

    $rootVersion = [regex]::new(
        '(?ms)("packages"\s*:\s*\{\s*""\s*:\s*\{\s*"name"\s*:\s*"[^"]+"\s*,\s*"version"\s*:\s*")[^"]+(")'
    )
    if ($rootVersion.Matches($updated).Count -ne 1) {
        throw 'Unable to locate unique root-package package-lock.json version'
    }
    $updated = $rootVersion.Replace(
        $updated,
        { param($match) $match.Groups[1].Value + $Version + $match.Groups[2].Value },
        1
    )
    [System.IO.File]::WriteAllText($path, $updated)

    $verified = Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
    $verifiedRoot = $verified.packages.PSObject.Properties[''].Value
    if ([string]$verified.version -ne $Version -or [string]$verifiedRoot.version -ne $Version) {
        throw "package-lock.json version refresh did not produce $Version"
    }
}

function Update-AndValidateDependencyLocks {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$Version
    )

    Set-PackageLockVersion -Root $Root -Version $Version
    $packageLockPath = Join-Path $Root 'apps\codex-plus-manager\package-lock.json'

    Push-Location $Root
    try {
        Write-Info 'Lock refresh: cargo update --workspace'
        & cargo update --workspace
        if ($LASTEXITCODE -ne 0) {
            throw "cargo workspace lock refresh failed (exit $LASTEXITCODE)"
        }

        Write-Info 'Lock validation: cargo metadata --locked --format-version 1 --no-deps'
        & cargo metadata --locked --format-version 1 --no-deps | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo.lock validation failed (exit $LASTEXITCODE)"
        }

        $packageLockHashBeforeValidation = (Get-FileHash -LiteralPath $packageLockPath -Algorithm SHA256).Hash
        Push-Location (Join-Path $Root 'apps\codex-plus-manager')
        try {
            Write-Info 'Lock validation: npm ci --ignore-scripts --no-audit --no-fund'
            & npm ci --ignore-scripts --no-audit --no-fund
            if ($LASTEXITCODE -ne 0) {
                throw "package-lock.json validation failed (exit $LASTEXITCODE)"
            }
        }
        finally {
            Pop-Location
        }
        $packageLockHashAfterValidation = (Get-FileHash -LiteralPath $packageLockPath -Algorithm SHA256).Hash
        if ($packageLockHashBeforeValidation -ne $packageLockHashAfterValidation) {
            throw 'package-lock validation changed package-lock.json; refusing unrelated dependency updates'
        }
    }
    finally {
        Pop-Location
    }
}

function Invoke-Gates {
    param([Parameter(Mandatory)][string]$Root)

    Push-Location $Root
    try {
        Write-Info 'Gate: generate-branding.ps1'
        & pwsh -NoProfile -File (Join-Path $Root 'scripts\generate-branding.ps1')
        if ($LASTEXITCODE -ne 0) { throw "generate-branding.ps1 failed (exit $LASTEXITCODE)" }

        Write-Info 'Gate: generate-branding.ps1 -Check'
        & pwsh -NoProfile -File (Join-Path $Root 'scripts\generate-branding.ps1') -Check
        if ($LASTEXITCODE -ne 0) { throw "generate-branding.ps1 -Check failed (exit $LASTEXITCODE)" }

        Write-Info 'Gate: verify-no-upstream-ads.ps1'
        & pwsh -NoProfile -File (Join-Path $Root 'scripts\verify-no-upstream-ads.ps1')
        if ($LASTEXITCODE -ne 0) { throw "verify-no-upstream-ads.ps1 failed (exit $LASTEXITCODE)" }

        Write-Info 'Gate: cargo fmt --check'
        & cargo fmt --check
        if ($LASTEXITCODE -ne 0) { throw "cargo fmt --check failed (exit $LASTEXITCODE)" }

        Write-Info 'Gate: cargo test --workspace --locked'
        & cargo test --workspace --locked
        if ($LASTEXITCODE -ne 0) { throw "cargo test --workspace --locked failed (exit $LASTEXITCODE)" }
    }
    finally {
        Pop-Location
    }
}

function Assert-ProtectedWorkflowTree {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$TrustedRef,
        [string]$CandidateRef = 'HEAD',
        [switch]$Cached
    )

    $diffArgs = @('diff')
    if ($Cached) { $diffArgs += '--cached' }
    $diffArgs += @('--quiet', $TrustedRef)
    if (-not $Cached) { $diffArgs += $CandidateRef }
    $diffArgs += @('--', '.github/workflows')
    $diff = Invoke-Git -WorkDir $Root -Args $diffArgs
    if ($diff.Code -eq 1) {
        throw "candidate changes protected workflow tree relative to $TrustedRef"
    }
    if ($diff.Code -ne 0) {
        throw "failed to verify protected workflow tree (exit $($diff.Code)): $($diff.Text)"
    }
}

function Restore-ProtectedWorkflowTree {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$TrustedRef
    )

    $restoreArgs = @('restore', "--source=$TrustedRef", '--staged', '--worktree', '--', '.github/workflows')
    $restore = Invoke-Git -WorkDir $Root -Args $restoreArgs
    Require-GitOk -Result $restore -Context 'restore protected workflow tree from trusted main'
    Assert-ProtectedWorkflowTree -Root $Root -TrustedRef $TrustedRef -Cached
}

function Test-RemoteSyncBranch {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$SyncBranch,
        [Parameter(Mandatory)][string]$ExpectedVersion,
        [Parameter(Mandatory)][string]$ExpectedUpstreamTag
    )

    $remoteRef = "refs/remotes/origin/$SyncBranch"
    $mainFetch = Invoke-Git -Args @(
        'fetch', 'origin', '+refs/heads/main:refs/remotes/origin/main', '--no-tags'
    )
    Require-GitOk -Result $mainFetch -Context 'fetch trusted origin/main for resume'
    $tagFetch = Invoke-Git -Args @(
        'fetch', 'upstream', "+refs/tags/${ExpectedUpstreamTag}:refs/tags/${ExpectedUpstreamTag}", '--no-tags'
    )
    Require-GitOk -Result $tagFetch -Context "fetch formal upstream tag $ExpectedUpstreamTag for resume"
    $fetch = Invoke-Git -Args @(
        'fetch', 'origin', "+refs/heads/${SyncBranch}:${remoteRef}", '--no-tags'
    )
    Require-GitOk -Result $fetch -Context "fetch existing remote sync branch $SyncBranch"
    $mainAncestry = Invoke-Git -Args @('merge-base', '--is-ancestor', 'origin/main', $remoteRef)
    if ($mainAncestry.Code -ne 0) {
        throw 'remote sync branch is not based on trusted origin/main'
    }
    $tagAncestry = Invoke-Git -Args @(
        'merge-base', '--is-ancestor', "refs/tags/$ExpectedUpstreamTag", $remoteRef
    )
    if ($tagAncestry.Code -ne 0) {
        throw "remote sync branch does not contain formal upstream tag $ExpectedUpstreamTag"
    }
    Assert-ProtectedWorkflowTree -Root $Root -TrustedRef 'origin/main' -CandidateRef $remoteRef
    $worktreePath = Join-Path ([System.IO.Path]::GetTempPath()) ("chimera-resume-" + [guid]::NewGuid().ToString('N'))
    $worktree = Invoke-Git -Args @('worktree', 'add', '--detach', $worktreePath, $remoteRef)
    Require-GitOk -Result $worktree -Context "create resume worktree for $SyncBranch"
    try {
        $actualVersion = Get-WorkspaceCargoVersion -Root $worktreePath
        if ($actualVersion -ne $ExpectedVersion) {
            throw "remote sync branch version mismatch: expected $ExpectedVersion, got $actualVersion"
        }
        $candidateHead = Invoke-Git -WorkDir $worktreePath -Args @('rev-parse', 'HEAD')
        if ($candidateHead.Code -ne 0) {
            throw "failed to resolve resumed branch HEAD: $($candidateHead.Text)"
        }
        $candidateSha = $candidateHead.Text.Trim()
        if ($candidateSha -notmatch '^[0-9a-f]{40}$') {
            throw "resumed branch produced unsafe candidate SHA: $candidateSha"
        }
        Invoke-Gates -Root $worktreePath
        $status = Invoke-Git -WorkDir $worktreePath -Args @('status', '--porcelain')
        Require-GitOk -Result $status -Context 'check resumed branch worktree'
        if (-not [string]::IsNullOrWhiteSpace($status.Text)) {
            throw "gates changed resumed branch worktree: $($status.Text)"
        }
        $afterGateHead = Invoke-Git -WorkDir $worktreePath -Args @('rev-parse', 'HEAD')
        if ($afterGateHead.Code -ne 0) {
            throw "failed to re-resolve resumed branch HEAD: $($afterGateHead.Text)"
        }
        $afterGateSha = $afterGateHead.Text.Trim()
        if ($afterGateSha -ne $candidateSha) {
            throw "gates changed resumed branch HEAD: expected $candidateSha, got $afterGateSha"
        }
        return $candidateSha
    }
    finally {
        Invoke-Git -Args @('worktree', 'remove', '--force', $worktreePath) | Out-Null
    }
}

function Get-Snapshot {
    $head = (Invoke-Git -Args @('rev-parse', 'HEAD')).Text.Trim()
    $refs = (Invoke-Git -Args @('for-each-ref', '--format=%(refname) %(objectname)', 'refs/heads', 'refs/tags')).Text
    return [pscustomobject]@{
        Head = $head
        Refs = $refs
    }
}

if ($SkipMain) {
    return
}

# --- main ---
$script:RepoRoot = Get-RepoRoot
$root = $script:RepoRoot
Set-Location $root

Write-Info '=== Chimera upstream sync ==='
Write-Info "mode: $(if ($DryRun) { 'DryRun' } else { 'apply' })"
Write-Info "repo: $root"

$remotes = Assert-Remotes
Write-Info "origin:   $($remotes.Origin)"
Write-Info "upstream: $($remotes.Upstream) (push=$($remotes.UpstreamPush))"

$shallow = (Invoke-Git -Args @('rev-parse', '--is-shallow-repository')).Text.Trim()
$clean = Test-CleanWorktree
$inProgress = Test-InProgressGitOp -Root $root
$statusText = (Invoke-Git -Args @('status', '-sb')).Text

Write-Info "shallow:  $shallow"
Write-Info "clean:    $clean"
Write-Info "git-op:   $(if ($inProgress) { $inProgress } else { 'none' })"
Write-Info "status:`n$statusText"

if ($shallow -eq 'true') {
    Set-ResultAndExit -Code 4 -Message 'Repository is shallow; refuse to sync (need fetch-depth: 0 / unshallow)' -Action 'error'
}

if (-not $DryRun) {
    if (-not $clean) {
        Set-ResultAndExit -Code 4 -Message 'Worktree is dirty; refuse to sync' -Action 'error'
    }
    if ($inProgress) {
        Set-ResultAndExit -Code 4 -Message "In-progress git operation ($inProgress); refuse to sync" -Action 'error'
    }
}

$before = $null
if ($DryRun) {
    $before = Get-Snapshot
}

$release = Get-LatestFormalUpstreamRelease -RequestedTag $UpstreamTag
$version = Normalize-UpstreamVersion -Tag $release.Tag
$workspaceVersion = Get-WorkspaceCargoVersion -Root $root
if (-not $workspaceVersion) {
    Set-ResultAndExit -Code 4 -Message 'Unable to read current workspace version baseline' -Action 'error'
}
try {
    $baselineVersion = Get-FormalBaselineVersion -WorkspaceVersion $workspaceVersion
    $versionDisposition = Get-UpstreamVersionDisposition -CandidateVersion $version -BaselineVersion $baselineVersion
}
catch {
    Set-ResultAndExit -Code 4 -Message "Unable to compare upstream release with current baseline: $_" -Action 'error'
}
$upstreamTag = if ($release.Tag.StartsWith('v') -or $release.Tag.StartsWith('V')) { $release.Tag } else { "v$($release.Tag)" }
$syncBranch = "sync/upstream-v$version"
$chimeraVersion = "$version-chimera.1"
$upstreamShaHint = Get-LsRemoteRef -Remote 'upstream' -Ref "refs/tags/$upstreamTag"

$script:Result.upstream_tag = $upstreamTag
$script:Result.upstream_sha = $upstreamShaHint
$script:Result.sync_branch = $syncBranch
$script:Result.chimera_version = $chimeraVersion

Write-Info ''
Write-Info '--- plan ---'
Write-Info "upstream release: $upstreamTag ($($release.HtmlUrl))"
Write-Info "current baseline: $baselineVersion ($workspaceVersion)"
Write-Info "upstream sha:     $(if ($upstreamShaHint) { $upstreamShaHint } else { '(ls-remote miss; will fetch on apply)' })"
Write-Info "sync branch:      $syncBranch"
Write-Info "chimera version:  $chimeraVersion"
Write-Info 'gates:            generate-branding (+Check), verify-no-upstream-ads, cargo fmt --check, cargo test --locked'
Write-Info 'forbidden:        modify main, create Release, push (workflow owns push/PR/Issue)'

if ($versionDisposition -eq 'regression') {
    if ($DryRun) {
        $after = Get-Snapshot
        if ($before.Head -ne $after.Head -or $before.Refs -ne $after.Refs) {
            Set-ResultAndExit -Code 4 -Message 'DryRun mutated HEAD/refs (unexpected)' -Action 'error'
        }
    }
    Set-ResultAndExit -Code 4 -Message "Refusing upstream regression: latest formal Release $version is older than current baseline $baselineVersion" -Action 'error'
}
if ($versionDisposition -eq 'duplicate') {
    Write-Info "idempotent:       YES - latest formal Release equals current baseline $baselineVersion"
    if ($DryRun) {
        $after = Get-Snapshot
        if ($before.Head -ne $after.Head -or $before.Refs -ne $after.Refs) {
            Set-ResultAndExit -Code 4 -Message 'DryRun mutated HEAD/refs (unexpected)' -Action 'error'
        }
        Write-Info 'DryRun: HEAD / refs unchanged (read-only: remote get-url, status, ls-remote, API)'
    }
    Set-ResultAndExit -Code 0 -Message "No sync needed: latest formal Release $version equals current baseline" -Action 'noop'
}

$idemp = Test-IdempotentAlreadySynced -Root $root -Version $version -SyncBranch $syncBranch
if ($idemp.Synced) {
    Write-Info "idempotent:       YES - $($idemp.Reason)"
    if ($DryRun) {
        $after = Get-Snapshot
        if ($before.Head -ne $after.Head -or $before.Refs -ne $after.Refs) {
            Set-ResultAndExit -Code 4 -Message 'DryRun mutated HEAD/refs (unexpected)' -Action 'error'
        }
        Write-Info 'DryRun: HEAD / refs unchanged (read-only: remote get-url, status, ls-remote, API)'
    }
    Set-ResultAndExit -Code 0 -Message "No sync needed: $($idemp.Reason)" -Action 'noop'
}

if ($idemp.Resume) {
    Write-Info "idempotent:       RESUME - $($idemp.Reason)"
    if ($DryRun) {
        $after = Get-Snapshot
        if ($before.Head -ne $after.Head -or $before.Refs -ne $after.Refs) {
            Set-ResultAndExit -Code 4 -Message 'DryRun mutated HEAD/refs (unexpected)' -Action 'error'
        }
        Write-Info 'DryRun: existing remote branch would be fetched and re-gated on apply'
        Set-ResultAndExit -Code 0 -Message "DryRun OK: would resume $syncBranch as $chimeraVersion" -Action 'plan'
    }

    try {
        $gatedSha = Test-RemoteSyncBranch -Root $root -SyncBranch $syncBranch -ExpectedVersion $chimeraVersion -ExpectedUpstreamTag $upstreamTag
    }
    catch {
        Set-ResultAndExit -Code 3 -Message "Gate failure while resuming ${syncBranch}: $_" -Action 'gate_failed'
    }
    $script:Result.gated_sha = $gatedSha
    Set-ResultAndExit -Code 0 -Message "Re-gated existing remote branch $syncBranch at $gatedSha" -Action 'resume'
}

Write-Info 'idempotent:       NO - sync would proceed'

if ($DryRun) {
    Write-Info ''
    Write-Info 'DryRun actions NOT taken: fetch, worktree add, merge, version write, gates, commit, push, release'
    $after = Get-Snapshot
    if ($before.Head -ne $after.Head -or $before.Refs -ne $after.Refs) {
        Set-ResultAndExit -Code 4 -Message 'DryRun mutated HEAD/refs (unexpected)' -Action 'error'
    }
    Write-Info 'DryRun: HEAD / refs unchanged (read-only: remote get-url, status, ls-remote, API)'
    Set-ResultAndExit -Code 0 -Message "DryRun OK: would sync $upstreamTag -> $syncBranch as $chimeraVersion" -Action 'plan'
}

# --- apply ---
Write-Info ''
Write-Info 'Fetching upstream tag (explicit)...'
$fetch = Invoke-Git -Args @('fetch', 'upstream', "refs/tags/${upstreamTag}:refs/tags/${upstreamTag}", '--no-tags')
if ($fetch.Code -ne 0) {
    # tag may already exist locally
    $fetch2 = Invoke-Git -Args @('fetch', 'upstream', "tag", $upstreamTag)
    if ($fetch2.Code -ne 0) {
        Set-ResultAndExit -Code 4 -Message "Failed to fetch upstream tag ${upstreamTag}: $($fetch.Text) | $($fetch2.Text)" -Action 'error'
    }
}

$tagSha = (Invoke-Git -Args @('rev-list', '-n', '1', $upstreamTag)).Text.Trim()
if (-not $tagSha) {
    Set-ResultAndExit -Code 4 -Message "Unable to resolve SHA for $upstreamTag" -Action 'error'
}
$script:Result.upstream_sha = $tagSha
Write-Info "resolved $upstreamTag -> $tagSha"

$baselineTag = "v$baselineVersion"
$baselineFetch = Invoke-Git -Args @(
    'fetch', 'upstream', "refs/tags/${baselineTag}:refs/tags/${baselineTag}", '--no-tags'
)
if ($baselineFetch.Code -ne 0) {
    $baselineFetch2 = Invoke-Git -Args @('fetch', 'upstream', 'tag', $baselineTag)
    if ($baselineFetch2.Code -ne 0) {
        Set-ResultAndExit -Code 4 -Message "Failed to fetch upstream baseline tag ${baselineTag}: $($baselineFetch.Text) | $($baselineFetch2.Text)" -Action 'error'
    }
}

$mainRef = 'main'
$mainSha = (Invoke-Git -Args @('rev-parse', $mainRef)).Text.Trim()
if (-not $mainSha -or (Invoke-Git -Args @('rev-parse', $mainRef)).Code -ne 0) {
    $mainRef = 'origin/main'
    $mainSha = (Invoke-Git -Args @('rev-parse', $mainRef)).Text.Trim()
}
if (-not $mainSha) {
    Set-ResultAndExit -Code 4 -Message 'Unable to resolve main / origin/main' -Action 'error'
}

$worktreePath = Join-Path ([System.IO.Path]::GetTempPath()) ("chimera-sync-" + [guid]::NewGuid().ToString('N'))
Write-Info "Creating isolated worktree: $worktreePath (branch $syncBranch from $mainRef)"

# Remove stale local branch if present without worktree
$existing = Invoke-Git -Args @('show-ref', '--verify', "--quiet", "refs/heads/$syncBranch")
if ($existing.Code -eq 0) {
    $del = Invoke-Git -Args @('branch', '-D', $syncBranch)
    if ($del.Code -ne 0) {
        Set-ResultAndExit -Code 4 -Message "Failed to replace existing local branch ${syncBranch}: $($del.Text)" -Action 'error'
    }
}

$wt = Invoke-Git -Args @('worktree', 'add', '-b', $syncBranch, $worktreePath, $mainRef)
if ($wt.Code -ne 0) {
    Set-ResultAndExit -Code 4 -Message "git worktree add failed: $($wt.Text)" -Action 'error'
}

$cleanupWorktree = {
    Invoke-Git -Args @('worktree', 'remove', '--force', $worktreePath) | Out-Null
    if (Test-Path -LiteralPath $worktreePath) {
        Remove-Item -LiteralPath $worktreePath -Recurse -Force -ErrorAction SilentlyContinue
    }
}

try {
    Ensure-UpstreamBaselineAncestry -Root $worktreePath -BaselineTag $baselineTag

    Write-Info "Merging $upstreamTag into $syncBranch..."
    $mergeArgs = Get-IdentifiedGitArgs -Arguments @('merge', '--no-ff', '--no-edit', $upstreamTag)
    $merge = Invoke-Git -WorkDir $worktreePath -Args $mergeArgs
    if ($merge.Code -ne 0) {
        $conflicts = Invoke-Git -WorkDir $worktreePath -Args @('diff', '--name-only', '--diff-filter=U')
        $disposition = Get-MergeFailureDisposition -MergeResult $merge -ConflictResult $conflicts
        if ($disposition.Kind -eq 'error') {
            & $cleanupWorktree
            Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
            Set-ResultAndExit -Code $disposition.ExitCode -Message $disposition.Message -Action $disposition.Action
        }
        $files = @($disposition.Files)
        $script:Result.conflict_files = $files
        Write-Err $disposition.Message
        if ($disposition.ShouldAbort) {
            $abort = Invoke-Git -WorkDir $worktreePath -Args @('merge', '--abort')
            if ($abort.Code -ne 0) {
                Write-Err "git merge --abort failed: $($abort.Text)"
            }
        }
        & $cleanupWorktree
        # Keep the sync branch deleted so a failed conflict does not leave a half branch
        Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
        Set-ResultAndExit -Code $disposition.ExitCode -Message "Merge conflict syncing $upstreamTag (aborted). Files: $($files -join ', ')" -Action $disposition.Action
    }

    Restore-ProtectedWorkflowTree -Root $worktreePath -TrustedRef $mainSha

    Write-Info "Setting version $chimeraVersion and bumping macos_build_number..."
    Set-WorkspaceChimeraVersion -Root $worktreePath -Version $chimeraVersion

    Write-Info 'Refreshing and validating dependency lockfiles before staging...'
    try {
        Update-AndValidateDependencyLocks -Root $worktreePath -Version $chimeraVersion
    }
    catch {
        & $cleanupWorktree
        Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
        Set-ResultAndExit -Code 3 -Message "Dependency lock refresh failure: $_" -Action 'gate_failed'
    }

    Write-Info 'Generating branded files before committing the candidate tree...'
    try {
        Push-Location $worktreePath
        try {
            & pwsh -NoProfile -File (Join-Path $worktreePath 'scripts\generate-branding.ps1')
            if ($LASTEXITCODE -ne 0) { throw "generate-branding.ps1 failed (exit $LASTEXITCODE)" }
        }
        finally {
            Pop-Location
        }
    }
    catch {
        & $cleanupWorktree
        Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
        Set-ResultAndExit -Code 3 -Message "Branding generation failure: $_" -Action 'gate_failed'
    }

    $add = Invoke-Git -WorkDir $worktreePath -Args @('add', '-A')
    Require-GitOk -Result $add -Context 'git add'
    $commitMsg = @"
chore: sync upstream $upstreamTag as $chimeraVersion

Upstream: $upstreamTag ($tagSha)
Sync branch: $syncBranch
Gates: branding, lock validation, no-promo scan, cargo fmt --check, cargo test --locked
"@
    $commitArgs = Get-IdentifiedGitArgs -Arguments @('commit', '-m', $commitMsg)
    $commit = Invoke-Git -WorkDir $worktreePath -Args $commitArgs
    if ($commit.Code -ne 0) {
        # Possibly nothing to commit if merge already brought identical tree + version
        $porcelain = Invoke-Git -WorkDir $worktreePath -Args @('status', '--porcelain')
        if (-not [string]::IsNullOrWhiteSpace($porcelain.Text)) {
            & $cleanupWorktree
            Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
            Set-ResultAndExit -Code 4 -Message "git commit failed: $($commit.Text)" -Action 'error'
        }
        Write-Info 'Nothing extra to commit after merge/version bump'
    }

    $beforeGates = Invoke-Git -WorkDir $worktreePath -Args @('status', '--porcelain')
    Require-GitOk -Result $beforeGates -Context 'check committed sync candidate'
    if (-not [string]::IsNullOrWhiteSpace($beforeGates.Text)) {
        & $cleanupWorktree
        Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
        Set-ResultAndExit -Code 4 -Message "Sync candidate is dirty before gates: $($beforeGates.Text)" -Action 'error'
    }

    $candidateHead = Invoke-Git -WorkDir $worktreePath -Args @('rev-parse', 'HEAD')
    Require-GitOk -Result $candidateHead -Context 'resolve committed sync candidate'
    $candidateSha = $candidateHead.Text.Trim()
    if ($candidateSha -notmatch '^[0-9a-f]{40}$') {
        & $cleanupWorktree
        Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
        Set-ResultAndExit -Code 4 -Message "Sync candidate produced unsafe SHA: $candidateSha" -Action 'error'
    }
    Assert-ProtectedWorkflowTree -Root $worktreePath -TrustedRef $mainSha -CandidateRef 'HEAD'

    if (-not $SkipGates) {
        try {
            Invoke-Gates -Root $worktreePath
            $afterGates = Invoke-Git -WorkDir $worktreePath -Args @('status', '--porcelain')
            if ($afterGates.Code -ne 0) {
                throw "git status after gates failed (exit $($afterGates.Code)): $($afterGates.Text)"
            }
            if (-not [string]::IsNullOrWhiteSpace($afterGates.Text)) {
                throw "gates changed committed sync candidate: $($afterGates.Text)"
            }
            $afterGateHead = Invoke-Git -WorkDir $worktreePath -Args @('rev-parse', 'HEAD')
            if ($afterGateHead.Code -ne 0) {
                throw "failed to re-resolve sync candidate HEAD: $($afterGateHead.Text)"
            }
            $afterGateSha = $afterGateHead.Text.Trim()
            if ($afterGateSha -ne $candidateSha) {
                throw "gates changed sync candidate HEAD: expected $candidateSha, got $afterGateSha"
            }
        }
        catch {
            & $cleanupWorktree
            Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
            Set-ResultAndExit -Code 3 -Message "Gate failure: $_" -Action 'gate_failed'
        }
    }
    else {
        Write-Info 'SkipGates: full ads/format/test gates were skipped by explicit request'
    }

    $gatedSha = $candidateSha
    $script:Result.gated_sha = $gatedSha

    Write-Info "Sync branch ready: $syncBranch ($gatedSha)"
    Write-Info 'Next: workflow pushes branch, opens PR, enables auto-merge (script will not push or create Release).'
}
catch {
    & $cleanupWorktree
    Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
    Set-ResultAndExit -Code 4 -Message "Sync apply failed: $_" -Action 'error'
}

# Remove worktree but keep branch ref for the workflow to push
& $cleanupWorktree

Set-ResultAndExit -Code 0 -Message "Prepared $syncBranch for $upstreamTag as $chimeraVersion" -Action 'prepared'
