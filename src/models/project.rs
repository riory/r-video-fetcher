use crate::models::structs::MediaOverrides;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    SingleVideo,
    Playlist,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DownloadLogEntry {
    GenericMessage { message: String },
    DownloadMessage { message: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Project {
    pub batch_name: String,
    pub url: String,
    pub created_at_unix_seconds: u64,
    pub size_bytes: u64,
    pub download_log: Vec<DownloadLogEntry>,
    pub target_folder: PathBuf,
    pub channel: Option<String>,
    pub playlist: Option<String>,
    pub media_type: Option<MediaType>,
    pub media_overrides: Option<MediaOverrides>,
}

pub type HistoryRecord = Project;

#[derive(Clone, Debug)]
pub struct ProjectState {
    project: Project,
    path: Option<PathBuf>,
    suggested_file_name: Option<String>,
    source_url: String,
}

impl ProjectState {
    pub fn new(target_folder: PathBuf, media_overrides: MediaOverrides) -> Self {
        Self {
            project: Project::empty(target_folder, media_overrides),
            path: None,
            suggested_file_name: None,
            source_url: String::new(),
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn source_url(&self) -> &str {
        &self.source_url
    }

    pub fn source_url_mut(&mut self) -> &mut String {
        &mut self.source_url
    }

    pub fn commit_source_url(&mut self) {
        self.project.set_url(&self.source_url);
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    pub fn suggested_file_name(&self) -> Option<&str> {
        self.suggested_file_name.as_deref()
    }

    pub fn set_suggested_file_name(&mut self, file_name: Option<String>) {
        self.suggested_file_name = file_name;
    }

    pub fn reset(&mut self, target_folder: PathBuf, media_overrides: MediaOverrides) {
        self.project = Project::empty(target_folder, media_overrides);
        self.path = None;
        self.suggested_file_name = None;
        self.source_url.clear();
    }

    pub fn open(&mut self, project: Project, path: PathBuf) {
        self.source_url = project.url.clone();
        self.project = project;
        self.path = Some(path);
        self.suggested_file_name = None;
    }

    pub fn replace_project(&mut self, project: Project) {
        self.source_url = project.url.clone();
        self.project = project;
    }

    pub fn set_inputs(&mut self, target_folder: PathBuf, media_overrides: MediaOverrides) {
        self.project
            .set_inputs(&self.source_url, target_folder, media_overrides);
    }

    pub fn apply_preview(
        &mut self,
        batch_name: String,
        url: String,
        size_bytes: u64,
        target_folder: PathBuf,
        channel: Option<String>,
        playlist: Option<String>,
        media_type: MediaType,
        media_overrides: MediaOverrides,
    ) {
        self.source_url = url.clone();
        self.project.apply_preview(
            batch_name,
            url,
            size_bytes,
            target_folder,
            channel,
            playlist,
            media_type,
            media_overrides,
        );
    }

    pub fn set_url(&mut self, url: &str) {
        self.source_url = url.to_owned();
        self.project.set_url(url);
    }

    pub fn set_target_folder(&mut self, target_folder: PathBuf) {
        self.project.set_target_folder(target_folder);
    }

    pub fn set_media_overrides(&mut self, media_overrides: MediaOverrides) {
        self.project.set_media_overrides(media_overrides);
    }

    pub fn prepare_for_save(&mut self, media_overrides: MediaOverrides) -> Result<()> {
        self.project.set_url(&self.source_url);
        self.project.set_media_overrides(media_overrides);
        self.project.ensure_created_at()
    }
}

impl Project {
    pub fn empty(target_folder: PathBuf, media_overrides: MediaOverrides) -> Self {
        Self {
            batch_name: String::new(),
            url: String::new(),
            created_at_unix_seconds: 0,
            size_bytes: 0,
            download_log: Vec::new(),
            target_folder,
            channel: None,
            playlist: None,
            media_type: None,
            media_overrides: Some(media_overrides),
        }
    }

    pub fn set_inputs(
        &mut self,
        url: &str,
        target_folder: PathBuf,
        media_overrides: MediaOverrides,
    ) {
        self.url = url.trim().to_owned();
        self.target_folder = target_folder;
        self.channel = None;
        self.playlist = None;
        self.media_type = None;
        self.media_overrides = Some(media_overrides);
    }

    pub fn apply_preview(
        &mut self,
        batch_name: String,
        url: String,
        size_bytes: u64,
        target_folder: PathBuf,
        channel: Option<String>,
        playlist: Option<String>,
        media_type: MediaType,
        media_overrides: MediaOverrides,
    ) {
        self.batch_name = batch_name;
        self.url = url;
        self.size_bytes = size_bytes;
        self.target_folder = target_folder;
        self.channel = channel;
        self.playlist = playlist;
        self.media_type = Some(media_type);
        self.media_overrides = Some(media_overrides);
    }

    pub fn set_url(&mut self, url: &str) {
        if self.url.trim() != url.trim() {
            self.channel = None;
            self.playlist = None;
            self.media_type = None;
        }
        self.url = url.trim().to_owned();
    }

    pub fn set_target_folder(&mut self, target_folder: PathBuf) {
        self.target_folder = target_folder;
    }

    pub fn set_media_overrides(&mut self, media_overrides: MediaOverrides) {
        self.media_overrides = Some(media_overrides);
    }

    pub fn ensure_created_at(&mut self) -> Result<()> {
        if self.created_at_unix_seconds == 0 {
            self.created_at_unix_seconds = current_timestamp()?;
        }
        Ok(())
    }

    pub fn download_folder(&self) -> Option<PathBuf> {
        if self.target_folder.as_os_str().is_empty() {
            return None;
        }
        let mut folder = self.target_folder.join(self.channel.as_ref()?);
        if let Some(playlist) = &self.playlist {
            folder.push(playlist);
        }
        Some(folder)
    }

    pub fn completed_download(
        batch_name: String,
        url: String,
        size_bytes: u64,
        download_log: Vec<DownloadLogEntry>,
        target_folder: PathBuf,
        channel: Option<String>,
        playlist: Option<String>,
        media_type: MediaType,
    ) -> Result<Self> {
        Ok(Self {
            batch_name,
            url,
            created_at_unix_seconds: current_timestamp()?,
            size_bytes,
            download_log,
            target_folder,
            channel,
            playlist,
            media_type: Some(media_type),
            media_overrides: None,
        })
    }
}

fn current_timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is earlier than the Unix epoch.")
        .map(|duration| duration.as_secs())
}
