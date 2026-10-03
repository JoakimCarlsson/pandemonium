//! Status-bar news and nonmodal language-server installation notifications.

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
    /// Server installation cards in arrival order.
    cards: Vec<Installation>,
    /// The card currently shown.
    selected: usize,
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

/// An explicit control on an installation notification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationAction {
    /// Start or retry the installation.
    Install,
    /// Open the offered language's settings.
    LanguageSettings,
    /// Open the automatic installation preference.
    Preference,
    /// Dismiss without installing.
    Close,
    /// Show the preceding card.
    Previous,
    /// Show the following card.
    Next,
}

/// The current stage of a managed server installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationStage {
    /// Waiting for an explicit install action.
    Offer,
    /// Running through the existing installer.
    Progress,
    /// Installed and attached to open documents.
    Done,
    /// Failed, with an explicit retry action.
    Failed,
}

/// One installation shared by every project requesting its command.
#[derive(Clone)]
pub struct Installation {
    /// Identity retained through progress and outcome updates.
    pub id: NoticeId,
    /// The executable being installed.
    pub command: &'static str,
    /// The language whose settings the offer opens.
    pub language: Option<&'static str>,
    /// The current installation stage.
    pub stage: InstallationStage,
    /// The wrapped message, including any failure reason.
    pub text: String,
    /// The message scroll shared with the notification view.
    pub scroll: pm_ui::Scrolled,
    /// The painted card bounds for wheel routing.
    pub bounds: pm_ui::Bounds,
    /// Scroll offset captured at the start of a scrollbar drag.
    pub scroll_origin: Option<f32>,
}

impl Notices {
    /// Creates or updates the card for this server, retaining its identity.
    pub fn installation(
        &mut self,
        command: &'static str,
        language: Option<&'static str>,
        stage: InstallationStage,
        text: String,
    ) -> NoticeId {
        if let Some(card) = self.cards.iter_mut().find(|card| card.command == command) {
            card.stage = stage;
            card.text = text;
            card.scroll.set(pm_ui::Scroll::default());
            return card.id;
        }
        let id = self.next;
        self.next = NoticeId(id.0 + 1);
        self.cards.push(Installation {
            id,
            command,
            language,
            stage,
            text,
            scroll: pm_ui::Scrolled::default(),
            bounds: pm_ui::Bounds::default(),
            scroll_origin: None,
        });
        id
    }

    /// Finds the installation addressed by an explicit notification control.
    pub fn installation_at(&self, id: NoticeId) -> Option<&Installation> {
        self.cards.iter().find(|card| card.id == id)
    }

    /// Returns the selected card and its position among all pending cards.
    pub fn shown_installation(&self) -> Option<(&Installation, usize, usize)> {
        let selected = self.selected.min(self.cards.len().saturating_sub(1));
        Some((self.cards.get(selected)?, selected + 1, self.cards.len()))
    }

    /// Moves between cards without losing or dismissing any of them.
    pub fn step_installation(&mut self, backwards: bool) {
        let count = self.cards.len();
        if count > 0 {
            let selected = self.selected.min(count - 1);
            self.selected = (selected + if backwards { count - 1 } else { 1 }) % count;
        }
    }

    /// Applies a scrollbar drag to the message viewport identified by `id`.
    pub fn scroll_installation(&mut self, id: NoticeId, event: pm_ui::ResizeEvent, step: f32) {
        if let Some(card) = self.cards.iter_mut().find(|card| card.id == id) {
            let mut scroll = card.scroll.get();
            let base = match event.phase {
                pm_ui::ResizePhase::Started => scroll.offset(),
                _ => card.scroll_origin.unwrap_or(scroll.offset()),
            };
            card.scroll_origin = match event.phase {
                pm_ui::ResizePhase::Ended => None,
                _ => Some(base),
            };
            scroll.by(scroll.offset() - base - event.delta(pm_ui::Axis::Vertical) * step);
            card.scroll.set(scroll);
        }
    }

    /// Dismisses a card without invoking any of its actions.
    pub fn dismiss_installation(&mut self, id: NoticeId) {
        if let Some(index) = self.cards.iter().position(|card| card.id == id) {
            self.cards.remove(index);
            self.selected = self.selected.min(self.cards.len().saturating_sub(1));
        }
    }
}
