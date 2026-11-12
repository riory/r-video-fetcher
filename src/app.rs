use crate::{
    dialogs::{
        folder_picker, open_project as open_project_dialog, save_project as save_project_dialog,
        settings::{SettingsAction, SettingsDialog},
    },
    downloader::{self, DownloadEvent, DownloadPlan},
    locales::{Label, LocaleStrings},
    model::{
        AudioChannels, AudioSampleRate, HistoryRecord, MediaOverrides, MediaType, Settings,
        VideoResolution,
    },
    storage,
};
use eframe::egui::{self, Color32};
use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::Url;

const INSTALLER_SCRIPT: &str = include_str!("../scripts/install-dependencies.ps1");

pub struct VideoFetcherApp {
    locales: LocaleStrings,
    settings: Settings,
    media_overrides: Option<MediaOverrides>,
    default_folder_text: String,
    target_folder_text: String,
    source_url: String,
    current_project: HistoryRecord,
    current_project_path: Option<PathBuf>,
    history: Vec<HistoryRecord>,
    plan: Option<DownloadPlan>,
    progress: Vec<FileProgress>,
    events_tx: Sender<DownloadEvent>,
    events_rx: Receiver<DownloadEvent>,
    planning: bool,
    downloading: bool,
    download_after_inspection: bool,
    settings_dialog: SettingsDialog,
    status: String,
    error: Option<String>,
}

#[derive(Clone)]
struct FileProgress {
    title: String,
    percent: f32,
    weight: f64,
    status: String,
}

enum MenuAction {
    New,
    Open,
    Save,
    Exit,
    FromClipboard,
    OpenRecent(PathBuf),
    ShowSettings,
}

impl VideoFetcherApp {
    pub fn new(locales: LocaleStrings) -> Self {
        let mut errors = Vec::new();
        let settings = match storage::load_settings() {
            Ok(settings) => settings,
            Err(error) => {
                errors.push(format!("{error:#}"));
                match storage::default_settings() {
                    Ok(settings) => settings,
                    Err(default_error) => {
                        errors.push(format!("{default_error:#}"));
                        Settings::default()
                    }
                }
            }
        };
        let history = match storage::load_recent_history() {
            Ok(history) => history,
            Err(error) => {
                errors.push(format!("{error:#}"));
                Vec::new()
            }
        };
        let default_folder_text = settings.download_root.to_string_lossy().into_owned();
        let target_folder_text = settings.download_root.to_string_lossy().into_owned();
        let (events_tx, events_rx) = mpsc::channel();
        let current_project = empty_project(settings.download_root.clone());

        Self {
            locales,
            settings,
            media_overrides: None,
            default_folder_text,
            target_folder_text,
            source_url: String::new(),
            current_project,
            current_project_path: None,
            history,
            plan: None,
            progress: Vec::new(),
            events_tx,
            events_rx,
            planning: false,
            downloading: false,
            download_after_inspection: false,
            settings_dialog: SettingsDialog::default(),
            status: String::new(),
            error: (!errors.is_empty()).then(|| errors.join("\n")),
        }
    }

