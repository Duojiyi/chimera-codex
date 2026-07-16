#Requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$syncScript = Join-Path $repoRoot 'scripts\sync-upstream.ps1'

# Import functions without running remotes, API calls, fetches, worktrees, or commits.
. $syncScript -SkipMain

function Assert-Equal {
    param(
        [Parameter(Mandatory)]$Actual,
        [Parameter(Mandatory)]$Expected,
        [Parameter(Mandatory)][string]$Context
    )
    if ($Actual -ne $Expected) {
        throw "${Context}: expected '$Expected', got '$Actual'"
    }
}

function Assert-RequestedReleaseError {
    param(
        [Parameter(Mandatory)][string]$Tag,
        [Parameter(Mandatory)][scriptblock]$RequestPage,
        [Parameter(Mandatory)][string]$Context
    )
    $fail = {
        param([int]$Code, [string]$Message, [string]$Action)
        throw "SYNC_FAILURE|$Code|$Action|$Message"
    }
    try {
        Get-LatestFormalUpstreamRelease -RequestedTag $Tag -RequestPage $RequestPage -Fail $fail | Out-Null
        throw "${Context}: selection unexpectedly succeeded"
    }
    catch {
        if ([string]$_.Exception.Message -notmatch '^SYNC_FAILURE\|4\|error\|') {
            throw "${Context}: expected code 4/action error, got $($_.Exception.Message)"
        }
    }
}

function New-ReleaseFixture {
    param(
        [Parameter(Mandatory)][string]$Tag,
        [bool]$Draft = $false,
        [bool]$Prerelease = $false
    )
    return [pscustomobject]@{
        tag_name         = $Tag
        name             = $Tag
        draft            = $Draft
        prerelease       = $Prerelease
        target_commitish = 'main'
        html_url         = "https://example.invalid/releases/$Tag"
        published_at     = '2026-01-01T00:00:00Z'
    }
}

Assert-Equal (Compare-FormalSemVer -Left '1.10.0' -Right '1.9.99') 1 'numeric SemVer comparison'
Assert-Equal (Compare-FormalSemVer -Left '2.0.0' -Right '2.0.0') 0 'equal SemVer comparison'
Assert-Equal (Compare-FormalSemVer -Left '2.0.0' -Right '10.0.0') -1 'major SemVer comparison'
Assert-Equal (Get-FormalBaselineVersion -WorkspaceVersion '3.4.5-chimera.7') '3.4.5' 'baseline extraction'

$unordered = @(
    (New-ReleaseFixture -Tag 'v2.9.99'),
    (New-ReleaseFixture -Tag 'v99.0.0' -Draft $true),
    (New-ReleaseFixture -Tag 'v100.0.0' -Prerelease $true),
    (New-ReleaseFixture -Tag 'not-semver'),
    (New-ReleaseFixture -Tag 'v2.10.0')
)
$selected = Select-LatestFormalRelease -Releases $unordered
Assert-Equal $selected.Tag 'v2.10.0' 'maximum formal SemVer selection'

$nonEnumeratedRequest = {
    param([string]$Uri, [hashtable]$Headers)
    $response = @(
        (New-ReleaseFixture -Tag 'v3.9.0'),
        (New-ReleaseFixture -Tag 'v3.10.0')
    )
    Write-Output -NoEnumerate $response
}
$nonEnumerated = Get-LatestFormalUpstreamRelease -RequestPage $nonEnumeratedRequest
Assert-Equal $nonEnumerated.Tag 'v3.10.0' 'Invoke-RestMethod array response expansion'

$requestedRelease = Get-LatestFormalUpstreamRelease -RequestedTag 'v3.9.0' -RequestPage $nonEnumeratedRequest
Assert-Equal $requestedRelease.Tag 'v3.9.0' 'manual validation selects the requested formal Release'
$emptyRequestedRelease = Get-LatestFormalUpstreamRelease -RequestedTag '' -RequestPage $nonEnumeratedRequest
Assert-Equal $emptyRequestedRelease.Tag 'v3.10.0' 'empty manual tag preserves latest Release selection'

