//! The code a sign-in waits for the reader to enter on a web page: the dialog that shows it while the sign-in runs, and what its buttons do.

use crate::areas::DialogEvent;
use crate::button;
use crate::target::Target;
use crate::theme;
use crate::theme::Role;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, TextOverflow};

use crate::Modal;
use crate::button::Variant;
use crate::parts::{line, spacer, wrap};
use crate::state::{Outgoing, WorkspaceState};

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
            .all_agents()
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
    pub(crate) fn press_sign_in_code(&mut self, action: Target) -> bool {
        let Some(shown) = self.sign_in_code.as_ref() else {
            return false;
        };
        match action {
            Target::Dialog(DialogEvent::SignInCodeCopy) => self.outbox.push(Outgoing::Copy {
                text: shown.code.clone(),
            }),
            Target::Dialog(DialogEvent::SignInCodeOpen) => self.outbox.push(Outgoing::Open {
                address: shown.url.clone(),
            }),
            Target::Dialog(DialogEvent::SignInCodeHide | DialogEvent::SignInCodeOutside) => {
                self.sign_in_code = None
            }
            Target::Dialog(DialogEvent::SignInCodeInside) => {}
            _ => return false,
        }
        true
    }
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
    Modal::new(
        Target::Dialog(DialogEvent::SignInCodeOutside),
        Target::Dialog(DialogEvent::SignInCodeInside),
    )
    .width(WIDTH)
    .build_with(move || {
        rustle::column_arranged(
            Modifier::new().padding(theme::sp(5)),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Start,
            || {
                rustle::basic_text(
                    Modifier::new(),
                    title,
                    theme::strong(15, Role::Text),
                    TextOverflow::Ellipsis,
                );
                for words in wrap(&words, 13, WIDTH - 40.0) {
                    line(words, 13, 1.5, Role::Muted);
                }
                crate::parts::title(code, 24, 1.6);
            },
        );
        rustle::row_arranged(
            Modifier::new().fill_max_width().padding_each(
                theme::sp(5),
                0.0,
                theme::sp(5),
                theme::sp(5),
            ),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            || {
                button::action(
                    "Hide",
                    Target::Dialog(DialogEvent::SignInCodeHide),
                    Variant::Ghost,
                );
                spacer();
                let copy = Target::Dialog(DialogEvent::SignInCodeCopy);
                button::action("Copy code", copy, Variant::Ghost);
                let open = Target::Dialog(DialogEvent::SignInCodeOpen);
                button::action("Open page", open, Variant::Default);
            },
        );
    });
}

/// The targets of the dialog buttons `state` lays out in a window 800 by 600 pixels: none when it shows no dialog.
#[cfg(test)]
fn laid_out_buttons(state: &WorkspaceState) -> Vec<Target> {
    let state = state.clone();
    let laid = crate::tagged::lay_out(800.0, 600.0, None, move || code_dialog(&state));
    laid.ids()
        .into_iter()
        .filter(|id| {
            [
                Target::Dialog(DialogEvent::SignInCodeCopy),
                Target::Dialog(DialogEvent::SignInCodeOpen),
                Target::Dialog(DialogEvent::SignInCodeHide),
            ]
            .contains(id)
        })
        .collect()
}

#[cfg(test)]
/// Tests for the code dialog.
mod tests {
    use super::*;
    use crate::areas::ComposerEvent;

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
        assert!(state.press_sign_in_code(Target::Dialog(DialogEvent::SignInCodeCopy)));
        assert!(state.press_sign_in_code(Target::Dialog(DialogEvent::SignInCodeOpen)));
        let sent = state.take_outbox();
        assert!(
            sent.iter()
                .any(|outgoing| matches!(outgoing, Outgoing::Copy { text } if text == "ABCD-1234"))
        );
        assert!(sent.iter().any(
            |outgoing| matches!(outgoing, Outgoing::Open { address } if address.contains("login/device"))
        ));
        assert!(!state.press_sign_in_code(Target::Composer(ComposerEvent::Prompt)));
        assert!(state.press_sign_in_code(Target::Dialog(DialogEvent::SignInCodeHide)));
        assert!(laid_out_buttons(&state).is_empty());
        let ended = WorkspaceState::new(&[])
            .with_sign_in_code("copilot", "u".to_owned(), "c".to_owned())
            .sign_in_code_ended();
        assert!(ended.sign_in_code.is_none());
    }
}
