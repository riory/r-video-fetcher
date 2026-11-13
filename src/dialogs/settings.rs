use crate::{
    dialogs::{folder_picker, restore_defaults::RestoreDefaultsDialog},
    models::locales::{Label, LocaleStrings},
    models::structs::{
        AppTheme, AudioChannels, AudioSampleRate, FileType, Language, MAX_RETRIES,
        MAX_ZOOM_PERCENT, MIN_ZOOM_PERCENT, Settings, VideoResolution,
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
    center_on_open: bool,
    tab: SettingsTab,
    restore_defaults: RestoreDefaultsDialog,
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            center_on_open: false,
            tab: SettingsTab::Preferences,
            restore_defaults: RestoreDefaultsDialog::default(),
        }
    }
}

impl SettingsDialog {
    pub fn open(&mut self) {
        if !self.open {
            self.center_on_open = true;
        }
        self.open = true;
    }

    pub fn show(
        &mut self,
        context: &egui::Context,
        settings: &mut Settings,
        default_folder_text: &mut String,
        locales: &LocaleStrings,
    ) -> bool {
        self.show_impl(context, settings, default_folder_text, locales, None)
    }

    pub fn show_with_parent(
        &mut self,
        context: &egui::Context,
        settings: &mut Settings,
        default_folder_text: &mut String,
        locales: &LocaleStrings,
        parent: &eframe::Frame,
    ) -> bool {
        self.show_impl(
            context,
            settings,
            default_folder_text,
            locales,
            Some(parent),
        )
    }

    fn show_impl(
        &mut self,
        context: &egui::Context,
        settings: &mut Settings,
        default_folder_text: &mut String,
        locales: &LocaleStrings,
        parent: Option<&eframe::Frame>,
    ) -> bool {
        if !self.open {
            return false;
        }
        let mut open = self.open;
        let preferences_label = text(locales, settings.language, Label::Preferences);
        let init_label = text(locales, settings.language, Label::Init);
        let screen = context.screen_rect().size();
        let width = 640.0_f32.min((screen.x - 16.0).max(1.0));
        let height = 400.0_f32.min((screen.y - 48.0).max(1.0));
        let window = egui::Window::new(text(locales, settings.language, Label::Settings))
            .id(egui::Id::new("settings_dialog"))
            .open(&mut open)
            .default_size([width, height])
            .collapsible(false)
            .resizable(false);
        let mut restore_requested = false;
        super::center_on_open(window, context, &mut self.center_on_open)
            .show(context, |ui| {
                egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
               ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, SettingsTab::Preferences, preferences_label);
                    ui.selectable_value(&mut self.tab, SettingsTab::Init, init_label);
                });
                ui.separator();
                match self.tab {
                    SettingsTab::Preferences => {
                        show_preferences(ui, settings, default_folder_text, locales, parent);
                        restore_requested = ui
                            .button(text(locales, settings.language, Label::RestoreDefaults))
                            .clicked();
                    }
                    SettingsTab::Init => {
                        ui.code(
                            "winget install --id yt-dlp.yt-dlp --exact\nwinget install --id Gyan.FFmpeg.Shared --exact",
                        );
                    }
                }
            });
            });
        self.open = open;
        if !self.open {
            self.restore_defaults.close();
            return false;
        }
        if restore_requested {
            self.restore_defaults.open();
        }
        self.restore_defaults
            .show(context, locales, settings.language)
    }
}

