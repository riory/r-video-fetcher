pub mod folder_picker;
pub mod history;
pub mod open_project;
pub mod restore_defaults;
pub mod save_project;
pub mod settings;

use eframe::egui;

fn center_on_open<'a>(
    window: egui::Window<'a>,
    context: &egui::Context,
    pending: &mut bool,
) -> egui::Window<'a> {
    let window = window.pivot(egui::Align2::CENTER_CENTER);
    if std::mem::take(pending) {
        window.current_pos(context.screen_rect().center())
    } else {
        window
    }
}
