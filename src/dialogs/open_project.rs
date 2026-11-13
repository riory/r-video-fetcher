use std::path::PathBuf;

pub fn open_project() -> Option<PathBuf> {
    open_project_impl(None)
}

pub fn open_project_with_parent(parent: &eframe::Frame) -> Option<PathBuf> {
    open_project_impl(Some(parent))
}

fn open_project_impl(parent: Option<&eframe::Frame>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(parent) = parent {
        dialog = dialog.set_parent(parent);
    }
    dialog
        .add_filter("r-video-fetcher project", &["dwnl"])
        .pick_file()
}