    fn text(&self, label: Label) -> &'static str {
        self.locales.text(self.settings.language, label)
    }

    fn start_inspection(&mut self) {
        self.current_project.url = self.source_url.trim().to_owned();
        self.current_project.target_folder = PathBuf::from(self.target_folder_text.trim());
        if self.source_url.trim().is_empty() {
            self.download_after_inspection = false;
            self.error = Some(self.text(Label::EnterUrl).to_owned());
            return;
        }
        if self.current_project.target_folder.as_os_str().is_empty() {
            self.download_after_inspection = false;
            self.error = Some(self.text(Label::ChooseFolderPreview).to_owned());
            return;
        }

        self.error = None;
        self.status = self.text(Label::Planning).to_owned();
        self.plan = None;
        self.progress.clear();
        let mut settings = self.settings.clone();
        settings.download_root = self.current_project.target_folder.clone();
        self.planning = true;
        let url = self.source_url.trim().to_owned();
        let events = self.events_tx.clone();
        thread::spawn(move || {
            let result =
                downloader::inspect_batch(&url, &settings).map_err(|error| format!("{error:#}"));
            let _ = events.send(DownloadEvent::PlanReady(result));
        });
    }

    fn start_download(&mut self) {
        let Some(plan) = self.plan.take() else {
            return;
        };
        self.current_project.url = plan.source_url.clone();
        self.current_project.target_folder = PathBuf::from(self.target_folder_text.trim());
        self.progress = initial_progress(&plan, self.text(Label::Ready));
        self.error = None;
        self.status = self.text(Label::Downloading).to_owned();
        self.downloading = true;
        let mut settings = self.settings.clone();
        settings.download_root = self.current_project.target_folder.clone();
        if let Some(overrides) = self.media_overrides.take() {
            settings.apply_media_overrides(overrides);
        }
        let events = self.events_tx.clone();
        thread::spawn(move || {
            let result = downloader::download_batch(plan, settings, events.clone())
                .map_err(|error| format!("{error:#}"));
            let _ = events.send(DownloadEvent::Finished(result));
        });
    }

    fn save_defaults(&mut self) {
        let default_folder = PathBuf::from(self.default_folder_text.trim());
        if default_folder.as_os_str().is_empty() {
            self.error = Some(self.text(Label::ChooseFolderSave).to_owned());
            return;
        }
        self.settings.download_root = default_folder;
        match storage::save_settings(&self.settings) {
            Ok(()) => {
                if self.current_project_path.is_none() && self.source_url.is_empty() {
                    self.target_folder_text = self.default_folder_text.clone();
                    self.current_project.target_folder = self.settings.download_root.clone();
                }
                self.error = None;
                self.status.clear();
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    fn new_project(&mut self) {
        self.current_project = empty_project(self.settings.download_root.clone());
        self.current_project_path = None;
        self.media_overrides = None;
        self.target_folder_text = self.settings.download_root.to_string_lossy().into_owned();
        self.source_url.clear();
        self.plan = None;
        self.progress.clear();
        self.error = None;
        self.status.clear();
    }

    fn open_project_dialog(&mut self) {
        if let Some(path) = open_project_dialog::open_project() {
            self.open_project(&path);
        }
    }

    fn open_project(&mut self, path: &std::path::Path) {
        match storage::load_project(path) {
            Ok(project) => {
                self.target_folder_text = if project.target_folder.as_os_str().is_empty() {
                    self.settings.download_root.to_string_lossy().into_owned()
                } else {
                    project.target_folder.to_string_lossy().into_owned()
                };
                self.source_url = project.url.clone();
                self.current_project = project;
                self.current_project_path = Some(path.to_owned());
                self.media_overrides = None;
                self.plan = None;
                self.progress.clear();
                self.error = None;
                self.status = self
                    .text(Label::OpenProjectStatus)
                    .replace("{path}", &path.display().to_string());
                if let Err(error) = self.remember_project(path) {
                    self.error = Some(format!("{error:#}"));
                }
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    fn save_project_dialog(&mut self) {
        let suggested_name = if self.current_project.batch_name.is_empty() {
            "download.dwnl".to_owned()
        } else {
            format!("{}.dwnl", self.current_project.batch_name)
        };
        if let Some(path) = save_project_dialog::save_project(&suggested_name) {
            match self.save_project_to(&path) {
                Ok(()) => {
                    self.status = self
                        .text(Label::SaveProjectStatus)
                        .replace("{path}", &path.display().to_string());
                }
                Err(error) => self.error = Some(format!("{error:#}")),
            }
        }
    }

    fn save_project(&mut self) {
        if let Some(path) = self.current_project_path.clone() {
            match self.save_project_to(&path) {
                Ok(()) => {
                    self.status = self
                        .text(Label::SaveProjectStatus)
                        .replace("{path}", &path.display().to_string());
                }
                Err(error) => self.error = Some(format!("{error:#}")),
            }
        } else {
            self.save_project_dialog();
        }
    }

    fn save_current_project_if_named(&mut self) -> anyhow::Result<()> {
        if let Some(path) = self.current_project_path.clone() {
            self.save_project_to(&path)?;
        }
        Ok(())
    }

    fn save_project_to(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        self.current_project.url = self.source_url.trim().to_owned();
        self.current_project.target_folder = PathBuf::from(self.target_folder_text.trim());
        if self.current_project.created_at_unix_seconds == 0 {
            self.current_project.created_at_unix_seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| anyhow::anyhow!("System clock is invalid: {error}"))?
                .as_secs();
        }
        storage::save_project(path, &self.current_project)?;
        self.current_project_path = Some(path.to_owned());
        self.remember_project(path)?;
        self.error = None;
        Ok(())
    }

    fn remember_project(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        storage::remember_recent_file(&mut self.settings, path);
        storage::save_settings(&self.settings)
    }

    fn download_from_clipboard(&mut self) {
        if self.planning || self.downloading {
            return;
        }
        let clipboard_text =
            match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
                Ok(text) => text.trim().to_owned(),
                Err(error) => {
                    self.error = Some(
                        self.text(Label::ClipboardReadError)
                            .replace("{error}", &error.to_string()),
                    );
                    return;
                }
            };
        let clipboard_url = match parse_clipboard_url(&clipboard_text) {
            Some(url) => url,
            None => {
                self.error = Some(self.text(Label::ClipboardInvalid).to_owned());
                return;
            }
        };
        self.new_project();
        self.source_url = clipboard_url;
        self.current_project.url = self.source_url.clone();
        self.current_project_path = match storage::clipboard_project_path() {
            Ok(path) => Some(path),
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                return;
            }
        };
        if let Err(error) = self.save_current_project_if_named() {
            self.error = Some(format!("{error:#}"));
            return;
        }
        self.download_after_inspection = true;
        self.start_inspection();
    }

    fn choose_target_folder(&mut self) {
        let initial_folder = PathBuf::from(self.target_folder_text.trim());
        if let Some(folder) = folder_picker::choose_folder(&initial_folder) {
            self.target_folder_text = folder.to_string_lossy().into_owned();
            self.current_project.target_folder = folder;
            self.plan = None;
        }
    }

    fn launch_dependency_installer(&mut self) {
        let Some(local_app_data) = env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
            self.error = Some(self.text(Label::LocalAppDataMissing).to_owned());
            return;
        };
        let script = local_app_data
            .join("r-video-fetcher")
            .join("scripts")
            .join("install-dependencies.ps1");
        let Some(script_directory) = script.parent() else {
            self.error = Some(self.text(Label::InstallerDirectoryMissing).to_owned());
            return;
        };
        if let Err(error) =
            fs::create_dir_all(script_directory).and_then(|()| fs::write(&script, INSTALLER_SCRIPT))
        {
            self.error = Some(
                self.text(Label::InstallerPrepareError)
                    .replace("{path}", &script.display().to_string())
                    .replace("{error}", &error.to_string()),
            );
            return;
        }
        match Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-NoExit",
                "-File",
            ])
            .arg(&script)
            .spawn()
        {
            Ok(_) => {
                self.error = None;
                self.status = self.text(Label::InstallerOpened).to_owned();
            }
            Err(error) => {
                self.error = Some(
                    self.text(Label::InstallerLaunchError)
                        .replace("{error}", &error.to_string()),
                )
            }
        }
    }

    fn poll_events(&mut self) {
        while let Ok(event) = self.events_rx.try_recv() {
            match event {
                DownloadEvent::PlanReady(result) => {
                    self.planning = false;
                    match result {
                        Ok(plan) => {
                            let plan_status =
                                format!("{}: {}", self.text(Label::Plan), plan.batch_name);
                            self.progress = initial_progress(&plan, self.text(Label::Ready));
                            self.status = plan_status.clone();
                            self.error = None;
                            self.current_project.batch_name = plan.batch_name.clone();
                            self.current_project.url = plan.source_url.clone();
                            self.current_project.size_bytes = plan.estimated_bytes;
                            self.current_project.target_folder =
                                PathBuf::from(self.target_folder_text.trim());
                            self.current_project.media_type =
                                Some(if plan.playlist_name.is_some() {
                                    MediaType::Playlist
                                } else {
                                    MediaType::SingleVideo
                                });
                            self.plan = Some(plan);
                            let project_saved = match self.save_current_project_if_named() {
                                Ok(()) => true,
                                Err(error) => {
                                    self.error = Some(format!("{error:#}"));
                                    false
                                }
                            };
                            self.status = plan_status;
                            if self.download_after_inspection {
                                self.download_after_inspection = false;
                                if project_saved {
                                    self.start_download();
                                }
                            }
                        }
                        Err(error) => {
                            self.download_after_inspection = false;
                            self.error = Some(error);
                            self.plan = None;
                        }
                    }
                }
                DownloadEvent::AttemptStarted {
                    index,
                    attempt,
                    total_attempts,
                } => {
                    let status = self.text(Label::AttemptStatus).to_owned();
                    if let Some(file) = self.progress.get_mut(index) {
                        file.percent = 0.0;
                        file.status = status
                            .replace("{attempt}", &attempt.to_string())
                            .replace("{total_attempts}", &total_attempts.to_string());
                    }
                }
                DownloadEvent::Progress { index, percent } => {
                    if let Some(file) = self.progress.get_mut(index) {
                        file.percent = percent;
                    }
                }
                DownloadEvent::ItemCompleted { index, size_bytes } => {
                    let status = self.text(Label::CompleteStatus).to_owned();
                    if let Some(file) = self.progress.get_mut(index) {
                        file.percent = 100.0;
                        file.status = status.replace("{size}", &format_bytes(size_bytes));
                    }
                }
                DownloadEvent::ItemFailed { index, message } => {
                    if let Some(file) = self.progress.get_mut(index) {
                        if message.starts_with("Failed after") {
                            file.percent = 100.0;
                        }
                        file.status = message;
                    }
                }
                DownloadEvent::Finished(result) => {
                    self.downloading = false;
                    match result {
                        Ok(record) => {
                            let failed = self
                                .progress
                                .iter()
                                .filter(|file| file.status.starts_with("Failed"))
                                .count();
                            self.status = self
                                .text(Label::FinishedStatus)
                                .replace("{status}", self.text(Label::Completed))
                                .replace("{files}", &self.progress.len().to_string())
                                .replace("{failed}", &failed.to_string())
                                .replace("{size}", &format_bytes(record.size_bytes));
                            self.current_project = record.clone();
                            self.error = None;
                            if let Err(error) = self.save_current_project_if_named() {
                                self.error = Some(format!("{error:#}"));
                            }
                            match storage::load_recent_history() {
                                Ok(history) => self.history = history,
                                Err(error) => self.error = Some(format!("{error:#}")),
                            }
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
        }
    }

    fn show_settings_dialog(&mut self, context: &egui::Context) {
        let action = self.settings_dialog.show(
            context,
            &mut self.settings,
            &mut self.default_folder_text,
            &self.locales,
        );
        match action {
            Some(SettingsAction::SaveDefaults) => self.save_defaults(),
            Some(SettingsAction::InstallDependencies) => self.launch_dependency_installer(),
            None => {}
        }
    }

    fn show_menu_bar(&mut self, context: &egui::Context) -> Option<MenuAction> {
        let mut action = None;
        let recent_files = self.settings.recent_files.clone();
        let busy = self.planning || self.downloading;
        let file_label = self.text(Label::File);
        let new_label = self.text(Label::New);
        let open_label = self.text(Label::Open);
        let save_label = self.text(Label::Save);
        let recent_label = self.text(Label::RecentFiles);
        let exit_label = self.text(Label::Exit);
        let download_label = self.text(Label::DownloadMenu);
        let clipboard_label = self.text(Label::FromClipboard);
        let settings_label = self.text(Label::Settings);

        egui::TopBottomPanel::top("app_menu_bar").show(context, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(file_label, |ui| {
                    if ui
                        .add_enabled(!busy, egui::Button::new(format!("{new_label}    Ctrl+N")))
                        .clicked()
                    {
                        action = Some(MenuAction::New);
                        ui.close();
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new(format!("{open_label}    Ctrl+O")))
                        .clicked()
                    {
                        action = Some(MenuAction::Open);
                        ui.close();
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new(format!("{save_label}    Ctrl+S")))
                        .clicked()
                    {
                        action = Some(MenuAction::Save);
                        ui.close();
                    }
                    ui.separator();
                    ui.menu_button(recent_label, |ui| {
                        if recent_files.is_empty() {
                            ui.label(self.text(Label::NoRecentFiles));
                        }
                        for path in &recent_files {
                            let name = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned();
                            if ui
                                .add_enabled(!busy, egui::Button::new(name))
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                action = Some(MenuAction::OpenRecent(path.clone()));
                                ui.close();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button(format!("{exit_label}    Alt+F4")).clicked() {
                        action = Some(MenuAction::Exit);
                        ui.close();
                    }
                });
                ui.menu_button(download_label, |ui| {
                    if ui
                        .add_enabled(
                            !busy,
                            egui::Button::new(format!("{clipboard_label}    Ctrl+B")),
                        )
                        .clicked()
                    {
                        action = Some(MenuAction::FromClipboard);
                        ui.close();
                    }
                });
                if ui.button(settings_label).clicked() {
                    action = Some(MenuAction::ShowSettings);
                }
            });
        });
        action
    }

    fn handle_menu_action(&mut self, action: MenuAction, context: &egui::Context) {
        match action {
            MenuAction::New => self.new_project(),
            MenuAction::Open => self.open_project_dialog(),
            MenuAction::Save => self.save_project(),
            MenuAction::Exit => context.send_viewport_cmd(egui::ViewportCommand::Close),
            MenuAction::FromClipboard => self.download_from_clipboard(),
            MenuAction::OpenRecent(path) => self.open_project(&path),
            MenuAction::ShowSettings => self.settings_dialog.open(),
        }
    }

    fn show_plan(&mut self, ui: &mut egui::Ui) {
        let Some(plan) = &self.plan else {
            return;
        };
        let warning = plan.estimated_bytes > plan.free_bytes || plan.unknown_size_count > 0;
        ui.separator();
        ui.heading(format!("{}: {}", self.text(Label::Plan), plan.batch_name));
        ui.label(format!(
            "{}: {}",
            self.text(Label::EstimatedSize),
            format_bytes(plan.estimated_bytes)
        ));
        ui.label(format!(
            "{}: {}",
            self.text(Label::FreeSpace),
            format_bytes(plan.free_bytes)
        ));
        if plan.unknown_size_count > 0 {
            ui.colored_label(
                Color32::YELLOW,
                format!(
                    "{}: {}",
                    self.text(Label::SizeUnknown),
                    plan.unknown_size_count
                ),
            );
            ui.label(self.text(Label::UnknownSizeWarning));
        }
        if plan.estimated_bytes > plan.free_bytes {
            ui.colored_label(Color32::LIGHT_RED, self.text(Label::LowSpaceWarning));
        }
        ui.label(format!(
            "{}: {}",
            self.text(Label::Videos),
            plan.items.len()
        ));
        ui.collapsing(self.text(Label::FileEstimates), |ui| {
            for item in &plan.items {
                let estimate = item
                    .estimated_bytes
                    .map(format_bytes)
                    .unwrap_or_else(|| self.text(Label::SizeUnknown).to_owned());
                ui.label(format!("{} - {}", item.title, estimate));
            }
        });
        ui.horizontal(|ui| {
            let button = if warning {
                self.text(Label::DownloadAnyway)
            } else {
                self.text(Label::Download)
            };
            if ui.button(button).clicked() {
                self.start_download();
            }
            if ui.button(self.text(Label::CancelPreview)).clicked() {
                self.plan = None;
                self.progress.clear();
            }
        });
    }

    fn show_progress(&self, ui: &mut egui::Ui) {
        if self.progress.is_empty() {
            return;
        }
        ui.separator();
        ui.heading(self.text(Label::BatchProgress));
        let total_weight = self.progress.iter().map(|file| file.weight).sum::<f64>();
        let overall = self
            .progress
            .iter()
            .map(|file| file.percent as f64 * file.weight)
            .sum::<f64>()
            / total_weight;
        ui.add(egui::ProgressBar::new((overall / 100.0) as f32).text(format!("{overall:.1}%")));
        for file in &self.progress {
            ui.label(&file.title);
            ui.add(
                egui::ProgressBar::new(file.percent / 100.0)
                    .text(format!("{:.1}% - {}", file.percent, file.status)),
            );
        }
    }

    fn show_history(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading(self.text(Label::History));
        if self.history.is_empty() {
            ui.label(self.text(Label::Ready));
            return;
        }
        let history = self.history.clone();
        let reuse_label = self.text(Label::Reuse);
        for (index, record) in history.into_iter().enumerate() {
            egui::CollapsingHeader::new(format!(
                "{}. {} - {}",
                index + 1,
                record.batch_name,
                format_bytes(record.size_bytes)
            ))
            .id_salt(("history", index))
            .show(ui, |ui| {
                ui.label(&record.url);
                if ui
                    .add_enabled(
                        !self.planning && !self.downloading,
                        egui::Button::new(reuse_label),
                    )
                    .clicked()
                {
                    self.source_url = record.url.clone();
                    self.plan = None;
                    self.progress.clear();
                    self.error = None;
                }
                for line in &record.download_log {
                    ui.label(line);
                }
            });
        }
    }

    fn show_media_overrides(&mut self, ui: &mut egui::Ui) {
        let override_label = self.text(Label::BatchOverrides).to_owned();
        let note = self.text(Label::BatchOverridesNote).to_owned();
        let any_label = self.text(Label::Any).to_owned();
        let best_label = self.text(Label::Best).to_owned();
        let video_resolution_label = self.text(Label::VideoResolution).to_owned();
        let audio_sample_rate_label = self.text(Label::AudioSampleRate).to_owned();
        let audio_channels_label = self.text(Label::AudioChannels).to_owned();
        let mono_label = self.text(Label::Mono).to_owned();
        let stereo_label = self.text(Label::Stereo).to_owned();
        let resolution_labels = [
            self.text(Label::Resolution360).to_owned(),
            self.text(Label::Resolution480).to_owned(),
            self.text(Label::Resolution720).to_owned(),
            self.text(Label::Resolution1080).to_owned(),
            self.text(Label::Resolution1440).to_owned(),
            self.text(Label::Resolution2160).to_owned(),
        ];
        let sample_rate_labels = [
            self.text(Label::SampleRate44100).to_owned(),
            self.text(Label::SampleRate48000).to_owned(),
        ];

        let mut enabled = self.media_overrides.is_some();
        if ui.checkbox(&mut enabled, override_label).changed() {
            self.media_overrides = if enabled {
                Some(MediaOverrides {
                    video_resolution: self.settings.video_resolution,
                    audio_sample_rate: self.settings.audio_sample_rate,
                    audio_channels: self.settings.audio_channels,
                })
            } else {
                None
            };
        }
        let Some(overrides) = self.media_overrides.as_mut() else {
            return;
        };
        ui.small(note);
        egui::Grid::new("batch_media_overrides")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(&video_resolution_label);
                let selected_resolution = match overrides.video_resolution {
                    VideoResolution::Best => &best_label,
                    VideoResolution::P360 => &resolution_labels[0],
                    VideoResolution::P480 => &resolution_labels[1],
                    VideoResolution::P720 => &resolution_labels[2],
                    VideoResolution::P1080 => &resolution_labels[3],
                    VideoResolution::P1440 => &resolution_labels[4],
                    VideoResolution::P2160 => &resolution_labels[5],
                };
                egui::ComboBox::from_id_salt("batch_video_resolution")
                    .selected_text(selected_resolution)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut overrides.video_resolution,
                            VideoResolution::Best,
                            &best_label,
                        );
                        for (resolution, label) in [
                            (VideoResolution::P360, &resolution_labels[0]),
                            (VideoResolution::P480, &resolution_labels[1]),
                            (VideoResolution::P720, &resolution_labels[2]),
                            (VideoResolution::P1080, &resolution_labels[3]),
                            (VideoResolution::P1440, &resolution_labels[4]),
                            (VideoResolution::P2160, &resolution_labels[5]),
                        ] {
                            ui.selectable_value(&mut overrides.video_resolution, resolution, label);
                        }
                    });
                ui.end_row();

                ui.label(&audio_sample_rate_label);
                let selected_sample_rate = match overrides.audio_sample_rate {
                    AudioSampleRate::Any => &any_label,
                    AudioSampleRate::Hz44100 => &sample_rate_labels[0],
                    AudioSampleRate::Hz48000 => &sample_rate_labels[1],
                };
                egui::ComboBox::from_id_salt("batch_audio_sample_rate")
                    .selected_text(selected_sample_rate)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut overrides.audio_sample_rate,
                            AudioSampleRate::Any,
                            &any_label,
                        );
                        ui.selectable_value(
                            &mut overrides.audio_sample_rate,
                            AudioSampleRate::Hz44100,
                            &sample_rate_labels[0],
                        );
                        ui.selectable_value(
                            &mut overrides.audio_sample_rate,
                            AudioSampleRate::Hz48000,
                            &sample_rate_labels[1],
                        );
                    });
                ui.end_row();

                ui.label(&audio_channels_label);
                let selected_channels = match overrides.audio_channels {
                    AudioChannels::Any => &any_label,
                    AudioChannels::Mono => &mono_label,
                    AudioChannels::Stereo => &stereo_label,
                };
                egui::ComboBox::from_id_salt("batch_audio_channels")
                    .selected_text(selected_channels)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut overrides.audio_channels,
                            AudioChannels::Any,
                            &any_label,
                        );
                        ui.selectable_value(
                            &mut overrides.audio_channels,
                            AudioChannels::Mono,
                            &mono_label,
                        );
                        ui.selectable_value(
                            &mut overrides.audio_channels,
                            AudioChannels::Stereo,
                            &stereo_label,
                        );
                    });
                ui.end_row();
            });
    }
}

