mod dark;
mod default;
mod zenburn;

use crate::model::AppTheme;
use eframe::egui;

pub fn visuals(theme: AppTheme) -> egui::Visuals {
    match theme {
        AppTheme::Default => default::visuals(),
        AppTheme::Dark => dark::visuals(),
        AppTheme::Zenburn => zenburn::visuals(),
    }
}