$invalidRequestedRelease = {
    param([string]$Uri, [hashtable]$Headers)
    @(
        (New-ReleaseFixture -Tag 'v4.0.0' -Draft $true),
        (New-ReleaseFixture -Tag 'v4.1.0' -Prerelease $true),
        (New-ReleaseFixture -Tag 'not-semver')
    )
}
Assert-RequestedReleaseError -Tag 'v4.0.0' -RequestPage $invalidRequestedRelease -Context 'requested draft Release'
Assert-RequestedReleaseError -Tag 'v4.1.0' -RequestPage $invalidRequestedRelease -Context 'requested prerelease'
Assert-RequestedReleaseError -Tag 'not-semver' -RequestPage $invalidRequestedRelease -Context 'requested non-SemVer tag'
Assert-RequestedReleaseError -Tag 'v4.2.0' -RequestPage $invalidRequestedRelease -Context 'requested missing Release'

$pageOne = @(1..100 | ForEach-Object { New-ReleaseFixture -Tag 'v9.0.0' })
$pageTwo = @(
    (New-ReleaseFixture -Tag 'v10.0.0'),
    (New-ReleaseFixture -Tag 'v999.0.0' -Draft $true),
    (New-ReleaseFixture -Tag 'v1000.0.0' -Prerelease $true)
)
$script:requestedPages = @()
$requestPage = {
    param([string]$Uri, [hashtable]$Headers)
    if ($Uri -notmatch '[?&]page=(\d+)') {
        throw "fixture request missing page: $Uri"
    }
    $page = [int]$Matches[1]
    $script:requestedPages += $page
    switch ($page) {
        1 { return $pageOne }
        2 { return $pageTwo }
        default { throw "unexpected fixture page: $page" }
    }
}
$paged = Get-LatestFormalUpstreamRelease -RequestPage $requestPage
Assert-Equal $paged.Tag 'v10.0.0' 'maximum SemVer across API pages'
Assert-Equal ($script:requestedPages -join ',') '1,2' 'release API pagination'

Assert-Equal (
    Get-UpstreamVersionDisposition -CandidateVersion '2.9.99' -BaselineVersion '2.10.0'
) 'regression' 'reject downgrade'
Assert-Equal (
    Get-UpstreamVersionDisposition -CandidateVersion '2.10.0' -BaselineVersion '2.10.0'
) 'duplicate' 'reject duplicate'
Assert-Equal (
    Get-UpstreamVersionDisposition -CandidateVersion '2.10.1' -BaselineVersion '2.10.0'
) 'advance' 'accept advance'

$streamProbe = Invoke-Git -Args @(
    '-c',
    'alias.stream-probe=!f() { echo out; echo warning:advisory >&2; }; f',
    'stream-probe'
)
Assert-Equal $streamProbe.Code 0 'Git stream probe exit code'
Assert-Equal ($streamProbe.StdoutLines -join ',') 'out' 'Git stdout separation'
Assert-Equal ($streamProbe.Lines -join ',') 'out' 'Git compatibility Lines contain stdout only'
Assert-Equal ($streamProbe.StderrLines -join ',') 'warning:advisory' 'Git stderr separation'
if (-not ($streamProbe.Text.Contains('out') -and $streamProbe.Text.Contains('warning:advisory'))) {
    throw 'Git combined diagnostic text must preserve stdout and stderr'
}

$identifiedMergeArgs = Get-IdentifiedGitArgs -Arguments @('merge', '--no-ff', '--no-edit', 'v1.2.36')
Assert-Equal (
    $identifiedMergeArgs -join '|'
) '-c|user.name=github-actions[bot]|-c|user.email=41898282+github-actions[bot]@users.noreply.github.com|merge|--no-ff|--no-edit|v1.2.36' 'merge command identity'
$identifiedCommitArgs = Get-IdentifiedGitArgs -Arguments @('commit', '-m', 'sync: merge upstream v1.2.36')
Assert-Equal (
    $identifiedCommitArgs -join '|'
) '-c|user.name=github-actions[bot]|-c|user.email=41898282+github-actions[bot]@users.noreply.github.com|commit|-m|sync: merge upstream v1.2.36' 'candidate commit identity'

$baselineCommit = (Invoke-Git -Args @(
    'log', 'HEAD', '--format=%H', '--grep=^release: v1.2.36$', '-1'
)).Text.Trim()
if ($baselineCommit -notmatch '^[0-9a-f]{40}$') {
    throw 'test fixture must resolve the upstream v1.2.36 release commit from candidate history'
}

