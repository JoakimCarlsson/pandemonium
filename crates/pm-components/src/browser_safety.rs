//! What keeps the reader safe in the browser pane: the questions a page asks about the reader's machine, a site whose certificate is not trusted, and the files a page downloaded.

use crate::areas::BrowserEvent;
use crate::button;
use crate::theme::Frame;
use rustle::TextOverflow;

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, row_arranged};

use std::time::{Duration, Instant};

use crate::events::Interactive;
use crate::theme::{Radius, Role};
use rustle::Modifier;

use crate::Button;
use crate::button::Variant;
use crate::parts::{clipped, wrapping};

/// How long after a question first shows, or the question shown changes, pressing Allow does nothing, in case the reader was about to press something the page moved under the pointer.
pub const ALLOW_DELAY: Duration = Duration::from_millis(500);

/// How tall the bar with a question or a note is, in logical pixels.
pub const BAR: f32 = 40.0;

/// A page asking to use something of the reader's machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Question {
    /// The number the engine knows the question by.
    pub id: u64,
    /// The origin that asks, such as `https://example.com`.
    pub origin: String,
    /// What it asks for, in words such as `camera` or `location`.
    pub permission: String,
}

/// A site whose certificate is not trusted, which the page did not load.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Certificate {
    /// The address that was being opened.
    pub url: String,
    /// The engine's name for what is wrong, such as `ERR_CERT_AUTHORITY_INVALID`.
    pub error: String,
    /// Who the certificate was issued to.
    pub subject: String,
    /// Who issued it.
    pub issuer: String,
    /// Its SHA-256 fingerprint.
    pub fingerprint: String,
    /// When it expires, when that is known.
    pub expires: String,
    /// Whether the reader pressed to proceed once and is being asked to press again.
    pub confirming: bool,
}

/// How saving a download is going.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Saving {
    /// The file is being saved.
    Started,
    /// The file was saved.
    Finished,
    /// The file could not be saved.
    Failed,
}

/// The last file a page downloaded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Download {
    /// The name it is saved under in the Downloads folder.
    pub name: String,
    /// How it is going.
    pub saving: Saving,
}

/// What the reader answered, for the window to carry to the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Answer {
    /// The reader allowed or refused the question `id`.
    Permission {
        /// The question.
        id: u64,
        /// Whether it was allowed.
        allow: bool,
    },
    /// The reader chose to load the site anyway, or to go back.
    Certificate {
        /// Whether to load it anyway.
        proceed: bool,
    },
}

/// What the pane shows for the reader's safety.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Safety {
    /// The questions of the page that wait for an answer, the oldest first.
    pub questions: Vec<Question>,
    /// The site whose certificate is not trusted, while the reader has not chosen.
    pub certificate: Option<Certificate>,
    /// The last download.
    pub download: Option<Download>,
    /// What the reader answered and the engine has not yet been told.
    pub answers: Vec<Answer>,
    /// The question that is showing and when it first showed, which is when [`ALLOW_DELAY`] starts.
    shown: Option<(u64, Instant)>,
}

impl Safety {
    /// Adds a question, unless it is already asked.
    pub fn ask(&mut self, question: Question) {
        self.ask_at(question, Instant::now());
    }

    /// Adds a question, unless it is already asked, at the time `now`.
    pub fn ask_at(&mut self, question: Question, now: Instant) {
        if !self.questions.iter().any(|asked| asked.id == question.id) {
            self.questions.push(question);
        }
        self.track(now);
    }

    /// Takes away the question `id`, as when the page went away.
    pub fn cancel(&mut self, id: u64) {
        self.cancel_at(id, Instant::now());
    }

    /// Takes away the question `id` at the time `now`.
    pub fn cancel_at(&mut self, id: u64, now: Instant) {
        self.questions.retain(|asked| asked.id != id);
        self.track(now);
    }

    /// Starts the delay over when the question showing is not the one the delay was counted for, and ends it when none shows.
    fn track(&mut self, now: Instant) {
        let first = self.questions.first().map(|asked| asked.id);
        match (first, self.shown) {
            (None, _) => self.shown = None,
            (Some(id), Some((counted, _))) if id == counted => {}
            (Some(id), _) => self.shown = Some((id, now)),
        }
    }

    /// Whether pressing Allow does anything at the time `now`: the first question has been showing for [`ALLOW_DELAY`]. Block is never held back.
    pub fn allow_ready(&self, now: Instant) -> bool {
        match (self.questions.first(), self.shown) {
            (Some(asked), Some((counted, since))) => {
                asked.id == counted && now.saturating_duration_since(since) >= ALLOW_DELAY
            }
            _ => false,
        }
    }

    /// Carries out pressing `id`, and says whether it was something of this part.
    pub fn press(&mut self, id: Target) -> bool {
        self.press_at(id, Instant::now())
    }

