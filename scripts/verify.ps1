$ErrorActionPreference = 'Stop'

Push-Location (Join-Path $PSScriptRoot '..')
try {
    cargo fmt
    if ($LASTEXITCODE -ne 0) {
        throw "Formatting failed with exit code $LASTEXITCODE."
    }

    cargo fmt -- --check
    if ($LASTEXITCODE -ne 0) {
        throw "Format check failed with exit code $LASTEXITCODE."
    }

    cargo test --locked
    if ($LASTEXITCODE -ne 0) {
        throw "Tests failed with exit code $LASTEXITCODE."
    }

    cargo build --locked
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}
