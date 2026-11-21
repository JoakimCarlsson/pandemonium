//! The prompts above the prompt box that wait for the agent's turn to end, each of which can be edited in the prompt box, sent now or removed.
//!
//! Each row shows the first few lines of its prompt, with the elements and captures it carries as chips, and only the first few rows are shown, so that a long prompt or many of them never push the prompt box out of the window; the whole of a prompt is read by editing it.

use crate::areas::ChatEvent;
use crate::button;
use crate::target::Target;
use crate::theme;
use crate::theme::Frame;
use rustle::Modifier;
use rustle::TextFieldState;
use rustle::{Alignment, Arrangement, TextOverflow};
use rustle::{Color, Icon, phosphor};

use crate::theme::{Radius, Role};

use crate::Button;
use crate::button::Variant;
use crate::parts::{glyph, line};
use crate::sample::{Queued, Session};
use crate::state::{Outgoing, Page, WorkspaceState, prompt_field};

/// How wide the three buttons beside the text of a waiting prompt are together, in logical pixels, which the text leaves free.
const BESIDE_TEXT: f32 = 240.0;

/// How wide the count of files beside the text of a waiting prompt is, in logical pixels.
const FILE_COUNT: f32 = 60.0;

/// How wide the button beside a waiting prompt being edited is, in logical pixels.
const BESIDE_EDIT: f32 = 100.0;

/// How much of a row's width its border, padding, glyph and gaps take, in logical pixels.
const CHROME: f32 = 50.0;

/// The most wrapped lines of a waiting prompt's text its row shows; the rest is read by editing it.
const SHOWN_LINES: usize = 3;

/// The most waiting prompts shown as rows; the others are counted in a line below them.
const SHOWN_ROWS: usize = 3;

/// How tall the type of a waiting prompt's text is, in logical pixels.
const TEXT_PIXELS: u32 = 13;

/// The line height of a waiting prompt's text, as a multiple of its type's height.
const TEXT_LEADING: f32 = 1.5;

/// A waiting prompt whose text is being edited in the prompt box.
#[derive(Clone, Debug, PartialEq)]
pub struct EditingQueued {
    /// The session it waits in, by the scope that names it however the sessions are renumbered.
    pub scope: u64,
    /// The prompt, by the id the server gave it.
    pub id: u64,
    /// What the prompt box held before the edit began, which goes back when it ends.
    pub draft: TextFieldState,
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
        let (scope, id) = (session.scope, queued.id);
        let text = prompt_field(crate::elements::split(&queued.text).0);
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

