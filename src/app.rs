use crate::{
    dialogs::{
        history::{HistoryDialog, HistoryDialogAction},
        open_project as open_project_dialog, save_project as save_project_dialog,
        settings::SettingsDialog,
    },
    models::structs::{
        AppTheme, AudioChannels, AudioSampleRate, MAX_ZOOM_PERCENT, MIN_ZOOM_PERCENT,
        MediaOverrides, Settings, VideoResolution, WindowState,
    },
    models::{
        download_engine::{self, DownloadEvent, InspectionProgress},
        download_session::{DownloadItemStatus, DownloadSession},
        formatting::format_bytes,
        locales::{Label, LocaleStrings},
        project::{MediaType, Project, ProjectState},
        storage,
    },
    themes,
};
use eframe::egui::{self, Color32};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};
use url::Url;

pub struct VideoFetcherApp {
    locales: LocaleStrings,
    settings: Settings,
    applied_theme: Option<AppTheme>,
    applied_zoom: Option<u16>,
    pending_window_state: Option<WindowState>,
    default_folder_text: String,
    project_state: ProjectState,
    history_dialog: HistoryDialog,
    download_session: DownloadSession,
    events_tx: Sender<DownloadEvent>,
    events_rx: Receiver<DownloadEvent>,
    abort_tx: Option<Sender<download_engine::AbortRequest>>,
    settings_dialog: SettingsDialog,
    focus_download_button: bool,
    status: String,
    error: Option<String>,
}

enum MenuAction {
    New,
    Open,
    Save,
    Exit,
    FromClipboard,
    OpenRecent(PathBuf),
    ShowSettings,
    ShowHistory,
    ConfirmClearHistory,
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
        let batch_settings = MediaOverrides::from(&settings);
        let pending_window_state = settings.window_state;
        let (events_tx, events_rx) = mpsc::channel();
        let project_state = ProjectState::new(settings.download_root.clone(), batch_settings);

