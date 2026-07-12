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

Write-Host 'sync-upstream contract tests passed'
