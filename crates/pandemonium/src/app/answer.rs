//! Answering the questions an agent puts to the reader.
//!
//! A form is drawn in the agent's pane a page at a time, its choices laid out
//! as rows to press and its lines of text and numbers as boxes written in
//! where they stand.

use pm_acp::Reply;

use crate::agent::{Form, TalkId};
use crate::app::{App, Writing};

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

    /// Picks the row of the reader's own words at `place` of the form under
    /// `ticket`, and gives its box the keyboard.
    pub(super) fn pick_answer_other(&mut self, session: TalkId, ticket: u64, place: usize) {
        let Some(form) = self.answer_form(session, ticket) else {
            return;
        };
        form.pick_other(place);
        self.write_in(Writing::Answer(session, ticket, place));
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
