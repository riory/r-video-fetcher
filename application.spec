This file outlines current project requirements.

Application Name: r-video-fetcher
Type: desktop
Language: Rust
Backend: standalone / yt-dlp / ffmpeg

Scope: utility application / console application

Features:
- pass url to single video or playlist
- for each video run download routine
- should have default setting for audio (frequency, mono/stereo etc) and video (resolution, playback)
    - should allow per-action override if requested, othervise use default
- save to predetermined folder - default is {VIDEOHOST}/{CHANNEL-NAME}/{PLAYLIST if applicable}/{VIDEO DESCRIPTION}
    - alternativelty allow to save to specified folder on disc


Download feature:
- estimate download files size
- check disk space available, warn if expected to overflow
- each file should be retried up to X times (default to 3) if not - should fail and added to download report
- for each video ensure that download is complete, verify file integrity
- download process should be interactive (progress bar for each file and overall batch) 

Local history:
- each download should be saved for future reference and reuse, save as *.dwnl file
    - use JSON format to store: batch name, url, size, separately download log
- files should be saved under current user app data location
- GUI should allow to view last 10 download batches

Settings:
- default root folder for downloads
- max retry
- audio/video resolution
- language