        Self {
            locales,
            settings,
            applied_theme: None,
            applied_zoom: None,
            pending_window_state,
            default_folder_text,
            project_state,
            history_dialog: HistoryDialog::new(history),
            download_session: DownloadSession::default(),
            events_tx,
            events_rx,
            abort_tx: None,
            settings_dialog: SettingsDialog::default(),
            focus_download_button: false,
            status: String::new(),
            error: (!errors.is_empty()).then(|| errors.join("\n")),
        }
    }

    fn text(&self, label: Label) -> &'static str {
        self.locales.text(self.settings.language, label)
    }

    fn batch_settings(&self) -> MediaOverrides {
        self.project_state
            .project()
            .media_overrides
            .unwrap_or_else(|| MediaOverrides::from(&self.settings))
    }

    fn apply_theme(&mut self, context: &egui::Context) {
        if self.applied_theme != Some(self.settings.theme) {
            context.set_visuals(themes::visuals(self.settings.theme));
            self.applied_theme = Some(self.settings.theme);
            context.request_repaint();
        }
    }

    fn sync_keyboard_zoom(&mut self, context: &egui::Context) {
        if let Some(applied) = self.applied_zoom {
            let actual = context.zoom_factor();
            if (actual - f32::from(applied) / 100.0).abs() > 0.001 {
                let zoom =
                    ((actual * 10.0).round() as u16 * 10).clamp(MIN_ZOOM_PERCENT, MAX_ZOOM_PERCENT);
                if self.settings.zoom_percent != zoom {
                    self.settings.zoom_percent = zoom;
                    self.save_settings();
                }
                context.set_zoom_factor(f32::from(zoom) / 100.0);
                self.applied_zoom = Some(zoom);
            }
        }
    }

    fn apply_zoom(&mut self, context: &egui::Context) {
        if self.applied_zoom != Some(self.settings.zoom_percent) {
            context.set_zoom_factor(f32::from(self.settings.zoom_percent) / 100.0);
            self.applied_zoom = Some(self.settings.zoom_percent);
        }
    }

    fn restore_window_state(&mut self, context: &egui::Context) {
        let Some(window_state) = self.pending_window_state else {
            return;
        };
        let (monitor_size, outer_rect, inner_rect) = context.input(|input| {
            let viewport = input.viewport();
            (
                viewport.monitor_size,
                viewport.outer_rect,
                viewport.inner_rect,
            )
        });
        let (Some(monitor_size), Some(outer_rect), Some(inner_rect)) =
            (monitor_size, outer_rect, inner_rect)
        else {
            context.request_repaint_after(Duration::from_millis(100));
            return;
        };
        let decorations = [
            (outer_rect.width() - inner_rect.width()).max(0.0),
            (outer_rect.height() - inner_rect.height()).max(0.0),
        ];
        let clamped = window_state.clamp_to([monitor_size.x, monitor_size.y], decorations);
        let max_inner_size = [
            (monitor_size.x - decorations[0]).max(1.0),
            (monitor_size.y - decorations[1]).max(1.0),
        ];

        context.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
            640.0_f32.min(max_inner_size[0]),
            560.0_f32.min(max_inner_size[1]),
        )));
        context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            clamped.x, clamped.y,
        )));
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            clamped.width,
            clamped.height,
        )));
        self.pending_window_state = None;
    }

    fn start_inspection(&mut self, auto_download: bool) {
        self.project_state.set_suggested_file_name(None);
        if self.project_state.source_url().trim().is_empty() {
            self.download_session.cancel_auto_download();
            self.error = Some(self.text(Label::EnterUrl).to_owned());
            return;
        }
        if self
            .project_state
            .project()
            .target_folder
            .as_os_str()
            .is_empty()
        {
            self.download_session.cancel_auto_download();
            self.error = Some(self.text(Label::ChooseFolderPreview).to_owned());
            return;
        }

        let target_folder = self.project_state.project().target_folder.clone();
        let batch_settings = self.batch_settings();
        self.project_state.set_inputs(target_folder, batch_settings);
        self.error = None;
        self.status = self.text(Label::Planning).to_owned();
        let mut settings = self.settings.clone();
        settings.download_root = self.project_state.project().target_folder.clone();
        self.download_session.begin_inspection(auto_download);
        let url = self.project_state.source_url().trim().to_owned();
        let events = self.events_tx.clone();
        thread::spawn(move || {
            let result =
                download_engine::inspect_batch_with_progress(&url, &settings, |progress| {
                    let _ = events.send(DownloadEvent::InspectionProgress(progress));
                })
                .map_err(|error| format!("{error:#}"));
            let _ = events.send(DownloadEvent::PlanReady(result));
        });
    }

    fn start_download(&mut self) {
        let Some(plan) = self.download_session.begin_download() else {
            return;
        };
        let batch_settings = self.batch_settings();
        self.error = None;
        self.status = self.text(Label::Downloading).to_owned();
        let mut settings = self.settings.clone();
        settings.download_root = self.project_state.project().target_folder.clone();
        settings.apply_media_overrides(batch_settings);
        self.project_state.set_media_overrides(batch_settings);
        let events = self.events_tx.clone();
        let (abort_tx, abort_rx) = mpsc::channel();
        self.abort_tx = Some(abort_tx);
        thread::spawn(move || {
            let result = download_engine::download_batch_with_abort(
                plan,
                settings,
                events.clone(),
                abort_rx,
            )
            .map_err(|error| format!("{error:#}"));
            let _ = events.send(DownloadEvent::Finished(result));
        });
    }

    fn save_settings(&mut self) {
        let previous_download_root = self.settings.download_root.clone();
        let default_folder = PathBuf::from(self.default_folder_text.trim());
        if !default_folder.as_os_str().is_empty() {
            self.settings.download_root = default_folder;
        }
        match storage::save_settings(&self.settings) {
            Ok(()) => {
                if self.settings.download_root != previous_download_root
                    && self.project_state.path().is_none()
                    && self.project_state.source_url().is_empty()
                {
                    self.project_state
                        .set_target_folder(self.settings.download_root.clone());
                }
                self.error = None;
                self.status.clear();
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    fn save_window_state(&mut self, context: &egui::Context) -> anyhow::Result<()> {
        let (outer_rect, inner_rect) = context.input(|input| {
            let viewport = input.viewport();
            (viewport.outer_rect, viewport.inner_rect)
        });
        let outer_rect = outer_rect.ok_or_else(|| {
            anyhow::anyhow!("Could not determine the main window's outer rectangle.")
        })?;
        let inner_rect = inner_rect.ok_or_else(|| {
            anyhow::anyhow!("Could not determine the main window's inner rectangle.")
        })?;
        self.settings.window_state = Some(WindowState {
            x: outer_rect.min.x,
            y: outer_rect.min.y,
            width: inner_rect.width(),
            height: inner_rect.height(),
        });
        storage::save_settings(&self.settings)
    }

    fn new_project(&mut self) {
        self.project_state.reset(
            self.settings.download_root.clone(),
            MediaOverrides::from(&self.settings),
        );
        self.history_dialog.close();
        self.download_session.clear();
        self.error = None;
        self.status.clear();
    }

    fn open_project_dialog(&mut self, frame: &eframe::Frame) {
        self.new_project();
        if let Some(path) = open_project_dialog::open_project_with_parent(frame) {
            self.open_project(&path);
        }
    }

    fn open_project(&mut self, path: &std::path::Path) {
        match storage::load_project(path) {
            Ok(mut project) => {
                if project.target_folder.as_os_str().is_empty() {
                    project.target_folder = self.settings.download_root.clone();
                }
                let batch_settings = project
                    .media_overrides
                    .unwrap_or_else(|| MediaOverrides::from(&self.settings));
                project.set_media_overrides(batch_settings);
                self.project_state.open(project, path.to_owned());
                self.download_session.clear();
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

    fn save_project_dialog(&mut self, frame: &eframe::Frame) {
        let suggested_name = self
            .project_state
            .suggested_file_name()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let batch_name = if self.project_state.project().batch_name.is_empty() {
                    "video"
                } else {
                    &self.project_state.project().batch_name
                };
                suggested_project_file_name("Unknown", batch_name)
            });
        if let Some(path) = save_project_dialog::save_project_with_parent(&suggested_name, frame) {
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

    fn save_project(&mut self, frame: &eframe::Frame) {
        if let Some(path) = self.project_state.path().map(PathBuf::from) {
            match self.save_project_to(&path) {
                Ok(()) => {
                    self.status = self
                        .text(Label::SaveProjectStatus)
                        .replace("{path}", &path.display().to_string());
                }
                Err(error) => self.error = Some(format!("{error:#}")),
            }
        } else {
            self.save_project_dialog(frame);
        }
    }

    fn save_current_project_if_named(&mut self) -> anyhow::Result<()> {
        if let Some(path) = self.project_state.path().map(PathBuf::from) {
            self.save_project_to(&path)?;
        }
        Ok(())
    }

    fn save_project_to(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        self.project_state.prepare_for_save(self.batch_settings())?;
        if self.project_state.path().is_none() && path.exists() {
            self.project_state.set_path(path.to_owned());
            self.remember_project(path)?;
            return Ok(());
        }
        storage::save_project(path, self.project_state.project())?;
        self.project_state.set_path(path.to_owned());
        self.remember_project(path)?;
        self.error = None;
        Ok(())
    }

    fn save_project_for_clipboard(&mut self) -> anyhow::Result<()> {
        let Some(file_name) = self.project_state.suggested_file_name().map(str::to_owned) else {
            return Err(anyhow::anyhow!(
                "Could not derive a project file name from inspected media."
            ));
        };
        if self.project_state.path().is_none() {
            let path = storage::projects_directory()?.join(file_name);
            self.save_project_to(&path)?;
        } else {
            self.save_current_project_if_named()?;
        }
        Ok(())
    }

    fn save_project_before_exit(&mut self) -> anyhow::Result<()> {
        if self.project_state.source_url().trim().is_empty() {
            return Ok(());
        }
        let path = match self.project_state.path().map(PathBuf::from) {
            Some(path) => path,
            None => {
                let file_name = self
                    .project_state
                    .suggested_file_name()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        let url_component = Url::parse(self.project_state.source_url().trim())
                            .ok()
                            .and_then(|url| {
                                url.query_pairs()
                                    .find(|(key, _)| key == "v")
                                    .map(|(_, value)| value.into_owned())
                            })
                            .unwrap_or_else(|| "video".to_owned());
                        suggested_project_file_name("Unknown", &url_component)
                    });
                storage::projects_directory()?.join(file_name)
            }
        };
        self.save_project_to(&path)
    }

    fn remember_project(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        storage::remember_recent_file(&mut self.settings, path);
        storage::save_settings(&self.settings)
    }

    fn download_from_clipboard(&mut self) {
        if self.download_session.is_busy() {
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
        self.project_state.set_url(&clipboard_url);
        self.start_inspection(true);
    }

    fn poll_events(&mut self) {
        while let Ok(event) = self.events_rx.try_recv() {
            match event {
                DownloadEvent::PlanReady(result) => {
                    let auto_download = self.download_session.finish_inspection();
                    match result {
                        Ok(plan) => {
                            let plan_status =
                                format!("{}: {}", self.text(Label::Plan), plan.batch_name);
                            self.project_state
                                .set_suggested_file_name(plan.items.first().map(|item| {
                                    suggested_project_file_name(&item.channel, &item.title)
                                }));
                            self.status = plan_status.clone();
                            self.error = None;
                            let target_folder = self.project_state.project().target_folder.clone();
                            let batch_settings = self.batch_settings();
                            self.project_state.apply_preview(
                                plan.batch_name.clone(),
                                plan.source_url.clone(),
                                plan.estimated_bytes,
                                target_folder,
                                plan.items.first().map(|item| {
                                    download_engine::sanitize_path_component(&item.channel)
                                }),
                                plan.playlist_name
                                    .as_deref()
                                    .map(download_engine::sanitize_path_component),
                                if plan.playlist_name.is_some() {
                                    MediaType::Playlist
                                } else {
                                    MediaType::SingleVideo
                                },
                                batch_settings,
                            );
                            self.download_session.set_plan(plan);
                            let project_saved = match if auto_download {
                                self.save_project_for_clipboard()
                            } else {
                                self.save_current_project_if_named()
                            } {
                                Ok(()) => true,
                                Err(error) => {
                                    self.error = Some(format!("{error:#}"));
                                    false
                                }
                            };
                            self.status = plan_status;
                            if auto_download && project_saved {
                                self.start_download();
                            } else {
                                self.focus_download_button = true;
                            }
                        }
                        Err(error) => {
                            self.download_session.clear();
                            self.error = Some(error);
                        }
                    }
                }
                DownloadEvent::InspectionProgress(progress) => {
                    self.status = match progress {
                        InspectionProgress::CheckingTools => {
                            self.text(Label::CheckingTools).to_owned()
                        }
                        InspectionProgress::FetchingMetadata => {
                            self.text(Label::FetchingMetadata).to_owned()
                        }
                        InspectionProgress::EstimatingItems {
                            completed,
                            total,
                            active,
                            unavailable,
                        } => self
                            .text(Label::EstimatingItems)
                            .replace("{completed}", &completed.to_string())
                            .replace("{total}", &total.to_string())
                            .replace("{active}", &active.to_string())
                            .replace("{unavailable}", &unavailable.to_string()),
                        InspectionProgress::CheckingDiskSpace => {
                            self.text(Label::CheckingDiskSpace).to_owned()
                        }
                    };
                }
                DownloadEvent::AttemptStarted {
                    index,
                    attempt,
                    total_attempts,
                } => self
                    .download_session
                    .mark_attempt_started(index, attempt, total_attempts),
                DownloadEvent::Progress { index, percent } => {
                    self.download_session.report_progress(index, percent);
                }
                DownloadEvent::ItemCompleted { index, size_bytes } => {
                    self.download_session.mark_completed(index, size_bytes);
                }
                DownloadEvent::ItemVerifying { index } => {
                    self.download_session.mark_verifying(index)
                }
                DownloadEvent::ItemAborted { index } => self.download_session.mark_aborted(index),
                DownloadEvent::ItemFailed { index, message } => {
                    let retrying = !message.starts_with("Failed after");
                    self.download_session.mark_failed(index, message, retrying);
                }
                DownloadEvent::Finished(result) => {
                    self.download_session.finish_download();
                    self.abort_tx = None;
                    match result {
                        Ok(mut record) => {
                            self.status = self
                                .text(Label::FinishedStatus)
                                .replace("{status}", self.text(Label::Completed))
                                .replace(
                                    "{files}",
                                    &self.download_session.items().len().to_string(),
                                )
                                .replace(
                                    "{failed}",
                                    &self.download_session.failed_count().to_string(),
                                )
                                .replace(
                                    "{aborted}",
                                    &self.download_session.aborted_count().to_string(),
                                )
                                .replace("{size}", &format_bytes(record.size_bytes));
                            record.set_media_overrides(self.batch_settings());
                            self.project_state.replace_project(record.clone());
                            self.error = None;
                            if let Err(error) = self.save_current_project_if_named() {
                                self.error = Some(format!("{error:#}"));
                            }
                            match storage::load_recent_history() {
                                Ok(history) => self.history_dialog.set_history(history),
                                Err(error) => self.error = Some(format!("{error:#}")),
                            }
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
        }
    }

    fn show_settings_dialog(&mut self, context: &egui::Context, frame: &eframe::Frame) {
        let previous_settings = self.settings.clone();
        let previous_default_folder = self.default_folder_text.clone();
        let restore_defaults = self.settings_dialog.show_with_parent(
            context,
            &mut self.settings,
            &mut self.default_folder_text,
            &self.locales,
            frame,
        );
        if restore_defaults {
            self.restore_defaults();
        } else if self.settings != previous_settings
            || self.default_folder_text != previous_default_folder
        {
            self.save_settings();
        }
    }

    fn restore_defaults(&mut self) {
        let result = storage::default_settings().and_then(|defaults| {
            let defaults = self.settings.restore_preferences(defaults);
            storage::save_settings(&defaults)?;
            Ok(defaults)
        });
        match result {
            Ok(defaults) => {
                self.default_folder_text = defaults.download_root.to_string_lossy().into_owned();
                self.settings = defaults;
                self.error = None;
                self.status.clear();
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    fn show_menu_bar(&mut self, context: &egui::Context) -> Option<MenuAction> {
        let mut action = None;
        let recent_files = self.settings.recent_files.clone();
        let busy = self.download_session.is_busy();
        let file_label = self.text(Label::File);
        let new_label = self.text(Label::New);
        let open_label = self.text(Label::Open);
        let save_label = self.text(Label::Save);
        let recent_label = self.text(Label::RecentFiles);
        let exit_label = self.text(Label::Exit);
        let download_label = self.text(Label::DownloadMenu);
        let clipboard_label = self.text(Label::FromClipboard);
        let history_label = self.text(Label::History);
        let clear_history_label = self.text(Label::ClearHistory);
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
                    if ui.button(format!("{settings_label}    Ctrl+,")).clicked() {
                        action = Some(MenuAction::ShowSettings);
                        ui.close();
                    }
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
                    ui.separator();
                    if ui.button(history_label).clicked() {
                        action = Some(MenuAction::ShowHistory);
                        ui.close();
                    }
                    if ui.button(clear_history_label).clicked() {
                        action = Some(MenuAction::ConfirmClearHistory);
                        ui.close();
                    }
                });
            });
        });
        action
    }

    fn handle_menu_action(
        &mut self,
        action: MenuAction,
        context: &egui::Context,
        frame: &eframe::Frame,
    ) {
        match action {
            MenuAction::New => self.new_project(),
            MenuAction::Open => self.open_project_dialog(frame),
            MenuAction::Save => self.save_project(frame),
            MenuAction::Exit => context.send_viewport_cmd(egui::ViewportCommand::Close),
            MenuAction::FromClipboard => self.download_from_clipboard(),
            MenuAction::OpenRecent(path) => self.open_project(&path),
            MenuAction::ShowSettings => self.settings_dialog.open(),
            MenuAction::ShowHistory => self.history_dialog.open(),
            MenuAction::ConfirmClearHistory => self.history_dialog.confirm_clear(),
        }
    }

    fn show_plan(&mut self, ui: &mut egui::Ui) {
        let Some(plan) = self.download_session.plan() else {
            self.focus_download_button = false;
            return;
        };
        let warning = plan.estimated_bytes > plan.free_bytes || plan.unknown_size_count > 0;
        ui.separator();
        ui.heading(format!("{}: {}", self.text(Label::Plan), plan.batch_name));
        if let Some(item) = plan.items.first() {
            let channel = &item.channel;
            ui.label(format!("{}: {channel}", self.text(Label::Channel)));
        }
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
        let downloading = self.download_session.is_downloading();
        ui.horizontal(|ui| {
            let download_text = if warning {
                self.text(Label::DownloadAnyway)
            } else {
                self.text(Label::Download)
            };
            let download_button_response = ui.add_enabled(
                !downloading,
                egui::Button::new(egui::RichText::new(download_text).heading())
                    .min_size(egui::vec2(180.0, 36.0)),
            );
            if std::mem::take(&mut self.focus_download_button) {
                download_button_response.request_focus();
            }
            if download_button_response.clicked() {
                self.start_download();
            }
            let abort_text = if downloading {
                self.text(Label::Abort)
            } else {
                self.text(Label::CancelPreview)
            };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(abort_text).heading())
                        .min_size(egui::vec2(180.0, 36.0)),
                )
                .clicked()
            {
                if downloading {
                    match self
                        .abort_tx
                        .as_ref()
                        .map(|sender| sender.send(download_engine::AbortRequest::Batch))
                    {
                        Some(Ok(())) => {}
                        Some(Err(error)) => {
                            self.error = Some(format!("Could not abort download batch: {error}"));
                        }
                        None => {
                            self.error = Some("Download cancellation is unavailable.".to_owned())
                        }
                    }
                } else {
                    self.download_session.clear();
                }
            }
        });
    }

    fn show_progress(&mut self, ui: &mut egui::Ui) {
        if !self.download_session.has_progress() {
            return;
        }
        ui.separator();
        ui.heading(self.text(Label::BatchProgress));
        let overall = self.download_session.overall_progress();
        ui.add(egui::ProgressBar::new(overall / 100.0).text(format!("{overall:.1}%")));
        let abort_label = self.text(Label::Abort).to_owned();
        let downloading = self.download_session.is_downloading();
        let mut abort_index = None;
        egui::ScrollArea::vertical()
            .id_salt("batch_progress_items")
            .max_height(ui.available_height())
            .show(ui, |ui| {
                for (index, item) in self.download_session.items().iter().enumerate() {
                    let status = self.progress_status_text(&item.status);
                    ui.horizontal(|ui| {
                        ui.label(&item.title);
                        if ui
                            .add_enabled(
                                downloading && item.can_abort,
                                egui::Button::new(&abort_label),
                            )
                            .clicked()
                        {
                            abort_index = Some(index);
                        }
                    });
                    ui.add(
                        egui::ProgressBar::new(item.percent / 100.0)
                            .text(format!("{:.1}% - {status}", item.percent)),
                    );
                }
            });
        if let Some(index) = abort_index {
            match self
                .abort_tx
                .as_ref()
                .map(|sender| sender.send(download_engine::AbortRequest::Item(index)))
            {
                Some(Ok(())) => {
                    self.download_session.mark_aborting(index);
                }
                Some(Err(error)) => {
                    self.error = Some(format!(
                        "Could not abort download item {}: {error}",
                        index + 1
                    ));
                }
                None => self.error = Some("Download cancellation is unavailable.".to_owned()),
            }
        }
    }

    fn progress_status_text(&self, status: &DownloadItemStatus) -> String {
        match status {
            DownloadItemStatus::Ready => self.text(Label::Ready).to_owned(),
            DownloadItemStatus::Attempt {
                attempt,
                total_attempts,
            } => self
                .text(Label::AttemptStatus)
                .replace("{attempt}", &attempt.to_string())
                .replace("{total_attempts}", &total_attempts.to_string()),
            DownloadItemStatus::Verifying => self.text(Label::VerifyingStatus).to_owned(),
            DownloadItemStatus::Completed { size_bytes } => self
                .text(Label::CompleteStatus)
                .replace("{size}", &format_bytes(*size_bytes)),
            DownloadItemStatus::Aborting => self.text(Label::AbortingStatus).to_owned(),
            DownloadItemStatus::Aborted => self.text(Label::AbortedStatus).to_owned(),
            DownloadItemStatus::Failed { message, .. } => message.clone(),
        }
    }

    fn show_history_dialog(&mut self, context: &egui::Context) {
        let busy = self.download_session.is_busy();
        match self
            .history_dialog
            .show(context, &self.locales, self.settings.language, busy)
        {
            Ok(Some(HistoryDialogAction::Reuse(record))) => self.reuse_history_record(record),
            Ok(Some(HistoryDialogAction::Cleared)) => self.error = None,
            Ok(None) => {}
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    fn reuse_history_record(&mut self, record: Project) {
        self.new_project();
        self.project_state.set_url(&record.url);
        if !record.target_folder.as_os_str().is_empty() {
            self.project_state.set_target_folder(record.target_folder);
        }
    }

    fn show_batch_settings(&mut self, ui: &mut egui::Ui) {
        let mut batch_settings = self.batch_settings();
        let from_source_video_label = self.text(Label::FromSourceVideo).to_owned();
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

        let mut changed = false;
        egui::Grid::new("batch_settings")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(&video_resolution_label);
                let selected_resolution = match batch_settings.video_resolution {
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
                        changed |= ui
                            .selectable_value(
                                &mut batch_settings.video_resolution,
                                VideoResolution::Best,
                                &best_label,
                            )
                            .changed();
                        for (resolution, label) in [
                            (VideoResolution::P360, &resolution_labels[0]),
                            (VideoResolution::P480, &resolution_labels[1]),
                            (VideoResolution::P720, &resolution_labels[2]),
                            (VideoResolution::P1080, &resolution_labels[3]),
                            (VideoResolution::P1440, &resolution_labels[4]),
                            (VideoResolution::P2160, &resolution_labels[5]),
                        ] {
                            changed |= ui
                                .selectable_value(
                                    &mut batch_settings.video_resolution,
                                    resolution,
                                    label,
                                )
                                .changed();
                        }
                    });
                ui.end_row();

                ui.label(&audio_sample_rate_label);
                let selected_sample_rate = match batch_settings.audio_sample_rate {
                    AudioSampleRate::FromSourceVideo => &from_source_video_label,
                    AudioSampleRate::Hz44100 => &sample_rate_labels[0],
                    AudioSampleRate::Hz48000 => &sample_rate_labels[1],
                };
                egui::ComboBox::from_id_salt("batch_audio_sample_rate")
                    .selected_text(selected_sample_rate)
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut batch_settings.audio_sample_rate,
                                AudioSampleRate::FromSourceVideo,
                                &from_source_video_label,
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut batch_settings.audio_sample_rate,
                                AudioSampleRate::Hz44100,
                                &sample_rate_labels[0],
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut batch_settings.audio_sample_rate,
                                AudioSampleRate::Hz48000,
                                &sample_rate_labels[1],
                            )
                            .changed();
                    });
                ui.end_row();

                ui.label(&audio_channels_label);
                let selected_channels = match batch_settings.audio_channels {
                    AudioChannels::FromSourceVideo => &from_source_video_label,
                    AudioChannels::Mono => &mono_label,
                    AudioChannels::Stereo => &stereo_label,
                };
                egui::ComboBox::from_id_salt("batch_audio_channels")
                    .selected_text(selected_channels)
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut batch_settings.audio_channels,
                                AudioChannels::FromSourceVideo,
                                &from_source_video_label,
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut batch_settings.audio_channels,
                                AudioChannels::Mono,
                                &mono_label,
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut batch_settings.audio_channels,
                                AudioChannels::Stereo,
                                &stereo_label,
                            )
                            .changed();
                    });
                ui.end_row();
            });
        if changed {
            self.project_state.set_media_overrides(batch_settings);
            if let Err(error) = self.save_current_project_if_named() {
                self.error = Some(format!("{error:#}"));
            }
        }
    }
}

