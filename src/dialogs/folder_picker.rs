use std::path::{Path, PathBuf};

pub fn choose_folder(initial_folder: &Path) -> Option<PathBuf> {
    choose_folder_impl(initial_folder, None)
}

pub fn choose_folder_with_parent(initial_folder: &Path, parent: &eframe::Frame) -> Option<PathBuf> {
    choose_folder_impl(initial_folder, Some(parent))
}

fn choose_folder_impl(initial_folder: &Path, parent: Option<&eframe::Frame>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(parent) = parent {
        dialog = dialog.set_parent(parent);
    }
    if initial_folder.is_dir() {
        dialog = dialog.set_directory(initial_folder);
    }
    dialog.pick_folder()
}
