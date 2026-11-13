# Code verification

Run `.\scripts\verify.ps1` in PowerShell when verifying code changes. It formats Rust files, checks formatting, runs the default tests, and builds the application.

Run `.\scripts\test-integration.ps1` separately when live integration tests are needed. These tests require network access, `yt-dlp`, and `ffmpeg`, and download media.