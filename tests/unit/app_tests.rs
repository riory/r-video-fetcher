use r_video_fetcher::{
    app::{VideoFetcherApp, parse_clipboard_url, suggested_project_file_name},
    models::structs::{
        AppTheme, AudioChannels, AudioSampleRate, FileType, MediaOverrides, Settings,
        VideoResolution, WindowState,
    },
    models::{
        download_session::{DownloadPlan, DownloadSession, MediaItem, progress_weights},
        project::{MediaType, ProjectState},
    },
    themes,
};

#[test]
fn enter_previews_only_when_url_editor_is_focused() {
    use eframe::egui::{self, Event, Key, Modifiers, Pos2, RawInput, Rect};
    use r_video_fetcher::models::{
        locales::{Label, LocaleStrings},
        structs::Language,
    };

    let context = egui::Context::default();
    let locales = LocaleStrings::load().unwrap();
    let mut app = VideoFetcherApp::new(LocaleStrings::load().unwrap());
    let mut app_frame = eframe::Frame::_new_kittest();
    let mut run = |events| {
        context.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1024.0, 900.0))),
                events,
                ..Default::default()
            },
            |ctx| eframe::App::update(&mut app, ctx, &mut app_frame),
        )
    };
    let enter = |pressed| Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let has_url_error = |output: &egui::FullOutput| {
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Text(text) => [Language::English, Language::Russian]
                .iter()
                .any(|language| text.galley.job.text == locales.text(*language, Label::EnterUrl)),
            _ => false,
        })
    };

    let _ = run(vec![]);
    let _ = run(vec![enter(true)]);
    let output = run(vec![enter(false)]);
    assert!(!has_url_error(&output));

    context.memory_mut(|memory| memory.request_focus(egui::Id::new("source_url")));
    let _ = run(vec![enter(true)]);
    let output = run(vec![enter(false)]);
    assert!(has_url_error(&output));
}

#[test]
fn weights_batch_progress_by_file_size_and_average_unknown_sizes() {
    let items = vec![
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
    ];

    assert_eq!(progress_weights(&items), vec![100.0, 300.0, 200.0]);
}

fn test_plan() -> DownloadPlan {
    DownloadPlan {
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
    }
}

#[test]
fn download_session_tracks_inspection_progress_and_terminal_states() {
    let plan = test_plan();
    let mut session = DownloadSession::default();

    session.begin_inspection(true);
    assert!(session.is_inspecting());
    assert!(session.is_busy());
    assert!(session.finish_inspection());

    session.set_plan(plan.clone());
    assert_eq!(session.overall_progress(), 0.0);
    assert_eq!(session.plan().unwrap().batch_name, plan.batch_name);
    assert_eq!(session.begin_download().unwrap().items.len(), 3);
    assert!(session.is_downloading());
    assert_eq!(session.plan().unwrap().batch_name, plan.batch_name);

    session.mark_attempt_started(0, 1, 4);
    session.report_progress(0, 50.0);
    session.mark_verifying(0);
    assert!(!session.mark_aborting(0));
    session.mark_completed(0, 100);
    assert!(session.mark_aborting(1));
    session.mark_aborted(1);
    session.mark_failed(2, "download failed".to_owned(), false);

    assert_eq!(session.aborted_count(), 1);
    assert_eq!(session.failed_count(), 1);
    assert!(session.overall_progress() > 0.0);
    session.finish_download();
    assert!(!session.is_busy());
    assert_eq!(session.plan().unwrap().batch_name, plan.batch_name);
}

#[test]
fn project_state_commits_editor_url_into_project_inputs() {
    let overrides = MediaOverrides::from(&Settings::default());
    let mut state = ProjectState::new("downloads".into(), overrides);

    *state.source_url_mut() = " https://example.com/video ".to_owned();
    state.commit_source_url();
    assert_eq!(state.project().url, "https://example.com/video");

    state.set_inputs("downloads".into(), overrides);
    assert_eq!(state.project().url, state.source_url().trim());
}

