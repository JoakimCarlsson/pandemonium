//! The prompts above the prompt box that wait for the agent's turn to end, each of which can be read in full, edited in the prompt box, sent now or removed.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_1, flex_col, flex_none, gap, items_center,
    items_start, mb, min_w_px, pl, pr, pt, py, rounded, w_full,
};
use pm_text::{Field, Shaper};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::Variant;
use crate::parts::{glyph, line, paragraph};
use crate::sample::{Queued, Session};
use crate::size::Size;
use crate::state::{HURRY_BASE, Outgoing, Page, REMOVE_QUEUED_BASE, WorkspaceState, prompt_field};

/// Gives up the edit of a waiting prompt and puts back what was in the prompt box before it.
pub const QUEUED_CANCEL: FocusId = FocusId(7_402_302);

/// The first of the buttons opening a waiting prompt for editing; the rest follow it.
pub const EDIT_QUEUED_BASE: u64 = 4_901_000_000_000;

/// How wide the three buttons beside the text of a waiting prompt are together, in logical pixels, which the text leaves free.
const BESIDE_TEXT: f32 = 240.0;

/// How wide the count of files beside the text of a waiting prompt is, in logical pixels.
const FILE_COUNT: f32 = 60.0;

/// How wide the button beside a waiting prompt being edited is, in logical pixels.
const BESIDE_EDIT: f32 = 100.0;

/// How much of a row's width its border, padding, glyph and gaps take, in logical pixels.
const CHROME: f32 = 50.0;

/// A waiting prompt whose text is being edited in the prompt box.
#[derive(Clone, Debug, PartialEq)]
pub struct EditingQueued {
    /// The session it waits in, by the scope that names it however the sessions are renumbered.
    pub scope: u64,
    /// The prompt, by the id the server gave it.
    pub id: u64,
    /// What the prompt box held before the edit began, which goes back when it ends.
    pub draft: Field,
}

impl WorkspaceState {
    /// Puts the text of the waiting prompt at `index` of the open session in the prompt box to be edited, keeping what the box held until the edit ends, and holds the session's queue so that no prompt behind it is handed to the agent meanwhile.
    pub(crate) fn open_queued_edit(&mut self, index: usize) {
        if self.editing_queued.is_some() {
            return;
        }
        let Page::Session(position) = self.page else {
            return;
        };
        let Some(session) = self.sessions.get(position) else {
            return;
        };
        let Some(queued) = session.queued.get(index) else {
            return;
        };
        let (scope, id, text) = (session.scope, queued.id, prompt_field(&queued.text));
        let draft = std::mem::replace(&mut self.prompt, text);
        self.editing_queued = Some(EditingQueued { scope, id, draft });
        self.outbox.push(Outgoing::HoldQueue {
            session: position,
            held: true,
        });
    }

    /// Ends the edit of the waiting prompt, puts back what the prompt box held before it and lets the session's queue go on.
    pub(crate) fn cancel_queued_edit(&mut self) {
        let Some(scope) = self.editing_queued.as_ref().map(|editing| editing.scope) else {
            return;
        };
        self.end_queued_edit();
        if let Some(session) = self.sessions.iter().position(|held| held.scope == scope) {
            self.outbox.push(Outgoing::HoldQueue {
                session,
                held: false,
            });
        }
    }

    /// Ends the edit of the waiting prompt and puts back what the prompt box held before it, asking nothing of the server.
    fn end_queued_edit(&mut self) {
        if let Some(editing) = self.editing_queued.take() {
            self.prompt = editing.draft;
        }
    }

    /// Hands the server the text in the prompt box as the text of the waiting prompt, which lets its queue go on, and ends the edit. Leaves the edit open when the text is empty and no file goes with the prompt. An edit that changed nothing only lets the queue go on.
    pub(crate) fn save_queued_edit(&mut self) {
        let Some(editing) = self.editing_queued.as_ref() else {
            return;
        };
        let found = self
            .sessions
            .iter()
            .position(|held| held.scope == editing.scope)
            .and_then(|session| {
                let queued = self.sessions[session]
                    .queued
                    .iter()
                    .find(|queued| queued.id == editing.id)?;
                Some((session, queued.clone()))
            });
        let Some((session, queued)) = found else {
            self.cancel_queued_edit();
            return;
        };
        let text = self.prompt.text().trim().to_owned();
        if text.is_empty() && queued.files.is_empty() {
            return;
        }
        self.end_queued_edit();
        self.outbox.push(if text != queued.text {
            Outgoing::EditQueued {
                session,
                queued: queued.id,
                text,
            }
        } else {
            Outgoing::HoldQueue {
                session,
                held: false,
            }
        });
    }
}

/// A button showing only `icon`, saying what it does with `tooltip`, answering to `id`.
fn icon_button(icon: IconName, tooltip: &str, id: FocusId) -> Node {
    Button::icon(icon, id)
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .build()
        .tooltip(tooltip)
}

/// The box a row of a waiting prompt is drawn in, its contents aligned to the top.
fn frame() -> Node {
    node().styles([
        flex(),
        items_start(),
        gap(2),
        w_full(),
        pl(3),
        pr(1),
        py(1),
        rounded(Radius::Lg),
        border(),
        border_color(Role::Border),
        bg(Role::Surface),
    ])
}

