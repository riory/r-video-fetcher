use std::path::PathBuf;

pub fn open_project() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("r-video-fetcher project", &["dwnl"])
        .pick_file()
}