#[test]
fn saving_project_state_preserves_preview_metadata() {
    let overrides = MediaOverrides::from(&Settings::default());
    let mut state = ProjectState::new("downloads".into(), overrides);
    state.apply_preview(
        "Video".to_owned(),
        "https://example.com/video".to_owned(),
        123,
        "downloads".into(),
        Some("channel".to_owned()),
        Some("playlist".to_owned()),
        MediaType::Playlist,
        overrides,
    );

    state.prepare_for_save(overrides).unwrap();

    assert_eq!(state.project().channel.as_deref(), Some("channel"));
    assert_eq!(state.project().playlist.as_deref(), Some("playlist"));
    assert_eq!(state.project().url, "https://example.com/video");
}

#[test]
fn new_inspection_inputs_clear_old_download_folder_metadata() {
    let overrides = MediaOverrides::from(&Settings::default());
    let mut state = ProjectState::new("downloads".into(), overrides);
    state.apply_preview(
        "Video".to_owned(),
        "https://example.com/video".to_owned(),
        123,
        "downloads".into(),
        Some("channel".to_owned()),
        Some("playlist".to_owned()),
        MediaType::Playlist,
        overrides,
    );
    assert!(state.project().download_folder().is_some());

    state.set_inputs("downloads".into(), overrides);

    assert!(state.project().download_folder().is_none());
    assert_eq!(state.project().media_type, None);
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
fn project_filename_uses_sanitized_channel_and_video_title() {
    assert_eq!(
        suggested_project_file_name("RoyaltyFreeVideos", "Rainy City Night - Stock Footage"),
        "RoyaltyFreeVideos-Rainy_City_Night_-_Stock_Footage.dwnl"
    );
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
    assert_eq!(defaults.audio_sample_rate, AudioSampleRate::FromSourceVideo);
    assert_eq!(defaults.audio_channels, AudioChannels::FromSourceVideo);
    assert_eq!(defaults.file_type, FileType::FromSourceVideo);
    assert_eq!(AudioSampleRate::default(), AudioSampleRate::FromSourceVideo);
    assert_eq!(AudioChannels::default(), AudioChannels::FromSourceVideo);
}

#[test]
fn batch_settings_are_initialized_from_system_settings() {
    let system_settings = Settings {
        video_resolution: VideoResolution::P720,
        audio_sample_rate: AudioSampleRate::Hz44100,
        audio_channels: AudioChannels::Mono,
        ..Settings::default()
    };

    assert_eq!(
        MediaOverrides::from(&system_settings),
        MediaOverrides {
            video_resolution: VideoResolution::P720,
            audio_sample_rate: AudioSampleRate::Hz44100,
            audio_channels: AudioChannels::Mono,
        }
    );
}

#[test]
fn theme_defaults_to_default_and_older_settings_remain_compatible() {
    assert_eq!(Settings::default().theme, AppTheme::Default);
    let older_settings: Settings = serde_json::from_str("{}").unwrap();

    assert_eq!(older_settings.theme, AppTheme::Default);
    assert_eq!(older_settings.window_state, None);
}

#[test]
fn theme_presets_select_the_expected_visual_mode() {
    assert!(!themes::visuals(AppTheme::Default).dark_mode);
    assert!(themes::visuals(AppTheme::Dark).dark_mode);

    let zenburn = themes::visuals(AppTheme::Zenburn);
    assert!(zenburn.dark_mode);
    assert_eq!(
        zenburn.panel_fill,
        eframe::egui::Color32::from_rgb(63, 63, 63)
    );
}

#[test]
fn window_state_clamps_size_and_position_to_the_current_screen() {
    let restored = WindowState {
        x: -200.0,
        y: 900.0,
        width: 1500.0,
        height: 800.0,
    }
    .clamp_to([1280.0, 720.0], [16.0, 40.0]);

    assert_eq!(
        restored,
        WindowState {
            x: 0.0,
            y: 0.0,
            width: 1264.0,
            height: 680.0,
        }
    );
}
