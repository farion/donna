use donna::app::DonnaApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    if std::env::args().skip(1).any(|arg| arg == "--auth") {
        println!("Donna auth setup is not implemented yet.");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Donna")
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Donna",
        options,
        Box::new(|creation| Ok(Box::new(DonnaApp::new(creation)))),
    )
}
