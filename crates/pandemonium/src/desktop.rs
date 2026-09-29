//! What the editor asks the desktop to do on its behalf.
//!
//! Copying to the clipboard, showing a file in the desktop's own file manager
//! and opening an address in its browser are the places the window steps
//! outside itself. All of them are best effort: a desktop without a clipboard
//! server or without a file manager is a desktop where nothing happens, not one where the editor reports an
//! error it cannot do anything about.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use image::ColorType;
use image::ImageEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};

/// The next pasted image's temporary file suffix.
static NEXT_PASTED_IMAGE: AtomicU64 = AtomicU64::new(0);

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

    launch(target.as_os_str());
}

/// The schemes an address must have for [`browse`] to hand it on.
const BROWSABLE: [&str; 4] = ["https://", "http://", "ftp://", "mailto:"];

/// Opens `address` in the desktop's browser or mail client.
///
/// The address may have come from anything a program printed, so only the
/// schemes a browser or a mail client answers are handed on: a path, a
/// `file://` pointing at a launcher, or a target that reads as a flag to the
/// opener is ignored rather than run.
pub fn browse(address: &str) {
    let lower = address.to_ascii_lowercase();
    if BROWSABLE.iter().any(|scheme| lower.starts_with(scheme)) {
        launch(std::ffi::OsStr::new(address));
    }
}

/// Hands `target` to the desktop's opener, without waiting on it.
fn launch(target: &std::ffi::OsStr) {
    let _ = Command::new(OPENER)
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// The program that opens a directory, a file or an address the desktop's way.
#[cfg(target_os = "linux")]
const OPENER: &str = "xdg-open";

/// The program that opens a directory, a file or an address the desktop's way.
#[cfg(target_os = "macos")]
const OPENER: &str = "open";

/// The program that opens a directory, a file or an address the desktop's way.
#[cfg(target_os = "windows")]
const OPENER: &str = "explorer";

/// What is on the system clipboard, if anything readable is.
///
/// Reading is done here and now rather than on a thread of its own: a paste
/// is a keypress the reader is waiting on, and a clipboard that does not
/// answer is a paste of nothing rather than a window that stops drawing.
pub fn paste() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// Existing files named by the system clipboard, in clipboard order.
pub fn paste_files() -> Option<Vec<PathBuf>> {
    let files: Vec<_> = arboard::Clipboard::new()
        .ok()?
        .get()
        .file_list()
        .ok()?
        .into_iter()
        .filter(|path| path.is_file())
        .collect();
    (!files.is_empty()).then_some(files)
}

/// The image on the system clipboard, as its width, its height and its
/// straight-alpha RGBA pixels.
pub fn paste_image() -> Option<(u32, u32, Vec<u8>)> {
    let image = arboard::Clipboard::new().ok()?.get_image().ok()?;
    Some((
        u32::try_from(image.width).ok()?,
        u32::try_from(image.height).ok()?,
        image.bytes.into_owned(),
    ))
}

/// The longest side of a pasted image sent to an agent.
pub const PASTED_IMAGE_MAX_SIDE: u32 = 2048;

/// `width` by `height` RGBA `pixels` as a PNG, compressed quickly rather than
/// small: a screenshot is sent once and waited on while it is encoded.
pub fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Adaptive)
        .write_image(pixels, width, height, ColorType::Rgba8.into())
        .ok()?;
    Some(png)
}

/// Keeps a pasted PNG readable by an agent that accepts file links.
pub fn save_pasted_image(png: &[u8]) -> Option<std::path::PathBuf> {
    let directory = std::env::temp_dir().join("pandemonium");
    std::fs::create_dir_all(&directory).ok()?;
    let sequence = NEXT_PASTED_IMAGE.fetch_add(1, Ordering::Relaxed);
    let path = directory.join(format!("pasted-{}-{sequence}.png", std::process::id()));
    std::fs::write(&path, png).ok()?;
    Some(path)
}
