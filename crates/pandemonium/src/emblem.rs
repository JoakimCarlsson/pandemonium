//! The editor's own icon, as the window hands it to the desktop.
//!
//! Where the desktop takes a window's icon from the window itself, on X11
//! and Windows, it is the picture below. Where it looks the icon up by the
//! window's application id instead, on Wayland, it is the id below, which
//! names the installed `pandemonium.desktop` entry and its `Icon=`.

use winit::window::Icon;

/// The icon, rendered at the largest size a title bar or task switcher asks for.
const PICTURE: &[u8] = include_bytes!("../../../assets/brand/png/pandemonium-256.png");

/// The application id the desktop entry is installed under.
pub const APP_ID: &str = "pandemonium";

/// Returns the icon decoded for the window, or nothing if it will not decode.
pub fn window_icon() -> Option<Icon> {
    let picture = image::load_from_memory_with_format(PICTURE, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (width, height) = picture.dimensions();
    Icon::from_rgba(picture.into_raw(), width, height).ok()
}
