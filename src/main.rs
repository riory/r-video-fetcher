#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use r_video_fetcher::{app::VideoFetcherApp, models::locales::LocaleStrings};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let locales = LocaleStrings::load()?;
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("resources/icon.png"))?;
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(icon)
            .with_inner_size([780.0, 860.0])
            .with_min_inner_size([640.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "r-video-fetcher",
        options,
        Box::new(move |_creation_context| Ok(Box::new(VideoFetcherApp::new(locales)))),
    )?;
    Ok(())
}
