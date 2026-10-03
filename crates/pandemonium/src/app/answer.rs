//! Answering the questions an agent puts to the reader.
//!
//! A form is drawn in the agent's pane a page at a time, its choices laid out
//! as rows to press and its lines of text and numbers as boxes written in
//! where they stand.

use pm_acp::Reply;
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

use crate::agent::{Form, Pending, TalkId, pending_messages};
use crate::app::{App, Writing};
use crate::panes::Item;

impl App {
    /// The form `session`'s agent put under `ticket`, while it is still waiting.
    pub(super) fn answer_form(&mut self, session: TalkId, ticket: u64) -> Option<&mut Form> {
        self.agents
            .get_mut(session)
            .and_then(|talk| talk.form_mut(ticket))
    }

    /// Chooses the `option`-th alternative of the `place`-th field of the form under `ticket`.
    pub(super) fn choose_answer(
        &mut self,
        session: TalkId,
        ticket: u64,
        place: usize,
        option: usize,
    ) {
        if let Some(form) = self.answer_form(session, ticket) {
            form.choose(place, option);
        }
    }

    /// Gives the keyboard to the box of the `place`-th field of the form
    /// under `ticket`, picking it where it is the reader's own words.
    pub(super) fn type_answer(&mut self, session: TalkId, ticket: u64, place: usize) {
        let Some(form) = self.answer_form(session, ticket) else {
            return;
        };
        form.pick_other(place);
        self.write_in(Writing::Answer(session, ticket, place));
    }

    /// Sends a keypress to the card the agent in front is waiting on the
    /// reader with, answering whether it took it.
    ///
    /// Escape refuses a permission and walks away from a question wherever
    /// the keyboard is in the pane. The arrows, Tab, Enter and the digits
    /// work the card's rows only while the prompt is empty and no box of the
    /// card is being written in, so a prompt being written keeps its keys.
    pub(super) fn send_to_pending(&mut self, event: &KeyEvent) -> bool {
        let session = match self.writing {
            Some(Writing::Prompt(session) | Writing::Answer(session, ..)) => Some(session),
            _ if self.editor_focused => self.active_tab().and_then(Item::session),
            _ => None,
        };
        let Some(session) = session else {
            return false;
        };
        let Some(talk) = self.agents.get_mut(session) else {
            return false;
        };
        let Some(pending) = talk.pending() else {
            return false;
        };
        let key = event.logical_key.as_ref();
        if key == Key::Named(NamedKey::Escape) {
            match pending {
                Pending::Ask(ask) => talk.deny(ask),
                Pending::Form(ticket) => self.dismiss_answer(session, ticket, Reply::Cancel),
                Pending::Login => return false,
            }
            return true;
        }
        let writing = matches!(self.writing, Some(Writing::Answer(..)));
        if writing || !talk.prompt().is_empty() {
            return false;
        }
        let row = match key {
            Key::Named(NamedKey::ArrowUp) => {
                talk.step_pending(-1);
                return true;
            }
            Key::Named(NamedKey::ArrowDown) => {
                talk.step_pending(1);
                return true;
            }
            Key::Named(NamedKey::Tab) => {
                let Pending::Form(ticket) = pending else {
                    return false;
                };
                let by = if self.modifiers.shift_key() { -1 } else { 1 };
                if let Some(form) = talk.form_mut(ticket) {
                    form.turn(by);
                }
                return true;
            }
            Key::Named(NamedKey::Enter) => talk.pending_cursor(),
            Key::Character(typed) => match typed.parse::<usize>() {
                Ok(digit @ 1..=9) => digit - 1,
                _ => return false,
            },
            _ => return false,
        };
        let Some(message) = pending_messages(talk).into_iter().nth(row) else {
            return false;
        };
        if let Pending::Form(ticket) = pending
            && let Some(form) = talk.form_mut(ticket)
        {
            form.point_at(row);
        }
        self.apply(message);
        true
    }

    /// Sends the form under `ticket` as it is filled in, unless it is not yet fit to send.
    pub(super) fn send_answer(&mut self, session: TalkId, ticket: u64) {
        let Some(form) = self.answer_form(session, ticket) else {
            return;
        };
        if let Some(problem) = form.settle().err().or_else(|| form.problem()) {
            self.notices.trouble(problem, None);
            return;
        }
        let reply = form.submission();
        self.dismiss_answer(session, ticket, reply);
    }

    /// Answers the question under `ticket` with `reply`, handing the
    /// keyboard back to the prompt if one of its boxes had it.
    pub(super) fn dismiss_answer(&mut self, session: TalkId, ticket: u64, reply: Reply) {
        if let Some(talk) = self.agents.get_mut(session) {
            talk.reply(ticket, &reply);
        }
        if matches!(self.writing, Some(Writing::Answer(answered, asked, _)) if answered == session && asked == ticket)
        {
            self.write_in(Writing::Prompt(session));
        }
    }

    /// Opens the page the agent sent the reader to, and answers that it has been.
    pub(super) fn open_answer_link(&mut self, session: TalkId, ticket: u64) {
        let Some(url) = self
            .answer_form(session, ticket)
            .and_then(|form| form.link().map(|link| link.url.clone()))
        else {
            return;
        };
        crate::desktop::browse(&url);
        self.dismiss_answer(session, ticket, Reply::Accept(Vec::new()));
    }
}
