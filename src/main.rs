#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use sergas_zip_shrinker::app::ShrinkApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 620.0])
            .with_min_inner_size([560.0, 480.0])
            .with_title("Sergas ZIP Shrinker")
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Sergas ZIP Shrinker",
        options,
        Box::new(|cc| Ok(Box::new(ShrinkApp::new(cc)))),
    )
}
