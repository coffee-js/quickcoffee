param([Parameter(Mandatory = $true)][string]$ArchivePath)

$archiveFile = (Resolve-Path -LiteralPath $ArchivePath -ErrorAction Stop).Path
$archiveName = Split-Path -Leaf $archiveFile
$archiveMatch = [regex]::Match($archiveName, '^quickcoffee-(\d+\.\d+\.\d+)-x86_64-pc-windows-msvc\.zip$')
if (-not $archiveMatch.Success) {
    throw "unexpected Windows archive name: $archiveName"
}
$archiveVersion = $archiveMatch.Groups[1].Value
$document = Get-Content -LiteralPath 'docs/releasing.md' -Raw -ErrorAction Stop
$blocks = [regex]::Matches($document, '(?ms)^```powershell\r?\n(.*?)^```')
if ($blocks.Count -ne 2 -or $blocks[0].Groups[1].Value -cne $blocks[1].Groups[1].Value) {
    throw 'the Chinese and English Windows install blocks must match'
}
$code = $blocks[0].Groups[1].Value
if (-not $code.Contains('$Archive = "quickcoffee-$Version-$Target.zip"')) {
    throw 'Windows install block no longer has the expected archive name'
}
$versionLine = [regex]::Match($code, '(?m)^  \$Version = "[^"]+"$')
if (-not $versionLine.Success) { throw 'Windows install block has no version line' }

$scratch = Join-Path ([System.IO.Path]::GetTempPath()) ("quickcoffee-windows-install-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $scratch -ErrorAction Stop | Out-Null
try {
    $badZip = Join-Path $scratch $archiveName
    Set-Content -LiteralPath $badZip -Value 'not a zip' -NoNewline
    $realHash = (Get-FileHash -LiteralPath $archiveFile -Algorithm SHA256).Hash.ToLowerInvariant()
    $badHash = (Get-FileHash -LiteralPath $badZip -Algorithm SHA256).Hash.ToLowerInvariant()

    # Run the documented block with only the archive version and selected
    # failure commands substituted. Downloads use local files, not the network.
    function Invoke-WebRequest {
        param([string]$Uri, [string]$OutFile, [string]$ErrorAction)
        if ($script:Failure -eq 'archive-download' -and $Uri.EndsWith('.zip')) {
            Write-Error 'simulated archive download failure' -ErrorAction $ErrorAction
            return
        }
        if ($script:Failure -eq 'manifest-download' -and $Uri.EndsWith('/SHA256SUMS')) {
            Write-Error 'simulated manifest download failure' -ErrorAction $ErrorAction
            return
        }
        $source = if ($Uri.EndsWith('/SHA256SUMS')) { $script:ManifestFile } else { $script:SourceArchive }
        Copy-Item -LiteralPath $source -Destination $OutFile -ErrorAction Stop
    }
    function Set-Location {
        param([string]$LiteralPath, [string]$ErrorAction)
        if ($script:Failure -eq 'directory') { throw 'simulated directory failure' }
        Microsoft.PowerShell.Management\Set-Location -LiteralPath $LiteralPath -ErrorAction $ErrorAction
    }

    foreach ($failure in @('success', 'archive-download', 'manifest-download', 'missing-entry',
            'checksum', 'extract', 'directory', 'native', 'missing-binary', 'qcson', 'missing-qcson')) {
        $work = Join-Path $scratch $failure
        New-Item -ItemType Directory -Path $work -ErrorAction Stop | Out-Null
        $script:Failure = $failure
        $script:SourceArchive = if ($failure -eq 'extract') { $badZip } else { $archiveFile }
        $hash = if ($failure -eq 'extract') { $badHash } else { $realHash }
        if ($failure -eq 'checksum') { $hash = '0' * 64 }
        $entry = if ($failure -eq 'missing-entry') { "$hash  other.zip" } else { "$hash  $archiveName" }
        $script:ManifestFile = Join-Path $work 'fixture-SHA256SUMS'
        Set-Content -LiteralPath $script:ManifestFile -Value $entry
        # Public download instructions still use the existing release. Only
        # this local rehearsal points the same documented code at the new archive.
        $runCode = $code.Replace($versionLine.Value, ('  $Version = "' + $archiveVersion + '"'))
        if ($failure -eq 'native') {
            $runCode = $runCode.Replace('Invoke-Checked { .\qcoffee.exe --version }',
                'Invoke-Checked { .\qcoffee.exe --unknown-install-test-option }')
        }
        if ($failure -eq 'missing-binary') {
            $runCode = $runCode.Replace('Invoke-Checked { .\qcoffee.exe --version }',
                'Invoke-Checked { .\missing-qcoffee.exe --version }')
        }
        if ($failure -eq 'qcson') {
            $runCode = $runCode.Replace('examples\pricing\config.cson', 'examples\pricing\missing.cson')
        }
        if ($failure -eq 'missing-qcson') {
            $runCode = $runCode.Replace('.\qcson.exe to-json', '.\missing-qcson.exe to-json')
        }
        $failed = $false
        $message = ''
        Push-Location -LiteralPath $work
        try {
            & ([scriptblock]::Create($runCode)) | Out-Null
        } catch {
            $failed = $true
            $message = $_.Exception.Message
        } finally {
            Pop-Location
        }
        if ($failed -ne ($failure -ne 'success')) {
            throw "unexpected outcome for ${failure}: $message"
        }
        $expectedMessage = switch ($failure) {
            'archive-download' { 'simulated archive download failure' }
            'manifest-download' { 'simulated manifest download failure' }
            'missing-entry' { 'expected one checksum entry' }
            'checksum' { 'checksum mismatch' }
            'directory' { 'simulated directory failure' }
            'native' { 'native command failed with exit code' }
            'missing-binary' { 'missing-qcoffee.exe' }
            'qcson' { 'qcson failed with exit code' }
            'missing-qcson' { 'missing-qcson.exe' }
            default { '' }
        }
        if ($expectedMessage -and -not $message.Contains($expectedMessage)) {
            throw "wrong failure for ${failure}: $message"
        }
        $extracted = Join-Path $work ($archiveName -replace '\.zip$', '')
        if ($failure -in @('archive-download', 'manifest-download', 'missing-entry', 'checksum', 'extract') -and
                (Test-Path -LiteralPath (Join-Path $extracted 'qcoffee.exe'))) {
            throw "unverified binary was extracted for $failure"
        }
        Write-Output "ok windows install: $failure"
    }
} finally {
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction Stop
}

# The final scenario deliberately runs a failing native executable. PowerShell
# otherwise passes its stale exit code to the hosting CI step after all assertions pass.
$global:LASTEXITCODE = 0
