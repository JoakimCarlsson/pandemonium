//! The keymap grid.

use pm_ui::{Div, Theme, toggle_grid};

use crate::keymap::BaseKeymap;
use crate::onboarding::section::section;
use crate::onboarding::setup::{Message, Setup};

/// The keymap grid.
pub(super) fn keymap_section(theme: &Theme, setup: &Setup) -> Div<Message> {
    let options = BaseKeymap::ALL
        .into_iter()
        .map(|keymap| (keymap.label().to_owned(), Message::SetKeymap(keymap)));

    section(
        theme,
        "Base Keymap",
        Some("Keep the bindings your hands already know"),
        toggle_grid(options, Some(setup.keymap.index()), 4),
    )
}
