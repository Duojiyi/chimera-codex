#Requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$scanner = Join-Path $PSScriptRoot 'verify-no-upstream-ads.ps1'
$hostExecutable = (Get-Process -Id $PID).Path

$selfTestOutput = @(& $hostExecutable -NoProfile -File $scanner -SelfTest 2>&1)
if ($LASTEXITCODE -ne 0) {
    throw "verify allowlist self-test failed with exit code $LASTEXITCODE"
}
$selfTestText = $selfTestOutput -join "`n"
if (-not $selfTestText.Contains('docs/images fail-closed fixtures: OK')) {
    throw 'verify scanner self-test did not exercise the docs/images fail-closed fixtures'
}
if (-not $selfTestText.Contains('assets/images fail-closed fixtures: OK')) {
    throw 'verify scanner self-test did not exercise the assets/images fail-closed fixtures'
}
if (-not $selfTestText.Contains('customer surface fail-closed fixtures: OK')) {
    throw 'verify scanner self-test did not exercise recommendation, community, GitHub UI, and third-party icon fixtures'
}
$selfTestOutput | ForEach-Object { Write-Host $_ }

Write-Host 'verify allowlist and docs/images contract tests: PASS' -ForegroundColor Green
