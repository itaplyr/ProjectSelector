mod app;
mod projects;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([620.0, 520.0])
            .with_min_inner_size([400.0, 300.0])
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        "Project Selector",
        options,
        Box::new(|_cc| Ok(Box::new(app::ProjectApp::default()))),
    )
}
