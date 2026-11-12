use crate::{
    dialogs::folder_picker,
    locales::{Label, LocaleStrings},
    model::{AudioChannels, AudioSampleRate, Language, MAX_RETRIES, Settings, VideoResolution},
};
use eframe::egui;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    Preferences,
    Init,
}

pub enum SettingsAction {
    SaveDefaults,
    InstallDependencies,
}

pub struct SettingsDialog {
    open: bool,
    tab: SettingsTab,
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            tab: SettingsTab::Preferences,
        }
    }
}

impl SettingsDialog {
    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn show(
        &mut self,
        context: &egui::Context,
        settings: &mut Settings,
        default_folder_text: &mut String,
        locales: &LocaleStrings,
    ) -> Option<SettingsAction> {
        if !self.open {
            return None;
        }
        let mut open = self.open;
        let mut action = None;
        let preferences_label = text(locales, settings.language, Label::Preferences);
        let init_label = text(locales, settings.language, Label::Init);
        egui::Window::new(text(locales, settings.language, Label::Settings))
            .open(&mut open)
            .resizable(true)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, SettingsTab::Preferences, preferences_label);
                    ui.selectable_value(&mut self.tab, SettingsTab::Init, init_label);
                });
                ui.separator();
                match self.tab {
                    SettingsTab::Preferences => {
                        show_preferences(ui, settings, default_folder_text, locales, &mut action)
                    }
                    SettingsTab::Init => {
                        ui.label(text(
                            locales,
                            settings.language,
                            Label::InstallerDescription,
                        ));
                        ui.code("winget install --id yt-dlp.yt-dlp --exact");
                        ui.code("winget install --id Gyan.FFmpeg.Shared --exact");
                        if ui
                            .button(text(locales, settings.language, Label::InstallDependencies))
                            .clicked()
                        {
                            action = Some(SettingsAction::InstallDependencies);
                        }
                    }
                }
            });
        self.open = open;
        action
    }
}

fn show_preferences(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    default_folder_text: &mut String,
    locales: &LocaleStrings,
    action: &mut Option<SettingsAction>,
) {
    let language = settings.language;
    let any_label = text(locales, language, Label::Any);
    let best_label = text(locales, language, Label::Best);
    let english_label = text(locales, language, Label::English);
    let mono_label = text(locales, language, Label::Mono);
    let russian_label = text(locales, language, Label::Russian);
    let stereo_label = text(locales, language, Label::Stereo);
    egui::Grid::new("settings_grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label(text(locales, language, Label::OutputFolder));
            ui.horizontal(|ui| {
                ui.text_edit_singleline(default_folder_text);
                if ui
                    .button(text(locales, language, Label::ChooseFolder))
                    .clicked()
                    && let Some(folder) =
                        folder_picker::choose_folder(&PathBuf::from(default_folder_text.trim()))
                {
                    *default_folder_text = folder.to_string_lossy().into_owned();
                }
            });
            ui.end_row();

            ui.label(text(locales, language, Label::Retries));
            ui.add(egui::DragValue::new(&mut settings.max_retries).range(0..=MAX_RETRIES));
            ui.end_row();

            ui.label(text(locales, language, Label::VideoResolution));
            egui::ComboBox::from_id_salt("video_resolution")
                .selected_text(text(
                    locales,
                    language,
                    resolution_label(settings.video_resolution),
                ))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut settings.video_resolution,
                        VideoResolution::Best,
                        best_label,
                    );
                    for (resolution, label) in [
                        (VideoResolution::P360, Label::Resolution360),
                        (VideoResolution::P480, Label::Resolution480),
                        (VideoResolution::P720, Label::Resolution720),
                        (VideoResolution::P1080, Label::Resolution1080),
                        (VideoResolution::P1440, Label::Resolution1440),
                        (VideoResolution::P2160, Label::Resolution2160),
                    ] {
                        ui.selectable_value(
                            &mut settings.video_resolution,
                            resolution,
                            text(locales, language, label),
                        );
                    }
                });
            ui.end_row();

            ui.label(text(locales, language, Label::AudioSampleRate));
            egui::ComboBox::from_id_salt("audio_sample_rate")
                .selected_text(text(
                    locales,
                    language,
                    sample_rate_label(settings.audio_sample_rate),
                ))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut settings.audio_sample_rate,
                        AudioSampleRate::Any,
                        any_label,
                    );
                    for (rate, label) in [
                        (AudioSampleRate::Hz44100, Label::SampleRate44100),
                        (AudioSampleRate::Hz48000, Label::SampleRate48000),
                    ] {
                        ui.selectable_value(
                            &mut settings.audio_sample_rate,
                            rate,
                            text(locales, language, label),
                        );
                    }
                });
            ui.end_row();

            ui.label(text(locales, language, Label::AudioChannels));
            egui::ComboBox::from_id_salt("audio_channels")
                .selected_text(text(
                    locales,
                    language,
                    channel_label(settings.audio_channels),
                ))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut settings.audio_channels,
                        AudioChannels::Any,
                        any_label,
                    );
                    ui.selectable_value(
                        &mut settings.audio_channels,
                        AudioChannels::Mono,
                        mono_label,
                    );
                    ui.selectable_value(
                        &mut settings.audio_channels,
                        AudioChannels::Stereo,
                        stereo_label,
                    );
                });
            ui.end_row();

            ui.label(text(locales, language, Label::Language));
            egui::ComboBox::from_id_salt("language")
                .selected_text(text(locales, language, language_label(language)))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut settings.language, Language::English, english_label);
                    ui.selectable_value(&mut settings.language, Language::Russian, russian_label);
                });
            ui.end_row();
        });
    ui.small(text(locales, language, Label::SettingsNote));
    if ui
        .button(text(locales, language, Label::SaveDefaults))
        .clicked()
    {
        *action = Some(SettingsAction::SaveDefaults);
    }
}

fn text(locales: &LocaleStrings, language: Language, label: Label) -> &'static str {
    locales.text(language, label)
}

fn resolution_label(resolution: VideoResolution) -> Label {
    match resolution {
        VideoResolution::Best => Label::Best,
        VideoResolution::P360 => Label::Resolution360,
        VideoResolution::P480 => Label::Resolution480,
        VideoResolution::P720 => Label::Resolution720,
        VideoResolution::P1080 => Label::Resolution1080,
        VideoResolution::P1440 => Label::Resolution1440,
        VideoResolution::P2160 => Label::Resolution2160,
    }
}

fn sample_rate_label(sample_rate: AudioSampleRate) -> Label {
    match sample_rate {
        AudioSampleRate::Any => Label::Any,
        AudioSampleRate::Hz44100 => Label::SampleRate44100,
        AudioSampleRate::Hz48000 => Label::SampleRate48000,
    }
}

fn channel_label(channels: AudioChannels) -> Label {
    match channels {
        AudioChannels::Any => Label::Any,
        AudioChannels::Mono => Label::Mono,
        AudioChannels::Stereo => Label::Stereo,
    }
}

fn language_label(language: Language) -> Label {
    match language {
        Language::English => Label::English,
        Language::Russian => Label::Russian,
    }
}
