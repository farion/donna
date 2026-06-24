use donna::app::{DonnaApp, native_options};
use donna::auth::run_auth_wizard;
use donna::config::AppConfig;
use donna::ipc::{
    IpcEvent, default_socket_path, release_socket_path, remove_stale_socket, send_wakeup,
    start_wakeup_listener, wakeup_window,
};
use donna::secrets::KeyringSecretStore;
use donna::storage::LocalStore;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let config_path = AppConfig::default_path();
    let socket_path = default_socket_path(&config_path);

    if args.iter().any(|arg| arg == "--wakeup") {
        if let Err(error) = send_wakeup(&socket_path) {
            remove_stale_socket(&socket_path);
            eprintln!("donna wakeup: {error}");
            std::process::exit(1);
        }
        wakeup_window();
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--auth") {
        if let Err(error) = run_auth_wizard(config_path.clone(), &KeyringSecretStore::default()) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let (wakeup_sender, wakeup_receiver) = mpsc::channel();
    let wakeup_receiver = Arc::new(Mutex::new(wakeup_receiver));
    if let Err(error) = start_wakeup_listener(socket_path.clone(), wakeup_sender) {
        eprintln!(
            "donna ipc: failed to listen on {}: {error}",
            socket_path.display()
        );
    }
    let hide_requested = Arc::new(AtomicBool::new(false));
    let app_hide_requested = hide_requested.clone();
    let app_wakeup_receiver = wakeup_receiver.clone();
    eframe::run_native(
        "Donna",
        native_options(),
        Box::new(move |creation| {
            Ok(Box::new(DonnaApp::new_with_hide_signal(
                creation,
                app_hide_requested.clone(),
                app_wakeup_receiver.clone(),
            )))
        }),
    )?;

    if hide_requested.load(Ordering::SeqCst) {
        run_hidden_daemon(wakeup_receiver, socket_path);
    }

    Ok(())
}

fn run_hidden_daemon(
    wakeup_receiver: Arc<Mutex<Receiver<IpcEvent>>>,
    socket_path: std::path::PathBuf,
) {
    eprintln!("donna hidden: background reminder loop is running");
    let (config, config_notice) = AppConfig::load_or_default_at(AppConfig::default_path());
    if let Some(notice) = config_notice {
        eprintln!("donna hidden: {notice}");
    }
    let store = match LocalStore::open(&config.data.database_path) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("donna hidden: storage unavailable: {error}");
            return;
        }
    };
    let mut last_check_minute = None;

    loop {
        let wakeup_requested = wakeup_receiver
            .lock()
            .map(|receiver| matches!(receiver.try_recv(), Ok(IpcEvent::Wakeup)))
            .unwrap_or(false);
        if wakeup_requested {
            eprintln!("donna hidden: wakeup requested, launching UI");
            release_socket_path(&socket_path);
            launch_ui_and_exit();
            return;
        }

        let Some(now) = unix_now_seconds() else {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        };
        let minute = now / 60;
        if last_check_minute != Some(minute) {
            last_check_minute = Some(minute);
            match store.create_todo_reminder_attention(now) {
                Ok(Some(item)) => {
                    eprintln!("donna hidden: reminder {} created, launching UI", item.id);
                    release_socket_path(&socket_path);
                    launch_ui_and_exit();
                    return;
                }
                Ok(None) => {}
                Err(error) => eprintln!("donna hidden: reminder check failed: {error}"),
            }
        }

        std::thread::sleep(Duration::from_millis(250));
    }
}

fn launch_ui_and_exit() {
    let Ok(exe) = std::env::current_exe() else {
        eprintln!("donna hidden: cannot resolve executable path");
        return;
    };
    match std::process::Command::new(exe).spawn() {
        Ok(mut child) => {
            let _ = child.wait();
        }
        Err(error) => eprintln!("donna hidden: failed to launch UI: {error}"),
    }
}

fn unix_now_seconds() -> Option<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds).ok()
}
