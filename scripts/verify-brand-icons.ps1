#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$findings = New-Object System.Collections.Generic.List[string]

function Add-Finding([string]$Message) {
    $findings.Add($Message) | Out-Null
}

function Resolve-RequiredFile([string]$RelativePath) {
    $path = Join-Path $root $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        Add-Finding "missing required icon file: $RelativePath"
        return $null
    }
    return $path
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

function Get-SvgFindings([string]$Content, [string]$Label) {
    $issues = New-Object System.Collections.Generic.List[string]
    if ($Content.Length -gt 1048576) {
        $issues.Add("${Label}: SVG exceeds the 1 MiB safety limit") | Out-Null
        return $issues.ToArray()
    }

    $settings = [System.Xml.XmlReaderSettings]::new()
    $settings.DtdProcessing = [System.Xml.DtdProcessing]::Prohibit
    $settings.XmlResolver = $null
    $settings.MaxCharactersInDocument = 1048576
    $settings.MaxCharactersFromEntities = 0
    $reader = $null
    $stringReader = $null
    try {
        $stringReader = [IO.StringReader]::new($Content)
        $reader = [System.Xml.XmlReader]::Create($stringReader, $settings)
        $document = [System.Xml.XmlDocument]::new()
        $document.XmlResolver = $null
        $document.Load($reader)

        if ($document.DocumentType) {
            $issues.Add("${Label}: document types are forbidden") | Out-Null
        }
        if ($document.SelectNodes('//processing-instruction()').Count -gt 0) {
            $issues.Add("${Label}: processing instructions are forbidden") | Out-Null
        }

        $svgNamespace = 'http://www.w3.org/2000/svg'
        $allowedElements = @('svg', 'g', 'rect', 'path', 'title', 'desc')
        $allowedAttributes = @(
            'viewBox', 'role', 'aria-labelledby', 'id',
            'x', 'y', 'width', 'height', 'rx', 'fill', 'd'
        )
        foreach ($element in $document.SelectNodes('//*')) {
            if ($element.NamespaceURI -ne $svgNamespace -or $allowedElements -notcontains $element.LocalName) {
                $issues.Add("${Label}: forbidden element <$($element.Name)>") | Out-Null
            }
            foreach ($attribute in $element.Attributes) {
                if ($attribute.Prefix -eq 'xmlns' -or $attribute.Name -eq 'xmlns') {
                    if ($attribute.Name -ne 'xmlns' -or $attribute.Value -ne $svgNamespace) {
                        $issues.Add("${Label}: forbidden namespace declaration '$($attribute.Name)'") | Out-Null
                    }
                    continue
                }
                if ($attribute.NamespaceURI -or $allowedAttributes -notcontains $attribute.LocalName) {
                    $issues.Add("${Label}: forbidden attribute '$($attribute.Name)'") | Out-Null
                }
                if ($attribute.LocalName -match '^(?i:on)' -or $attribute.LocalName -in @('style', 'href')) {
                    $issues.Add("${Label}: active or external attribute '$($attribute.Name)'") | Out-Null
                }
                if ($attribute.Value -match '(?i)(url\s*\(|@import|javascript\s*:|data\s*:|https?\s*:)') {
                    $issues.Add("${Label}: active or external attribute value in '$($attribute.Name)'") | Out-Null
                }
            }
        }

        $rootNode = $document.DocumentElement
        if (-not $rootNode -or $rootNode.LocalName -ne 'svg' -or $rootNode.NamespaceURI -ne $svgNamespace) {
            $issues.Add("${Label}: root must be an SVG element in the SVG namespace") | Out-Null
        }
        elseif ($rootNode.GetAttribute('viewBox') -ne '0 0 512 512') {
            $issues.Add("${Label}: SVG must use viewBox=`"0 0 512 512`"") | Out-Null
        }
        if ($rootNode -and ($rootNode.HasAttribute('width') -or $rootNode.HasAttribute('height'))) {
            $issues.Add("${Label}: SVG must not set fixed width or height") | Out-Null
        }
    }
    catch {
        $issues.Add("${Label}: invalid or unsafe XML: $($_.Exception.Message)") | Out-Null
    }
    finally {
        if ($reader) { $reader.Dispose() }
        if ($stringReader) { $stringReader.Dispose() }
    }
    return $issues.ToArray()
}

function Get-IcoFindings([byte[]]$Bytes, [string]$Label) {
    $issues = New-Object System.Collections.Generic.List[string]
    if ($Bytes.Length -lt 6 -or [BitConverter]::ToUInt16($Bytes, 0) -ne 0 -or [BitConverter]::ToUInt16($Bytes, 2) -ne 1) {
        $issues.Add("${Label}: invalid ICO header") | Out-Null
        return $issues.ToArray()
    }
    $count = [BitConverter]::ToUInt16($Bytes, 4)
    if ($count -lt 1 -or $count -gt 64 -or (6 + (16 * $count)) -gt $Bytes.Length) {
        $issues.Add("${Label}: invalid ICO directory length") | Out-Null
        return $issues.ToArray()
    }

    $directoryEnd = 6 + (16 * $count)
    for ($index = 0; $index -lt $count; $index++) {
        $entry = 6 + (16 * $index)
        $payloadLength = [uint64][BitConverter]::ToUInt32($Bytes, $entry + 8)
        $payloadOffset = [uint64][BitConverter]::ToUInt32($Bytes, $entry + 12)
        $payloadEnd = $payloadOffset + $payloadLength
        if ($payloadLength -lt 8 -or $payloadOffset -lt $directoryEnd -or $payloadEnd -gt $Bytes.Length) {
            $issues.Add("${Label}: ICO entry $index has an invalid payload range") | Out-Null
            continue
        }
        $offset = [int]$payloadOffset
        $isPng = $payloadLength -ge 8 -and
            $Bytes[$offset] -eq 0x89 -and $Bytes[$offset + 1] -eq 0x50 -and
            $Bytes[$offset + 2] -eq 0x4E -and $Bytes[$offset + 3] -eq 0x47 -and
            $Bytes[$offset + 4] -eq 0x0D -and $Bytes[$offset + 5] -eq 0x0A -and
            $Bytes[$offset + 6] -eq 0x1A -and $Bytes[$offset + 7] -eq 0x0A
        if (-not $isPng) {
            $issues.Add("${Label}: ICO entry $index is not a PNG payload from the locked exporter") | Out-Null
            continue
        }

        $payload = [byte[]]::new([int]$payloadLength)
        [Array]::Copy($Bytes, $offset, $payload, 0, [int]$payloadLength)
        $stream = $null
        $image = $null
        try {
            Add-Type -AssemblyName System.Drawing
            $stream = [IO.MemoryStream]::new($payload, $false)
            $image = [System.Drawing.Image]::FromStream($stream, $true, $true)
            $expectedWidth = if ($Bytes[$entry] -eq 0) { 256 } else { [int]$Bytes[$entry] }
            $expectedHeight = if ($Bytes[$entry + 1] -eq 0) { 256 } else { [int]$Bytes[$entry + 1] }
            if ($image.Width -ne $expectedWidth -or $image.Height -ne $expectedHeight) {
                $issues.Add("${Label}: ICO entry $index directory dimensions do not match its decoded PNG") | Out-Null
            }
        }
        catch {
            $issues.Add("${Label}: ICO entry $index PNG cannot be decoded: $($_.Exception.Message)") | Out-Null
        }
        finally {
            if ($image) { $image.Dispose() }
            if ($stream) { $stream.Dispose() }
        }
    }
    return $issues.ToArray()
}

function Invoke-SelfTest {
    $safe = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><title>safe</title><g><rect x="1" y="1" width="2" height="2" fill="#000"/></g></svg>'
    if (@(Get-SvgFindings $safe 'safe fixture').Count -ne 0) {
        throw 'safe SVG fixture was rejected'
    }
    $unsafeFixtures = @(
        '<!DOCTYPE svg [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><title>&xxe;</title></svg>',
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><script>alert(1)</script></svg>',
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><style>@import url(https://example.invalid/a.css)</style></svg>',
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" onload="alert(1)"><path d="M0 0"/></svg>',
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><path style="fill:url(https://example.invalid/a.svg)" d="M0 0"/></svg>',
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><image href="data:image/png;base64,AA=="/></svg>'
    )
    foreach ($fixture in $unsafeFixtures) {
        if (@(Get-SvgFindings $fixture 'unsafe fixture').Count -eq 0) {
            throw "unsafe SVG fixture was accepted: $fixture"
        }
    }

    $fakeIco = [byte[]]::new(22)
    [BitConverter]::GetBytes([uint16]1).CopyTo($fakeIco, 2)
    [BitConverter]::GetBytes([uint16]1).CopyTo($fakeIco, 4)
    $fakeIco[6] = 16
    $fakeIco[7] = 16
    [BitConverter]::GetBytes([uint32]0).CopyTo($fakeIco, 14)
    [BitConverter]::GetBytes([uint32]22).CopyTo($fakeIco, 18)
    if (@(Get-IcoFindings $fakeIco 'fake ICO').Count -eq 0) {
        throw 'zero-payload ICO fixture was accepted'
    }

    $signatureOnlyIco = [byte[]]::new(30)
    [BitConverter]::GetBytes([uint16]1).CopyTo($signatureOnlyIco, 2)
    [BitConverter]::GetBytes([uint16]1).CopyTo($signatureOnlyIco, 4)
    $signatureOnlyIco[6] = 16
    $signatureOnlyIco[7] = 16
    [BitConverter]::GetBytes([uint32]8).CopyTo($signatureOnlyIco, 14)
    [BitConverter]::GetBytes([uint32]22).CopyTo($signatureOnlyIco, 18)
    $pngSignature = [byte[]]@(0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A)
    $pngSignature.CopyTo($signatureOnlyIco, 22)
    if (@(Get-IcoFindings $signatureOnlyIco 'signature-only ICO').Count -eq 0) {
        throw 'truncated PNG ICO fixture was accepted'
    }
    Write-Output 'verify-brand-icons self-test: PASS'
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

$svgPath = Resolve-RequiredFile 'brand/icon/logo.svg'
$provenancePath = Resolve-RequiredFile 'brand/icon/PROVENANCE.md'
$pngTargets = @(
    'apps/codex-plus-manager/src-tauri/icons/icon.png',
    'assets/images/codex-plus-plus.png',
    'docs/images/codex-plus-plus.png'
)
$icoTargets = @(
    'apps/codex-plus-manager/src-tauri/icons/icon.ico',
    'assets/images/codex-plus-plus.ico',
    'docs/images/codex-plus-plus.ico'
)
$resolvedPng = @($pngTargets | ForEach-Object { Resolve-RequiredFile $_ } | Where-Object { $_ })
$resolvedIco = @($icoTargets | ForEach-Object { Resolve-RequiredFile $_ } | Where-Object { $_ })

if ($svgPath) {
    $svgContent = [IO.File]::ReadAllText($svgPath, [Text.Encoding]::UTF8)
    foreach ($issue in @(Get-SvgFindings $svgContent 'brand/icon/logo.svg')) {
        Add-Finding $issue
    }
}

if ($provenancePath) {
    $provenance = Get-Content -LiteralPath $provenancePath -Raw -Encoding UTF8
    foreach ($required in @(
        'Chimera++',
        'OpenAI/ChatGPT/Codex assets were not used as inputs',
        'logo-designer',
        'GPT-5',
        'concept-1-monogram.svg',
        'AGPL-3.0-only'
    )) {
        if (-not $provenance.Contains($required)) {
            Add-Finding "brand/icon/PROVENANCE.md missing '$required'"
        }
    }
}

$legacyHashes = @(
    '76BDB06A3FB0FD157326D5C7F0FC6E6F4D4901D47AED47DB6CD53109F361A428',
    'D2D80BB01BD5FA6F26529884F614A402933D59219D0991141BFE04E861B485E0'
)
$allTargets = @($resolvedPng + $resolvedIco)
foreach ($path in $allTargets) {
    if ($legacyHashes -contains (Get-Sha256 $path)) {
        Add-Finding "legacy icon hash remains: $([IO.Path]::GetRelativePath($root, $path))"
    }
}

if ($resolvedPng.Count -eq $pngTargets.Count) {
    $pngHashes = @($resolvedPng | ForEach-Object { Get-Sha256 $_ } | Select-Object -Unique)
    if ($pngHashes.Count -ne 1) {
        Add-Finding 'PNG icon copies are not byte-identical'
    }
    Add-Type -AssemblyName System.Drawing
    try {
        $bitmap = [System.Drawing.Bitmap]::FromFile($resolvedPng[0])
        try {
            if ($bitmap.Width -ne 1024 -or $bitmap.Height -ne 1024) {
                Add-Finding "primary PNG must be 1024x1024, found $($bitmap.Width)x$($bitmap.Height)"
            }
            $lastX = $bitmap.Width - 1
            $lastY = $bitmap.Height - 1
            foreach ($point in @(@(0, 0), @($lastX, 0), @(0, $lastY), @($lastX, $lastY))) {
                if ($bitmap.GetPixel($point[0], $point[1]).A -ne 0) {
                    Add-Finding 'primary PNG must keep transparent outer corners'
                    break
                }
            }
        }
        finally {
            $bitmap.Dispose()
        }
    }
    catch {
        Add-Finding "primary PNG cannot be decoded: $($_.Exception.Message)"
    }
}

if ($resolvedIco.Count -eq $icoTargets.Count) {
    $icoHashes = @($resolvedIco | ForEach-Object { Get-Sha256 $_ } | Select-Object -Unique)
    if ($icoHashes.Count -ne 1) {
        Add-Finding 'ICO icon copies are not byte-identical'
    }
    $bytes = [IO.File]::ReadAllBytes($resolvedIco[0])
    foreach ($issue in @(Get-IcoFindings $bytes 'primary ICO')) {
        Add-Finding $issue
    }
    try {
        $count = [BitConverter]::ToUInt16($bytes, 4)
        $sizes = for ($index = 0; $index -lt $count; $index++) {
            $offset = 6 + (16 * $index)
            if ($offset + 16 -gt $bytes.Length) { throw 'truncated ICO directory' }
            if ($bytes[$offset] -eq 0) { 256 } else { [int]$bytes[$offset] }
        }
        foreach ($requiredSize in @(16, 32, 48, 256)) {
            if ($sizes -notcontains $requiredSize) {
                Add-Finding "ICO is missing ${requiredSize}px entry"
            }
        }
    }
    catch {
        Add-Finding "primary ICO cannot be decoded: $($_.Exception.Message)"
    }
}

if ($svgPath -and $resolvedPng.Count -eq $pngTargets.Count -and $resolvedIco.Count -eq $icoTargets.Count) {
    $tauri = Join-Path $root 'apps/codex-plus-manager/node_modules/.bin/tauri.cmd'
    if (-not (Test-Path -LiteralPath $tauri -PathType Leaf)) {
        Add-Finding 'locked Tauri CLI is missing; run npm ci in apps/codex-plus-manager before the icon gate'
    }
    else {
        $exportRoot = Join-Path $root 'target/verify-brand-icons'
        $defaultOutput = Join-Path $exportRoot 'default'
        $pngOutput = Join-Path $exportRoot 'png'
        New-Item -ItemType Directory -Force -Path $defaultOutput, $pngOutput | Out-Null

        & $tauri icon $svgPath --output $defaultOutput | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Add-Finding "Tauri CLI failed to regenerate ICO (exit $LASTEXITCODE)"
        }
        & $tauri icon $svgPath --output $pngOutput --png 1024 | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Add-Finding "Tauri CLI failed to regenerate PNG (exit $LASTEXITCODE)"
        }

        $generatedIco = Join-Path $defaultOutput 'icon.ico'
        $generatedPng = Join-Path $pngOutput '1024x1024.png'
        if (-not (Test-Path -LiteralPath $generatedIco -PathType Leaf) -or
            (Get-Sha256 $generatedIco) -ne (Get-Sha256 $resolvedIco[0])) {
            Add-Finding 'distributed ICO is not the locked Tauri export of brand/icon/logo.svg'
        }
        if (-not (Test-Path -LiteralPath $generatedPng -PathType Leaf) -or
            (Get-Sha256 $generatedPng) -ne (Get-Sha256 $resolvedPng[0])) {
            Add-Finding 'distributed PNG is not the locked Tauri export of brand/icon/logo.svg'
        }
    }
}

$referenceChecks = @{
    'apps/codex-plus-manager/src-tauri/tauri.conf.json' = 'icons/icon.ico'
    'apps/codex-plus-launcher/build.rs' = 'icons/icon.ico'
    'scripts/installer/windows/CodexPlusPlus.nsi' = 'icons\icon.ico'
    'scripts/installer/macos/package-dmg.sh' = 'icons/icon.png'
    'crates/codex-plus-core/src/install/windows.rs' = 'codex-plus-plus.ico'
    'crates/codex-plus-core/src/install/macos.rs' = 'codex-plus-plus.png'
    'README.md' = 'docs/images/codex-plus-plus.png'
    'README_EN.md' = 'docs/images/codex-plus-plus.png'
}
foreach ($entry in $referenceChecks.GetEnumerator()) {
    $path = Resolve-RequiredFile $entry.Key
    if ($path -and -not (Get-Content -LiteralPath $path -Raw -Encoding UTF8).Contains($entry.Value)) {
        Add-Finding "$($entry.Key) missing icon reference '$($entry.Value)'"
    }
}

if ($findings.Count -gt 0) {
    Write-Error "verify-brand-icons: FAILED ($($findings.Count) finding(s))`n  - $($findings -join "`n  - ")"
    exit 1
}

Write-Output 'verify-brand-icons: PASS'
