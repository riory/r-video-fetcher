use std::path::{Path, PathBuf};

pub fn save_project(suggested_name: &str) -> Option<PathBuf> {
    save_project_impl(suggested_name, None)
}

pub fn save_project_with_parent(suggested_name: &str, parent: &eframe::Frame) -> Option<PathBuf> {
    save_project_impl(suggested_name, Some(parent))
}

fn save_project_impl(suggested_name: &str, parent: Option<&eframe::Frame>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(parent) = parent {
        dialog = dialog.set_parent(parent);
    }
    dialog
        .add_filter("r-video-fetcher project", &["dwnl"])
        .set_file_name(suggested_name)
        .save_file()
        .map(|path| with_project_extension(path, suggested_name))
}

fn with_project_extension(path: PathBuf, suggested_name: &str) -> PathBuf {
    if path.extension().is_none() {
        path.with_extension(Path::new(suggested_name).extension().unwrap_or_default())
    } else {
        path
    }
}
