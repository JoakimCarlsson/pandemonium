//! What has happened that the reader should hear about, held for the status
//! bar.
//!
//! A notice is news, not a question: an agent that went away on its own, a
//! shell that exited badly, a remote that answered. Nothing waits on the
//! reader's reply, so a notice is never a dialog; it sits in the bar, it
//! takes the reader to what it is about when clicked, and it goes when
//! dismissed. News that something went well goes by itself after a while;
//! news that something went wrong stays until it has been read.

use std::time::{Duration, Instant};

use crate::message::Message;

/// How long news that something went well stays in the bar.
const DONE_FOR: Duration = Duration::from_secs(6);

/// A notice's identity for as long as it is held.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NoticeId(u64);

/// Whether a notice says something went wrong or something went well.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tone {
    /// Something went wrong, and it stays until it is dismissed.
    Trouble,
    /// Something went well, and it goes by itself.
    Done,
}

/// One thing the reader should hear about.
struct Notice {
    /// Which notice this is.
    id: NoticeId,
    /// Whether it is trouble or good news.
    tone: Tone,
    /// What it says.
    text: String,
    /// What clicking it does, besides dismissing it.
    action: Option<Message>,
    /// When it arrived.
    at: Instant,
}

/// The notice the status bar shows, and how many more are behind it.
#[derive(Clone)]
pub struct Shown {
    /// Which notice it is.
    pub id: NoticeId,
    /// Whether it is trouble or good news.
    pub tone: Tone,
    /// What it says.
    pub text: String,
    /// How many older notices are still held.
    pub more: usize,
}

/// Every notice the window is holding, oldest first.
#[derive(Default)]
pub struct Notices {
    /// The notices, oldest first.
    held: Vec<Notice>,
    /// The id the next notice will be given.
    next: NoticeId,
}

impl Notices {
    /// Holds news that something went wrong, until it is dismissed.
    pub fn trouble(&mut self, text: impl Into<String>, action: Option<Message>) {
        self.push(Tone::Trouble, text.into(), action);
    }

    /// Holds news that something went well, for a while.
    pub fn done(&mut self, text: impl Into<String>, action: Option<Message>) {
        self.push(Tone::Done, text.into(), action);
    }

    /// Holds a running operation in the status bar until it is dismissed.
    pub fn progress(&mut self, text: impl Into<String>) -> NoticeId {
        let id = self.next;
        self.push(Tone::Done, text.into(), None);
        if let Some(notice) = self.held.last_mut() {
            notice.at += Duration::from_secs(24 * 60 * 60);
        }
        id
    }

    /// Holds one notice after the rest.
    fn push(&mut self, tone: Tone, text: String, action: Option<Message>) {
        let id = self.next;
        self.next = NoticeId(id.0 + 1);
        self.held.push(Notice {
            id,
            tone,
            text,
            action,
            at: Instant::now(),
        });
    }

    /// Lets go of the notice `id` names, handing back what clicking it does.
    pub fn dismiss(&mut self, id: NoticeId) -> Option<Message> {
        let at = self.held.iter().position(|notice| notice.id == id)?;
        self.held.remove(at).action
    }

    /// The newest notice, for the status bar.
    pub fn shown(&self) -> Option<Shown> {
        let newest = self.held.last()?;
        Some(Shown {
            id: newest.id,
            tone: newest.tone,
            text: newest.text.clone(),
            more: self.held.len() - 1,
        })
    }

    /// Lets go of the good news that has been shown long enough, and says
    /// whether there was any.
    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.held.len();
        self.held
            .retain(|notice| notice.tone == Tone::Trouble || now < notice.at + DONE_FOR);
        self.held.len() != before
    }

    /// When the next piece of good news is due to go.
    pub fn next_expiry(&self) -> Option<Instant> {
        self.held
            .iter()
            .filter(|notice| notice.tone == Tone::Done)
            .map(|notice| notice.at + DONE_FOR)
            .min()
    }
}
