use r_video_fetcher::{
    app::{parse_clipboard_url, progress_weights},
    downloader::{DownloadPlan, MediaItem},
    model::{AudioChannels, AudioSampleRate, MediaOverrides, Settings, VideoResolution},
};

#[test]
fn weights_batch_progress_by_file_size_and_average_unknown_sizes() {
    let plan = DownloadPlan {
        source_url: "https://example.com/playlist".to_owned(),
        batch_name: "Playlist".to_owned(),
        playlist_name: Some("Playlist".to_owned()),
        items: vec![
            MediaItem {
                title: "Small".to_owned(),
                url: "https://example.com/small".to_owned(),
                channel: "Channel".to_owned(),
                estimated_bytes: Some(100),
            },
            MediaItem {
                title: "Large".to_owned(),
                url: "https://example.com/large".to_owned(),
                channel: "Channel".to_owned(),
                estimated_bytes: Some(300),
            },
            MediaItem {
                title: "Unknown".to_owned(),
                url: "https://example.com/unknown".to_owned(),
                channel: "Channel".to_owned(),
                estimated_bytes: None,
            },
        ],
        estimated_bytes: 400,
        unknown_size_count: 1,
        free_bytes: 1_000,
    };

    assert_eq!(progress_weights(&plan), vec![100.0, 300.0, 200.0]);
}

#[test]
fn clipboard_accepts_only_http_urls_with_a_host() {
    assert_eq!(
        parse_clipboard_url(" https://example.com/video "),
        Some("https://example.com/video".to_owned())
    );
    assert!(parse_clipboard_url("file:///C:/video.mp4").is_none());
    assert!(parse_clipboard_url("not a url").is_none());
}

#[test]
fn media_overrides_change_only_the_batch_settings() {
    let defaults = Settings::default();
    let mut batch_settings = defaults.clone();

    batch_settings.apply_media_overrides(MediaOverrides {
        video_resolution: VideoResolution::P720,
        audio_sample_rate: AudioSampleRate::Hz44100,
        audio_channels: AudioChannels::Mono,
    });

    assert_eq!(batch_settings.video_resolution, VideoResolution::P720);
    assert_eq!(batch_settings.audio_sample_rate, AudioSampleRate::Hz44100);
    assert_eq!(batch_settings.audio_channels, AudioChannels::Mono);
    assert_eq!(defaults.video_resolution, VideoResolution::Best);
    assert_eq!(defaults.audio_sample_rate, AudioSampleRate::Hz48000);
    assert_eq!(defaults.audio_channels, AudioChannels::Stereo);
}
