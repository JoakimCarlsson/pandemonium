//! Telling the reader something happened while they may be looking elsewhere: a sound, or a notification of the desktop's own, through the programs each desktop has for them.

use std::process::{Command, Stdio};

use pm_config::Notification;

/// Tells the reader `title`, with `body` under it, the way `how` says, on a thread of its own so the window does not wait for it.
pub fn tell(how: Notification, title: &str, body: &str) {
    let (title, body) = (title.to_owned(), body.to_owned());
    std::thread::spawn(move || match how {
        Notification::Sound => play(),
        Notification::Desktop => show(&title, &body),
        Notification::None => {}
    });
}

/// Runs `program` with `arguments`, quietly, and says whether it ran and ended well.
fn run(program: &str, arguments: &[&str]) -> bool {
    Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Plays the desktop's sound for something that finished.
fn play() {
    if cfg!(target_os = "macos") {
        run("afplay", &["/System/Library/Sounds/Glass.aiff"]);
    } else if cfg!(target_os = "windows") {
        run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "[System.Media.SystemSounds]::Asterisk.Play()",
            ],
        );
    } else if !run("canberra-gtk-play", &["-i", "complete"]) {
        run(
            "paplay",
            &["/usr/share/sounds/freedesktop/stereo/complete.oga"],
        );
    }
}

/// `text` written inside a quoted string of AppleScript or PowerShell.
fn quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "'")
}

/// The bundle identifier of the app this program runs inside, read from its `Info.plist`, or `None` when it runs outside a bundle.
#[cfg(target_os = "macos")]
fn bundle_identifier() -> Option<String> {
    let executable = std::env::current_exe().ok()?;
    let contents = executable.parent()?.parent()?;
    let plist = std::fs::read_to_string(contents.join("Info.plist")).ok()?;
    let after_key = plist.split("<key>CFBundleIdentifier</key>").nth(1)?;
    let value = after_key
        .split("<string>")
        .nth(1)?
        .split("</string>")
        .next()?;
    Some(value.trim().to_owned())
}

/// Shows a notification from this app's own bundle, and says whether macOS took it.
#[cfg(target_os = "macos")]
fn show_from_bundle(title: &str, body: &str) -> bool {
    let Some(identifier) = bundle_identifier() else {
        return false;
    };
    let _ = mac_notification_sys::set_application(&identifier);
    mac_notification_sys::send_notification(title, None, body, None).is_ok()
}

/// Shows a notification of the desktop's own saying `title`, with `body` under it.
fn show(title: &str, body: &str) {
    if cfg!(target_os = "macos") {
        #[cfg(target_os = "macos")]
        if show_from_bundle(title, body) {
            return;
        }
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            quoted(body),
            quoted(title)
        );
        run("osascript", &["-e", &script]);
    } else if cfg!(target_os = "windows") {
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $n = New-Object System.Windows.Forms.NotifyIcon; $n.Icon = [System.Drawing.SystemIcons]::Information; $n.Visible = $true; $n.ShowBalloonTip(5000, \"{}\", \"{}\", 'Info'); Start-Sleep -Seconds 6; $n.Dispose()",
            quoted(title),
            quoted(body)
        );
        run("powershell", &["-NoProfile", "-Command", &script]);
    } else {
        run(
            "notify-send",
            &["--app-name", pm_store::app_name(), title, body],
        );
    }
}