    /// Gives up an edit of a waiting prompt left open before a prompt of the session at `session` is sent now. An edit in that session only ends here, since the server lets go of the queue's hold for the hurry itself and letting go first would hand it the oldest prompt ahead of the hurried one; an edit in another session lets that session's queue go on.
    pub(crate) fn give_up_edit_for_hurry(&mut self, session: usize) {
        let Some(scope) = self.editing_queued.as_ref().map(|editing| editing.scope) else {
            return;
        };
        if self.sessions.get(session).map(|held| held.scope) == Some(scope) {
            self.end_queued_edit();
        } else {
            self.cancel_queued_edit();
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
        let text = with_selection(self.prompt.text().trim(), &queued.text);
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

/// `typed`, the text of a waiting prompt as edited in the prompt box, followed by the section of picked elements and held captures that `original`, its text before the edit, ends with, which the prompt box never showed, so that an edit keeps them.
fn with_selection(typed: &str, original: &str) -> String {
    let kept = crate::elements::split(original).0;
    let section = original[kept.len()..].trim_start();
    match (typed.is_empty(), section.is_empty()) {
        (_, true) => typed.to_owned(),
        (true, false) => section.to_owned(),
        (false, false) => format!("{typed}\n\n{section}"),
    }
}

/// Emits the first `SHOWN_LINES` lines of `content` wrapped to `width` logical pixels, in `tone`, the last of them ended with an ellipsis when more of it is left out.
#[track_caller]
fn clamped(content: &str, tone: Role, width: f32) {
    let tone: Color = tone.into();
    let kind = theme::text_style(TEXT_PIXELS, TEXT_LEADING);
    let lines = rustle::wrap_at_spaces(content, &kind, width.max(32.0));
    let cut = lines.len() > SHOWN_LINES;
    let shown = lines.len().min(SHOWN_LINES);
    rustle::column_arranged(
        Modifier::new(),
        Arrangement::Start,
        Alignment::Start,
        || {
            for (at, part) in lines.into_iter().take(shown).enumerate() {
                let words = content[part.start..part.end].trim_end();
                match (cut && at + 1 == shown, words.is_empty()) {
                    (true, _) => line(format!("{words}\u{2026}"), TEXT_PIXELS, TEXT_LEADING, tone),
                    (false, true) => rustle::spacer(Modifier::new().height(theme::sp(3))),
                    (false, false) => rustle::basic_text(
                        Modifier::new(),
                        words,
                        kind.color(theme::color(tone)),
                        TextOverflow::Ellipsis,
                    ),
                }
            }
        },
    );
}

/// Emits what a row shows of a waiting prompt's `content` in a column `width` logical pixels wide: the elements and captures it carries as chips, as a sent message shows them, then the first lines of what the reader typed, in `tone`.
#[track_caller]
fn body(content: &str, tone: Role, width: f32) {
    let (typed, labels) = crate::elements::split(content);
    rustle::column_arranged(
        Modifier::new()
            .weight(1.0)
            .padding_each(0.0, theme::sp(1), 0.0, 0.0),
        Arrangement::SpacedBy(theme::sp(1.5)),
        Alignment::Start,
        || {
            if !labels.is_empty() {
                crate::elements::sent_chips(&labels);
            }
            if !typed.is_empty() {
                clamped(typed, tone, width);
            }
        },
    );
}

/// Emits a button showing only `icon`, saying what it does with `tooltip`, answering to `id`.
#[track_caller]
pub(crate) fn icon_button(icon: Icon, tooltip: &str, id: Target) {
    Button::icon(icon, id).tooltip(tooltip).build();
}

/// Emits a small ghost button saying `label` with `icon`, answering to `id`: the actions of a waiting prompt's row.
#[track_caller]
fn ghost(label: &str, id: Target, icon: Icon) {
    button::small_button(label, id, Variant::Ghost)
        .with_icon(icon)
        .build();
}

/// Emits the box a row of a waiting prompt is drawn in, its contents aligned to the top, holding what `content` emits.
#[track_caller]
fn frame(content: impl FnOnce()) {
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .panel(Radius::Lg)
            .padding_each(theme::sp(3), theme::sp(1), theme::sp(1), theme::sp(1)),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Start,
        content,
    );
}

/// Emits `name` in `tone`, lowered by a step to sit on the first line of a row's text.
#[track_caller]
fn leading_glyph(name: Icon, tone: Role) {
    rustle::row_arranged(
        Modifier::new().padding_each(0.0, theme::sp(1), 0.0, 0.0),
        Arrangement::Start,
        Alignment::Start,
        || glyph(name, 3.5, tone),
    );
}

/// Emits one waiting prompt, at `index`, in a row `width` logical pixels wide: the first lines of its text and chips for what it carries, how many files go with it and buttons to edit it, send it now or remove it.
#[track_caller]
fn waiting(index: usize, queued: &Queued, width: f32) {
    let counted = if queued.files.is_empty() {
        0.0
    } else {
        FILE_COUNT
    };
    let room = (width - CHROME - BESIDE_TEXT - counted).max(120.0);
    frame(|| {
        leading_glyph(phosphor::TIMER, Role::Muted);
        body(&queued.text, Role::Text, room);
        if !queued.files.is_empty() {
            let count = queued.files.len();
            let files = if count == 1 { "file" } else { "files" };
            rustle::row_arranged(
                Modifier::new().padding_each(0.0, theme::sp(1.5), 0.0, 0.0),
                Arrangement::Start,
                Alignment::Start,
                || line(format!("{count} {files}"), 12, 1.4, Role::Muted),
            );
        }
        rustle::row_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(theme::sp(1)),
            Alignment::Center,
            || {
                icon_button(
                    phosphor::PENCIL_SIMPLE,
                    "Edit",
                    Target::Chat(ChatEvent::EditQueued(index)),
                );
                ghost(
                    "Send now",
                    Target::Chat(ChatEvent::Hurry(index)),
                    phosphor::LIGHTNING,
                );
                ghost(
                    "Remove",
                    Target::Chat(ChatEvent::RemoveQueued(index)),
                    phosphor::TRASH,
                );
            },
        );
    });
}

/// Emits the waiting prompt being edited, in a row `width` logical pixels wide: the first lines of the text it has until the edit is saved, which the prompt box holds whole, chips for what it carries, and a button to give the edit up.
#[track_caller]
fn editing(queued: &Queued, width: f32) {
    let room = (width - CHROME - BESIDE_EDIT).max(120.0);
    frame(|| {
        leading_glyph(phosphor::PENCIL_SIMPLE, Role::Accent);
        body(&queued.text, Role::Muted, room);
        ghost("Cancel", Target::Chat(ChatEvent::QueuedCancel), phosphor::X);
    });
}

/// Emits the prompts waiting for `session`'s agent, oldest first, in a column `width` logical pixels wide, or nothing when none waits. Only the first `SHOWN_ROWS` are shown, and the one being edited wherever it is; a line counts the others.
#[track_caller]
pub fn queued(state: &WorkspaceState, session: &Session, width: f32) {
    if session.queued.is_empty() {
        return;
    }
    let editing_id = state
        .editing_queued
        .as_ref()
        .filter(|open| open.scope == session.scope)
        .map(|open| open.id);
    rustle::column_arranged(
        Modifier::new().padding_each(0.0, 0.0, 0.0, theme::sp(2)),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Start,
        || {
            let mut hidden = 0;
            for (index, waiting_prompt) in session.queued.iter().enumerate() {
                let edited = editing_id == Some(waiting_prompt.id);
                if index >= SHOWN_ROWS && !edited {
                    hidden += 1;
                    continue;
                }
                rustle::key(waiting_prompt.id, || {
                    if edited {
                        editing(waiting_prompt, width);
                    } else {
                        waiting(index, waiting_prompt, width);
                    }
                });
            }
            if hidden > 0 {
                let prompts = if hidden == 1 { "prompt" } else { "prompts" };
                let said = format!("+{hidden} more {prompts} waiting");
                rustle::row_arranged(
                    Modifier::new().padding_each(theme::sp(3), 0.0, 0.0, 0.0),
                    Arrangement::Start,
                    Alignment::Start,
                    || line(said, 12, 1.4, Role::Muted),
                );
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use pm_harness::Event;

    use super::*;
    use crate::areas::ChatEvent;
    use crate::areas::ComposerEvent;
    use crate::areas::SidebarEvent;
    use crate::state::WorkspaceState;

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
        let state = state.handle(crate::events::Event::Sidebar(SidebarEvent::Session(
            session,
        )));
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
            .typed(Target::Composer(ComposerEvent::Prompt), "my draft")
            .drained()
    }

    /// Editing holds the queue and loads the waiting text into the prompt box, sending asks the server to replace the text and lets the queue go on, and the draft comes back.
    #[test]
    fn sending_while_editing_saves_the_edit_and_restores_the_draft() {
        let mut state =
            waiting_state().handle(crate::events::Event::Chat(ChatEvent::EditQueued(0)));
        assert_eq!(state.prompt.text(), "first");
        assert!(matches!(
            state.take_outbox().as_slice(),
            [Outgoing::HoldQueue { held: true, .. }]
        ));
        state = state
            .typed(Target::Composer(ComposerEvent::Prompt), ", edited")
            .handle(crate::events::Event::Composer(ComposerEvent::Send));
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
            .handle(crate::events::Event::Chat(ChatEvent::EditQueued(0)))
            .typed(Target::Composer(ComposerEvent::Prompt), "x")
            .handle(crate::events::Event::Chat(ChatEvent::QueuedCancel));
        assert_eq!(state.prompt.text(), "my draft");
        assert!(matches!(
            state.take_outbox().last(),
            Some(Outgoing::HoldQueue { held: false, .. })
        ));
    }

    /// A state whose open session has one prompt waiting that carries a picked element after what the reader typed.
    fn selection_state() -> (WorkspaceState, String) {
        let state = WorkspaceState::sample();
        let session = state.sessions.len() - 1;
        let state = state.handle(crate::events::Event::Sidebar(SidebarEvent::Session(
            session,
        )));
        let section = crate::elements::message(&[crate::elements::Element {
            tag: "button".to_owned(),
            text: "Save".to_owned(),
            ..Default::default()
        }]);
        let text = format!("fix this\n\n{section}");
        let state = state
            .heard(
                session,
                Event::Queued {
                    id: 1,
                    text: text.clone(),
                    files: Vec::new(),
                },
            )
            .drained();
        (state, section)
    }

    /// Editing a prompt that carries a picked element puts only what the reader typed in the prompt box, and saving it hands the server the edited text with the element's section after it, as before.
    #[test]
    fn editing_keeps_the_selected_elements() {
        let (state, section) = selection_state();
        let mut state = state.handle(crate::events::Event::Chat(ChatEvent::EditQueued(0)));
        assert_eq!(state.prompt.text(), "fix this");
        state.take_outbox();
        state = state
            .typed(Target::Composer(ComposerEvent::Prompt), ", please")
            .handle(crate::events::Event::Composer(ComposerEvent::Send));
        let expected = format!("fix this, please\n\n{section}");
        assert!(matches!(
            state.take_outbox().as_slice(),
            [Outgoing::EditQueued { queued: 1, text, .. }] if *text == expected
        ));
    }

    /// Saving an edit of a prompt that carries a picked element without changing what was typed only lets the queue go on.
    #[test]
    fn an_unchanged_edit_with_elements_changes_nothing() {
        let (state, _) = selection_state();
        let mut state = state
            .handle(crate::events::Event::Chat(ChatEvent::EditQueued(0)))
            .drained();
        state = state.handle(crate::events::Event::Composer(ComposerEvent::Send));
        assert!(matches!(
            state.take_outbox().as_slice(),
            [Outgoing::HoldQueue { held: false, .. }]
        ));
    }

    /// Send now is heard however many branches the prompt box has loaded, whose rows are targets of their own.
    #[test]
    fn sending_now_with_many_branches_loaded_hurries_the_prompt() {
        let mut state = waiting_state();
        state.branches = (0..500).map(|n| format!("branch-{n}")).collect();
        let mut state = state.handle(crate::events::Event::Chat(ChatEvent::Hurry(1)));
        assert!(matches!(
            state.take_outbox().as_slice(),
            [Outgoing::Hurry { queued: 2, .. }]
        ));
    }

    /// Sending another prompt now while one is being edited ends the edit, puts the draft back and hurries the prompt without first letting the queue go on.
    #[test]
    fn sending_now_while_editing_ends_the_edit() {
        let mut state = waiting_state()
            .handle(crate::events::Event::Chat(ChatEvent::EditQueued(0)))
            .drained()
            .handle(crate::events::Event::Chat(ChatEvent::Hurry(1)));
        assert!(state.editing_queued.is_none());
        assert_eq!(state.prompt.text(), "my draft");
        assert!(matches!(
            state.take_outbox().as_slice(),
            [Outgoing::Hurry { queued: 2, .. }]
        ));
    }
}
