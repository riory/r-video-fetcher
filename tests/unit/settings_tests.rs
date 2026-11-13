use eframe::egui::{
    self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, epaint::ClippedShape,
};
use r_video_fetcher::{
    app::VideoFetcherApp,
    dialogs::settings::SettingsDialog,
    models::{locales::LocaleStrings, storage::write_json, structs::Settings},
};
use std::{
    env, fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn text_position(shapes: &[ClippedShape], label: &str) -> Pos2 {
    shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                Some(text.pos + text.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            let labels: Vec<_> = shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                    _ => None,
                })
                .collect();
            panic!("Could not find UI label: {label}; visible labels: {labels:?}")
        })
}

fn frame(
    context: &egui::Context,
    dialog: &mut SettingsDialog,
    settings: &mut Settings,
    locales: &LocaleStrings,
    events: Vec<Event>,
) -> Vec<ClippedShape> {
    frame_with_action(context, dialog, settings, locales, events).0
}

fn frame_with_action(
    context: &egui::Context,
    dialog: &mut SettingsDialog,
    settings: &mut Settings,
    locales: &LocaleStrings,
    events: Vec<Event>,
) -> (Vec<ClippedShape>, bool) {
    let mut folder = String::new();
    let mut restore = false;
    let output = context.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1024.0, 900.0))),
            events,
            ..Default::default()
        },
        |context| {
            restore = dialog.show(context, settings, &mut folder, locales);
        },
    );
    (output.shapes, restore)
}

fn click(
    context: &egui::Context,
    dialog: &mut SettingsDialog,
    settings: &mut Settings,
    locales: &LocaleStrings,
    pos: Pos2,
) -> Vec<ClippedShape> {
    click_with_action(context, dialog, settings, locales, pos).0
}

