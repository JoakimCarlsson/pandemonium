//! The question itself: what is being asked, and the ways to answer it.

use crate::message::Message;

/// One way to answer a question the window has asked.
pub struct Answer {
    /// What the button says.
    pub label: String,
    /// What choosing it does, or nothing at all for the one that backs out.
    pub taken: Option<Message>,
}

impl Answer {
    /// An answer labelled `label` that carries `taken` out.
    pub fn new(label: impl Into<String>, taken: Message) -> Self {
        Self {
            label: label.into(),
            taken: Some(taken),
        }
    }

    /// The answer that backs out, which every question has and none carries out.
    pub fn cancel() -> Self {
        Self {
            label: "Cancel".to_owned(),
            taken: None,
        }
    }
}

/// A question the window is asking before it does something it cannot undo.
///
/// The question is modal: nothing else takes a keystroke while it is up, and
/// the only ways past it are one of its answers or backing out. What it is
/// asking about is the caller's to remember — the question carries what to do
/// when it is answered and nothing else.
pub struct Prompt {
    /// What is being asked.
    message: String,
    /// What else there is to say about it, a line at a time, under it.
    detail: Vec<String>,
    /// The ways to answer, in the order they are offered.
    answers: Vec<Answer>,
    /// The answer Enter takes, which a click or the arrows move.
    active: usize,
}

impl Prompt {
    /// A question reading `message`, answered one of `answers` ways.
    ///
    /// The first answer is the one Enter takes, as it is in every editor that
    /// asks this way: the answer offered first is the one the question was
    /// asked for, and backing out is the one the keyboard already has.
    pub fn asking(message: impl Into<String>, detail: Vec<String>, answers: Vec<Answer>) -> Self {
        Self {
            message: message.into(),
            detail,
            answers,
            active: 0,
        }
    }

    /// What is being asked.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// What else there is to say about it, a line at a time.
    pub fn detail(&self) -> &[String] {
        &self.detail
    }

    /// The ways to answer, in the order they are offered.
    pub fn answers(&self) -> &[Answer] {
        &self.answers
    }

    /// Which answer Enter would take.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Moves to the answer `steps` along, wrapping round at either end.
    pub fn step(&mut self, steps: isize) {
        let count = self.answers.len() as isize;
        if count == 0 {
            return;
        }
        self.active = (self.active as isize + steps).rem_euclid(count) as usize;
    }

    /// What the `place`-th answer does, if the question is answered that way.
    pub fn taken(&self, place: usize) -> Option<Message> {
        self.answers.get(place)?.taken
    }

    /// What the answer Enter would take does.
    pub fn chosen(&self) -> Option<Message> {
        self.taken(self.active)
    }
}
