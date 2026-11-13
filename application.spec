This file outlines current project requirements.

Application Name: r-video-fetcher
Type: desktop
Language: Rust
Backend: standalone / yt-dlp / ffmpeg

Scope: utility application / console application

Features:
- pass url to single video or playlist
- for each video run download routine
- download playlist items concurrently, with a maximum of 4 active downloads
- should have default setting for audio (frequency, mono/stereo etc) and video (resolution, playback)
    - always show batch settings initialized from saved global preferences
    - persist each batch-setting change to the active saved project without changing global preferences
- save to predetermined folder - default is {VIDEOHOST}/{CHANNEL-NAME}/{PLAYLIST if applicable}/{VIDEO DESCRIPTION}
    - alternativelty allow to save to specified folder on disc


Download feature:
- estimate download files size
- check disk space available, warn if expected to overflow
- each file should be retried up to X times (default to 3) if not - should fail and added to download report
- for each video ensure that download is complete, verify file integrity
- download process should be interactive (progress bar for each file and overall batch) 
- provide an Abort control for each download item; aborting one item stops its retries and does not stop the rest of the batch
- keep the download plan, estimate, warnings, file estimates, and batch settings visible while a batch is downloading and after it completes
- disable the Download button while the batch is downloading; re-enable it after completion
- show an enabled Abort button while downloading; aborting the batch stops active downloads and prevents pending items from starting
- after completion, show the Cancel button again and keep it enabled
- show inspection status in the status bar, including playlist metadata retrieval, per-item size estimation, and disk-space checks
- estimate playlist item sizes concurrently and report completed, active, and unavailable item counts

Local history:
- each download should be saved for future reference and reuse, save as *.dwnl file
    - use JSON format to store: batch name, url, size, target folder, channel, playlist, separately download log
    - show one link to the composed target/channel/playlist folder after the "Use this URL again" button
    - download log entries are plain text and do not contain folder links
- files should be saved under current user app data location
- GUI should allow to view last 10 download batches

Window states:
- on application exit save main window rect coordinates
- on application start restore main window coordinates (clamp to current screen resolution)

Settings:
- Download
    - default root folder for downloads
    - max retry
    - audio/video resolution
    - audio sample rate and channels default to "From source video"
    - file format: "From source video" (default) / MP4 (recode)
- User interface
    - UI language (English or Russian)
    - Theme (default/dark/Zenburn)
    - Zoom factor
- Buttons
    - Restore defaults (behind confirmation dialog)

Coding guidelines:
- use structured approach, ensure adhering to DRY and SOLID 
- code should be structured, when possible use self-explaining variable and function names
- code should be human readable
- code should be unit testable
- tests should be separate from source code, use top-level 'tests' folder
- each form/dialog should be in a separate file
- resource strings should be stored separately from code, each locale should be stored as separate file under 'src/locales' folder
- keep the locale resource lookup in place for English and Russian
- DO NOT CUT CORNERS!
    - no ad-hoc string parsing when proper object property can be used
    - no ad-hoc unit tests in source files!!!!!

UI:
- main window
    - top menu
        - File
            - New (Ctrl-N)
                - Clear out main form, no url or recent downloads
            - Open (Ctrl-O)
                - Clear out main form, no url or recent downloads
                - Load url, any overrides
            - Save (Ctrl-S)
                - Save current project to file, file name should be in format {channel}-{video name}.dwnl
                    - if matched with existing file, do not re-create
            - --separator--
            - Recent (list of recently opened files)
            - --separator--
            - Settings (Ctrl-,)
            - on click opens modal window with Settings
                - tab 1: Preferences (lists global preferences)
                    - group download preferences separately from UI preferences
                    - Changing any value should trigger automatic save
                    - No confirm button
                - tab 2: Init (lists commands to initiate ps1 script that calls winget to install necessary dependencies (e.g. yt-dlp))
                    - Only list commands as text, single multi-line block
            - Exit (Alt-F4)
                - Save current project if url is not empty
                - Terminate app
        - Download
            - From clipboard (Ctrl-B)
                - check clipboard has string, it is valid url
                - open, see if playlist or single video
                - create new file, save passed url, file name should be in format {channel}-{video name}.dwnl
                    - if matched with existing file, do not re-create
                - update project file
                - for each video generate filesystem friendly name
                    - replace spaces with underscore
                - download list should be scrollable
                - start download
                - save to recent
            - --separator--
            - Show history
                - opens new modal window with download history
            - Clear history
                - should show confirmation dialog
                    - if confirmed, clear all recent downloads        
- work space:
    - list current url
    - list type (playlist/single video)                
    - target folder (should be hyperlink that on click opens system directory search dialog, on close updates target folder)
        - default to settings value
    - always show batch video resolution, audio sample rate, and audio channel settings
    - button to start download
        - on click, start download, update progress bars (current file, overall batch)      
        - keep the plan and all fields visible while downloading and after completion
        - disable Download during the batch, then re-enable Download and restore the enabled Cancel button when it completes
        - show an enabled Abort button during the batch that aborts the entire batch
        - show an Abort button per item; disable it when the item is verifying or has completed, failed, or been aborted
    - should be spread out, colors should be contrasting
    - only shows current project, no history

- status bar
    - Contains single label that shows current status/events/errors


Test cases:
    - integration:
        - Only the source URL is provided as media input; do not provide or mock media files or metadata.
        - Inspect each URL at runtime to obtain video URLs, channel names, playlist names, and file names.
        - Download real media from each URL and verify the resulting non-empty files exist under the current project folder + /download-test.
        - Use a unique subfolder under /download-test for each run to avoid stale files satisfying download checks.
        - load playlist: https://www.youtube.com/watch?v=m4hjb4d8ri0&list=PLUiUn_cZl605Fpznit1AyT_c6d993P68K
            - should resolve to valid url
            - should return 50 video files
            - should return list name: Smiles 4 Grade 4 Pupil’s book
            - should provide a size estimate for each video
            - download first 2 files
            - ensure two non-empty downloaded files exist under the runtime-derived channel and playlist folders
        - load single file: https://www.youtube.com/watch?v=GpcaJQ40q1Y
            - should resolve to valid url
            - should return 1 video file
            - should return video name: Rainy City Night - Royalty Free Stock Footage
            - download the Rainy City Night clip
            - ensure a non-empty downloaded file exists under the runtime-derived channel folder