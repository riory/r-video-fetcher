use std::path::{Path, PathBuf};

pub fn choose_folder(initial_folder: &Path) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if initial_folder.is_dir() {
        dialog = dialog.set_directory(initial_folder);
    }
    dialog.pick_folder()
}
