use eframe::egui::{
    self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, epaint::ClippedShape,
};
use r_video_fetcher::{
    dialogs::settings::SettingsDialog,
    models::{
        locales::LocaleStrings,
        structs::{Language, Settings},
    },
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
    let mut folder = String::new();
    context
        .run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1024.0, 900.0))),
                events,
                ..Default::default()
            },
            |context| dialog.show(context, settings, &mut folder, locales),
        )
        .shapes
}

fn click(
    context: &egui::Context,
    dialog: &mut SettingsDialog,
    settings: &mut Settings,
    locales: &LocaleStrings,
    pos: Pos2,
) -> Vec<ClippedShape> {
    for pressed in [true, false] {
        let shapes = frame(
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
            return shapes;
        }
    }
    unreachable!()
}

#[test]
fn settings_dialog_switches_the_language() {
    let context = egui::Context::default();
    let locales = LocaleStrings::load().unwrap();
    let mut dialog = SettingsDialog::default();
    let mut settings = Settings::default();
    dialog.open();

    frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let position = text_position(&shapes, "English");
    click(&context, &mut dialog, &mut settings, &locales, position);
    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let position = text_position(&shapes, "Russian");
    click(&context, &mut dialog, &mut settings, &locales, position);

    assert_eq!(settings.language, Language::Russian);
    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    text_position(&shapes, "Язык интерфейса");

    let position = text_position(&shapes, "Русский");
    click(&context, &mut dialog, &mut settings, &locales, position);
    let shapes = frame(&context, &mut dialog, &mut settings, &locales, vec![]);
    let position = text_position(&shapes, "Английский");
    click(&context, &mut dialog, &mut settings, &locales, position);
    assert_eq!(settings.language, Language::English);
}
