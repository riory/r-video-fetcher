use crate::{
    models::project::HistoryRecord,
    models::structs::{MAX_RETRIES, Settings},
};
use anyhow::{Context, Result, anyhow};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const RECENT_HISTORY_LIMIT: usize = 10;

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
    load_recent_history_from(&history_directory()?)
}

pub fn load_recent_history_from(directory: &Path) -> Result<Vec<HistoryRecord>> {
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Could not read history folder {}", directory.display()));
        }
    };

    let files = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("Could not list history folder {}", directory.display()))?;
    let mut files = files
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "dwnl")
        })
        .filter_map(|path| match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => Some(Ok(path)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("Could not inspect history folder {}", directory.display()))?;
    files.sort_by_key(|path| path.file_name().unwrap_or_default().to_os_string());
    files.reverse();
    files
        .into_iter()
        .take(RECENT_HISTORY_LIMIT)
        .map(|path| {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("Could not read history file {}", path.display()))?;
            serde_json::from_str(&contents)
                .with_context(|| format!("Could not parse history file {}", path.display()))
        })
        .collect()
}

pub fn clear_history() -> Result<()> {
    clear_history_from(&history_directory()?)
}

pub fn open_history_folder() -> Result<()> {
    let directory = history_directory()?;
    fs::create_dir_all(&directory)
        .with_context(|| format!("Could not create history folder {}", directory.display()))?;
    open_folder(&directory)
}

pub fn open_folder(path: &Path) -> Result<()> {
    launch_folder(path)
}

pub fn clear_history_from(directory: &Path) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Could not read history folder {}", directory.display()));
        }
    };

    for entry in entries {
        let path = entry
            .with_context(|| format!("Could not list history folder {}", directory.display()))?
            .path();
        if path
            .extension()
            .is_some_and(|extension| extension == "dwnl")
            && fs::metadata(&path)
                .with_context(|| format!("Could not inspect history file {}", path.display()))?
                .is_file()
        {
            fs::remove_file(&path)
                .with_context(|| format!("Could not remove history file {}", path.display()))?;
        }
    }
    Ok(())
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

pub fn projects_directory() -> Result<PathBuf> {
    app_data_directory("LOCALAPPDATA").map(|directory| directory.join("projects"))
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

#[cfg(target_os = "windows")]
fn launch_folder(path: &Path) -> Result<()> {
    Command::new("explorer")
        .arg(path)
        .spawn()
        .with_context(|| format!("Could not open folder {}", path.display()))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn launch_folder(path: &Path) -> Result<()> {
    Command::new("open")
        .arg(path)
        .spawn()
        .with_context(|| format!("Could not open folder {}", path.display()))?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn launch_folder(path: &Path) -> Result<()> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .with_context(|| format!("Could not open folder {}", path.display()))?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
fn launch_folder(_path: &Path) -> Result<()> {
    Err(anyhow!(
        "Opening the history folder is not supported on this platform."
    ))
}

fn app_data_directory(variable: &str) -> Result<PathBuf> {
    env::var_os(variable)
        .map(|directory| PathBuf::from(directory).join("r-video-fetcher"))
        .ok_or_else(|| anyhow!("The {variable} environment variable is not set."))
}
