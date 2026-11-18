//! What each window shows: the root composables the live windows and the
//! snapshots draw, from the screen the app holds.

use pm_components::kit::report::{self, WindowKey};
use pm_components::kit::theme::{self, Tokens};
use pm_components::kit::{Interactive, Styled, block, overlay};
use pm_pages::{Screen, picker_screen, screen, settings_window};
use rustle::{CursorIcon, Modifier};

/// The window key the settings window keeps its report under.
pub const SETTINGS_WINDOW: WindowKey = 1;

/// The window key the folder picker's window keeps its report under.
pub const PICKER_WINDOW: WindowKey = 2;

/// The area that takes files dragged in from other programs: the whole main window.
pub const DROP_AREA: pm_components::kit::FocusId = pm_components::kit::FocusId(9_990_001);

/// Sets interface text in the family `sans` and code in `mono`, each by name,
/// falling back to the fonts the app ships; an empty name is the one the app ships.
pub fn set_fonts(sans: &str, mono: &str) {
    theme::set_font_choice(sans, mono);
    theme::fonts_changed();
}

/// Lays out what follows on this thread as closely, and with interface text
/// as big, as the window's settings in `state` say.
pub fn scale_for(state: &Screen) {
    let appearance = match state {
        Screen::Workspace(held) => held.window.appearance.clone(),
    };
    theme::set_scales(appearance.density.scale(), appearance.text_scale());
}

/// Emits `content`, a window's frame `width` by `height` logical pixels
/// across, inset in a window larger by the band that holds the frame's
/// shadow while there is one.
#[track_caller]
pub fn banded(content: impl FnOnce(f32, f32), width: f32, height: f32) {
    let band = pm_components::shadow_margin();
    block(Modifier::new().fill_max_size().padding(band), || {
        content(width - 2.0 * band, height - 2.0 * band)
    });
}

/// Emits the main window's frame for `state` in a window `width` by `height`
/// logical pixels, inset by the band that holds the frame's shadow.
#[track_caller]
pub fn main_frame(state: &Screen, width: f32, height: f32) {
    banded(|width, height| screen(state, width, height), width, height);
}

/// Emits what the window beside the main one kept under `window` shows for
/// `state`, in a window `width` by `height` logical pixels, inset by the band
/// that holds its frame's shadow.
#[track_caller]
pub fn aside_frame(state: &Screen, window: WindowKey, width: f32, height: f32) {
    banded(
        |width, height| {
            if window == SETTINGS_WINDOW {
                settings_window(state, width, height);
            } else {
                picker_screen(state, width, height);
            }
        },
        width,
        height,
    );
}

/// Emits the main window's root for `state` in a window `width` by `height`
/// logical pixels drawn with `tokens`: the screen in its frame, the tooltips
/// over it, the pointer shape `cursor` over everything when the host decides
/// it, and the area that takes dropped files.
#[track_caller]
pub fn main_root(
    state: &Screen,
    width: f32,
    height: f32,
    tokens: Tokens,
    cursor: Option<CursorIcon>,
) {
    scale_for(state);
    theme::provide(tokens, || {
        report::root(|| {
            block(
                Modifier::new().fill_max_size().file_drop_id(DROP_AREA),
                || main_frame(state, width, height),
            );
            overlay::tooltips(report::MAIN);
            if let Some(cursor) = cursor {
                rustle::spacer(Modifier::new().cover().cursor(cursor));
            }
        });
    });
}

/// Emits the root of the window beside the main one kept under `window`,
/// `width` by `height` logical pixels, drawn with `tokens`: the settings
/// window or the folder picker's, with the tooltips over it.
#[track_caller]
pub fn aside_root(state: &Screen, window: WindowKey, width: f32, height: f32, tokens: Tokens) {
    scale_for(state);
    theme::provide(tokens, || {
        report::root_of(window, || {
            aside_frame(state, window, width, height);
            overlay::tooltips(window);
        });
    });
}
