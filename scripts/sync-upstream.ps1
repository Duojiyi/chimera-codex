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
    [string]$ResultPath = ''
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
    $output = & git @argList 2>&1
    $code = $LASTEXITCODE
    $text = ($output | ForEach-Object { "$_" }) -join "`n"
    return [pscustomobject]@{ Code = $code; Text = $text; Lines = @($output | ForEach-Object { "$_" }) }
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

function Get-LatestFormalUpstreamRelease {
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

    $uri = "https://api.github.com/repos/$($script:UpstreamOwnerRepo)/releases?per_page=30"
    try {
        $releases = Invoke-RestMethod -Uri $uri -Headers $headers -Method Get
    }
    catch {
        Set-ResultAndExit -Code 4 -Message "Failed to query upstream releases: $_" -Action 'error'
    }

    foreach ($rel in $releases) {
        if ($rel.draft -eq $true) { continue }
        if ($rel.prerelease -eq $true) { continue }
        $tag = [string]$rel.tag_name
        if ($tag -notmatch '^v?\d+\.\d+\.\d+$') { continue }
        $sha = $null
        # Prefer target commit via ls-remote when available
        return [pscustomobject]@{
            Tag         = $tag
            Name        = [string]$rel.name
            Targetish   = [string]$rel.target_commitish
            HtmlUrl     = [string]$rel.html_url
            PublishedAt = [string]$rel.published_at
        }
    }
    Set-ResultAndExit -Code 4 -Message 'No formal (non-draft/non-prerelease) upstream Release found' -Action 'error'
}

function Normalize-UpstreamVersion([string]$Tag) {
    $t = $Tag.Trim()
    if ($t.StartsWith('v') -or $t.StartsWith('V')) {
        $t = $t.Substring(1)
    }
    if ($t -notmatch '^\d+\.\d+\.\d+$') {
        Set-ResultAndExit -Code 4 -Message "Upstream tag is not X.Y.Z: $Tag" -Action 'error'
    }
    return $t
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
            Reason  = "origin already has tag $chimeraTag ($tagSha)"
        }
    }
    $branchSha = Get-LsRemoteRef -Remote 'origin' -Ref "refs/heads/$SyncBranch"
    if ($branchSha) {
        return [pscustomobject]@{
            Synced  = $true
            Reason  = "origin already has branch $SyncBranch ($branchSha)"
        }
    }
    $localVer = Get-WorkspaceCargoVersion -Root $Root
    if ($localVer -and $localVer -match "^$([regex]::Escape($Version))-chimera\.\d+$") {
        return [pscustomobject]@{
            Synced  = $true
            Reason  = "workspace already at upstream $Version ($localVer)"
        }
    }
    return [pscustomobject]@{ Synced = $false; Reason = '' }
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

        Write-Info 'Gate: cargo test --workspace'
        & cargo test --workspace
        if ($LASTEXITCODE -ne 0) { throw "cargo test --workspace failed (exit $LASTEXITCODE)" }
    }
    finally {
        Pop-Location
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

$release = Get-LatestFormalUpstreamRelease
$version = Normalize-UpstreamVersion -Tag $release.Tag
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
Write-Info "upstream sha:     $(if ($upstreamShaHint) { $upstreamShaHint } else { '(ls-remote miss; will fetch on apply)' })"
Write-Info "sync branch:      $syncBranch"
Write-Info "chimera version:  $chimeraVersion"
Write-Info 'gates:            generate-branding (+Check), verify-no-upstream-ads, cargo fmt --check, cargo test'
Write-Info 'forbidden:        modify main, create Release, push (workflow owns push/PR/Issue)'

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
    Write-Info "Merging $upstreamTag into $syncBranch..."
    $merge = Invoke-Git -WorkDir $worktreePath -Args @('merge', '--no-ff', '--no-edit', $upstreamTag)
    if ($merge.Code -ne 0) {
        $conflicts = Invoke-Git -WorkDir $worktreePath -Args @('diff', '--name-only', '--diff-filter=U')
        $files = @($conflicts.Lines | Where-Object { $_ -match '\S' })
        $script:Result.conflict_files = $files
        Write-Err "Merge conflict in: $($files -join ', ')"
        $abort = Invoke-Git -WorkDir $worktreePath -Args @('merge', '--abort')
        if ($abort.Code -ne 0) {
            Write-Err "git merge --abort failed: $($abort.Text)"
        }
        & $cleanupWorktree
        # Keep the sync branch deleted so a failed conflict does not leave a half branch
        Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
        Set-ResultAndExit -Code 2 -Message "Merge conflict syncing $upstreamTag (aborted). Files: $($files -join ', ')" -Action 'conflict'
    }

    Write-Info "Setting version $chimeraVersion and bumping macos_build_number..."
    Set-WorkspaceChimeraVersion -Root $worktreePath -Version $chimeraVersion

    if (-not $SkipGates) {
        try {
            Invoke-Gates -Root $worktreePath
        }
        catch {
            & $cleanupWorktree
            Invoke-Git -Args @('branch', '-D', $syncBranch) | Out-Null
            Set-ResultAndExit -Code 3 -Message "Gate failure: $_" -Action 'gate_failed'
        }
    }
    else {
        Write-Info 'SkipGates: running generate-branding only (no cargo/ads gates)'
        Push-Location $worktreePath
        try {
            & pwsh -NoProfile -File (Join-Path $worktreePath 'scripts\generate-branding.ps1')
            if ($LASTEXITCODE -ne 0) { throw "generate-branding.ps1 failed (exit $LASTEXITCODE)" }
        }
        finally {
            Pop-Location
        }
    }

    $add = Invoke-Git -WorkDir $worktreePath -Args @('add', '-A')
    Require-GitOk -Result $add -Context 'git add'
    $commitMsg = @"
chore: sync upstream $upstreamTag as $chimeraVersion

Upstream: $upstreamTag ($tagSha)
Sync branch: $syncBranch
Gates: branding, no-promo scan, cargo fmt --check, cargo test
"@
    $commit = Invoke-Git -WorkDir $worktreePath -Args @('commit', '-m', $commitMsg)
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

    Write-Info "Sync branch ready: $syncBranch"
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
