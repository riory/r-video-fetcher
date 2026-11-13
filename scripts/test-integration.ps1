$ErrorActionPreference = 'Stop'

Push-Location (Join-Path $PSScriptRoot '..')
try {
    cargo test --locked --target-dir target\pre-push-tests --test integration -- --ignored --test-threads=1
    if ($LASTEXITCODE -ne 0) {
        throw "Integration tests failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}