$squashedBaseline = Get-UpstreamBaselineAncestryDisposition `
    -Root $repoRoot `
    -BaselineTag $baselineCommit `
    -CandidateRef 'origin/main'
Assert-Equal $squashedBaseline 'stitch' 'squash-merged main requires baseline ancestry stitch'

$upstreamBaseline = Get-UpstreamBaselineAncestryDisposition `
    -Root $repoRoot `
    -BaselineTag $baselineCommit `
    -CandidateRef 'HEAD'
Assert-Equal $upstreamBaseline 'present' 'upstream descendant already contains baseline ancestry'

$mergeConflict = Get-MergeFailureDisposition `
    -MergeResult ([pscustomobject]@{ Code = 1; Text = 'Automatic merge failed'; Lines = @(); StdoutLines = @(); StderrLines = @() }) `
    -ConflictResult ([pscustomobject]@{ Code = 0; Text = 'Cargo.toml'; Lines = @('Cargo.toml'); StdoutLines = @('Cargo.toml'); StderrLines = @() })
Assert-Equal $mergeConflict.Kind 'conflict' 'merge failure with unmerged paths'
Assert-Equal ($mergeConflict.Files -join ',') 'Cargo.toml' 'merge conflict path preservation'
Assert-Equal $mergeConflict.ExitCode 2 'merge conflict exit code'
Assert-Equal $mergeConflict.Action 'conflict' 'merge conflict action'
Assert-Equal $mergeConflict.ShouldAbort $true 'merge conflict abort requirement'

$mergeExecutionError = Get-MergeFailureDisposition `
    -MergeResult ([pscustomobject]@{ Code = 128; Text = 'Committer identity unknown'; Lines = @(); StdoutLines = @(); StderrLines = @('Committer identity unknown') }) `
    -ConflictResult ([pscustomobject]@{ Code = 0; Text = ''; Lines = @(); StdoutLines = @(); StderrLines = @() })
Assert-Equal $mergeExecutionError.Kind 'error' 'merge failure without unmerged paths'
Assert-Equal $mergeExecutionError.ExitCode 4 'merge execution error exit code'
Assert-Equal $mergeExecutionError.Action 'error' 'merge execution error action'
Assert-Equal $mergeExecutionError.ShouldAbort $false 'merge execution error abort requirement'
if (-not $mergeExecutionError.Message.Contains('Committer identity unknown')) {
    throw 'merge execution error must retain the original git merge diagnostic'
}

$mergeProbeError = Get-MergeFailureDisposition `
    -MergeResult ([pscustomobject]@{ Code = 1; Text = 'merge failed'; Lines = @(); StdoutLines = @(); StderrLines = @('merge failed') }) `
    -ConflictResult ([pscustomobject]@{ Code = 128; Text = 'index unavailable'; Lines = @(); StdoutLines = @(); StderrLines = @('index unavailable') })
Assert-Equal $mergeProbeError.Kind 'error' 'failed unmerged-path probe'
if (-not $mergeProbeError.Message.Contains('index unavailable')) {
    throw 'merge probe error must retain the conflict-probe diagnostic'
}

$warningOnlyProbe = Get-MergeFailureDisposition `
    -MergeResult ([pscustomobject]@{ Code = 128; Text = 'merge failed'; Lines = @(); StdoutLines = @(); StderrLines = @('merge failed') }) `
    -ConflictResult ([pscustomobject]@{ Code = 0; Text = 'warning: advisory'; Lines = @('warning: advisory'); StdoutLines = @(); StderrLines = @('warning: advisory') })
Assert-Equal $warningOnlyProbe.Kind 'error' 'warning-only conflict probe'

