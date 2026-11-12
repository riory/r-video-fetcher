use r_video_fetcher::{
    downloader::{
        download_output_directory, format_selector, parse_metadata, parse_progress_percent,
        sanitize_path_component,
    },
    model::{AudioChannels, AudioSampleRate, Settings, VideoResolution},
};

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
