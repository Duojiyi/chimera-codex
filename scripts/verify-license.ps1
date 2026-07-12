#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$requiredPaths = @(
    'LICENSE',
    'NOTICE',
    'Cargo.toml',
    'README.md',
    'README_EN.md',
    'scripts/installer/windows/CodexPlusPlus.nsi',
    'scripts/installer/macos/package-dmg.sh',
    'scripts/release-manifest.mjs',
    '.github/workflows/pr-build.yml',
    '.github/workflows/release-assets.yml'
)

function Get-RepositorySnapshot {
    $files = @{}
    foreach ($relativePath in $requiredPaths) {
        $path = Join-Path $root $relativePath
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            $files[$relativePath] = Get-Content -LiteralPath $path -Raw -Encoding UTF8
        }
    }
    return $files
}

function Test-LicenseSnapshot {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$Files,
        [switch]$CheckLicenseHash
    )

    $findings = New-Object System.Collections.Generic.List[string]

    function Add-Finding([string]$Message) {
        $findings.Add($Message) | Out-Null
    }

    function Read-Required([string]$RelativePath) {
        if (-not $Files.ContainsKey($RelativePath) -or [string]::IsNullOrEmpty([string]$Files[$RelativePath])) {
            Add-Finding "missing required file: $RelativePath"
            return ''
        }
        return [string]$Files[$RelativePath]
    }

    function Assert-Contains([string]$RelativePath, [string]$Text, [string[]]$Required) {
        foreach ($value in $Required) {
            if (-not $Text.Contains($value)) {
                Add-Finding "${RelativePath}: missing '$value'"
            }
        }
    }

    function Assert-ActiveLine([string]$RelativePath, [string]$Text, [string]$RequiredLine) {
        $matches = @($Text -split "`r?`n" | Where-Object { $_.Trim() -eq $RequiredLine })
        if ($matches.Count -eq 0) {
            Add-Finding "${RelativePath}: missing active line '$RequiredLine'"
        }
    }

    $license = Read-Required 'LICENSE'
    $notice = Read-Required 'NOTICE'
    $cargo = Read-Required 'Cargo.toml'
    $readmeZh = Read-Required 'README.md'
    $readmeEn = Read-Required 'README_EN.md'
    $windowsInstaller = Read-Required 'scripts/installer/windows/CodexPlusPlus.nsi'
    $macosPackager = Read-Required 'scripts/installer/macos/package-dmg.sh'
    $releaseManifest = Read-Required 'scripts/release-manifest.mjs'
    $prWorkflow = Read-Required '.github/workflows/pr-build.yml'
    $releaseWorkflow = Read-Required '.github/workflows/release-assets.yml'

    Assert-Contains 'LICENSE' $license @(
        'GNU AFFERO GENERAL PUBLIC LICENSE',
        'Version 3, 19 November 2007',
        'END OF TERMS AND CONDITIONS'
    )
    if ($CheckLicenseHash -and $license) {
        $canonicalLicense = $license
        $bomMarker = [string]([char]0xFEFF)
        if ($canonicalLicense.StartsWith($bomMarker, [StringComparison]::Ordinal)) {
            $canonicalLicense = $canonicalLicense.Substring(1)
        }
        $canonicalLicense = $canonicalLicense.Replace("`r`n", "`n").Replace("`r", "`n")
        $sha256 = [Security.Cryptography.SHA256]::Create()
        try {
            $licenseHash = [BitConverter]::ToString(
                $sha256.ComputeHash([Text.Encoding]::UTF8.GetBytes($canonicalLicense))
            ).Replace('-', '')
        }
        finally {
            $sha256.Dispose()
        }
        if ($licenseHash -ne '8486A10C4393CEE1C25392769DDD3B2D6C242D6EC7928E1414EFFF7DFB2F07EF') {
            Add-Finding "LICENSE SHA-256 mismatch: $licenseHash"
        }
    }

    Assert-Contains 'NOTICE' $notice @(
        'Chimera++',
        'BigPizzaV3/CodexPlusPlus',
        'v1.2.34',
        'a0506ae',
        '7f72aec',
        'MIT',
        'AGPL-3.0-only',
        'farion1231/cc-switch',
        'Copyright (C) 2026 BigPizzaV3',
        'Copyright (c) 2025 Jason Young',
        'MIT-covered upstream baseline: BigPizzaV3/CodexPlusPlus through a0506ae',
        'The MIT terms below apply independently to each work listed above.',
        'Permission is hereby granted, free of charge',
        'The above copyright notice and this permission notice shall be included',
        'THE SOFTWARE IS PROVIDED "AS IS"',
        'AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM'
    )

    if ($cargo -notmatch '(?m)^license\s*=\s*"AGPL-3\.0-only"\s*$') {
        Add-Finding 'Cargo.toml workspace license must be AGPL-3.0-only'
    }
    if ($cargo -match '(?m)^license\s*=\s*"MIT"\s*$') {
        Add-Finding 'Cargo.toml still declares MIT'
    }

    foreach ($entry in @(
        [pscustomobject]@{ Path = 'README.md'; Text = $readmeZh },
        [pscustomobject]@{ Path = 'README_EN.md'; Text = $readmeEn }
    )) {
        Assert-Contains $entry.Path $entry.Text @(
            'AGPL-3.0-only',
            '(LICENSE)',
            '(NOTICE)',
            'https://github.com/Duojiyi/chimera-codex'
        )
    }

    Assert-Contains 'scripts/installer/windows/CodexPlusPlus.nsi' $windowsInstaller @(
        'File "/oname=LICENSE.new" "${ROOT}\LICENSE"',
        'File "/oname=NOTICE.new" "${ROOT}\NOTICE"',
        'File "/oname=SOURCE_CODE.txt.new" "${ROOT}\SOURCE_CODE.txt"',
        'Rename "$INSTDIR\LICENSE.new" "$INSTDIR\LICENSE"',
        'Rename "$INSTDIR\NOTICE.new" "$INSTDIR\NOTICE"',
        'Rename "$INSTDIR\SOURCE_CODE.txt.new" "$INSTDIR\SOURCE_CODE.txt"',
        'Function un.onInit',
        '${SETUP_MUTEX_NAME}',
        'Delete "$INSTDIR\LICENSE"',
        'Delete "$INSTDIR\NOTICE"',
        'Delete "$INSTDIR\SOURCE_CODE.txt"'
    )
    Assert-Contains 'scripts/installer/macos/package-dmg.sh' $macosPackager @(
        'cp "$ROOT/LICENSE" "$STAGE/LICENSE"',
        'cp "$ROOT/NOTICE" "$STAGE/NOTICE"',
        'cp "$ROOT/SOURCE_CODE.txt" "$STAGE/SOURCE_CODE.txt"'
    )

    foreach ($entry in @(
        [pscustomobject]@{ Path = '.github/workflows/pr-build.yml'; Text = $prWorkflow },
        [pscustomobject]@{ Path = '.github/workflows/release-assets.yml'; Text = $releaseWorkflow }
    )) {
        Assert-Contains $entry.Path $entry.Text @(
            'Copy-Item LICENSE,NOTICE,SOURCE_CODE.txt dist/windows/app/',
            'cp LICENSE NOTICE SOURCE_CODE.txt "dist/macos/app-${{ matrix.arch }}/"',
            'test -f "dist/macos/stage/SOURCE_CODE.txt"'
        )
    }

    Assert-Contains '.github/workflows/pr-build.yml' $prWorkflow @(
        'https://github.com/$env:REPO/archive/$($env:TARGET_SHA).tar.gz',
        'https://github.com/${REPO}/archive/${TARGET_SHA}.tar.gz'
    )
    Assert-Contains '.github/workflows/release-assets.yml' $releaseWorkflow @(
        'https://github.com/Duojiyi/chimera-codex/releases/download/$env:TAG/$sourceAsset',
        'https://github.com/Duojiyi/chimera-codex/releases/download/${TAG}/${source_asset}',
        'Release commit: ${TARGET_SHA}',
        'test -f "dist/macos/stage/LICENSE"',
        'test -f "dist/macos/stage/NOTICE"',
        'test -f "dist/macos/stage/SOURCE_CODE.txt"',
        'git archive --format=tar --prefix="ChimeraPlusPlus-${VERSION}-source/" "$TARGET_SHA"',
        'git ls-tree -rz --name-only "$TARGET_SHA" | LC_ALL=C sort -z > /tmp/source-tree-expected.z',
        'tar -xzf "$source_asset" -C /tmp/source-tree-root',
        'find . -mindepth 1 ! -type d -print0',
        "sed -z 's#^\./##'",
        '| LC_ALL=C sort -z > /tmp/source-tree-actual.z',
        'cmp /tmp/source-tree-expected.z /tmp/source-tree-actual.z',
        'apps/codex-plus-manager/package-lock.json',
        'scripts/installer/windows/CodexPlusPlus.nsi',
        'scripts/installer/macos/package-dmg.sh',
        'ChimeraPlusPlus-${VERSION}-source.tar.gz',
        'node scripts/release-manifest.mjs --generate release-assets',
        'gh release upload "$TAG" "${upload_list[@]}" --repo "$REPO" --clobber'
    )
    Assert-Contains 'scripts/release-manifest.mjs' $releaseManifest @(
        'name !== `ChimeraPlusPlus-${resolved.version}-source.tar.gz`',
        'License: AGPL-3.0-only; third-party notices: NOTICE',
        'Corresponding source: ${baseUrl}/ChimeraPlusPlus-${resolved.version}-source.tar.gz'
    )
    Assert-ActiveLine '.github/workflows/release-assets.yml' $releaseWorkflow 'verify_draft_assets_content'

    return [pscustomobject]@{ Findings = $findings }
}

