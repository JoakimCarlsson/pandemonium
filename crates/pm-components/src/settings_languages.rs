//! The languages page of the settings window: how the language picked is indented, formatted and read by its server, then every language installed and every one that can be installed, each with the button that installs or removes it.

use crate::target::Target;
use pm_protocol::ExtensionInfo;

use crate::theme::Role;

use crate::extensions::{group, pickable, row};
use crate::parts::line;
use crate::settings::Section;
use crate::settings_pages::page_rows;
use crate::settings_rows::LANGUAGE_ROWS;
use crate::state::WorkspaceState;

impl WorkspaceState {
    /// The languages the page lists: those installed first, then those that can be, each by name.
    fn settings_languages(&self) -> Vec<&ExtensionInfo> {
        let mut listed: Vec<&ExtensionInfo> = pickable(&self.extensions).collect();
        listed.sort_by_key(|extension| (!extension.installed, extension.name.to_lowercase()));
        listed
    }

    /// Carries out pressing `id` on the languages page, installing or removing a language, and says whether it was something of it.
    pub(crate) fn languages_pressed(&mut self, id: Target) -> bool {
        let Some(place) = LANGUAGE_ROWS.index_of(id) else {
            return false;
        };
        if let Some(extension) = self.settings_languages().get(place).copied().cloned() {
            self.install_or_remove(&extension);
        }
        true
    }
}

/// Emits the body of the languages page, `room` logical pixels across: the settings of the page,
/// then the languages installed and those that can be, each in its group; languages past the
/// room for their buttons are left out.
#[track_caller]
pub(crate) fn body(state: &WorkspaceState, room: f32) {
    let (installed, available): (Vec<_>, Vec<_>) = state
        .settings_languages()
        .into_iter()
        .enumerate()
        .map_while(|(place, extension)| Some((extension, LANGUAGE_ROWS.id(place)?)))
        .partition(|(extension, _)| extension.installed);
    page_rows(state, Section::Languages, room);
    if state.extensions.is_empty() {
        line(
            "Asking the server which languages there are…",
            13,
            1.4,
            Role::Subtle,
        );
    }
    listed("Installed", &installed);
    listed("Available to install", &available);
}

/// Emits the group titled `title` holding a row for each of `members`, a language with the
/// control that installs or removes it; nothing when there are none.
#[track_caller]
fn listed(title: &str, members: &[(&ExtensionInfo, Target)]) {
    if members.is_empty() {
        return;
    }
    group(title, || {
        for &(extension, action) in members {
            rustle::key(action, || row(extension, action));
        }
    });
}