/// One waiting prompt, at `index`, in a row `width` logical pixels wide: all its text wrapped, how many files go with it and buttons to edit it, send it now or remove it.
fn waiting(index: usize, queued: &Queued, width: f32, shaper: &mut Shaper) -> Node {
    let counted = if queued.files.is_empty() {
        0.0
    } else {
        FILE_COUNT
    };
    let room = (width - CHROME - BESIDE_TEXT - counted).max(120.0);
    let mut buttons = node()
        .styles([flex(), items_center(), gap(1), flex_none()])
        .child(icon_button(
            IconName::Pencil,
            "Edit",
            FocusId(EDIT_QUEUED_BASE + index as u64),
        ));
    buttons = buttons
        .child(
            Button::new("Send now", FocusId(HURRY_BASE + index as u64))
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .with_icon(IconName::Zap)
                .build(),
        )
        .child(
            Button::new("Remove", FocusId(REMOVE_QUEUED_BASE + index as u64))
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .with_icon(IconName::Trash)
                .build(),
        );
    let mut row = frame()
        .child(
            node()
                .styles([pt(1)])
                .child(glyph(IconName::Progress, 3.5, Role::Muted)),
        )
        .child(
            node()
                .styles([flex_1(), min_w_px(0), pt(1)])
                .child(paragraph(&queued.text, 13, 1.5, Role::Text, room, shaper)),
        );
    if !queued.files.is_empty() {
        let count = queued.files.len();
        let files = if count == 1 { "file" } else { "files" };
        row = row.child(node().styles([flex_none(), pt(1.5)]).child(line(
            format!("{count} {files}"),
            12,
            1.4,
            Role::Muted,
        )));
    }
    row.child(buttons)
}

/// The waiting prompt being edited, in a row `width` logical pixels wide: the text it has until the edit is saved, and a button to give the edit up.
fn editing(queued: &Queued, width: f32, shaper: &mut Shaper) -> Node {
    let room = (width - CHROME - BESIDE_EDIT).max(120.0);
    frame()
        .child(
            node()
                .styles([pt(1)])
                .child(glyph(IconName::Pencil, 3.5, Role::Accent)),
        )
        .child(
            node()
                .styles([flex_1(), min_w_px(0), pt(1)])
                .child(paragraph(&queued.text, 13, 1.5, Role::Muted, room, shaper)),
        )
        .child(
            Button::new("Cancel", QUEUED_CANCEL)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .with_icon(IconName::Close)
                .build(),
        )
}

/// The prompts waiting for `session`'s agent, oldest first, in a column `width` logical pixels wide, or nothing when none waits.
pub fn queued(
    state: &WorkspaceState,
    session: &Session,
    width: f32,
    shaper: &mut Shaper,
) -> Option<Node> {
    if session.queued.is_empty() {
        return None;
    }
    let editing_id = state
        .editing_queued
        .as_ref()
        .filter(|open| open.scope == session.scope)
        .map(|open| open.id);
    let mut rows = node().styles([flex(), flex_col(), gap(1), mb(2)]);
    for (index, waiting_prompt) in session.queued.iter().enumerate() {
        rows = rows.child(if editing_id == Some(waiting_prompt.id) {
            editing(waiting_prompt, width, shaper)
        } else {
            waiting(index, waiting_prompt, width, shaper)
        });
    }
    Some(rows)
}

#[cfg(test)]
mod tests {
    use pm_acp::Event;
    use pm_text::Edit;

    use super::*;
    use crate::state::{PROMPT, WorkspaceState};

    /// What `WorkspaceState` is given for the tests: the same state with nothing waiting in its outbox.
    trait Drained {
        /// The state with its outbox emptied.
        fn drained(self) -> Self;
    }

    impl Drained for WorkspaceState {
        /// The state with its outbox emptied.
        fn drained(mut self) -> Self {
            self.take_outbox();
            self
        }
    }

    /// A state whose open session has two prompts waiting and a draft in the prompt box.
    fn waiting_state() -> WorkspaceState {
        let state = WorkspaceState::sample();
        let session = state.sessions.len() - 1;
        let state = state.after(FocusId(crate::state::SESSION_BASE + session as u64));
        [(1, "first"), (2, "second")]
            .into_iter()
            .fold(state, |state, (id, text)| {
                state.heard(
                    session,
                    Event::Queued {
                        id,
                        text: text.to_owned(),
                        files: Vec::new(),
                    },
                )
            })
            .edited(PROMPT, Edit::Paste("my draft".to_owned()))
            .drained()
    }

    /// Editing holds the queue and loads the waiting text into the prompt box, sending asks the server to replace the text and lets the queue go on, and the draft comes back.
    #[test]
    fn sending_while_editing_saves_the_edit_and_restores_the_draft() {
        let mut state = waiting_state().after(FocusId(EDIT_QUEUED_BASE));
        assert_eq!(state.prompt.text(), "first");
        assert!(matches!(
            state.take_outbox().as_slice(),
            [Outgoing::HoldQueue { held: true, .. }]
        ));
        state = state
            .edited(PROMPT, Edit::Paste(", edited".to_owned()))
            .after(crate::state::SEND);
        assert_eq!(state.prompt.text(), "my draft");
        assert!(state.editing_queued.is_none());
        assert!(matches!(
            state.take_outbox().as_slice(),
            [Outgoing::EditQueued { queued: 1, text, .. }] if text == "first, edited"
        ));
    }

    /// Cancelling restores the draft and lets the queue go on without changing any prompt.
    #[test]
    fn cancelling_an_edit_restores_the_draft_and_releases_the_queue() {
        let mut state = waiting_state()
            .after(FocusId(EDIT_QUEUED_BASE))
            .edited(PROMPT, Edit::Paste("x".to_owned()))
            .after(QUEUED_CANCEL);
        assert_eq!(state.prompt.text(), "my draft");
        assert!(matches!(
            state.take_outbox().last(),
            Some(Outgoing::HoldQueue { held: false, .. })
        ));
    }
}
