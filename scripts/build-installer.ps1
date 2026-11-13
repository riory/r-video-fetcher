$ErrorActionPreference = 'Stop'

$compiler = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1

if (-not $compiler) {
    $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($command) {
        $compiler = $command.Source
    } else {
        throw 'Inno Setup compiler (ISCC.exe) not found.'
    }
}

Push-Location (Join-Path $PSScriptRoot '..')
try {
    $versionMatch = Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $versionMatch) {
        throw 'Package version not found in Cargo.toml.'
    }
    $version = $versionMatch.Matches[0].Groups[1].Value

    cargo build --release --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed with exit code $LASTEXITCODE."
    }

    & $compiler "/DAppVersion=$version" 'packaging\r-video-fetcher.iss'
    if ($LASTEXITCODE -ne 0) {
        throw "Installer build failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}
