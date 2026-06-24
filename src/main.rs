use donna::app::{DonnaApp, native_options};
use donna::config::AppConfig;
use donna::microsoft::auth::run_auth_wizard;
use donna::secrets::KeyringSecretStore;

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

    eframe::run_native(
        "Donna",
        native_options(),
        Box::new(|creation| Ok(Box::new(DonnaApp::new(creation)))),
    )
}
