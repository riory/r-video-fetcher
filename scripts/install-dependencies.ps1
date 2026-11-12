$ErrorActionPreference = 'Stop'

$wingetCommand = Get-Command winget.exe -ErrorAction SilentlyContinue
if (-not $wingetCommand) {
    throw 'Windows Package Manager (winget) is required. Install or update App Installer from the Microsoft Store.'
}

$packages = @(
    @{ Id = 'yt-dlp.yt-dlp'; Name = 'yt-dlp' },
    @{ Id = 'Gyan.FFmpeg.Shared'; Name = 'FFmpeg' }
)

foreach ($package in $packages) {
    Write-Output "Installing $($package.Name) ($($package.Id))..."
    & $wingetCommand.Source install `
        --id $package.Id `
        --exact `
        --source winget `
        --accept-source-agreements `
        --accept-package-agreements

    if ($LASTEXITCODE -ne 0) {
        throw "winget failed to install $($package.Name) with exit code $LASTEXITCODE."
    }
}

Write-Output 'Dependency installation completed. Restart r-video-fetcher if it was already open.'