impl eframe::App for VideoFetcherApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
        if self.planning || self.downloading {
            context.request_repaint_after(Duration::from_millis(100));
        }

        let mut menu_action = self.show_menu_bar(context);
        egui::TopBottomPanel::bottom("app_status_bar").show(context, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(Color32::LIGHT_RED, error);
            } else if self.status.is_empty() {
                ui.label(self.text(Label::Ready));
            } else {
                ui.label(&self.status);
            }
        });
        if !self.planning && !self.downloading {
            let shortcuts = context.input_mut(|input| {
                let new = input.consume_key(egui::Modifiers::CTRL, egui::Key::N);
                let open = input.consume_key(egui::Modifiers::CTRL, egui::Key::O);
                let save = input.consume_key(egui::Modifiers::CTRL, egui::Key::S);
                let clipboard = input.consume_key(egui::Modifiers::CTRL, egui::Key::B);
                (new, open, save, clipboard)
            });
            if shortcuts.0 {
                menu_action = Some(MenuAction::New);
            } else if shortcuts.1 {
                menu_action = Some(MenuAction::Open);
            } else if shortcuts.2 {
                menu_action = Some(MenuAction::Save);
            } else if shortcuts.3 {
                menu_action = Some(MenuAction::FromClipboard);
            }
        }
        if let Some(action) = menu_action {
            self.handle_menu_action(action, context);
        }

        egui::CentralPanel::default().show(context, |ui| {
            ui.heading("r-video-fetcher");

            let editor_enabled = !self.planning && !self.downloading && self.plan.is_none();
            ui.add_enabled_ui(editor_enabled, |ui| {
                ui.group(|ui| {
                    ui.label(self.text(Label::Url));
                    ui.text_edit_singleline(&mut self.source_url);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(self.text(Label::TargetFolder));
                        if ui.link(&self.target_folder_text).clicked() {
                            self.choose_target_folder();
                        }
                    });
                    let media_type = match self.current_project.media_type {
                        Some(MediaType::SingleVideo) => self.text(Label::SingleVideo),
                        Some(MediaType::Playlist) => self.text(Label::Playlist),
                        None => self.text(Label::UnknownType),
                    };
                    ui.label(format!("{}: {media_type}", self.text(Label::MediaType)));
                    ui.horizontal(|ui| {
                        if ui.button(self.text(Label::Preview)).clicked() {
                            self.start_inspection();
                        }
                        if self.planning {
                            ui.spinner();
                            ui.label(self.text(Label::Planning));
                        }
                    });
                });
                self.show_media_overrides(ui);
            });
            self.show_plan(ui);
            self.show_progress(ui);
            self.show_history(ui);
        });
        self.show_settings_dialog(context);
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