function Copy-Snapshot([hashtable]$Snapshot) {
    $copy = @{}
    foreach ($key in $Snapshot.Keys) {
        $copy[$key] = $Snapshot[$key]
    }
    return $copy
}

function Invoke-SelfTests([hashtable]$Snapshot) {
    $selfTestFailures = New-Object System.Collections.Generic.List[string]
    $baseline = Test-LicenseSnapshot -Files $Snapshot -CheckLicenseHash
    if ($baseline.Findings.Count -gt 0) {
        $selfTestFailures.Add("baseline fixture failed: $($baseline.Findings -join '; ')") | Out-Null
    }

    foreach ($lineEnding in @("`n", "`r`n", "`r")) {
        $fixture = Copy-Snapshot $Snapshot
        $fixture['LICENSE'] = ([string]$fixture['LICENSE']).Replace("`r`n", "`n").Replace("`r", "`n").Replace("`n", $lineEnding)
        $result = Test-LicenseSnapshot -Files $fixture -CheckLicenseHash
        if ($result.Findings.Count -gt 0) {
            $selfTestFailures.Add("canonical LICENSE hash rejected a supported line ending: $($result.Findings -join '; ')") | Out-Null
        }
    }

    $tamperedLicense = Copy-Snapshot $Snapshot
    $tamperedLicense['LICENSE'] = ([string]$tamperedLicense['LICENSE']) + "`nlicense-hash-tamper-fixture"
    $tamperedResult = Test-LicenseSnapshot -Files $tamperedLicense -CheckLicenseHash
    if (-not ($tamperedResult.Findings -match 'LICENSE SHA-256 mismatch')) {
        $selfTestFailures.Add('LICENSE content mutation did not fail the canonical hash') | Out-Null
    }

    $bomLicense = Copy-Snapshot $Snapshot
    $bomLicense['LICENSE'] = ([char]0xFEFF) + [string]$bomLicense['LICENSE']
    $bomResult = Test-LicenseSnapshot -Files $bomLicense -CheckLicenseHash
    if ($bomResult.Findings.Count -gt 0) {
        $selfTestFailures.Add("canonical LICENSE hash rejected a leading UTF-8 BOM: $($bomResult.Findings -join '; ')") | Out-Null
    }

    $embeddedBomLicense = Copy-Snapshot $Snapshot
    $embeddedBomLicense['LICENSE'] = [regex]::Replace(
        [string]$embeddedBomLicense['LICENSE'],
        "`n",
        "`n$([char]0xFEFF)",
        1
    )
    $embeddedBomResult = Test-LicenseSnapshot -Files $embeddedBomLicense -CheckLicenseHash
    if (-not ($embeddedBomResult.Findings -match 'LICENSE SHA-256 mismatch')) {
        $selfTestFailures.Add('embedded LICENSE BOM did not fail the canonical hash') | Out-Null
    }

    function Assert-MissingFileFails([string]$Name, [string]$Path) {
        $fixture = Copy-Snapshot $Snapshot
        $fixture.Remove($Path)
        $result = Test-LicenseSnapshot -Files $fixture
        if ($result.Findings.Count -eq 0) {
            $selfTestFailures.Add("negative case passed unexpectedly: $Name") | Out-Null
        }
    }

    function Assert-ReplacementFails([string]$Name, [string]$Path, [string]$Old, [string]$New) {
        $fixture = Copy-Snapshot $Snapshot
        $before = [string]$fixture[$Path]
        $after = $before.Replace($Old, $New)
        if ($after -eq $before) {
            $selfTestFailures.Add("negative case did not mutate fixture: $Name") | Out-Null
            return
        }
        $fixture[$Path] = $after
        $result = Test-LicenseSnapshot -Files $fixture
        if ($result.Findings.Count -eq 0) {
            $selfTestFailures.Add("negative case passed unexpectedly: $Name") | Out-Null
        }
    }

    function Assert-RegexReplacementFails([string]$Name, [string]$Path, [string]$Pattern, [string]$Replacement) {
        $fixture = Copy-Snapshot $Snapshot
        $before = [string]$fixture[$Path]
        $after = [regex]::Replace($before, $Pattern, $Replacement)
        if ($after -eq $before) {
            $selfTestFailures.Add("negative case did not mutate fixture: $Name") | Out-Null
            return
        }
        $fixture[$Path] = $after
        $result = Test-LicenseSnapshot -Files $fixture
        if ($result.Findings.Count -eq 0) {
            $selfTestFailures.Add("negative case passed unexpectedly: $Name") | Out-Null
        }
    }

    Assert-MissingFileFails 'missing LICENSE' 'LICENSE'
    Assert-MissingFileFails 'missing NOTICE' 'NOTICE'
    Assert-ReplacementFails 'missing cc-switch copyright' 'NOTICE' 'Copyright (c) 2025 Jason Young' 'copyright removed'
    Assert-ReplacementFails 'missing upstream MIT baseline scope' 'NOTICE' 'MIT-covered upstream baseline: BigPizzaV3/CodexPlusPlus through a0506ae' 'upstream scope removed'
    Assert-ReplacementFails 'missing independent MIT scope statement' 'NOTICE' 'The MIT terms below apply independently to each work listed above.' 'shared scope removed'
    Assert-ReplacementFails 'missing MIT grant' 'NOTICE' 'Permission is hereby granted, free of charge' 'grant removed'
    Assert-ReplacementFails 'missing MIT notice condition' 'NOTICE' 'The above copyright notice and this permission notice shall be included' 'condition removed'
    Assert-ReplacementFails 'missing MIT disclaimer' 'NOTICE' 'THE SOFTWARE IS PROVIDED "AS IS"' 'disclaimer removed'
    Assert-ReplacementFails 'Cargo license mismatch' 'Cargo.toml' 'license = "AGPL-3.0-only"' 'license = "MIT"'
    Assert-ReplacementFails 'Chinese README license mismatch' 'README.md' 'AGPL-3.0-only' 'MIT'
    Assert-ReplacementFails 'English README license mismatch' 'README_EN.md' 'AGPL-3.0-only' 'MIT'
    Assert-ReplacementFails 'source URL leaves origin' '.github/workflows/release-assets.yml' 'https://github.com/Duojiyi/chimera-codex/releases/download/${TAG}/${source_asset}' 'https://example.invalid/source.tar.gz'
    Assert-ReplacementFails 'source archive not bound to TARGET_SHA' '.github/workflows/release-assets.yml' 'git archive --format=tar --prefix="ChimeraPlusPlus-${VERSION}-source/" "$TARGET_SHA"' 'git archive --format=tar HEAD'
    Assert-ReplacementFails 'source tree expected NUL sorting integrity' '.github/workflows/release-assets.yml' 'git ls-tree -rz --name-only "$TARGET_SHA" | LC_ALL=C sort -z > /tmp/source-tree-expected.z' 'git ls-tree -rz --name-only "$TARGET_SHA" | LC_ALL=C sort > /tmp/source-tree-expected.z'
    Assert-ReplacementFails 'source archive extraction integrity' '.github/workflows/release-assets.yml' 'tar -xzf "$source_asset" -C /tmp/source-tree-root' 'printf archive-extraction-disabled'
    Assert-ReplacementFails 'source tree NUL traversal integrity' '.github/workflows/release-assets.yml' 'find . -mindepth 1 ! -type d -print0' 'find . -mindepth 1 ! -type d -print'
    Assert-ReplacementFails 'source tree NUL normalization integrity' '.github/workflows/release-assets.yml' "sed -z 's#^\./##'" "sed 's#^\./##'"
    Assert-ReplacementFails 'source tree NUL sorting integrity' '.github/workflows/release-assets.yml' '| LC_ALL=C sort -z > /tmp/source-tree-actual.z' '| LC_ALL=C sort > /tmp/source-tree-actual.z'
    Assert-ReplacementFails 'source tree comparison integrity' '.github/workflows/release-assets.yml' 'cmp /tmp/source-tree-expected.z /tmp/source-tree-actual.z' 'printf tree-comparison-disabled'
    Assert-ReplacementFails 'source required-file integrity' '.github/workflows/release-assets.yml' 'apps/codex-plus-manager/package-lock.json' 'required-file-check-disabled'
    Assert-RegexReplacementFails 'commented draft asset content gate' '.github/workflows/release-assets.yml' '(?m)^(\s*)verify_draft_assets_content\s*$' '$1# verify_draft_assets_content'

    foreach ($entry in @(
        [pscustomobject]@{ Name = 'Windows installer LICENSE'; Path = 'scripts/installer/windows/CodexPlusPlus.nsi'; Token = 'File "/oname=LICENSE.new" "${ROOT}\LICENSE"' },
        [pscustomobject]@{ Name = 'Windows installer NOTICE'; Path = 'scripts/installer/windows/CodexPlusPlus.nsi'; Token = 'File "/oname=NOTICE.new" "${ROOT}\NOTICE"' },
        [pscustomobject]@{ Name = 'Windows installer SOURCE_CODE'; Path = 'scripts/installer/windows/CodexPlusPlus.nsi'; Token = 'File "/oname=SOURCE_CODE.txt.new" "${ROOT}\SOURCE_CODE.txt"' },
        [pscustomobject]@{ Name = 'Windows zip LICENSE'; Path = '.github/workflows/release-assets.yml'; Token = 'Copy-Item LICENSE,NOTICE,SOURCE_CODE.txt dist/windows/app/' },
        [pscustomobject]@{ Name = 'Windows zip NOTICE'; Path = '.github/workflows/release-assets.yml'; Token = 'Copy-Item LICENSE,NOTICE,SOURCE_CODE.txt dist/windows/app/' },
        [pscustomobject]@{ Name = 'Windows zip SOURCE_CODE'; Path = '.github/workflows/release-assets.yml'; Token = 'Copy-Item LICENSE,NOTICE,SOURCE_CODE.txt dist/windows/app/' },
        [pscustomobject]@{ Name = 'macOS DMG LICENSE'; Path = 'scripts/installer/macos/package-dmg.sh'; Token = 'cp "$ROOT/LICENSE" "$STAGE/LICENSE"' },
        [pscustomobject]@{ Name = 'macOS DMG NOTICE'; Path = 'scripts/installer/macos/package-dmg.sh'; Token = 'cp "$ROOT/NOTICE" "$STAGE/NOTICE"' },
        [pscustomobject]@{ Name = 'macOS DMG SOURCE_CODE'; Path = 'scripts/installer/macos/package-dmg.sh'; Token = 'cp "$ROOT/SOURCE_CODE.txt" "$STAGE/SOURCE_CODE.txt"' },
        [pscustomobject]@{ Name = 'macOS zip LICENSE'; Path = '.github/workflows/release-assets.yml'; Token = 'cp LICENSE NOTICE SOURCE_CODE.txt "dist/macos/app-${{ matrix.arch }}/"' },
        [pscustomobject]@{ Name = 'macOS zip NOTICE'; Path = '.github/workflows/release-assets.yml'; Token = 'cp LICENSE NOTICE SOURCE_CODE.txt "dist/macos/app-${{ matrix.arch }}/"' },
        [pscustomobject]@{ Name = 'macOS zip SOURCE_CODE'; Path = '.github/workflows/release-assets.yml'; Token = 'cp LICENSE NOTICE SOURCE_CODE.txt "dist/macos/app-${{ matrix.arch }}/"' }
    )) {
        Assert-ReplacementFails $entry.Name $entry.Path $entry.Token "removed-$($entry.Name)"
    }

    if ($selfTestFailures.Count -gt 0) {
        Write-Host "verify-license self-test: FAILED ($($selfTestFailures.Count) finding(s))"
        foreach ($failure in $selfTestFailures) {
            Write-Host "  - $failure"
        }
        return $false
    }

    Write-Host 'verify-license self-test: PASS'
    return $true
}

$snapshot = Get-RepositorySnapshot
if ($SelfTest) {
    if (-not (Invoke-SelfTests -Snapshot $snapshot)) {
        exit 1
    }
    exit 0
}

$validation = Test-LicenseSnapshot -Files $snapshot -CheckLicenseHash
if ($validation.Findings.Count -gt 0) {
    Write-Host "verify-license: FAILED ($($validation.Findings.Count) finding(s))"
    foreach ($failure in $validation.Findings) {
        Write-Host "  - $failure"
    }
    exit 1
}

Write-Host 'verify-license: PASS'
