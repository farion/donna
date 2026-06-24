use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;

const WAKEUP_MESSAGE: &[u8] = b"wakeup\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcEvent {
    Wakeup,
}

pub fn default_socket_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("donna.sock")
}

#[cfg(unix)]
pub fn send_wakeup(socket_path: &Path) -> std::io::Result<()> {
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path)?;
    stream.write_all(WAKEUP_MESSAGE)
}

#[cfg(unix)]
pub fn remove_stale_socket(socket_path: &Path) {
    if send_wakeup(socket_path).is_err() {
        let _ = std::fs::remove_file(socket_path);
    }
}

#[cfg(not(unix))]
pub fn remove_stale_socket(_socket_path: &Path) {}

#[cfg(not(unix))]
pub fn send_wakeup(_socket_path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Donna wakeup IPC is only implemented on Unix platforms for now",
    ))
}

#[cfg(unix)]
pub fn start_wakeup_listener(
    socket_path: PathBuf,
    sender: Sender<IpcEvent>,
) -> std::io::Result<()> {
    use std::os::unix::net::UnixListener;

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = [0; WAKEUP_MESSAGE.len()];
            if stream.read(&mut buffer).is_ok() && buffer == WAKEUP_MESSAGE {
                let _ = sender.send(IpcEvent::Wakeup);
            }
        }
    });
    Ok(())
}

#[cfg(not(unix))]
pub fn start_wakeup_listener(
    _socket_path: PathBuf,
    _sender: Sender<IpcEvent>,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Donna wakeup IPC is only implemented on Unix platforms for now",
    ))
}

pub fn release_socket_path(socket_path: &Path) {
    let _ = std::fs::remove_file(socket_path);
}

pub fn wakeup_window() -> bool {
    if std::env::var_os("SWAYSOCK").is_some() {
        return command_succeeds("swaymsg", &[r#"[app_id="donna"] focus"#])
            || command_succeeds("swaymsg", &[r#"[app_id="donna"] scratchpad show"#]);
    }
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return command_succeeds("hyprctl", &["dispatch", "focuswindow", "class:donna"])
            || command_succeeds("hyprctl", &["dispatch", "togglespecialworkspace", "donna"]);
    }

    false
}

fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn socket_path_lives_next_to_config() {
        let path = default_socket_path(Path::new("/tmp/donna/donna.toml"));

        assert_eq!(path, Path::new("/tmp/donna/donna.sock"));
    }

    #[cfg(unix)]
    #[test]
    fn wakeup_round_trips_over_unix_socket() {
        let dir = tempfile::tempdir().expect("dir");
        let socket = dir.path().join("donna.sock");
        let (sender, receiver) = mpsc::channel();

        start_wakeup_listener(socket.clone(), sender).expect("listener");
        send_wakeup(&socket).expect("send wakeup");

        assert_eq!(
            receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("event"),
            IpcEvent::Wakeup
        );
        release_socket_path(&socket);
    }
}
