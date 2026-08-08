#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, IconData};
use sergas_zip_shrinker::app::ShrinkApp;
use std::sync::Arc;

fn load_app_icon() -> Option<Arc<IconData>> {
    let img = image::load_from_memory(include_bytes!("../assets/app_icon.png"))
        .ok()?
        .into_rgba8();
    let (width, height) = img.dimensions();
    Some(Arc::new(IconData {
        rgba: img.into_raw(),
        width,
        height,
    }))
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([860.0, 780.0])
        .with_min_inner_size([720.0, 640.0])
        .with_title("Sergas ZIP Shrinker")
        .with_drag_and_drop(true);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Sergas ZIP Shrinker",
        options,
        Box::new(|cc| Ok(Box::new(ShrinkApp::new(cc)))),
    )
}
