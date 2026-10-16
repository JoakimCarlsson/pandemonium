//! The languages page of the settings panel: every language there is, whether the server on this machine has it, and a button to install or remove it.

use pm_input::FocusId;
use pm_protocol::LanguageInfo;
use pm_style::{Role, flex, flex_1, flex_col, font_mono, gap, items_center, min_w_px};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::{Size, Variant};
use crate::parts::line;
use crate::settings_servers::{boxed, flat_row, note};
use crate::state::{Outgoing, WorkspaceState};

/// The first of the buttons installing or removing a language; the rest follow it.
pub const LANGUAGE_BASE: u64 = 20_000_400;

/// One past the last of the buttons installing or removing a language.
const LANGUAGE_END: u64 = 20_000_500;

/// Whether `id` names a button installing or removing a language.
pub fn is_language_button(id: FocusId) -> bool {
    (LANGUAGE_BASE..LANGUAGE_END).contains(&id.0)
}

/// How many characters of why installing a language failed its row has room for.
const PROBLEM_LENGTH: usize = 64;

/// `words` cut to `most` characters, with an ellipsis where it was cut.
fn shortened(words: &str, most: usize) -> String {
    if words.chars().count() <= most {
        return words.to_owned();
    }
    let kept: String = words.chars().take(most - 1).collect();
    format!("{}…", kept.trim_end())
}

/// The languages the reader picks from: every one but those installed only as part of another.
pub fn pickable(languages: &[LanguageInfo]) -> impl Iterator<Item = &LanguageInfo> {
    languages.iter().filter(|language| !language.hidden)
}

/// What a language is doing or has, in the words of its row.
pub fn standing(language: &LanguageInfo) -> (String, Role) {
    if let Some(busy) = &language.busy {
        return (format!("{busy}…"), Role::Muted);
    }
    if let Some(problem) = &language.problem {
        return (shortened(problem, PROBLEM_LENGTH), Role::Danger);
    }
    match (&language.server, language.installed) {
        (Some(server), true) => (server.clone(), Role::Muted),
        (None, true) => ("Installed, with no language server".to_owned(), Role::Muted),
        (_, false) => ("Not installed".to_owned(), Role::Subtle),
    }
}

/// The endings of the file names a language is written in, as `.rs .py`.
pub fn endings(language: &LanguageInfo) -> String {
    language
        .suffixes
        .iter()
        .take(4)
        .map(|suffix| format!(".{suffix}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl WorkspaceState {
    /// The state after the server on this machine said which languages there are.
    pub fn with_languages(mut self, languages: Vec<LanguageInfo>) -> Self {
        self.languages = languages;
        self
    }

    /// The state after the button installing or removing a language was pressed.
    pub(crate) fn language_pressed(mut self, action: FocusId) -> Self {
        let place = (action.0 - LANGUAGE_BASE) as usize;
        let Some(language) = pickable(&self.languages).nth(place).cloned() else {
            return self;
        };
        if language.busy.is_some() {
            return self;
        }
        self.outbox.push(if language.installed {
            Outgoing::RemoveLanguage { id: language.id }
        } else {
            Outgoing::InstallLanguages {
                ids: vec![language.id],
            }
        });
        self
    }
}

/// The page listing the languages, under `page`'s heading.
pub(crate) fn list(state: &WorkspaceState, page: Node) -> Node {
    if state.languages.is_empty() {
        return page.child(note("Asking the server which languages there are…"));
    }
    let rows: Vec<Node> = pickable(&state.languages)
        .enumerate()
        .map(|(place, language)| {
            let (said, tone) = standing(language);
            let action = FocusId(LANGUAGE_BASE + place as u64);
            let button = match (language.busy.is_some(), language.installed) {
                (true, _) => Button::new("Installing", action)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .disabled(true)
                    .build(),
                (false, true) => Button::new("Remove", action)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .build(),
                (false, false) => Button::new("Install", action)
                    .variant(Variant::Outline)
                    .size(Size::Sm)
                    .build(),
            };
            flat_row()
                .child(
                    node()
                        .styles([flex(), flex_col(), flex_1(), min_w_px(0)])
                        .child(
                            node()
                                .styles([flex(), items_center(), gap(2)])
                                .child(line(language.name.clone(), 14, 1.4, Role::Text))
                                .child(
                                    line(endings(language), 12, 1.4, Role::Subtle)
                                        .styles([font_mono()]),
                                ),
                        )
                        .child(line(said, 12, 1.4, tone)),
                )
                .child(button)
        })
        .collect();
    page.child(boxed(rows))
}
