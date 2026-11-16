[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$ArchivePath
)

$ErrorActionPreference = 'Stop'

function Invoke-Git {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$GitArgs)
    & git @GitArgs
    if ($LASTEXITCODE -ne 0) {
        throw "git $($GitArgs -join ' ') failed with exit code $LASTEXITCODE."
    }
}

$repoRoot = (& git rev-parse --show-toplevel 2>$null)
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($repoRoot)) {
    throw 'Run this script from inside a cloned ShuKanji repository.'
}
$repoRoot = (Resolve-Path -LiteralPath $repoRoot).Path
Set-Location -LiteralPath $repoRoot

$branch = (& git branch --show-current)
if ($LASTEXITCODE -ne 0 -or $branch -ne 'main') {
    throw "Switch to the main branch before importing. Current branch: $branch"
}
$existingChanges = (& git status --porcelain -- app live-rs)
if ($LASTEXITCODE -ne 0) { throw 'Could not inspect the existing app/ and live-rs/ worktree.' }
if (-not [string]::IsNullOrWhiteSpace(($existingChanges -join ''))) {
    throw "app/ or live-rs/ already has local changes. Commit or back them up before importing: $($existingChanges -join [Environment]::NewLine)"
}

$origin = (& git remote get-url origin 2>$null)
if ($LASTEXITCODE -ne 0 -or $origin -notmatch 'github\.com[:/]avelatojr-source/Shukanji(?:\.git)?$') {
    throw "The origin remote does not appear to be avelatojr-source/Shukanji. Found: $origin"
}

$archive = (Resolve-Path -LiteralPath $ArchivePath).Path
if ([System.IO.Path]::GetExtension($archive) -ne '.zip') {
    throw 'ArchivePath must point to ShuKanji-Organized-Source.zip.'
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("ShuKanji-import-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempRoot | Out-Null

try {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::ExtractToDirectory($archive, $tempRoot)

    $sourceRoot = Join-Path $tempRoot 'ShuKanji-Organized'
    $appSource = Join-Path $sourceRoot 'app'
    $liveSource = Join-Path $sourceRoot 'live-rs'
    foreach ($required in @(
        (Join-Path $appSource 'Cargo.toml'),
        (Join-Path $appSource 'Cargo.lock'),
        (Join-Path $appSource 'build.rs'),
        (Join-Path $appSource 'src/lib.rs'),
        (Join-Path $appSource 'assets/fonts/Inter.ttf'),
        (Join-Path $appSource 'assets/app-icon.ico'),
        (Join-Path $liveSource 'Cargo.toml'),
        (Join-Path $liveSource 'src/lib.rs'),
        (Join-Path $liveSource 'LICENSE')
    )) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "The archive is missing a required project file: $required"
        }
    }

    $appManifest = Get-Content -LiteralPath (Join-Path $appSource 'Cargo.toml') -Raw
    if ($appManifest -notmatch 'path\s*=\s*["'']\.\./live-rs["'']') {
        throw 'The app Cargo manifest no longer references ../live-rs. Stopping to avoid importing a broken crate layout.'
    }

    $forbidden = Get-ChildItem -LiteralPath $sourceRoot -Force -Recurse | Where-Object {
        $_.Name -eq '.git' -or
        $_.Name -eq 'target' -or
        $_.Name -eq 'dist' -or
        $_.Name -eq 'Source code' -or
        $_.Name -eq 'config.toml' -or
        ($_.PSIsContainer -eq $false -and $_.Extension -eq '.exe')
    }
    if ($forbidden) {
        $paths = ($forbidden | ForEach-Object { $_.FullName }) -join "`n"
        throw "The archive contains generated, local, or nested-repository content. Nothing was staged:`n$paths"
    }

    foreach ($pair in @(
        @{ Source = $appSource; Destination = (Join-Path $repoRoot 'app') },
        @{ Source = $liveSource; Destination = (Join-Path $repoRoot 'live-rs') }
    )) {
        New-Item -ItemType Directory -Path $pair.Destination -Force | Out-Null
        Get-ChildItem -LiteralPath $pair.Source -Force | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination $pair.Destination -Recurse -Force
        }
    }

    if (-not (Test-Path -LiteralPath (Join-Path $repoRoot 'app/Cargo.toml')) -or
        -not (Test-Path -LiteralPath (Join-Path $repoRoot 'live-rs/src/lib.rs'))) {
        throw 'Source copy did not produce both sibling Rust crates.'
    }

    Write-Host ''
    Write-Host 'Source copied. Review the staged changes below:' -ForegroundColor Green
    Invoke-Git add -- app live-rs
    Invoke-Git status --short
    Write-Host ''
    Write-Host 'The app and live-rs crates are staged. No commit or push has happened yet.' -ForegroundColor Yellow
    $answer = Read-Host 'Commit these source files and push main to GitHub now? (y/N)'
    if ($answer -match '^(y|yes)$') {
        Invoke-Git commit -m 'Add complete ShuKanji Rust source'
        Invoke-Git push -u origin main
        Write-Host 'Source commit pushed successfully.' -ForegroundColor Green
    } else {
        Write-Host 'Changes remain staged locally. When ready, run:'
        Write-Host '  git commit -m "Add complete ShuKanji Rust source"'
        Write-Host '  git push -u origin main'
    }
}
finally {
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
