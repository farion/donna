use donna::app::DonnaApp;
use donna::config::AppConfig;
use donna::microsoft::auth::run_auth_wizard;
use donna::secrets::KeyringSecretStore;
use eframe::egui;

fn main() -> eframe::Result<()> {
    if std::env::args().skip(1).any(|arg| arg == "--auth") {
        if let Err(error) =
            run_auth_wizard(AppConfig::default_path(), &KeyringSecretStore::default())
        {
            eprintln!("{error}");
            std::process::exit(1);
        }
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
