use crate::{
    dialogs::folder_picker,
    locales::{Label, LocaleStrings},
    model::{
        AppTheme, AudioChannels, AudioSampleRate, FileType, Language, MAX_RETRIES, Settings,
        VideoResolution,
    },
};
use eframe::egui;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    Preferences,
    Init,
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
    ) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        let preferences_label = text(locales, settings.language, Label::Preferences);
        let init_label = text(locales, settings.language, Label::Init);
        egui::Window::new(text(locales, settings.language, Label::Settings))
            .open(&mut open)
            .default_size([640.0, 560.0])
            .min_size([640.0, 560.0])
            .resizable(false)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, SettingsTab::Preferences, preferences_label);
                    ui.selectable_value(&mut self.tab, SettingsTab::Init, init_label);
                });
                ui.separator();
                match self.tab {
                    SettingsTab::Preferences => show_preferences(
                        ui,
                        settings,
                        default_folder_text,
                        locales,
                    ),
                    SettingsTab::Init => {
                        ui.code(
                            "winget install --id yt-dlp.yt-dlp --exact\nwinget install --id Gyan.FFmpeg.Shared --exact",
                        );
                    }
                }
            });
        self.open = open;
    }
}

fn show_preferences(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    default_folder_text: &mut String,
    locales: &LocaleStrings,
) {
    let language = settings.language;
    let from_source_video_label = text(locales, language, Label::FromSourceVideo);
    let best_label = text(locales, language, Label::Best);
    let default_theme_label = text(locales, language, Label::DefaultTheme);
    let dark_label = text(locales, language, Label::Dark);
    let english_label = text(locales, language, Label::English);
    let file_type_label = text(locales, language, Label::FileType);
    let mono_label = text(locales, language, Label::Mono);
    let mp4_label = text(locales, language, Label::Mp4);
    let stereo_label = text(locales, language, Label::Stereo);
    let theme_label = text(locales, language, Label::Theme);
    let zenburn_label = text(locales, language, Label::Zenburn);
    ui.group(|ui| {
        ui.heading(text(locales, language, Label::DownloadSettings));
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

                ui.label(file_type_label);
                egui::ComboBox::from_id_salt("file_type")
                    .selected_text(match settings.file_type {
                        FileType::FromSourceVideo => from_source_video_label,
                        FileType::Mp4 => mp4_label,
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut settings.file_type,
                            FileType::FromSourceVideo,
                            from_source_video_label,
                        );
                        ui.selectable_value(&mut settings.file_type, FileType::Mp4, mp4_label);
                    });
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
                            AudioSampleRate::FromSourceVideo,
                            from_source_video_label,
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
                            AudioChannels::FromSourceVideo,
                            from_source_video_label,
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
            });
    });
    ui.add_space(8.0);
    ui.group(|ui| {
        ui.heading(text(locales, language, Label::UiSettings));
        egui::Grid::new("settings_ui_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(text(locales, language, Label::Language));
                ui.label(english_label);
                ui.end_row();

                ui.label(theme_label);
                egui::ComboBox::from_id_salt("app_theme")
                    .selected_text(match settings.theme {
                        AppTheme::Default => default_theme_label,
                        AppTheme::Dark => dark_label,
                        AppTheme::Zenburn => zenburn_label,
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut settings.theme,
                            AppTheme::Default,
                            default_theme_label,
                        );
                        ui.selectable_value(&mut settings.theme, AppTheme::Dark, dark_label);
                        ui.selectable_value(&mut settings.theme, AppTheme::Zenburn, zenburn_label);
                    });
                ui.end_row();
            });
    });
    ui.small(text(locales, language, Label::SettingsNote));
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
        AudioSampleRate::FromSourceVideo => Label::FromSourceVideo,
        AudioSampleRate::Hz44100 => Label::SampleRate44100,
        AudioSampleRate::Hz48000 => Label::SampleRate48000,
    }
}

fn channel_label(channels: AudioChannels) -> Label {
    match channels {
        AudioChannels::FromSourceVideo => Label::FromSourceVideo,
        AudioChannels::Mono => Label::Mono,
        AudioChannels::Stereo => Label::Stereo,
    }
}
