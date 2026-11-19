//! The code a sign-in waits for the reader to enter on a web page: the dialog that shows it while the sign-in runs, and what its buttons do.

use crate::focus::FocusId;
use crate::theme;
use crate::theme::Role;
use crate::theme::sp;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use crate::button::Variant;
use crate::parts::{line, spacer, wrap};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Button, Modal};

/// The dimmed window behind the dialog.
pub const SIGN_IN_CODE_OUTSIDE: FocusId = FocusId(7_442_000);

/// The panel of the dialog.
pub const SIGN_IN_CODE_INSIDE: FocusId = FocusId(7_442_001);

/// Puts the code on the clipboard.
pub const SIGN_IN_CODE_COPY: FocusId = FocusId(7_442_002);

/// Opens the page the code is entered on.
pub const SIGN_IN_CODE_OPEN: FocusId = FocusId(7_442_003);

/// Hides the dialog, leaving the sign-in running.
pub const SIGN_IN_CODE_HIDE: FocusId = FocusId(7_442_004);

/// How wide the dialog is, in logical pixels.
const WIDTH: f32 = 420.0;

/// A code a sign-in waits for the reader to enter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignInCode {
    /// What the agent being signed in is called.
    pub agent: String,
    /// The page to enter the code on.
    pub url: String,
    /// The code to enter.
    pub code: String,
}

impl WorkspaceState {
    /// The state showing that the sign-in of the agent kept under `agent` waits for `code` to be entered on the page `url`.
    pub fn with_sign_in_code(mut self, agent: &str, url: String, code: String) -> Self {
        let name = self
            .settings_servers
            .agents
            .iter()
            .chain(&self.settings_servers.retired)
            .find(|entry| entry.id == agent)
            .map_or_else(|| agent.to_owned(), |entry| entry.name.clone());
        self.sign_in_code = Some(SignInCode {
            agent: name,
            url,
            code,
        });
        self
    }

    /// Takes the code away, since the sign-in it belonged to is over.
    pub fn sign_in_code_ended(mut self) -> Self {
        self.sign_in_code = None;
        self
    }

    /// Carries out `action` if it presses a part of the code dialog, and says whether it did.
    pub(crate) fn press_sign_in_code(&mut self, action: FocusId) -> bool {
        let Some(shown) = self.sign_in_code.as_ref() else {
            return false;
        };
        match action {
            SIGN_IN_CODE_COPY => self.outbox.push(Outgoing::Copy {
                text: shown.code.clone(),
            }),
            SIGN_IN_CODE_OPEN => self.outbox.push(Outgoing::Open {
                address: shown.url.clone(),
            }),
            SIGN_IN_CODE_HIDE | SIGN_IN_CODE_OUTSIDE => self.sign_in_code = None,
            SIGN_IN_CODE_INSIDE => {}
            _ => return false,
        }
        true
    }
}

/// Emits a button of the dialog saying `label` in the look of `variant`, answering to `id`.
#[track_caller]
fn button(label: &str, id: FocusId, variant: Variant) {
    Button::new(label, id)
        .variant(variant)
        .size(Size::Sm)
        .build();
}

/// Emits the dialog showing the code a sign-in waits for, while there is one; nothing otherwise.
#[track_caller]
pub fn code_dialog(state: &WorkspaceState) {
    let Some(shown) = state.sign_in_code.as_ref() else {
        return;
    };
    let title = format!("Sign in to {}", shown.agent);
    let words = format!(
        "Enter this code on {} to finish signing in. This closes by itself once you have.",
        shown.url
    );
    let code = shown.code.clone();
    Modal::new(SIGN_IN_CODE_OUTSIDE, SIGN_IN_CODE_INSIDE)
        .width(WIDTH)
        .build_with(move || {
            column_arranged(
                Modifier::new().padding(theme::sp(5)),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Start,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        title,
                        theme::text_style(15, 1.4)
                            .font_weight(rustle::FontWeight::MEDIUM)
                            .color(theme::color(Role::Text))
                            .max_lines(1),
                        rustle::TextOverflow::Ellipsis,
                    );
                    for words in wrap(&words, 13, WIDTH - 40.0) {
                        line(words, 13, 1.5, Role::Muted);
                    }
                    row_arranged(
                        Modifier::new(),
                        Arrangement::Start,
                        Alignment::Center,
                        || {
                            rustle::basic_text(
                                Modifier::new(),
                                code,
                                theme::text_style(24, 1.6)
                                    .font_weight(rustle::FontWeight::MEDIUM)
                                    .color(theme::color(Role::Text))
                                    .max_lines(1),
                                rustle::TextOverflow::Ellipsis,
                            );
                        },
                    );
                },
            );
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .padding_xy(theme::sp(5), 0.0)
                    .padding_each(0.0, 0.0, 0.0, theme::sp(5)),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    button("Hide", SIGN_IN_CODE_HIDE, Variant::Ghost);
                    spacer();
                    button("Copy code", SIGN_IN_CODE_COPY, Variant::Ghost);
                    button("Open page", SIGN_IN_CODE_OPEN, Variant::Default);
                },
            );
        });
}

/// The ids of the dialog buttons `state` lays out in a window 800 by 600 pixels: none when it shows no dialog.
#[cfg(test)]
fn laid_out_buttons(state: &WorkspaceState) -> Vec<u64> {
    let state = state.clone();
    let laid = crate::tagged::lay_out(800.0, 600.0, None, move || code_dialog(&state));
    laid.ids()
        .iter()
        .map(|id| id.0)
        .filter(|id| {
            [
                SIGN_IN_CODE_COPY.0,
                SIGN_IN_CODE_OPEN.0,
                SIGN_IN_CODE_HIDE.0,
            ]
            .contains(id)
        })
        .collect()
}

#[cfg(test)]
/// Tests for the code dialog.
mod tests {
    use super::*;

    /// A shown code names its agent, its buttons copy the code and open the page, and hiding or the end of the sign-in takes it away.
    #[test]
    fn the_code_is_shown_until_hidden_or_ended() {
        let state = WorkspaceState::new(&[]).with_sign_in_code(
            "copilot",
            "https://github.com/login/device".to_owned(),
            "ABCD-1234".to_owned(),
        );
        assert!(!laid_out_buttons(&state).is_empty());
        let mut state = state;
        assert!(state.press_sign_in_code(SIGN_IN_CODE_COPY));
        assert!(state.press_sign_in_code(SIGN_IN_CODE_OPEN));
        let sent = state.take_outbox();
        assert!(
            sent.iter()
                .any(|outgoing| matches!(outgoing, Outgoing::Copy { text } if text == "ABCD-1234"))
        );
        assert!(sent.iter().any(
            |outgoing| matches!(outgoing, Outgoing::Open { address } if address.contains("login/device"))
        ));
        assert!(!state.press_sign_in_code(FocusId(1)));
        assert!(state.press_sign_in_code(SIGN_IN_CODE_HIDE));
        assert!(laid_out_buttons(&state).is_empty());
        let ended = WorkspaceState::new(&[])
            .with_sign_in_code("copilot", "u".to_owned(), "c".to_owned())
            .sign_in_code_ended();
        assert!(ended.sign_in_code.is_none());
    }
}
