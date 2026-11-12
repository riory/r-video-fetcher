use crate::model::{HistoryRecord, MAX_RETRIES, Settings};
use anyhow::{Context, Result, anyhow};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn default_settings() -> Result<Settings> {
    let profile = env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("The USERPROFILE environment variable is not set."))?;
    Ok(Settings {
        download_root: profile.join("Downloads").join("r-video-fetcher"),
        ..Settings::default()
    })
}

pub fn load_settings() -> Result<Settings> {
    let path = settings_path()?;
    match fs::read_to_string(&path) {
        Ok(contents) => {
            let settings: Settings = serde_json::from_str(&contents)
                .with_context(|| format!("Could not parse settings at {}", path.display()))?;
            if settings.download_root.as_os_str().is_empty() {
                return Err(anyhow!(
                    "The download folder is empty in {}.",
                    path.display()
                ));
            }
            if settings.max_retries > MAX_RETRIES {
                return Err(anyhow!(
                    "The maximum retry count in {} must not exceed {}.",
                    path.display(),
                    MAX_RETRIES
                ));
            }
            Ok(settings)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => default_settings(),
        Err(error) => {
            Err(error).with_context(|| format!("Could not read settings at {}", path.display()))
        }
    }
}

pub fn save_settings(settings: &Settings) -> Result<()> {
    let path = settings_path()?;
    write_json(&path, settings)
        .with_context(|| format!("Could not save settings at {}", path.display()))
}

pub fn load_recent_history() -> Result<Vec<HistoryRecord>> {
    let directory = history_directory()?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Could not read history folder {}", directory.display()));
        }
    };

    let mut files = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("Could not list history folder {}", directory.display()))?;
    files.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "dwnl")
    });
    files.sort_by_key(|path| path.file_name().unwrap_or_default().to_os_string());
    files.reverse();
    files
        .into_iter()
        .take(10)
        .map(|path| {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("Could not read history file {}", path.display()))?;
            serde_json::from_str(&contents)
                .with_context(|| format!("Could not parse history file {}", path.display()))
        })
        .collect()
}

pub fn save_history(record: &HistoryRecord) -> Result<()> {
    let directory = history_directory()?;
    fs::create_dir_all(&directory)
        .with_context(|| format!("Could not create history folder {}", directory.display()))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is earlier than the Unix epoch.")?
        .as_nanos();
    let path = directory.join(format!("batch-{timestamp}.dwnl"));
    write_json(&path, record)
        .with_context(|| format!("Could not save download history at {}", path.display()))
}

pub fn load_project(path: &Path) -> Result<HistoryRecord> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("Could not read {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("Could not parse project file {}", path.display()))
}

pub fn save_project(path: &Path, project: &HistoryRecord) -> Result<()> {
    let has_project_extension = path
        .extension()
        .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("dwnl"));
    if !has_project_extension {
        return Err(anyhow!(
            "Project files must use the .dwnl extension: {}.",
            path.display()
        ));
    }
    write_json(path, project).with_context(|| format!("Could not save project {}", path.display()))
}

pub fn clipboard_project_path() -> Result<PathBuf> {
    let directory = app_data_directory("LOCALAPPDATA")?.join("projects");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is earlier than the Unix epoch.")?
        .as_nanos();
    Ok(directory.join(format!("clipboard-{timestamp}.dwnl")))
}

pub fn remember_recent_file(settings: &mut Settings, path: &Path) {
    settings.recent_files.retain(|recent| recent != path);
    settings.recent_files.insert(0, path.to_owned());
    settings.recent_files.truncate(10);
}

pub fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("The file path {} has no parent folder.", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Could not create folder {}", parent.display()))?;
    let contents = serde_json::to_vec_pretty(value).context("Could not encode JSON.")?;
    fs::write(path, contents).with_context(|| format!("Could not write {}", path.display()))
}

fn settings_path() -> Result<PathBuf> {
    app_data_directory("APPDATA").map(|directory| directory.join("settings.json"))
}

fn history_directory() -> Result<PathBuf> {
    app_data_directory("LOCALAPPDATA").map(|directory| directory.join("history"))
}

fn app_data_directory(variable: &str) -> Result<PathBuf> {
    env::var_os(variable)
        .map(|directory| PathBuf::from(directory).join("r-video-fetcher"))
        .ok_or_else(|| anyhow!("The {variable} environment variable is not set."))
}