fn empty_project(target_folder: PathBuf) -> HistoryRecord {
    HistoryRecord {
        batch_name: String::new(),
        url: String::new(),
        created_at_unix_seconds: 0,
        size_bytes: 0,
        download_log: Vec::new(),
        target_folder,
        media_type: None,
    }
}

pub fn parse_clipboard_url(text: &str) -> Option<String> {
    let url = Url::parse(text.trim()).ok()?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Some(url.to_string())
    } else {
        None
    }
}

fn initial_progress(plan: &DownloadPlan, ready_label: &str) -> Vec<FileProgress> {
    plan.items
        .iter()
        .zip(progress_weights(plan))
        .map(|(item, weight)| FileProgress {
            title: item.title.clone(),
            percent: 0.0,
            weight,
            status: ready_label.to_owned(),
        })
        .collect()
}

pub fn progress_weights(plan: &DownloadPlan) -> Vec<f64> {
    let known_sizes = plan
        .items
        .iter()
        .filter_map(|item| item.estimated_bytes)
        .collect::<Vec<_>>();
    let fallback_weight = if known_sizes.is_empty() {
        1.0
    } else {
        known_sizes
            .iter()
            .fold(0u64, |total, size| total.saturating_add(*size)) as f64
            / known_sizes.len() as f64
    };
    plan.items
        .iter()
        .map(|item| {
            item.estimated_bytes
                .map(|size| size as f64)
                .unwrap_or(fallback_weight)
        })
        .map(|weight| weight.max(1.0))
        .collect()
}