function Test-BuiltInTokenWorkflowContract {
    param([Parameter(Mandatory)][string]$Workflow)

    $text = $Workflow.Replace("`r`n", "`n").Replace("`r", "`n")
    if ($text.Contains('CHIMERA_AUTOMATION_TOKEN')) { return $false }
    $match = [regex]::Match(
        $text,
        '(?ms)^  publish-sync-pr:\s*$\n(?<job>.*?)(?=^  report-blocked:\s*$)'
    )
    if (-not $match.Success) { return $false }
    $job = $match.Groups['job'].Value
    $permissionBlock = @"
    permissions:
      contents: write
      pull-requests: write
      actions: write
      issues: write
"@.Replace("`r`n", "`n").TrimEnd()

    $verify = '          git diff --quiet $trustedMainSha "refs/heads/$branch" -- .github/workflows'
    $push = '            git push -u origin "refs/heads/${branch}:refs/heads/${branch}"'
    $dispatch = '          gh workflow run pr-build.yml --ref $branch'
    $autoMerge = '          gh pr merge $pr --auto --squash'
    $verifyIndex = $job.IndexOf($verify, [System.StringComparison]::Ordinal)
    $pushIndex = $job.IndexOf($push, [System.StringComparison]::Ordinal)
    $dispatchIndex = $job.IndexOf($dispatch, [System.StringComparison]::Ordinal)
    $autoMergeIndex = $job.IndexOf($autoMerge, [System.StringComparison]::Ordinal)

    return $job.Contains($permissionBlock) -and
        $job -match '(?m)^          GH_TOKEN: \$\{\{ github\.token \}\}\s*$' -and
        $job -match '(?m)^          TRUSTED_MAIN_SHA: \$\{\{ github\.sha \}\}\s*$' -and
        $verifyIndex -ge 0 -and
        $verifyIndex -lt $pushIndex -and
        $pushIndex -lt $dispatchIndex -and
        $dispatchIndex -lt $autoMergeIndex
}

$syncWorkflowPath = Join-Path $repoRoot '.github\workflows\sync-upstream.yml'
$syncWorkflow = Get-Content -LiteralPath $syncWorkflowPath -Raw
if (-not (Test-BuiltInTokenWorkflowContract -Workflow $syncWorkflow)) {
    throw 'sync workflow must use the short-lived GITHUB_TOKEN and explicitly dispatch PR checks'
}

$normalizedWorkflow = $syncWorkflow.Replace("`r`n", "`n").Replace("`r", "`n")

function Test-TwoHourlyScheduleContract {
    param([Parameter(Mandatory)][string]$Workflow)

    $text = $Workflow.Replace("`r`n", "`n").Replace("`r", "`n")
    $match = [regex]::Match(
        $text,
        '(?ms)^on:\s*$\n  schedule:\s*$\n(?<schedule>.*?)(?=^  workflow_dispatch:)'
    )
    if (-not $match.Success) { return $false }
    [string[]]$activeLines = @(
        $match.Groups['schedule'].Value -split "`n" |
            ForEach-Object { $_.TrimEnd() } |
            Where-Object { $_.Trim().Length -gt 0 -and -not $_.TrimStart().StartsWith('#') }
    )
    return $activeLines.Count -eq 1 -and
        $activeLines[0] -ceq '    - cron: "23 */2 * * *"'
}

if (-not (Test-TwoHourlyScheduleContract -Workflow $normalizedWorkflow)) {
    throw 'sync workflow must poll every two hours at minute 23 UTC'
}
$slowScheduleMutation = $normalizedWorkflow.Replace('23 */2 * * *', '0 6 * * *')
if (Test-TwoHourlyScheduleContract -Workflow $slowScheduleMutation) {
    throw 'twice-daily schedule mutation must fail the workflow contract'
}
$extraScheduleMutation = $normalizedWorkflow.Replace(
    '    - cron: "23 */2 * * *"',
    "    - cron: `"23 */2 * * *`"`n    - cron: `"0 18 * * *`""
)
if (Test-TwoHourlyScheduleContract -Workflow $extraScheduleMutation) {
    throw 'extra schedule mutation must fail the workflow contract'
}
$singleQuotedExtraScheduleMutation = $normalizedWorkflow.Replace(
    '    - cron: "23 */2 * * *"',
    "    - cron: `"23 */2 * * *`"`n    - cron: '0 18 * * *'"
)
if (Test-TwoHourlyScheduleContract -Workflow $singleQuotedExtraScheduleMutation) {
    throw 'single-quoted extra schedule mutation must fail the workflow contract'
}
$unquotedExtraScheduleMutation = $normalizedWorkflow.Replace(
    '    - cron: "23 */2 * * *"',
    "    - cron: `"23 */2 * * *`"`n    - cron: 0 18 * * *"
)
if (Test-TwoHourlyScheduleContract -Workflow $unquotedExtraScheduleMutation) {
    throw 'unquoted extra schedule mutation must fail the workflow contract'
}
$quotedKeyExtraScheduleMutation = $normalizedWorkflow.Replace(
    '    - cron: "23 */2 * * *"',
    "    - cron: `"23 */2 * * *`"`n    - 'cron': '0 18 * * *'"
)
if (Test-TwoHourlyScheduleContract -Workflow $quotedKeyExtraScheduleMutation) {
    throw 'quoted-key extra schedule mutation must fail the workflow contract'
}
$flowMapExtraScheduleMutation = $normalizedWorkflow.Replace(
    '    - cron: "23 */2 * * *"',
    "    - cron: `"23 */2 * * *`"`n    - { cron: `"0 18 * * *`" }"
)
if (Test-TwoHourlyScheduleContract -Workflow $flowMapExtraScheduleMutation) {
    throw 'flow-map extra schedule mutation must fail the workflow contract'
}

$secretMutation = $normalizedWorkflow.Replace('${{ github.token }}', '${{ secrets.CHIMERA_AUTOMATION_TOKEN }}')
if (Test-BuiltInTokenWorkflowContract -Workflow $secretMutation) {
    throw 'external-token mutation must fail the workflow contract'
}
$permissionMutation = $normalizedWorkflow.Replace("      actions: write`n", '')
if (Test-BuiltInTokenWorkflowContract -Workflow $permissionMutation) {
    throw 'missing Actions permission mutation must fail the workflow contract'
}
$crlfPermissionMutation = $normalizedWorkflow.Replace("`n", "`r`n").Replace(
    "      actions: write`r`n",
    ''
)
if (Test-BuiltInTokenWorkflowContract -Workflow $crlfPermissionMutation) {
    throw 'CRLF missing Actions permission mutation must fail the workflow contract'
}
$dispatchMutation = $normalizedWorkflow.Replace(
    '          gh workflow run pr-build.yml --ref $branch',
    '          # gh workflow run pr-build.yml --ref $branch'
)
if (Test-BuiltInTokenWorkflowContract -Workflow $dispatchMutation) {
    throw 'commented dispatch mutation must fail the workflow contract'
}
$verifyLine = '          git diff --quiet $trustedMainSha "refs/heads/$branch" -- .github/workflows'
$pushLine = '            git push -u origin "refs/heads/${branch}:refs/heads/${branch}"'
$postPushVerificationMutation = $normalizedWorkflow.Replace("$verifyLine`n", '').Replace(
    "$pushLine`n",
    "$pushLine`n$verifyLine`n"
)
if (Test-BuiltInTokenWorkflowContract -Workflow $postPushVerificationMutation) {
    throw 'post-push workflow verification mutation must fail the workflow contract'
}

function Test-ProtectedScriptContract {
    param([Parameter(Mandatory)][string]$Source)

    $text = $Source.Replace("`r`n", "`n").Replace("`r", "`n")
    return $text.Contains('function Restore-ProtectedWorkflowTree') -and
        $text.Contains("@('restore', `"--source=`$TrustedRef`", '--staged', '--worktree', '--', '.github/workflows')") -and
        $text.Contains('Assert-ProtectedWorkflowTree -Root $Root -TrustedRef $TrustedRef -Cached') -and
        $text.Contains("Assert-ProtectedWorkflowTree -Root `$Root -TrustedRef 'origin/main' -CandidateRef `$remoteRef") -and
        $text.Contains('Restore-ProtectedWorkflowTree -Root $worktreePath -TrustedRef $mainSha') -and
        $text.Contains("Assert-ProtectedWorkflowTree -Root `$worktreePath -TrustedRef `$mainSha -CandidateRef 'HEAD'")
}

$syncScriptSource = (Get-Content -LiteralPath $syncScript -Raw).Replace("`r`n", "`n").Replace("`r", "`n")

function Test-MergeExecutionSourceContract {
    param([Parameter(Mandatory)][string]$Source)

    $text = $Source.Replace("`r`n", "`n").Replace("`r", "`n")
    return $text.Contains("`$mergeArgs = Get-IdentifiedGitArgs -Arguments @('merge', '--no-ff', '--no-edit', `$upstreamTag)") -and
        $text.Contains('$merge = Invoke-Git -WorkDir $worktreePath -Args $mergeArgs') -and
        $text.Contains("`$commitArgs = Get-IdentifiedGitArgs -Arguments @('commit', '-m', `$commitMsg)") -and
        $text.Contains('$commit = Invoke-Git -WorkDir $worktreePath -Args $commitArgs') -and
        $text.Contains('$files = @($ConflictResult.StdoutLines | Where-Object') -and
        $text.Contains('if ($disposition.ShouldAbort)') -and
        $text.Contains('Set-ResultAndExit -Code $disposition.ExitCode -Message $disposition.Message -Action $disposition.Action') -and
        $text.Contains('Set-ResultAndExit -Code $disposition.ExitCode -Message "Merge conflict syncing $upstreamTag (aborted). Files: $($files -join '', '')" -Action $disposition.Action')
}

function Test-BaselineAncestrySourceContract {
    param([Parameter(Mandatory)][string]$Source)

    $text = $Source.Replace("`r`n", "`n").Replace("`r", "`n")
    $fetch = '$baselineFetch = Invoke-Git -Args @('
    $stitch = '    Ensure-UpstreamBaselineAncestry -Root $worktreePath -BaselineTag $baselineTag'
    $candidate = '    Write-Info "Merging $upstreamTag into $syncBranch..."'
    $fetchIndex = $text.IndexOf($fetch, [System.StringComparison]::Ordinal)
    $stitchIndex = $text.IndexOf($stitch, [System.StringComparison]::Ordinal)
    $candidateIndex = $text.IndexOf($candidate, [System.StringComparison]::Ordinal)

    return $text.Contains('function Get-UpstreamBaselineAncestryDisposition') -and
        $text.Contains('function Ensure-UpstreamBaselineAncestry') -and
        $text.Contains("'merge', '--no-ff', '--no-edit', '-s', 'ours', `$BaselineTag") -and
        $fetchIndex -ge 0 -and
        $fetchIndex -lt $stitchIndex -and
        $stitchIndex -lt $candidateIndex
}

if (-not (Test-MergeExecutionSourceContract -Source $syncScriptSource)) {
    throw 'sync merge execution must use identity, separated stdout, and disposition code/action wiring'
}
$mergeIdentityMutation = $syncScriptSource.Replace(
    '$merge = Invoke-Git -WorkDir $worktreePath -Args $mergeArgs',
    '$merge = Invoke-Git -WorkDir $worktreePath -Args @(''merge'', ''--no-ff'', ''--no-edit'', $upstreamTag)'
)
if (Test-MergeExecutionSourceContract -Source $mergeIdentityMutation) {
    throw 'merge identity callsite mutation must fail the script contract'
}
$commitIdentityMutation = $syncScriptSource.Replace(
    '$commit = Invoke-Git -WorkDir $worktreePath -Args $commitArgs',
    '$commit = Invoke-Git -WorkDir $worktreePath -Args @(''commit'', ''-m'', $commitMsg)'
)
if (Test-MergeExecutionSourceContract -Source $commitIdentityMutation) {
    throw 'commit identity callsite mutation must fail the script contract'
}
$errorActionMutation = $syncScriptSource.Replace(
    'Set-ResultAndExit -Code $disposition.ExitCode -Message $disposition.Message -Action $disposition.Action',
    'Set-ResultAndExit -Code 2 -Message $disposition.Message -Action ''conflict'''
)
if (Test-MergeExecutionSourceContract -Source $errorActionMutation) {
    throw 'merge execution error mapping mutation must fail the script contract'
}

if (-not (Test-BaselineAncestrySourceContract -Source $syncScriptSource)) {
    throw 'sync candidate construction must stitch the fetched upstream baseline before candidate merge'
}
$missingStitchMutation = $syncScriptSource.Replace(
    '    Ensure-UpstreamBaselineAncestry -Root $worktreePath -BaselineTag $baselineTag',
    '    # baseline ancestry stitch removed'
)
if (Test-BaselineAncestrySourceContract -Source $missingStitchMutation) {
    throw 'missing baseline ancestry stitch mutation must fail the script contract'
}
$lateStitchMutation = $syncScriptSource.Replace(
    "    Ensure-UpstreamBaselineAncestry -Root `$worktreePath -BaselineTag `$baselineTag`n`n    Write-Info `"Merging `$upstreamTag into `$syncBranch...`"",
    "    Write-Info `"Merging `$upstreamTag into `$syncBranch...`"`n    Ensure-UpstreamBaselineAncestry -Root `$worktreePath -BaselineTag `$baselineTag"
)
if (Test-BaselineAncestrySourceContract -Source $lateStitchMutation) {
    throw 'post-candidate baseline ancestry stitch mutation must fail the script contract'
}

if (-not (Test-ProtectedScriptContract -Source $syncScriptSource)) {
    throw 'sync candidate construction must restore and verify the trusted workflow tree'
}
$scriptMutation = $syncScriptSource.Replace("'.github/workflows'", "'.github/untrusted-workflows'")
if (Test-ProtectedScriptContract -Source $scriptMutation) {
    throw 'protected workflow path mutation must fail the script contract'
}
$crlfScriptMutation = $syncScriptSource.Replace("`n", "`r`n").Replace(
    "'.github/workflows'",
    "'.github/untrusted-workflows'"
)
if (Test-ProtectedScriptContract -Source $crlfScriptMutation) {
    throw 'CRLF protected workflow path mutation must fail the script contract'
}
$resumeMutation = $syncScriptSource.Replace(
    "    Assert-ProtectedWorkflowTree -Root `$Root -TrustedRef 'origin/main' -CandidateRef `$remoteRef`n",
    ''
)
if (Test-ProtectedScriptContract -Source $resumeMutation) {
    throw 'missing resume workflow-tree verification mutation must fail the script contract'
}
$candidateMutation = $syncScriptSource.Replace(
    "    Assert-ProtectedWorkflowTree -Root `$worktreePath -TrustedRef `$mainSha -CandidateRef 'HEAD'`n",
    ''
)
if (Test-ProtectedScriptContract -Source $candidateMutation) {
    throw 'missing committed candidate verification mutation must fail the script contract'
}
$cachedMutation = $syncScriptSource.Replace(
    '    Assert-ProtectedWorkflowTree -Root $Root -TrustedRef $TrustedRef -Cached',
    '    Assert-ProtectedWorkflowTree -Root $Root -TrustedRef $TrustedRef'
)
if (Test-ProtectedScriptContract -Source $cachedMutation) {
    throw 'missing cached-index verification mutation must fail the script contract'
}

function Invoke-FixtureGit {
    param([Parameter(Mandatory)][string[]]$Arguments)

    $output = & git -C $repoRoot @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "fixture git $($Arguments -join ' ') failed: $($output -join "`n")"
    }
    return ($output -join "`n").Trim()
}

function Assert-CachedWorkflowMutationRejected {
    param([Parameter(Mandatory)][string]$Context)

    $rejected = $false
    try {
        Assert-ProtectedWorkflowTree -Root $repoRoot -TrustedRef 'HEAD' -Cached
    }
    catch {
        $rejected = $true
    }
    if (-not $rejected) {
        throw "$Context must be rejected by the protected workflow tree check"
    }
}

$previousIndex = $env:GIT_INDEX_FILE
$fixtureIndex = Join-Path ([System.IO.Path]::GetTempPath()) (
    'chimera-workflow-index-' + [guid]::NewGuid().ToString('N')
)
try {
    $env:GIT_INDEX_FILE = $fixtureIndex
    $foreignBlob = Invoke-FixtureGit -Arguments @('rev-parse', 'HEAD:LICENSE')

    Invoke-FixtureGit -Arguments @('read-tree', 'HEAD') | Out-Null
    Invoke-FixtureGit -Arguments @(
        'update-index', '--add', '--cacheinfo', '100644', $foreignBlob,
        '.github/workflows/untrusted.yml'
    ) | Out-Null
    Assert-CachedWorkflowMutationRejected -Context 'added workflow'

    Invoke-FixtureGit -Arguments @('read-tree', 'HEAD') | Out-Null
    Invoke-FixtureGit -Arguments @(
        'update-index', '--cacheinfo', '100644', $foreignBlob,
        '.github/workflows/pr-build.yml'
    ) | Out-Null
    Assert-CachedWorkflowMutationRejected -Context 'modified workflow'

    Invoke-FixtureGit -Arguments @('read-tree', 'HEAD') | Out-Null
    Invoke-FixtureGit -Arguments @(
        'update-index', '--force-remove', '.github/workflows/pr-build.yml'
    ) | Out-Null
    Assert-CachedWorkflowMutationRejected -Context 'deleted workflow'

    Invoke-FixtureGit -Arguments @('read-tree', 'HEAD') | Out-Null
    Assert-ProtectedWorkflowTree -Root $repoRoot -TrustedRef 'HEAD' -Cached
}
finally {
    $env:GIT_INDEX_FILE = $previousIndex
    if (Test-Path -LiteralPath $fixtureIndex -PathType Leaf) {
        Remove-Item -LiteralPath $fixtureIndex -Force
    }
}

Write-Host 'sync-upstream contract tests passed'
