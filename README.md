# r-video-fetcher

A Windows desktop video and playlist downloader built with Rust. Downloads are handled by `yt-dlp`; `ffmpeg` is used to verify each completed file.

## Initial setup

1. Install the [Rust Analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) and [LLDB-DAP](https://marketplace.visualstudio.com/items?itemName=llvm-vs-code-extensions.lldb-dap) VS Code extensions.
2. Install LLVM from the [official LLVM release page](https://github.com/llvm/llvm-project/releases). For Windows x64, choose the `clang+llvm-19.1.0-x86_64-pc-windows-msvc.tar.xz` package; newer compatible releases are also available.
3. Use the standard Windows LLVM installer location so `lldb-dap.exe` is at `%ProgramFiles%\LLVM\bin\lldb-dap.exe`; the workspace launch configuration points there. For another install location, update `debugAdapterExecutable` in `.vscode/launch.json`.

See the [LLDB-DAP setup documentation](https://github.com/llvm/vscode-lldb#procuring-the-lldb-dap-binary) for other ways to obtain the debugger and additional requirements.

## Requirements

- Rust (to build and run from source)
- `yt-dlp` and `ffmpeg` available on `PATH`

The app reports a clear error if either executable is missing. Install them from **Settings → Init → Install dependencies with winget**, or run `scripts/install-dependencies.ps1` in PowerShell. The script uses the `yt-dlp.yt-dlp` and `Gyan.FFmpeg.Shared` winget packages.

## Run

```powershell
cargo run
```

### VS Code

Install the recommended Rust Analyzer and LLDB-DAP extensions. To run the app without a debugger, use **Terminal → Run Task → Run r-video-fetcher**; this runs `cargo run` and does not require LLDB-DAP. **F5** (or **Ctrl+F5**) uses the LLDB-DAP launch configuration and requires `lldb-dap.exe` at the configured path.

The app's **File** menu supports New/Open/Save/Exit for `.dwnl` project files, with recent projects and Ctrl+N/Ctrl+O/Ctrl+S shortcuts; Exit also works with Alt+F4. **Download → From clipboard** (Ctrl+B) creates a project from a valid clipboard URL and starts its download. The **Settings** menu opens Preferences and Init tabs.

Enter an HTTP(S) video or playlist URL, review the size and free-space estimate, then confirm the download. Files are organized under the configured root folder as:

```text
<root>/<channel>/<playlist if applicable>/<title> [<id>].<extension>
```

Quality and audio selections use the saved preferences by default. Enable **Override quality for this batch** in the workspace to choose one-off video resolution, audio sample rate, and channel settings without changing saved defaults. These preferences are applied through yt-dlp format selection. If the requested source formats are unavailable, yt-dlp may fall back to another available format.

## Settings and history

- Settings: `%APPDATA%\r-video-fetcher\settings.json`
- Download history: `%LOCALAPPDATA%\r-video-fetcher\history\*.dwnl`

History files are JSON and include the batch URL, total downloaded size, and a separate download log. The app displays the ten most recent batches.
Use a history entry's **Use this URL again** button to prefill a previous batch URL.
Project files store the URL, detected media type, target folder, estimated/downloaded size, and download log; the ten most recent project files appear in **File → Recent**.

## Project structure

- Each dialog is implemented in its own file under `src/dialogs/`.
- English and Russian UI resources are stored separately in `src/locales/`.
- Unit tests are grouped under `tests/unit/`; integration tests are grouped under `tests/integration/`.

## Validate

```powershell
cargo fmt -- --check
cargo test
cargo build
```

The live YouTube integration tests require network access, `yt-dlp`, and `ffmpeg`; they inspect live metadata, estimate playlist sizes, and download media. They are ignored by default. Run them explicitly with:

```powershell
cargo test --target-dir target/pre-push-tests --test integration -- --ignored --test-threads=1
```

The Git pre-push hook runs only the tests under `tests/unit/`; it excludes `tests/integration/`. Enable the repository hook after cloning with:

```powershell
git config core.hooksPath .githooks
```
