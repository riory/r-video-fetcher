use eframe::egui::{self, Pos2, RawInput, Rect};
use r_video_fetcher::{
    dialogs::history::HistoryDialog,
    models::{locales::LocaleStrings, structs::Language},
};

fn frame(context: &egui::Context, dialog: &mut HistoryDialog, locales: &LocaleStrings) {
    let _ = context.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1024.0, 900.0))),
            ..Default::default()
        },
        |context| {
            dialog
                .show(context, locales, Language::English, false)
                .unwrap();
        },
    );
}

fn assert_centered(context: &egui::Context, id: &str) {
    let rect = egui::containers::AreaState::load(context, egui::Id::new(id))
        .unwrap()
        .rect();
    assert!(
        (rect.center() - egui::pos2(512.0, 450.0)).length() <= 1.5,
        "{id} not centered: {rect:?}"
    );
}

#[test]
fn history_and_confirmation_open_centered() {
    let context = egui::Context::default();
    let locales = LocaleStrings::load().unwrap();
    let mut dialog = HistoryDialog::new(Vec::new());

    dialog.open();
    frame(&context, &mut dialog, &locales);
    frame(&context, &mut dialog, &locales);
    assert_centered(&context, "download_history_window");

    dialog.confirm_clear();
    frame(&context, &mut dialog, &locales);
    frame(&context, &mut dialog, &locales);
    assert_centered(&context, "clear_history_confirmation_window");
}
