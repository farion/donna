use std::process::{Command, Stdio};

pub fn open_url(url: &str) -> Result<(), String> {
    let commands: &[(&str, &[&str])] = if cfg!(target_os = "windows") {
        &[("cmd", &["/C", "start", "", url])]
    } else if cfg!(target_os = "macos") {
        &[("open", &[url])]
    } else {
        &[
            ("xdg-open", &[url]),
            ("gio", &["open", url]),
            ("kde-open5", &[url]),
            ("gnome-open", &[url]),
        ]
    };

    for (program, args) in commands {
        let Ok(status) = Command::new(program)
            .args(*args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        else {
            continue;
        };
        if status.success() {
            return Ok(());
        }
    }

    Err("could not open the browser automatically".to_owned())
}
