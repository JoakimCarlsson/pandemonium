//! The languages page of the settings window: how the language picked is indented, formatted and read by its server, then every language installed and every one that can be installed, each with the button that installs or removes it.

use pm_input::FocusId;
use pm_protocol::ExtensionInfo;
use pm_style::Role;
use pm_ui::Node;

use crate::extensions::{group, pickable, row};
use crate::parts::line;
use crate::settings::Section;
use crate::settings_ids::LANGUAGE_ROWS;
use crate::settings_pages::page_rows;
use crate::state::WorkspaceState;

impl WorkspaceState {
    /// The languages the page lists: those installed first, then those that can be, each by name.
    fn settings_languages(&self) -> Vec<&ExtensionInfo> {
        let mut listed: Vec<&ExtensionInfo> = pickable(&self.extensions).collect();
        listed.sort_by_key(|extension| (!extension.installed, extension.name.to_lowercase()));
        listed
    }

    /// Carries out pressing `id` on the languages page, installing or removing a language, and says whether it was something of it.
    pub(crate) fn languages_pressed(&mut self, id: FocusId) -> bool {
        let Some(place) = LANGUAGE_ROWS.index_of(id) else {
            return false;
        };
        if let Some(extension) = self.settings_languages().get(place).copied().cloned() {
            self.install_or_remove(&extension);
        }
        true
    }
}

/// The body of the languages page, `room` logical pixels across; languages past the room for their buttons are left out.
pub(crate) fn body(state: &WorkspaceState, room: f32) -> Vec<Node> {
    let mut installed = Vec::new();
    let mut available = Vec::new();
    for (place, extension) in state.settings_languages().into_iter().enumerate() {
        let Some(action) = LANGUAGE_ROWS.id(place) else {
            break;
        };
        let built = row(extension, action);
        if extension.installed {
            installed.push(built);
        } else {
            available.push(built);
        }
    }
    let mut parts = vec![page_rows(state, Section::Languages, room)];
    if state.extensions.is_empty() {
        parts.push(line(
            "Asking the server which languages there are…",
            13,
            1.4,
            Role::Subtle,
        ));
    }
    parts.extend(group("Installed", installed));
    parts.extend(group("Available to install", available));
    parts
}
