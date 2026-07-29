use donna_config::AppConfig;
use donna_integrations::auth::run_auth_wizard;
use donna_integrations::microsoft::background_sync::run_sync_once as run_microsoft_sync_once;
use donna_integrations::microsoft::calendar::CALENDAR_SOURCE;
use donna_integrations::microsoft::outlook::OUTLOOK_MAIL_SOURCE;
use donna_integrations::microsoft::teams::{TEAMS_CHANNEL_SOURCE, TEAMS_CHAT_SOURCE};
use donna_integrations::secrets::KeyringSecretStore;
use donna_storage::LocalStore;
use donna_ui::app::{DonnaApp, native_options};
use donna_ui::ipc::{
    IpcEvent, default_socket_path, new_repaint_signal, release_socket_path, remove_stale_socket,
    send_wakeup, start_wakeup_listener,
};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let config_path = AppConfig::default_path();
    let socket_path = default_socket_path(&config_path);

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    if let Some(index) = args.iter().position(|arg| arg == "--reset-sync") {
        let target = args.get(index + 1).map(String::as_str).unwrap_or("all");
        if let Err(error) = reset_sync(&config_path, target) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--wakeup") {
        if let Err(error) = send_wakeup(&socket_path) {
            remove_stale_socket(&socket_path);
            eprintln!("donna wakeup: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--auth") {
        if let Err(error) = run_auth_wizard(config_path.clone(), &KeyringSecretStore::default()) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // Single-instance guard: if another Donna is already listening on the
    // wakeup socket, wake it up and exit instead of starting a second full
    // UI process. Without this check, a second launch would silently delete
    // the first instance's socket file (see `start_wakeup_listener` below)
    // and orphan it — both processes then keep running side by side, which
    // is how duplicate `donna-ui` processes accumulate.
    if send_wakeup(&socket_path).is_ok() {
        eprintln!("donna: another instance is already running; sent wakeup and exiting");
        return Ok(());
    }
    remove_stale_socket(&socket_path);

    let (wakeup_sender, wakeup_receiver) = mpsc::channel();
    let wakeup_receiver = Arc::new(Mutex::new(wakeup_receiver));
    let repaint_signal = new_repaint_signal();
    if let Err(error) =
        start_wakeup_listener(socket_path.clone(), wakeup_sender, repaint_signal.clone())
    {
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
                repaint_signal.clone(),
            )))
        }),
    )?;

    if hide_requested.load(Ordering::SeqCst) {
        run_hidden_daemon(wakeup_receiver, socket_path);
    }

    Ok(())
}

fn print_help() {
    println!(
        "Donna — a local-first personal work-life assistant.

USAGE:
    donna [OPTIONS]

OPTIONS:
    --auth                  Run the Microsoft Graph / AI provider auth setup wizard.
    --wakeup                Wake and show an already-running hidden Donna instance.
    --reset-sync [TARGET]   Clear synced Microsoft data's sync progress so the next
                            start does a full resync instead of an incremental one.
                            TARGET is one of: all (default), calendar, outlook, teams.
    --help, -h              Show this help message.

With no options, Donna launches its desktop chat UI."
    );
}

fn reset_sync(config_path: &std::path::Path, target: &str) -> Result<(), String> {
    let sources: &[&str] = match target {
        "all" => &[
            CALENDAR_SOURCE,
            OUTLOOK_MAIL_SOURCE,
            TEAMS_CHAT_SOURCE,
            TEAMS_CHANNEL_SOURCE,
        ],
        "calendar" => &[CALENDAR_SOURCE],
        "outlook" => &[OUTLOOK_MAIL_SOURCE],
        "teams" => &[TEAMS_CHAT_SOURCE, TEAMS_CHANNEL_SOURCE],
        other => {
            return Err(format!(
                "donna --reset-sync: unknown target '{other}'. Use all, calendar, outlook, or teams."
            ));
        }
    };

    let (config, _) = AppConfig::load_or_default_at(config_path);
    let store = LocalStore::open(&config.data.database_path)
        .map_err(|error| format!("donna --reset-sync: storage unavailable: {error}"))?;

    let mut cleared = 0usize;
    for source in sources {
        cleared += store
            .reset_sync_state(Some(source))
            .map_err(|error| format!("donna --reset-sync: {error}"))?;
    }

    println!(
        "donna: cleared sync state for '{target}' ({cleared} source record(s)). \
         The next start will do a full resync."
    );
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
    eprintln!("donna hidden: triggering Microsoft sync (startup)");
    if let Err(error) = run_microsoft_sync_once(&store, &config, &KeyringSecretStore::default()) {
        eprintln!("donna hidden: microsoft sync failed on startup: {error}");
    }
    let mut last_check_minute = None;
    let mut last_microsoft_sync_minute = unix_now_seconds().map(|seconds| seconds / 60);

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
        if last_microsoft_sync_minute != Some(minute) {
            last_microsoft_sync_minute = Some(minute);
            eprintln!("donna hidden: triggering Microsoft sync (minute={minute})");
            if let Err(error) = run_microsoft_sync_once(&store, &config, &KeyringSecretStore::default()) {
                eprintln!("donna hidden: microsoft sync failed: {error}");
            }
        }
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