fn click_with_action(
    context: &egui::Context,
    dialog: &mut SettingsDialog,
    settings: &mut Settings,
    locales: &LocaleStrings,
    pos: Pos2,
) -> (Vec<ClippedShape>, bool) {
    for pressed in [true, false] {
        let output = frame_with_action(
            context,
            dialog,
            settings,
            locales,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        if !pressed {
            return output;
        }
    }
    unreachable!()
}

#[test]
fn settings_dialog_selects_zoom_factor() {
    let context = egui::Context::default();
    let locales = LocaleStrings::load().unwrap();
    let mut dialog = SettingsDialog::default();
    let mut settings = Settings::default();
    dialog.open();
    frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let position = text_position(&shapes, "100%");
    click(&context, &mut dialog, &mut settings, &locales, position);
    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let position = text_position(&shapes, "130%");
    click(&context, &mut dialog, &mut settings, &locales, position);
    assert_eq!(settings.zoom_percent, 130);
}

#[test]
fn app_applies_saved_zoom_and_persists_keyboard_zoom() {
    const CHILD: &str = "R_VIDEO_FETCHER_ZOOM_TEST_CHILD";
    if env::var_os(CHILD).is_some() {
        let context = egui::Context::default();
        let mut app = VideoFetcherApp::new(LocaleStrings::load().unwrap());
        let mut frame = eframe::Frame::_new_kittest();
        let input = || RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1024.0, 900.0))),
            ..Default::default()
        };
        let _ = context.run(input(), |context| {
            eframe::App::update(&mut app, context, &mut frame)
        });
        let _ = context.run(input(), |context| {
            eframe::App::update(&mut app, context, &mut frame)
        });
        assert!((context.zoom_factor() - 1.5).abs() < 0.001);

        let _ = context.run(input(), |context| {
            eframe::App::update(&mut app, context, &mut frame);
            context.set_zoom_factor(1.62);
        });
        let _ = context.run(input(), |context| {
            eframe::App::update(&mut app, context, &mut frame)
        });
        let _ = context.run(input(), |context| {
            eframe::App::update(&mut app, context, &mut frame)
        });
        let path = env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .unwrap()
            .join("r-video-fetcher")
            .join("settings.json");
        let saved: Settings = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(saved.zoom_percent, 160);
        assert!((context.zoom_factor() - 1.6).abs() < 0.001);
        return;
    }

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = env::temp_dir().join(format!(
        "r-video-fetcher-zoom-{}-{unique}",
        std::process::id()
    ));
    let settings_path = directory.join("r-video-fetcher").join("settings.json");
    write_json(
        &settings_path,
        &Settings {
            download_root: directory.join("downloads"),
            zoom_percent: 150,
            ..Settings::default()
        },
    )
    .unwrap();
    let output = Command::new(env::current_exe().unwrap())
        .args([
            "--exact",
            "settings_tests::app_applies_saved_zoom_and_persists_keyboard_zoom",
        ])
        .env(CHILD, "1")
        .env("APPDATA", &directory)
        .env("LOCALAPPDATA", &directory)
        .env("USERPROFILE", &directory)
        .output()
        .unwrap();
    fs::remove_file(settings_path).unwrap();
    fs::remove_dir(directory.join("r-video-fetcher")).unwrap();
    fs::remove_dir(directory).unwrap();
    assert!(
        output.status.success(),
        "child test failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn restoring_defaults_requires_confirmation() {
    let context = egui::Context::default();
    let locales = LocaleStrings::load().unwrap();
    let mut dialog = SettingsDialog::default();
    let mut settings = Settings {
        zoom_percent: 150,
        ..Settings::default()
    };
    dialog.open();
    frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let position = text_position(&shapes, "Restore defaults");
    let (_, restore) = click_with_action(&context, &mut dialog, &mut settings, &locales, position);
    assert!(!restore);

    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let position = text_position(&shapes, "Cancel");
    let (_, restore) = click_with_action(&context, &mut dialog, &mut settings, &locales, position);
    assert!(!restore);
    assert_eq!(settings.zoom_percent, 150);

    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let position = text_position(&shapes, "Restore defaults");
    click(&context, &mut dialog, &mut settings, &locales, position);
    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let rect =
        egui::containers::AreaState::load(&context, egui::Id::new("restore_defaults_confirmation"))
            .unwrap()
            .rect();
    let position = shapes
        .iter()
        .rev()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) if text.galley.job.text == "Restore defaults" => {
                let center = text.pos + text.galley.size() / 2.0;
                (rect.contains(center) && center.y > rect.top() + 35.0).then_some(center)
            }
            _ => None,
        })
        .expect("restore confirmation button");
    let (_, restore) = click_with_action(&context, &mut dialog, &mut settings, &locales, position);
    assert!(restore);
}

#[test]
fn settings_dialog_opens_centered_and_remains_movable() {
    let context = egui::Context::default();
    let locales = LocaleStrings::load().unwrap();
    let mut dialog = SettingsDialog::default();
    let mut settings = Settings::default();
    dialog.open();

    frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let area = || {
        egui::containers::AreaState::load(&context, egui::Id::new("settings_dialog"))
            .unwrap()
            .rect()
    };
    let rect = area();
    assert!((rect.center() - egui::pos2(512.0, 450.0)).length() <= 1.5);

    let start = egui::pos2(rect.center().x, rect.top() + 10.0);
    frame(
        &context,
        &mut dialog,
        &mut settings,
        &locales,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    let end = start + egui::vec2(60.0, 30.0);
    frame(
        &context,
        &mut dialog,
        &mut settings,
        &locales,
        vec![Event::PointerMoved(end)],
    );
    frame(
        &context,
        &mut dialog,
        &mut settings,
        &locales,
        vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!((area().center() - rect.center()).length() > 20.0);

    dialog = SettingsDialog::default();
    dialog.open();
    frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    assert!((area().center() - egui::pos2(512.0, 450.0)).length() <= 1.5);
}