    /// Carries out pressing `id` at the time `now`, and says whether it was something of this part. A press of Allow or Block answers only the question it was named for, which is the one that was showing when the press began, and Allow only once [`ALLOW_DELAY`] has passed.
    pub fn press_at(&mut self, id: Target, now: Instant) -> bool {
        match id {
            Target::Browser(BrowserEvent::BrowserQuestionBar) => {}
            Target::Browser(BrowserEvent::BrowserAllow(asked))
            | Target::Browser(BrowserEvent::BrowserBlock(asked)) => {
                let allow = matches!(id, Target::Browser(BrowserEvent::BrowserAllow(_)));
                let showing = self.questions.first().map(|question| question.id);
                let named = showing.filter(|shown| *shown == asked);
                if let Some(question) = named
                    && (!allow || self.allow_ready(now))
                {
                    self.questions.remove(0);
                    self.answers.push(Answer::Permission {
                        id: question,
                        allow,
                    });
                    self.track(now);
                }
            }
            Target::Browser(BrowserEvent::BrowserDismiss) => self.download = None,
            Target::Browser(BrowserEvent::BrowserCertBack) => {
                if self.certificate.take().is_some() {
                    self.answers.push(Answer::Certificate { proceed: false });
                }
            }
            Target::Browser(BrowserEvent::BrowserCertProceed) => match self.certificate.as_mut() {
                Some(certificate) if !certificate.confirming => certificate.confirming = true,
                Some(_) => {
                    self.certificate = None;
                    self.answers.push(Answer::Certificate { proceed: true });
                }
                None => {}
            },
            _ => return false,
        }
        true
    }

    /// Emits the bars to show above the page: the note about the last download. The question is not among them; it is laid over the page by [`Safety::question_overlay`], so that it appearing does not move the page.
    #[track_caller]
    pub fn bar_nodes(&self) {
        if let Some(download) = self.download.as_ref() {
            download_bar(download);
        }
    }

    /// How long the first question still waits at the time `now` before pressing Allow does anything, when there is a question.
    pub fn allow_wait(&self, now: Instant) -> Option<Duration> {
        let (_, since) = self.shown?;
        Some(ALLOW_DELAY.saturating_sub(now.saturating_duration_since(since)))
    }

    /// Emits the bar with the first question, when there is one, laid over the top of the page across the width of the page's box, at its top start, in the layer of the bar for finding in the page; Allow is drawn faint and cannot be pressed until [`ALLOW_DELAY`] has passed, which an effect waits for on the frame clock.
    #[track_caller]
    pub fn question_overlay(&self) {
        let ready = rustle::remember(|| None::<u64>);
        let Some(asked) = self.questions.first() else {
            return;
        };
        let id = asked.id;
        let wait = self.allow_wait(Instant::now()).unwrap_or_default();
        let readied = ready.clone();
        rustle::launched_effect(id, move |scope| async move {
            scope.delay(wait).await;
            readied.set(Some(id));
        });
        question_bar(
            Modifier::new()
                .align(rustle::BoxAlignment::TopStart)
                .fill_max_width()
                .z_index(40)
                .control(
                    Target::Browser(BrowserEvent::BrowserQuestionBar),
                    send(Event::Browser(BrowserEvent::BrowserQuestionBar)),
                ),
            asked,
            self.questions.len() - 1,
            ready.get() == Some(id),
        );
    }
}

/// The host of `origin`, without its scheme.
fn host_of(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, host)| host)
}

/// Emits a bar as tall as [`BAR`] and as wide as its parent, with `modifier` around it, holding what `content` emits side by side: its words, then its buttons.
#[track_caller]
fn bar(modifier: Modifier, content: impl FnOnce()) {
    rustle::row_arranged(
        modifier
            .height(BAR)
            .fill_max_width()
            .background(theme::color(Role::Surface))
            .border_bottom(1.0, theme::color(Role::Border))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        content,
    );
}

/// Emits the bar that says which page asks for what, and has the buttons that allow or refuse it, with `modifier` around it; `waiting` more questions come after it, and Allow is faint and inert until `ready`.
fn question_bar(modifier: Modifier, asked: &Question, waiting: usize, ready: bool) {
    let mut words = format!(
        "{} wants to use your {}",
        host_of(&asked.origin),
        asked.permission
    );
    if waiting > 0 {
        words.push_str(&format!(" (+{waiting} more)"));
    }
    bar(modifier, || {
        clipped(words, 13, Role::Text);
        button::action(
            "Block",
            Target::Browser(BrowserEvent::BrowserBlock(asked.id)),
            Variant::Outline,
        );
        button::small_button(
            "Allow",
            Target::Browser(BrowserEvent::BrowserAllow(asked.id)),
            if ready {
                Variant::Default
            } else {
                Variant::Secondary
            },
        )
        .disabled(!ready)
        .build();
    });
}

/// Emits the bar that says where the last download was saved, or that it could not be.
fn download_bar(download: &Download) {
    let words = match download.saving {
        Saving::Started => format!("Downloading {} to your Downloads folder", download.name),
        Saving::Finished => format!("Saved {} in your Downloads folder", download.name),
        Saving::Failed => format!("Could not save {}", download.name),
    };
    let tone = if download.saving == Saving::Failed {
        Role::Danger
    } else {
        Role::Text
    };
    bar(Modifier::new(), || {
        clipped(words, 13, tone);
        Button::icon(phosphor::X, Target::Browser(BrowserEvent::BrowserDismiss)).build();
    });
}

