//! The keymap grid.

use pm_ui::{Div, Theme, toggle_grid};

use crate::onboarding::section::section;
use crate::onboarding::setup::{KEYMAPS, Message, Setup};

/// The keymap grid.
pub(super) fn keymap_section(theme: &Theme, setup: &Setup) -> Div<Message> {
    let options = KEYMAPS
        .into_iter()
        .enumerate()
        .map(|(index, keymap)| (keymap.to_owned(), Message::SetKeymap(index)));

    section(
        theme,
        "Base Keymap",
        Some("Keep the bindings your hands already know"),
        toggle_grid(options, Some(setup.keymap), 4),
    )
}
