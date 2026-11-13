use r_video_fetcher::{
    models::project::{DownloadLogEntry, HistoryRecord, MediaType},
    models::storage::{
        clear_history_from, load_recent_history_from, remember_recent_file, write_json,
    },
    models::structs::{
        AppTheme, AudioChannels, AudioSampleRate, Language, MediaOverrides, Settings,
        VideoResolution, WindowState,
    },
};
use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn writes_and_reads_json_history_files() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = env::temp_dir().join(format!("r-video-fetcher-test-{unique}"));
    let path = directory.join("batch.dwnl");
    let expected = HistoryRecord {
        batch_name: "Test batch".to_owned(),
        url: "https://example.com/video".to_owned(),
        created_at_unix_seconds: 1,
        size_bytes: 42,
        download_log: vec![DownloadLogEntry::DownloadMessage {
            message: "Downloaded test video.".to_owned(),
        }],
        target_folder: PathBuf::from("downloads"),
        channel: Some("positionmusic".to_owned()),
        playlist: Some("Konata_Small_Playlist".to_owned()),
        media_type: Some(MediaType::Playlist),
        media_overrides: Some(MediaOverrides {
            video_resolution: VideoResolution::P720,
            audio_sample_rate: AudioSampleRate::Hz44100,
            audio_channels: AudioChannels::Mono,
        }),
    };

    write_json(&path, &expected).unwrap();
    let actual: HistoryRecord = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();

    assert_eq!(actual.batch_name, expected.batch_name);
    assert_eq!(actual.url, expected.url);
    assert_eq!(actual.size_bytes, expected.size_bytes);
    assert_eq!(actual.download_log, expected.download_log);
    assert_eq!(actual.target_folder, expected.target_folder);
    assert_eq!(actual.channel, expected.channel);
    assert_eq!(actual.playlist, expected.playlist);
    assert_eq!(
        actual.download_folder(),
        Some(
            PathBuf::from("downloads")
                .join("positionmusic")
                .join("Konata_Small_Playlist")
        )
    );
    assert_eq!(actual.media_type, expected.media_type);
    assert_eq!(actual.media_overrides, expected.media_overrides);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn recent_projects_are_deduplicated_and_limited_to_ten() {
    let mut settings = Settings::default();
    let paths = (0..11)
        .map(|index| PathBuf::from(format!("project-{index}.dwnl")))
        .collect::<Vec<_>>();
    for path in &paths {
        remember_recent_file(&mut settings, path);
    }
    remember_recent_file(&mut settings, &paths[5]);

    assert_eq!(settings.recent_files.len(), 10);
    assert_eq!(settings.recent_files[0], paths[5]);
    assert!(!settings.recent_files.contains(&paths[0]));
}

#[test]
fn recent_history_is_limited_to_ten_and_clear_preserves_non_history_files() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = env::temp_dir().join(format!("r-video-fetcher-history-test-{unique}"));
    fs::create_dir_all(&directory).unwrap();
    for index in 0..12 {
        let record = HistoryRecord {
            batch_name: format!("Batch {index}"),
            url: format!("https://example.com/{index}"),
            created_at_unix_seconds: index,
            size_bytes: index,
            download_log: Vec::new(),
            target_folder: PathBuf::new(),
            channel: None,
            playlist: None,
            media_type: None,
            media_overrides: None,
        };
        write_json(&directory.join(format!("batch-{index:02}.dwnl")), &record).unwrap();
    }
    let non_history_file = directory.join("notes.txt");
    fs::write(&non_history_file, "keep").unwrap();
    let non_history_directory = directory.join("archive.dwnl");
    fs::create_dir(&non_history_directory).unwrap();

    let recent = load_recent_history_from(&directory).unwrap();

    assert_eq!(recent.len(), 10);
    assert_eq!(recent[0].batch_name, "Batch 11");
    assert_eq!(recent[9].batch_name, "Batch 2");
    clear_history_from(&directory).unwrap();

    assert!(non_history_file.is_file());
    assert!(non_history_directory.is_dir());
    assert_eq!(
        fs::read_dir(&directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "dwnl")
                    && entry.path().is_file()
            })
            .count(),
        0
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn settings_without_recent_projects_keep_compatibility() {
    let settings: Settings =
        serde_json::from_str(r#"{"download_root":"downloads","max_retries":3}"#).unwrap();

    assert!(settings.recent_files.is_empty());
    assert_eq!(settings.max_retries, 3);
    assert_eq!(settings.zoom_percent, 100);
}

#[test]
fn restoring_preferences_preserves_recent_files_and_window_state() {
    let old = Settings {
        download_root: "custom".into(),
        max_retries: 9,
        theme: AppTheme::Zenburn,
        language: Language::Russian,
        zoom_percent: 150,
        recent_files: vec!["recent.dwnl".into()],
        window_state: Some(WindowState {
            x: 10.0,
            y: 20.0,
            width: 700.0,
            height: 600.0,
        }),
        ..Settings::default()
    };
    let defaults = Settings {
        download_root: "default-downloads".into(),
        ..Settings::default()
    };

    let restored = old.restore_preferences(defaults.clone());
    assert_eq!(restored.download_root, defaults.download_root);
    assert_eq!(restored.max_retries, defaults.max_retries);
    assert_eq!(restored.theme, defaults.theme);
    assert_eq!(restored.language, defaults.language);
    assert_eq!(restored.zoom_percent, defaults.zoom_percent);
    assert_eq!(restored.recent_files, old.recent_files);
    assert_eq!(restored.window_state, old.window_state);
}
