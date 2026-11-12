use std::path::{Path, PathBuf};

pub fn save_project(suggested_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
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