/// What the engine's name for a certificate problem means.
fn reason(error: &str) -> &'static str {
    match error {
        "ERR_CERT_AUTHORITY_INVALID" => {
            "The certificate was not issued by an authority this machine trusts. It may be self-signed."
        }
        "ERR_CERT_DATE_INVALID" => "The certificate has expired or is not valid yet.",
        "ERR_CERT_COMMON_NAME_INVALID" => "The certificate was issued for a different site name.",
        "ERR_CERT_REVOKED" => "The certificate was revoked by the authority that issued it.",
        "ERR_CERT_WEAK_SIGNATURE_ALGORITHM" | "ERR_CERT_WEAK_KEY" => {
            "The certificate is protected too weakly to be trusted."
        }
        _ => "The certificate cannot be trusted.",
    }
}

/// Emits `content` broken into lines no more than `room` logical pixels across, one under another, in `tone` at `pixels` tall type.
#[track_caller]
fn paragraph(content: &str, pixels: u32, tone: Role, room: f32) {
    wrapping(
        Modifier::new().width_in(0.0, room),
        content,
        pixels,
        1.45,
        tone,
    );
}

/// Emits a row of the certificate's details: `name`, and what it says in `value`, set in the code face when `mono`.
#[track_caller]
fn detail(name: &str, value: &str, mono: bool) {
    let value = if value.is_empty() { "unknown" } else { value };
    let kind = theme::line(12, Role::Text);
    let kind = if mono { theme::in_mono(kind) } else { kind };
    rustle::row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Start,
        || {
            rustle::basic_text(
                Modifier::new().width(108.0),
                name,
                theme::line(12, Role::Muted),
                TextOverflow::Ellipsis,
            );
            rustle::basic_text(
                Modifier::new().weight(1.0),
                value,
                kind,
                TextOverflow::Ellipsis,
            );
        },
    );
}

/// Emits the page that stands in for a site whose certificate is not trusted, `width` by `height` logical pixels: what is wrong with the certificate and the buttons that go back or, deliberately, go on.
pub fn certificate_page(certificate: &Certificate, width: f32, height: f32) {
    let host = host_of(&certificate.url)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let proceed = if certificate.confirming {
        "Yes, proceed anyway (unsafe)"
    } else {
        "Proceed anyway (unsafe)"
    };
    let across = (width - 48.0).clamp(0.0, 560.0);
    let room = (across - 40.0).max(0.0);
    row_arranged(
        Modifier::new()
            .width(width)
            .height(height)
            .background(theme::color(Role::Background))
            .padding(theme::sp(6)),
        Arrangement::Center,
        Alignment::Center,
        || {
            rustle::column_arranged(
                Modifier::new()
                    .width(across)
                    .panel(Radius::Xl)
                    .padding(theme::sp(5)),
                Arrangement::SpacedBy(theme::sp(3)),
                Alignment::Start,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        "Not secure: certificate not trusted",
                        theme::text_style(18, 1.3)
                            .font_weight(rustle::FontWeight::SEMI_BOLD)
                            .color(theme::color(Role::Danger))
                            .max_lines(1),
                        TextOverflow::Ellipsis,
                    );
                    paragraph(
                        &format!(
                            "Pandemonium could not verify that {host} is who it says it is. Anything you send to it could be read or changed on the way."
                        ),
                        13,
                        Role::Text,
                        room,
                    );
                    paragraph(
                        &format!("{} ({})", reason(&certificate.error), certificate.error),
                        12,
                        Role::Muted,
                        room,
                    );
                    rustle::column_arranged(
                        Modifier::new()
                            .fill_max_width()
                            .padding_xy(0.0, theme::sp(2)),
                        Arrangement::SpacedBy(theme::sp(1)),
                        Alignment::Start,
                        || {
                            detail("Site", &certificate.url, false);
                            detail("Issued to", &certificate.subject, false);
                            detail("Issued by", &certificate.issuer, false);
                            detail("Expires", &certificate.expires, false);
                            detail("SHA-256", &certificate.fingerprint, true);
                        },
                    );
                    rustle::row_arranged(
                        Modifier::new(),
                        Arrangement::SpacedBy(theme::sp(2)),
                        Alignment::Center,
                        || {
                            Button::new("Go back", Target::Browser(BrowserEvent::BrowserCertBack))
                                .build();
                            let button = Button::new(
                                proceed,
                                Target::Browser(BrowserEvent::BrowserCertProceed),
                            );
                            if certificate.confirming {
                                button.variant(Variant::Destructive)
                            } else {
                                button.variant(Variant::Outline).tone(Role::Danger)
                            }
                            .build();
                        },
                    );
                    paragraph(
                        if certificate.confirming {
                            "Press again to load the site. This is remembered for this site until the browser closes."
                        } else {
                            "Going on is not recommended."
                        },
                        12,
                        Role::Muted,
                        room,
                    );
                },
            );
        },
    );
}

#[cfg(test)]
mod tests;
