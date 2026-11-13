use crate::models::{
    locales::{Label, LocaleStrings},
    structs::Language,
};
use eframe::egui;

#[derive(Default)]
pub struct RestoreDefaultsDialog {
    open: bool,
    center_on_open: bool,
}

impl RestoreDefaultsDialog {
    pub fn open(&mut self) {
        if !self.open {
            self.center_on_open = true;
        }
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn show(
        &mut self,
        context: &egui::Context,
        locales: &LocaleStrings,
        language: Language,
    ) -> bool {
        if !self.open {
            return false;
        }

        let mut open = self.open;
        let mut confirm = false;
        let mut cancel = false;
        let window = egui::Window::new(locales.text(language, Label::RestoreDefaults))
            .id(egui::Id::new("restore_defaults_confirmation"))
            .open(&mut open)
            .default_width(320.0)
            .resizable(false)
            .collapsible(false);
        super::center_on_open(window, context, &mut self.center_on_open).show(context, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.label(locales.text(language, Label::RestoreDefaultsPrompt));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        confirm = ui
                            .button(locales.text(language, Label::RestoreDefaults))
                            .clicked();
                        cancel = ui
                            .button(locales.text(language, Label::CancelPreview))
                            .clicked();
                    });
                });
        });
        self.open = open && !confirm && !cancel;
        confirm
    }
}
