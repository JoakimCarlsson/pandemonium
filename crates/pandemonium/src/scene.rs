//! What each window shows: the root composables the live windows and the
//! snapshots draw, from the screen the app holds.

use pm_components::theme::{self, Tokens};
use pm_pages::{Screen, picker_screen, screen, settings_window};
use rustle::{Alignment, Arrangement, column_arranged};
use rustle::{CursorIcon, Modifier};

/// Which window a root composable fills: the main window is [`MAIN_WINDOW`], the windows
/// beside it are numbered.
pub type WindowKey = u64;

/// The key of the main window.
pub const MAIN_WINDOW: WindowKey = 0;

/// The window key of the settings window.
pub const SETTINGS_WINDOW: WindowKey = 1;

/// The window key of the folder picker's window.
pub const PICKER_WINDOW: WindowKey = 2;

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
    column_arranged(
        Modifier::new().fill_max_size().padding(band),
        Arrangement::Start,
        Alignment::Start,
        || content(width - 2.0 * band, height - 2.0 * band),
    );
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
        rustle::stack_with(Modifier::new().fill_max_size(), || {
            column_arranged(
                Modifier::new()
                    .fill_max_size()
                    .chain(app_keys())
                    .chain(pm_components::events::carry_target())
                    .chain(pm_components::events::file_drop()),
                Arrangement::Start,
                Alignment::Start,
                || main_frame(state, width, height),
            );
            if let Some(cursor) = cursor {
                rustle::spacer(Modifier::new().match_parent_size().cursor(cursor));
            }
            pm_components::resize_edges();
        });
    });
}

/// Emits the root of the window beside the main one kept under `window`,
/// `width` by `height` logical pixels, drawn with `tokens`: the settings
/// window or the folder picker's.
#[track_caller]
pub fn aside_root(state: &Screen, window: WindowKey, width: f32, height: f32, tokens: Tokens) {
    scale_for(state);
    theme::provide(tokens, || {
        rustle::stack_with(Modifier::new().fill_max_size(), || {
            rustle::stack_with(Modifier::new().fill_max_size().chain(app_keys()), || {
                aside_frame(state, window, width, height);
            });
            pm_components::resize_edges();
        });
    });
}

/// The modifier that hands every key of the window to the app's state holder before
/// whatever holds focus sees it, the root key handler that keeps the app-wide shortcuts:
/// a key the app takes goes no further, and one it leaves goes on to the focused control,
/// then to rustle (Tab walks focus).
fn app_keys() -> Modifier {
    let operations = pm_components::events::operations();
    Modifier::new().on_preview_key_event(move |event| operations.key(event))
}
