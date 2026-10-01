//! Answering the questions an agent puts to the reader.
//!
//! A form is drawn in the agent's pane with a row per field; pressing a row
//! edits that field here. A yes-or-no flips on the spot, a line of text or a
//! number is typed into the prompt, and a choice is made from a list that
//! stays open for a field that takes several.

use pm_acp::{Input, Reply};

use crate::agent::TalkId;
use crate::app::App;
use crate::picker::{Choice, Kind, Row};

/// Which field of which form the prompt or the list on screen is editing.
#[derive(Clone, Copy, Debug)]
pub(super) struct Editing {
    /// The agent session the form belongs to.
    session: TalkId,
    /// The ticket the form was put under.
    ticket: u64,
    /// The field, counted from the top of the form.
    place: usize,
}

impl App {
    /// Starts editing the `place`-th field of the form under `ticket`.
    pub(super) fn edit_answer(&mut self, session: TalkId, ticket: u64, place: usize) {
        let Some(form) = self
            .agents
            .get_mut(session)
            .and_then(|talk| talk.form_mut(ticket))
        else {
            return;
        };
        let Some(input) = form.fields().get(place).map(|field| field.input.clone()) else {
            return;
        };
        let seeded = match form.given(place) {
            Some(_) => form.shown(place),
            None => String::new(),
        };
        self.answering = Some(Editing {
            session,
            ticket,
            place,
        });
        match input {
            Input::Toggle { .. } => form.flip(place),
            Input::Text { .. } | Input::Number { .. } => {
                self.open_picker_with(Kind::AnswerText, Vec::new(), seeded);
            }
            Input::One { .. } | Input::Many { .. } => self.open_answer_options(0),
        }
    }

    /// Opens the list of what the field being edited can be, with `selected` under the cursor.
    fn open_answer_options(&mut self, selected: usize) {
        let rows = self.answer_rows();
        self.open_picker_with(Kind::AnswerOptions, rows, String::new());
        if let Some(picker) = self.picker.as_mut() {
            picker.select(selected);
        }
    }

    /// A row for each thing the field being edited can be, ticked where it is chosen.
    fn answer_rows(&mut self) -> Vec<Row> {
        let Some(editing) = self.answering else {
            return Vec::new();
        };
        let Some(form) = self
            .agents
            .get_mut(editing.session)
            .and_then(|talk| talk.form_mut(editing.ticket))
        else {
            return Vec::new();
        };
        let chosen = match form.given(editing.place) {
            Some(pm_acp::Given::Chosen(value)) => vec![value.clone()],
            Some(pm_acp::Given::ChosenMany(values)) => values.clone(),
            _ => Vec::new(),
        };
        let options = match form.fields().get(editing.place).map(|field| &field.input) {
            Some(Input::One { options, .. } | Input::Many { options, .. }) => options.clone(),
            _ => Vec::new(),
        };
        options
            .into_iter()
            .enumerate()
            .map(|(option, alternative)| {
                let tick = match chosen.contains(&alternative.value) {
                    true => "✓",
                    false => "  ",
                };
                Row {
                    section: None,
                    label: format!("{tick}  {}", alternative.title),
                    detail: String::new(),
                    choice: Choice::AnswerOption(option),
                    enabled: true,
                }
            })
            .collect()
    }

    /// Takes the line typed for the field being edited.
    pub(super) fn type_answer(&mut self, typed: &str) {
        let Some(editing) = self.answering.take() else {
            return;
        };
        let Some(form) = self
            .agents
            .get_mut(editing.session)
            .and_then(|talk| talk.form_mut(editing.ticket))
        else {
            return;
        };
        if let Err(trouble) = form.enter(editing.place, typed) {
            self.notices.trouble(trouble, None);
        }
    }

    /// Chooses the `option`-th alternative of the field being edited.
    ///
    /// A field that takes one choice is done with; one that takes several
    /// shows the list again where it was, so the next can be chosen.
    pub(super) fn choose_answer(&mut self, option: usize) {
        let Some(editing) = self.answering else {
            return;
        };
        let Some(form) = self
            .agents
            .get_mut(editing.session)
            .and_then(|talk| talk.form_mut(editing.ticket))
        else {
            return;
        };
        form.choose(editing.place, option);
        let several = matches!(
            form.fields().get(editing.place).map(|field| &field.input),
            Some(Input::Many { .. })
        );
        match several {
            true => self.open_answer_options(option),
            false => self.answering = None,
        }
    }

    /// Sends the form under `ticket` as it is filled in, unless it is not yet fit to send.
    pub(super) fn send_answer(&mut self, session: TalkId, ticket: u64) {
        let Some(talk) = self.agents.get_mut(session) else {
            return;
        };
        let Some(form) = talk.form_mut(ticket) else {
            return;
        };
        if let Some(problem) = form.problem() {
            self.notices.trouble(problem, None);
            return;
        }
        let reply = form.submission();
        talk.reply(ticket, &reply);
    }

    /// Answers the question under `ticket` without filling anything in.
    pub(super) fn dismiss_answer(&mut self, session: TalkId, ticket: u64, reply: Reply) {
        if let Some(talk) = self.agents.get_mut(session) {
            talk.reply(ticket, &reply);
        }
    }

    /// Opens the page the agent sent the reader to, and answers that it has been.
    pub(super) fn open_answer_link(&mut self, session: TalkId, ticket: u64) {
        let Some(talk) = self.agents.get_mut(session) else {
            return;
        };
        let Some(url) = talk
            .form_mut(ticket)
            .and_then(|form| form.link().map(|link| link.url.clone()))
        else {
            return;
        };
        crate::desktop::browse(&url);
        talk.reply(ticket, &Reply::Accept(Vec::new()));
    }
}
