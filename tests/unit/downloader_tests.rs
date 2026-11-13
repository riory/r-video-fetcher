use r_video_fetcher::{
    downloader::{
        DownloadPlan, download_concurrency_limit, download_output_directory, file_type_args,
        find_downloaded_file, format_selector, parse_metadata, parse_progress_percent,
        sanitize_path_component,
    },
    model::{AudioChannels, AudioSampleRate, FileType, Settings, VideoResolution},
};
use std::{
    env, fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn playlists_allow_four_concurrent_downloads_but_single_videos_do_not() {
    let plan = |playlist_name| DownloadPlan {
        source_url: "https://example.com".to_owned(),
        batch_name: "Test".to_owned(),
        playlist_name,
        items: Vec::new(),
        estimated_bytes: 0,
        unknown_size_count: 0,
        free_bytes: 0,
    };

    assert_eq!(
        download_concurrency_limit(&plan(Some("Playlist".to_owned()))),
        4
    );
    assert_eq!(download_concurrency_limit(&plan(None)), 1);
}

#[test]
fn finds_windows_sanitized_output_by_video_id() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = env::temp_dir().join(format!("r-video-fetcher-output-test-{unique}"));
    fs::create_dir_all(&directory).unwrap();
    let expected = directory.join(format!(
        "Konata Small - \u{ff02}For The Glory\u{ff02} (Official Audio) [tc9Q5VizsHE].mkv"
    ));
    fs::write(&expected, b"video").unwrap();

    let actual = find_downloaded_file(&directory, "tc9Q5VizsHE", "mkv").unwrap();

    assert_eq!(actual, expected);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn extracts_playlist_items_and_estimated_sizes() {
    let metadata = serde_json::json!({
        "title": "Playlist",
        "entries": [
            {
                "title": "First video",
                "webpage_url": "https://example.com/first",
                "uploader": "Channel",
                "filesize_approx": 100
            },
            {
                "id": "second",
                "url": "https://example.com/second",
                "filesize": 200
            }
        ]
    });

    let (batch, playlist, items) =
        parse_metadata(&metadata, "https://example.com/playlist").unwrap();

    assert_eq!(batch, "Playlist");
    assert_eq!(playlist.as_deref(), Some("Playlist"));
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].estimated_bytes, Some(100));
    assert_eq!(items[1].title, "second");
}

#[test]
fn flat_playlist_metadata_uses_channel_handle_and_expected_folder_name() {
    let metadata = serde_json::json!({
        "title": "Smiles 4 Grade 4 Pupil’s book",
        "uploader_id": "@Agylshyntyly",
        "entries": [
            {
                "title": "First video",
                "url": "https://www.youtube.com/watch?v=first"
            },
            {
                "title": "Second video",
                "url": "https://www.youtube.com/watch?v=second"
            }
        ]
    });

    let (batch, playlist, items) =
        parse_metadata(&metadata, "https://www.youtube.com/playlist?list=test").unwrap();

    assert_eq!(batch, "Smiles 4 Grade 4 Pupil’s book");
    assert_eq!(playlist.as_deref(), Some("Smiles 4 Grade 4 Pupil’s book"));
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].channel, "Agylshyntyly");
    assert!(items.iter().all(|item| item.estimated_bytes.is_none()));
    assert!(
        items
            .iter()
            .all(|item| item.url.starts_with("https://www.youtube.com/watch?v="))
    );
    assert_eq!(
        download_output_directory(std::path::Path::new("root"), &items[0], playlist.as_deref()),
        std::path::PathBuf::from("root/Agylshyntyly/Smiles_4_Grade_4_Pupils_book")
    );
}

#[test]
fn uploader_handle_is_used_for_single_video_output_folder() {
    let metadata = serde_json::json!({
        "title": "Rainy City Night - Royalty Free Stock Footage",
        "uploader_id": "@RoyaltyFreeVideos",
        "webpage_url": "https://www.youtube.com/watch?v=GpcaJQ40q1Y"
    });

    let (batch, playlist, items) =
        parse_metadata(&metadata, "https://www.youtube.com/watch?v=GpcaJQ40q1Y").unwrap();

    assert_eq!(batch, "Rainy City Night - Royalty Free Stock Footage");
    assert_eq!(playlist, None);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].channel, "RoyaltyFreeVideos");
    assert_eq!(
        download_output_directory(std::path::Path::new("root"), &items[0], None),
        std::path::PathBuf::from("root/RoyaltyFreeVideos")
    );
}

#[test]
fn builds_resolution_and_audio_format_preferences() {
    let settings = Settings {
        video_resolution: VideoResolution::P1080,
        audio_sample_rate: AudioSampleRate::Hz44100,
        audio_channels: AudioChannels::Mono,
        ..Settings::default()
    };
    assert_eq!(
        format_selector(&settings),
        "bestvideo[height<=1080]+bestaudio[asr=44100][audio_channels=1]/best[height<=1080]/best"
    );
}

#[test]
fn selects_recode_arguments_for_mp4_file_type() {
    let mut settings = Settings::default();
    assert!(file_type_args(&settings).is_empty());

    settings.file_type = FileType::Mp4;
    assert_eq!(file_type_args(&settings), &["--recode-video", "mp4"]);
}

#[test]
fn parses_download_progress_and_rejects_non_progress_lines() {
    assert_eq!(
        parse_progress_percent("[download]  42.5% of 10MiB at 1MiB/s"),
        Some(42.5)
    );
    assert_eq!(
        parse_progress_percent("[download] Destination: file.mp4"),
        None
    );
    assert_eq!(parse_progress_percent("[download] 101%"), None);
}

#[test]
fn sanitizes_path_traversal_and_windows_reserved_characters() {
    assert_eq!(sanitize_path_component("..\\..\\video%"), "_.._video_");
    assert_eq!(sanitize_path_component("..."), "unknown");
    assert_eq!(
        sanitize_path_component("Smiles 4 Grade 4 Pupil’s book"),
        "Smiles_4_Grade_4_Pupils_book"
    );
}