fn show_preferences(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    default_folder_text: &mut String,
    locales: &LocaleStrings,
    parent: Option<&eframe::Frame>,
) {
    let language = settings.language;
    let from_source_video_label = text(locales, language, Label::FromSourceVideo);
    let best_label = text(locales, language, Label::Best);
    let default_theme_label = text(locales, language, Label::DefaultTheme);
    let dark_label = text(locales, language, Label::Dark);
    let english_label = text(locales, language, Label::English);
    let russian_label = text(locales, language, Label::Russian);
    let file_type_label = text(locales, language, Label::FileType);
    let mono_label = text(locales, language, Label::Mono);
    let mp4_label = text(locales, language, Label::Mp4);
    let stereo_label = text(locales, language, Label::Stereo);
    let theme_label = text(locales, language, Label::Theme);
    let zenburn_label = text(locales, language, Label::Zenburn);
    let choose_folder_label = text(locales, language, Label::ChooseFolder);
    let label_font = egui::TextStyle::Body.resolve(ui.style());
    let button_font = egui::TextStyle::Button.resolve(ui.style());
    let color = ui.visuals().text_color();
    let label_width = [
        Label::OutputFolder,
        Label::Retries,
        Label::FileType,
        Label::VideoResolution,
        Label::AudioSampleRate,
        Label::AudioChannels,
        Label::Language,
        Label::Theme,
        Label::ZoomFactor,
    ]
    .into_iter()
    .map(|label| {
        ui.fonts(|fonts| {
            fonts
                .layout_no_wrap(
                    text(locales, language, label).to_owned(),
                    label_font.clone(),
                    color,
                )
                .size()
                .x
        })
    })
    .fold(0.0, f32::max);
    let choose_button_width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(choose_folder_label.to_owned(), button_font, color)
            .size()
            .x
    }) + 2.0 * ui.spacing().button_padding.x;
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        let stacked = ui.available_width() < label_width + 12.0 + choose_button_width + 160.0;
        let input_width = settings_input_width(ui, label_width, choose_button_width, stacked);
        ui.heading(text(locales, language, Label::DownloadSettings));
        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                settings_label(
                    ui,
                    text(locales, language, Label::OutputFolder),
                    label_width,
                    stacked,
                );
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [input_width, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(default_folder_text),
                    );
                    if ui
                        .add_sized(
                            [choose_button_width, ui.spacing().interact_size.y],
                            egui::Button::new(choose_folder_label),
                        )
                        .clicked()
                    {
                        let initial_folder = PathBuf::from(default_folder_text.trim());
                        let folder = match parent {
                            Some(parent) => {
                                folder_picker::choose_folder_with_parent(&initial_folder, parent)
                            }
                            None => folder_picker::choose_folder(&initial_folder),
                        };
                        if let Some(folder) = folder {
                            *default_folder_text = folder.to_string_lossy().into_owned();
                        }
                    }
                });
                ui.end_row();

                settings_label(
                    ui,
                    text(locales, language, Label::Retries),
                    label_width,
                    stacked,
                );
                ui.add_sized(
                    [input_width, ui.spacing().interact_size.y],
                    egui::DragValue::new(&mut settings.max_retries).range(0..=MAX_RETRIES),
                );
                ui.end_row();

                settings_label(ui, file_type_label, label_width, stacked);
                egui::ComboBox::from_id_salt("file_type")
                    .width(input_width)
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

                settings_label(
                    ui,
                    text(locales, language, Label::VideoResolution),
                    label_width,
                    stacked,
                );
                egui::ComboBox::from_id_salt("video_resolution")
                    .width(input_width)
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

                settings_label(
                    ui,
                    text(locales, language, Label::AudioSampleRate),
                    label_width,
                    stacked,
                );
                egui::ComboBox::from_id_salt("audio_sample_rate")
                    .width(input_width)
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

                settings_label(
                    ui,
                    text(locales, language, Label::AudioChannels),
                    label_width,
                    stacked,
                );
                egui::ComboBox::from_id_salt("audio_channels")
                    .width(input_width)
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
        ui.set_min_width(ui.available_width());
        let stacked = ui.available_width() < label_width + 12.0 + choose_button_width + 160.0;
        let input_width = settings_input_width(ui, label_width, choose_button_width, stacked);
        ui.heading(text(locales, language, Label::UiSettings));
        egui::Grid::new("settings_ui_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                settings_label(
                    ui,
                    text(locales, language, Label::Language),
                    label_width,
                    stacked,
                );
                egui::ComboBox::from_id_salt("ui_language")
                    .width(input_width)
                    .selected_text(match settings.language {
                        Language::English => english_label,
                        Language::Russian => russian_label,
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut settings.language,
                            Language::English,
                            english_label,
                        );
                        ui.selectable_value(
                            &mut settings.language,
                            Language::Russian,
                            russian_label,
                        );
                    });
                ui.end_row();

                settings_label(ui, theme_label, label_width, stacked);
                egui::ComboBox::from_id_salt("app_theme")
                    .width(input_width)
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

                settings_label(
                    ui,
                    text(locales, language, Label::ZoomFactor),
                    label_width,
                    stacked,
                );
                egui::ComboBox::from_id_salt("zoom_factor")
                    .width(input_width)
                    .selected_text(format!("{}%", settings.zoom_percent))
                    .show_ui(ui, |ui| {
                        for zoom in (MIN_ZOOM_PERCENT..=MAX_ZOOM_PERCENT).step_by(10) {
                            ui.selectable_value(
                                &mut settings.zoom_percent,
                                zoom,
                                format!("{zoom}%"),
                            );
                        }
                    });
                ui.end_row();
            });
    });

    ui.add_space(8.0);
    ui.small(text(locales, language, Label::SettingsNote));
    ui.add_space(8.0);
}

fn settings_input_width(ui: &egui::Ui, label_width: f32, button_width: f32, stacked: bool) -> f32 {
    (ui.available_width()
        - if stacked { 0.0 } else { label_width + 12.0 }
        - button_width
        - ui.spacing().item_spacing.x)
        .max(0.0)
}

fn settings_label(ui: &mut egui::Ui, label: &str, width: f32, stacked: bool) {
    ui.add_sized(
        [width, ui.spacing().interact_size.y],
        egui::Label::new(label),
    );
    if stacked {
        ui.end_row();
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