impl eframe::App for VideoFetcherApp {
    fn update(&mut self, context: &egui::Context, frame: &mut eframe::Frame) {
        self.sync_keyboard_zoom(context);
        self.apply_zoom(context);
        self.restore_window_state(context);
        self.apply_theme(context);
        self.poll_events();

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
        if !self.download_session.is_busy() {
            let shortcuts = context.input_mut(|input| {
                let new = input.consume_key(egui::Modifiers::CTRL, egui::Key::N);
                let open = input.consume_key(egui::Modifiers::CTRL, egui::Key::O);
                let save = input.consume_key(egui::Modifiers::CTRL, egui::Key::S);
                let clipboard = input.consume_key(egui::Modifiers::CTRL, egui::Key::B);
                let settings = input.consume_key(egui::Modifiers::CTRL, egui::Key::Comma);
                (new, open, save, clipboard, settings)
            });
            if shortcuts.0 {
                menu_action = Some(MenuAction::New);
            } else if shortcuts.1 {
                menu_action = Some(MenuAction::Open);
            } else if shortcuts.2 {
                menu_action = Some(MenuAction::Save);
            } else if shortcuts.3 {
                menu_action = Some(MenuAction::FromClipboard);
            } else if shortcuts.4 {
                menu_action = Some(MenuAction::ShowSettings);
            }
        }
        if let Some(action) = menu_action {
            self.handle_menu_action(action, context, frame);
        }

        let target_folder = self
            .project_state
            .project()
            .target_folder
            .to_string_lossy()
            .into_owned();
        let download_folder = self.project_state.project().download_folder();
        let mut open_target_folder = None;
        egui::CentralPanel::default().show(context, |ui| {
            let editor_enabled =
                !self.download_session.is_busy() && self.download_session.plan().is_none();
            ui.group(|ui| {
                ui.add_enabled_ui(editor_enabled, |ui| {
                    ui.label(self.text(Label::Url));
                    let url_text_edit =
                        egui::TextEdit::singleline(self.project_state.source_url_mut())
                            .id(egui::Id::new("source_url"))
                            .desired_width(f32::INFINITY);
                    let response = ui.add(url_text_edit);
                    if response.changed() {
                        self.project_state.commit_source_url();
                        self.project_state.set_suggested_file_name(None);
                    }
                    let submit = editor_enabled
                        && response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    ui.separator();
                    ui.label(format!(
                        "{}: {target_folder}",
                        self.text(Label::TargetFolder)
                    ));
                    if let Some(folder) = &download_folder {
                        let folder_label = folder.display().to_string();
                        if ui.link(folder_label).clicked() {
                            open_target_folder = Some(folder.clone());
                        }
                    }
                    let media_type = match self.project_state.project().media_type {
                        Some(MediaType::SingleVideo) => self.text(Label::SingleVideo),
                        Some(MediaType::Playlist) => self.text(Label::Playlist),
                        None => self.text(Label::UnknownType),
                    };
                    ui.label(format!("{}: {media_type}", self.text(Label::MediaType)));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(self.text(Label::Preview)).heading(),
                                )
                                .min_size(egui::vec2(180.0, 36.0)),
                            )
                            .clicked()
                            || submit
                        {
                            self.start_inspection(false);
                        }
                        if self.download_session.is_inspecting() {
                            ui.spinner();
                            ui.label(self.text(Label::Planning));
                        }
                    });
                });
                ui.separator();
                ui.add_enabled_ui(!self.download_session.is_busy(), |ui| {
                    self.show_batch_settings(ui);
                });
            });
            self.show_plan(ui);
            self.show_progress(ui);
        });
        if let Some(folder) = open_target_folder {
            if let Err(error) = storage::open_folder(&folder) {
                self.error = Some(format!("{error:#}"));
            }
        }
        self.show_settings_dialog(context, frame);
        self.apply_zoom(context);
        self.apply_theme(context);
        self.show_history_dialog(context);
        if context.input(|input| input.viewport().close_requested()) {
            if let Err(error) = self
                .save_window_state(context)
                .and_then(|()| self.save_project_before_exit())
            {
                self.error = Some(format!("{error:#}"));
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        if self.download_session.is_busy() {
            context.request_repaint_after(Duration::from_millis(100));
        }
    }
}

pub fn suggested_project_file_name(channel: &str, title: &str) -> String {
    format!(
        "{}-{}.dwnl",
        download_engine::sanitize_path_component(channel),
        download_engine::sanitize_path_component(title)
    )
}

pub fn parse_clipboard_url(text: &str) -> Option<String> {
    let url = Url::parse(text.trim()).ok()?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Some(url.to_string())
    } else {
        None
    }
}
