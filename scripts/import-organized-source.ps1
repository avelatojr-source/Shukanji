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

# Require a clean clone so this import cannot accidentally include unrelated work.
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

$existingChanges = (& git status --porcelain)
if ($LASTEXITCODE -ne 0) {
    throw 'Could not inspect the worktree.'
}
if (-not [string]::IsNullOrWhiteSpace(($existingChanges -join ''))) {
    throw ("The clone has local changes. Commit, stash, or back them up before importing:" + [Environment]::NewLine + ($existingChanges -join [Environment]::NewLine))
}

$origin = (& git remote get-url origin 2>$null)
if ($LASTEXITCODE -ne 0 -or $origin -notmatch 'github\.com[:/]avelatojr-source/Shukanji(?:\.git)?$') {
    throw "The origin remote does not appear to be avelatojr-source/Shukanji. Found: $origin"
}

Invoke-Git fetch origin main
& git merge-base --is-ancestor origin/main HEAD
if ($LASTEXITCODE -ne 0) {
    throw 'Local main is not up to date with origin/main. Run git pull --ff-only origin main, then rerun this script.'
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

    # Validate both sibling crates and every binary asset compiled into the app.
    $required = @(
        (Join-Path $appSource 'Cargo.toml'),
        (Join-Path $appSource 'Cargo.lock'),
        (Join-Path $appSource 'build.rs'),
        (Join-Path $appSource 'src/lib.rs'),
        (Join-Path $appSource 'assets/fonts/Inter.ttf'),
        (Join-Path $appSource 'assets/fonts/NotoEmoji.ttf'),
        (Join-Path $appSource 'assets/fonts/ZenAntique-Wordmark.ttf'),
        (Join-Path $appSource 'assets/fonts/JetBrainsMono.ttf'),
        (Join-Path $appSource 'assets/app-icon.ico'),
        (Join-Path $appSource 'assets/app-icon.png'),
        (Join-Path $appSource 'assets/tebby/avatar.webp'),
        (Join-Path $appSource 'assets/tebby/idle.webp'),
        (Join-Path $appSource 'assets/tebby/duck.webp'),
        (Join-Path $liveSource 'Cargo.toml'),
        (Join-Path $liveSource 'src/lib.rs'),
        (Join-Path $liveSource 'LICENSE')
    )
    foreach ($path in $required) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "The archive is missing a required project file: $path"
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
        $paths = ($forbidden | ForEach-Object { $_.FullName }) -join [Environment]::NewLine
        throw ("The archive contains generated, local, or nested-repository content. Nothing was staged:" + [Environment]::NewLine + $paths)
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
    Write-Host 'Source copied and validated. Review the staged changes below:' -ForegroundColor Green
    Invoke-Git add -- app live-rs
    Invoke-Git status --short
    Write-Host ''
    Write-Host 'Only app/ and live-rs/ will be included in this commit.' -ForegroundColor Yellow
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