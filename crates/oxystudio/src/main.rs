#![windows_subsystem = "windows"]

mod app;

use app::OxyStudioApp;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("Oxy Studio")
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Oxy Studio",
        native_options,
        Box::new(|cc| Ok(Box::new(OxyStudioApp::new(cc)))),
    )
}
