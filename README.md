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

The app reports a clear error if either executable is missing. The **Settings → Init** tab lists the winget commands, or run `scripts/install-dependencies.ps1` in PowerShell. The script uses the `yt-dlp.yt-dlp` and `Gyan.FFmpeg.Shared` winget packages.

## Run

```powershell
cargo run
```

## Release and Winget

Releases are built for Windows x64 by GitHub Actions. The release workflow runs the tests, builds the application, packages it with Inno Setup, and attaches the installer to a GitHub Release.

To build the installer locally with Inno Setup installed, run `.\scripts\build-installer.ps1` in PowerShell. The installer is written to `dist/`.

The application icon is maintained as `src/resources/icon.svg`. Its rendered `src/resources/icon.png` is used for the application window, and `src/resources/icon.ico` is embedded in the Windows executable and installer. Regenerate both rendered files when changing the SVG.

To publish a release, make sure the version in `Cargo.toml` is the intended release version, then push a matching version tag (for example, `v0.1.0`). The workflow rejects tags that do not match the Cargo package version.

After the GitHub Release is available, submit its installer URL to the [Windows Package Manager Community Repository](https://github.com/microsoft/winget-pkgs) using Microsoft's [WinGet manifest guide](https://learn.microsoft.com/en-us/windows/package-manager/package/manifest). The proposed package ID is `Riory.RVideoFetcher`; confirm it is available before submitting. WinGet users will also need `yt-dlp` and `ffmpeg`; the app's Init tab lists the commands to install them.

### VS Code

Install the recommended Rust Analyzer and LLDB-DAP extensions. To run the app without a debugger, use **Terminal → Run Task → Run r-video-fetcher**; this runs `cargo run` and does not require LLDB-DAP. **F5** (or **Ctrl+F5**) uses the LLDB-DAP launch configuration and requires `lldb-dap.exe` at the configured path.

The app's **File** menu supports New/Open/Save/Settings/Exit for `.dwnl` project files, with recent projects and Ctrl+N/Ctrl+O/Ctrl+S shortcuts; Exit also works with Alt+F4 and saves the current URL. The **Download** menu supports clipboard downloads and a modal history with URL reuse and confirmed clearing. In-app dialogs open centered in the main window; native file and folder dialogs are owned and positioned by Windows. Preferences are grouped into Download and User interface; the UI supports English and Russian, Default, Dark, and Zenburn themes, and 50%–200% zoom. Changes save automatically. Restore defaults requires confirmation and leaves recent files, window state, history, and the current project unchanged. The Init tab lists the dependency commands.

The main window's position and size are saved on exit and restored on startup, clamped to the current monitor.

Enter an HTTP(S) video or playlist URL and press Enter or select Estimate and preview. Review the size and free-space estimate, then confirm the download. Files are organized under the configured root folder as:

```text
<root>/<channel>/<playlist if applicable>/<title> [<id>].<extension>
```

Video resolution, audio sample rate, and audio channels are always visible in the workspace and start with the saved preferences. Changing a selection updates the current project and immediately saves it when the project already has a `.dwnl` file; unsaved projects retain the selections until saved. Batch selections do not change global preferences. The global **File type** preference defaults to the source format or can recode downloads to MP4. These preferences are applied through yt-dlp; if the requested source formats are unavailable, yt-dlp may fall back to another available format.

Inspection uses the system temporary directory for yt-dlp's temporary files; downloads use their output folder. The installed app does not open console windows for yt-dlp or ffmpeg.

## Settings and history

- Settings: `%APPDATA%\r-video-fetcher\settings.json`
- Download history: `%LOCALAPPDATA%\r-video-fetcher\history\*.dwnl`

History files are JSON and include the batch URL, total downloaded size, and a separate download log. The app displays the ten most recent batches, formatting individual download sizes for display without changing stored logs.
Use a history entry's **Use this URL again** button to prefill a previous batch URL.
Project files store the URL, detected media type, target folder, estimated/downloaded size, and download log; the ten most recent project files appear in **File → Recent**.

## Project structure

- Shared settings and types are in `src/models/structs.rs`.
- Download state is in `src/models/download_session.rs`; inspection and downloads run through `src/models/download_engine.rs`.
- Formatting, locale loading, and storage modules are under `src/models/`.
- Each dialog is implemented in its own file under `src/dialogs/`.
- English and Russian UI resources are stored separately in `src/locales/`.
- Unit tests are grouped under `tests/unit/`; integration tests are grouped under `tests/integration/`.

## Validate

```powershell
.\scripts\verify.ps1
```

This formats Rust files, checks formatting, runs the default tests, and builds the application.

The live YouTube integration tests require network access, `yt-dlp`, and `ffmpeg`; they inspect live metadata, estimate playlist sizes, and download media. They are ignored by default. Run them explicitly with:

```powershell
.\scripts\test-integration.ps1
```

The Git pre-push hook runs the verification script. It formats files in the working tree, checks formatting, runs the default tests, and builds; live integration tests remain excluded. Enable the repository hook after cloning with:

```powershell
git config core.hooksPath .githooks
```
