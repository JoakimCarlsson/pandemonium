//! What the editor asks the desktop to do on its behalf.
//!
//! Copying to the clipboard and showing a file in the desktop's own file
//! manager are the two places the window steps outside itself. Both are best
//! effort: a desktop without a clipboard server or without a file manager is
//! a desktop where nothing happens, not one where the editor reports an
//! error it cannot do anything about.

use std::path::Path;
use std::process::{Command, Stdio};

/// Puts `text` on the system clipboard.
///
/// The clipboard is served for as long as another application has not taken
/// it over, which on Wayland and X11 alike means somebody has to stay and
/// hand the text out. That somebody is a thread of ours, so the window goes
/// on drawing while it waits.
pub fn copy(text: String) {
    std::thread::spawn(move || {
        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            return;
        };
        #[cfg(target_os = "linux")]
        {
            use arboard::SetExtLinux;
            let _ = clipboard.set().wait().text(text);
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = clipboard.set_text(text);
        }
    });
}

/// Shows `path` in the desktop's file manager.
pub fn reveal(path: &Path) {
    let target = if path.is_dir() {
        Some(path)
    } else {
        path.parent()
    };
    let Some(target) = target else {
        return;
    };

    let _ = Command::new(FILE_MANAGER)
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// The program that opens a directory in the desktop's file manager.
#[cfg(target_os = "linux")]
const FILE_MANAGER: &str = "xdg-open";

/// The program that opens a directory in the desktop's file manager.
#[cfg(target_os = "macos")]
const FILE_MANAGER: &str = "open";

/// The program that opens a directory in the desktop's file manager.
#[cfg(target_os = "windows")]
const FILE_MANAGER: &str = "explorer";
