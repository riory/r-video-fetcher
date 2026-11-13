use crate::{
    models::formatting::{format_bytes, format_download_log_message},
    models::locales::{Label, LocaleStrings},
    models::project::{DownloadLogEntry, HistoryRecord},
    models::storage,
    models::structs::Language,
};
use anyhow::Result;
use eframe::egui;
use std::path::Path;

pub enum HistoryDialogAction {
    Reuse(HistoryRecord),
    Cleared,
}

pub struct HistoryDialog {
    history: Vec<HistoryRecord>,
    open: bool,
    center_on_open: bool,
    clear_confirmation_open: bool,
    center_confirmation_on_open: bool,
}

impl HistoryDialog {
    pub fn new(history: Vec<HistoryRecord>) -> Self {
        Self {
            history,
            open: false,
            center_on_open: false,
            clear_confirmation_open: false,
            center_confirmation_on_open: false,
        }
    }

    pub fn open(&mut self) {
        if !self.open {
            self.center_on_open = true;
        }
        self.open = true;
    }

    pub fn confirm_clear(&mut self) {
        if !self.clear_confirmation_open {
            self.center_confirmation_on_open = true;
        }
        self.clear_confirmation_open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.clear_confirmation_open = false;
    }

    pub fn set_history(&mut self, history: Vec<HistoryRecord>) {
        self.history = history;
    }

    pub fn show(
        &mut self,
        context: &egui::Context,
        locales: &LocaleStrings,
        language: Language,
        busy: bool,
    ) -> Result<Option<HistoryDialogAction>> {
        let selected_record = self.show_history(context, locales, language, busy)?;
        if self.show_clear_history_confirmation(context, locales, language)? {
            return Ok(Some(HistoryDialogAction::Cleared));
        }
        Ok(selected_record.map(HistoryDialogAction::Reuse))
    }

    fn show_history(
        &mut self,
        context: &egui::Context,
        locales: &LocaleStrings,
        language: Language,
        busy: bool,
    ) -> Result<Option<HistoryRecord>> {
        if !self.open {
            return Ok(None);
        }
        let title = locales.text(language, Label::History).to_owned();
        let no_history = locales.text(language, Label::NoHistory).to_owned();
        let reuse_label = locales.text(language, Label::Reuse).to_owned();
        let mut selected_record = None;
        let mut open = self.open;
        let mut open_target_folder = None;
        let min_window_size = 200.0 / context.pixels_per_point();
        let window = egui::Window::new(title)
            .id(egui::Id::new("download_history_window"))
            .open(&mut open)
            .min_size([min_window_size, min_window_size])
            .resizable(true)
            .collapsible(false);
        super::center_on_open(window, context, &mut self.center_on_open).show(context, |ui| {
            ui.set_min_size(egui::vec2(min_window_size, min_window_size));
            if self.history.is_empty() {
                ui.label(no_history);
            } else {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (index, record) in self.history.iter().enumerate() {
                            egui::CollapsingHeader::new(format!(
                                "{}. {} - {}",
                                index + 1,
                                record.batch_name,
                                format_bytes(record.size_bytes)
                            ))
                            .id_salt(("history", index))
                            .show(ui, |ui| {
                                ui.label(&record.url);
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled(!busy, egui::Button::new(&reuse_label))
                                        .clicked()
                                    {
                                        selected_record = Some(record.clone());
                                    }
                                    if let Some(folder) = record.download_folder() {
                                        let folder_label = folder.display().to_string();
                                        let link = ui
                                            .add(
                                                egui::Label::new(
                                                    egui::RichText::new(folder_label)
                                                        .underline()
                                                        .color(ui.visuals().hyperlink_color),
                                                )
                                                .sense(egui::Sense::click()),
                                            )
                                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                                        if link.clicked() {
                                            open_target_folder = Some(folder);
                                        }
                                    }
                                });
                                for entry in &record.download_log {
                                    match entry {
                                        DownloadLogEntry::GenericMessage { message } => {
                                            ui.label(message);
                                        }
                                        DownloadLogEntry::DownloadMessage { message } => {
                                            ui.label(format_download_log_message(message).as_ref());
                                        }
                                    }
                                }
                            });
                        }
                    });
            }
        });
        if let Some(folder) = open_target_folder {
            storage::open_folder(Path::new(&folder))?;
        }
        self.open = open;
        if selected_record.is_some() {
            self.open = false;
        }
        Ok(selected_record)
    }

    fn show_clear_history_confirmation(
        &mut self,
        context: &egui::Context,
        locales: &LocaleStrings,
        language: Language,
    ) -> Result<bool> {
        if !self.clear_confirmation_open {
            return Ok(false);
        }
        let title = locales.text(language, Label::ClearHistory).to_owned();
        let prompt = locales.text(language, Label::ClearHistoryPrompt).to_owned();
        let clear_label = locales
            .text(language, Label::ClearHistoryConfirm)
            .to_owned();
        let cancel_label = locales.text(language, Label::CancelPreview).to_owned();
        let mut open = self.clear_confirmation_open;
        let mut confirm = false;
        let mut cancel = false;
        let window = egui::Window::new(title)
            .id(egui::Id::new("clear_history_confirmation_window"))
            .open(&mut open)
            .default_width(320.0)
            .resizable(false)
            .collapsible(false);
        super::center_on_open(window, context, &mut self.center_confirmation_on_open).show(
            context,
            |ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.label(prompt);
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            confirm = ui.button(&clear_label).clicked();
                            cancel = ui.button(&cancel_label).clicked();
                        });
                    });
            },
        );
        if confirm {
            self.close();
            storage::clear_history()?;
            self.history.clear();
            Ok(true)
        } else {
            self.clear_confirmation_open = open && !cancel;
            Ok(false)
        }
    }
}
