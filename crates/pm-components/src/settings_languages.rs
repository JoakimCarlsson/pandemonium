//! The languages page of the settings window: how the language picked is indented, formatted and read by its server, then every language installed and every one that can be installed, each with the button that installs or removes it.

use pm_input::FocusId;
use pm_protocol::ExtensionInfo;
use pm_style::Role;
use pm_ui::Node;

use crate::extensions::{group, pickable, row};
use crate::parts::line;
use crate::settings::Section;
use crate::settings_pages::page_rows;
use crate::state::WorkspaceState;

/// The first of the buttons installing or removing a language, in the order the page lists them; the rest follow it.
const LANGUAGE_ROW_BASE: u64 = 20_005_500;

/// One past the last of them.
const LANGUAGE_ROW_END: u64 = 20_006_500;

/// The first of the buttons updating a language, in the order the page lists those that are behind; the rest follow it.
const LANGUAGE_UPDATE_BASE: u64 = 20_005_400;

/// One past the last of them.
const LANGUAGE_UPDATE_END: u64 = 20_005_500;

impl WorkspaceState {
    /// The languages the page lists: those installed first, then those that can be, each by name.
    fn settings_languages(&self) -> Vec<&ExtensionInfo> {
        let mut listed: Vec<&ExtensionInfo> = pickable(&self.extensions).collect();
        listed.sort_by_key(|extension| (!extension.installed, extension.name.to_lowercase()));
        listed
    }

    /// Carries out pressing `id` on the languages page, installing or removing a language, and says whether it was something of it.
    pub(crate) fn languages_pressed(&mut self, id: FocusId) -> bool {
        if (LANGUAGE_UPDATE_BASE..LANGUAGE_UPDATE_END).contains(&id.0) {
            let place = (id.0 - LANGUAGE_UPDATE_BASE) as usize;
            let behind: Vec<ExtensionInfo> = self
                .settings_languages()
                .into_iter()
                .filter(|extension| extension.installed && extension.update.is_some())
                .cloned()
                .collect();
            if let Some(extension) = behind.get(place) {
                self.update_extension(extension);
            }
            return true;
        }
        if !(LANGUAGE_ROW_BASE..LANGUAGE_ROW_END).contains(&id.0) {
            return false;
        }
        let place = (id.0 - LANGUAGE_ROW_BASE) as usize;
        if let Some(extension) = self.settings_languages().get(place).copied().cloned() {
            self.install_or_remove(&extension);
        }
        true
    }
}

/// The body of the languages page, `room` logical pixels across.
pub(crate) fn body(state: &WorkspaceState, room: f32) -> Vec<Node> {
    let mut installed = Vec::new();
    let mut available = Vec::new();
    let mut behind = 0;
    for (place, extension) in state.settings_languages().into_iter().enumerate() {
        let update = (extension.installed && extension.update.is_some()).then(|| {
            behind += 1;
            FocusId(
                LANGUAGE_UPDATE_BASE
                    + (behind - 1).min(LANGUAGE_UPDATE_END - LANGUAGE_UPDATE_BASE - 1),
            )
        });
        let built = row(extension, FocusId(LANGUAGE_ROW_BASE + place as u64), update);
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
