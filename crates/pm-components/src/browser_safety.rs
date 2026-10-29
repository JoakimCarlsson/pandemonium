//! What keeps the reader safe in the browser pane: the questions a page asks about the reader's machine, a site whose certificate is not trusted, and the files a page downloaded.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_b, border_color, flex, flex_1, flex_col, flex_none, font_mono,
    font_semibold, gap, h_px, items_center, justify_center, min_w_px, p, px, py, rounded, w_full,
    w_px,
};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::Variant;
use crate::parts::{clipped, line, wrap};
use crate::size::Size;

/// Allows what the first question of the page asks.
pub const BROWSER_ALLOW: FocusId = FocusId(7_440_800);

/// Refuses what the first question of the page asks.
pub const BROWSER_BLOCK: FocusId = FocusId(7_440_801);

/// Takes away the note about a download.
pub const BROWSER_DISMISS: FocusId = FocusId(7_440_802);

/// Takes the page back from a site whose certificate is not trusted.
pub const BROWSER_CERT_BACK: FocusId = FocusId(7_440_803);

/// Loads a site whose certificate is not trusted anyway; the first press asks again, the second does it.
pub const BROWSER_CERT_PROCEED: FocusId = FocusId(7_440_804);

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
}

impl Safety {
    /// Adds a question, unless it is already asked.
    pub fn ask(&mut self, question: Question) {
        if !self.questions.iter().any(|asked| asked.id == question.id) {
            self.questions.push(question);
        }
    }

    /// Takes away the question `id`, as when the page went away.
    pub fn cancel(&mut self, id: u64) {
        self.questions.retain(|asked| asked.id != id);
    }

    /// Carries out pressing `id`, and says whether it was something of this part.
    pub fn press(&mut self, id: FocusId) -> bool {
        match id {
            BROWSER_ALLOW | BROWSER_BLOCK => {
                if !self.questions.is_empty() {
                    let asked = self.questions.remove(0);
                    self.answers.push(Answer::Permission {
                        id: asked.id,
                        allow: id == BROWSER_ALLOW,
                    });
                }
            }
            BROWSER_DISMISS => self.download = None,
            BROWSER_CERT_BACK => {
                if self.certificate.take().is_some() {
                    self.answers.push(Answer::Certificate { proceed: false });
                }
            }
            BROWSER_CERT_PROCEED => match self.certificate.as_mut() {
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

    /// How tall the bars above the page are together, in logical pixels.
    pub fn bars(&self) -> f32 {
        let count = usize::from(!self.questions.is_empty()) + usize::from(self.download.is_some());
        count as f32 * BAR
    }

    /// The bars to show above the page: the first question, and the note about the last download.
    pub fn bar_nodes(&self) -> Vec<Node> {
        let mut bars = Vec::new();
        if let Some(asked) = self.questions.first() {
            bars.push(question_bar(asked, self.questions.len() - 1));
        }
        if let Some(download) = self.download.as_ref() {
            bars.push(download_bar(download));
        }
        bars
    }
}

/// The host of `origin`, without its scheme.
fn host_of(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, host)| host)
}

/// A bar that has `body` before and `buttons` after, as tall as [`BAR`].
fn bar(body: Node, buttons: Vec<Node>) -> Node {
    let mut row = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            px(3),
            h_px(BAR),
            flex_none(),
            w_full(),
            bg(Role::Surface),
            border_b(),
            border_color(Role::Border),
        ])
        .child(body);
    for button in buttons {
        row = row.child(button);
    }
    row
}

/// The bar that says which page asks for what, and has the buttons that allow or refuse it; `waiting` more questions come after it.
fn question_bar(asked: &Question, waiting: usize) -> Node {
    let mut words = format!(
        "{} wants to use your {}",
        host_of(&asked.origin),
        asked.permission
    );
    if waiting > 0 {
        words.push_str(&format!(" (+{waiting} more)"));
    }
    bar(
        clipped(words, 13, Role::Text),
        vec![
            Button::new("Block", BROWSER_BLOCK)
                .variant(Variant::Outline)
                .size(Size::Sm)
                .build(),
            Button::new("Allow", BROWSER_ALLOW).size(Size::Sm).build(),
        ],
    )
}

/// The bar that says where the last download was saved, or that it could not be.
fn download_bar(download: &Download) -> Node {
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
    bar(
        clipped(words, 13, tone),
        vec![Button::icon(pm_icon::IconName::Close, BROWSER_DISMISS).build()],
    )
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

/// `content` broken into lines no more than `room` logical pixels across, one under another, in `tone` at `pixels` tall type.
fn paragraph(content: &str, pixels: u32, tone: Role, room: f32) -> Node {
    node().styles([flex(), flex_col()]).children(
        wrap(content, pixels, room)
            .into_iter()
            .map(|words| line(words, pixels, 1.45, tone)),
    )
}

/// A row of the certificate's details: `label`, and what it says in `value`.
fn detail(label: &str, value: &str, mono: bool) -> Node {
    let value = if value.is_empty() { "unknown" } else { value };
    let shown = line(value, 12, 1.4, Role::Text);
    node()
        .styles([flex(), gap(3), w_full(), min_w_px(0)])
        .child(
            node()
                .styles([w_px(108.0), flex_none()])
                .child(line(label, 12, 1.4, Role::Muted)),
        )
        .child(node().styles([flex_1(), min_w_px(0)]).child(if mono {
            shown.styles([font_mono()])
        } else {
            shown
        }))
}

/// The page that stands in for a site whose certificate is not trusted, `width` by `height` logical pixels: what is wrong with the certificate and the buttons that go back or, deliberately, go on.
pub fn certificate_page(certificate: &Certificate, width: f32, height: f32) -> Node {
    let host = certificate
        .url
        .split_once("://")
        .map_or(certificate.url.as_str(), |(_, rest)| rest)
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
    let card = node()
        .styles([
            flex(),
            flex_col(),
            gap(3),
            p(5),
            w_px(across),
            flex_none(),
            rounded(Radius::Xl),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(
            line("Not secure: certificate not trusted", 18, 1.3, Role::Danger)
                .styles([font_semibold()]),
        )
        .child(paragraph(
            &format!(
                "Pandemonium could not verify that {host} is who it says it is. Anything you send to it could be read or changed on the way."
            ),
            13,
            Role::Text,
            room,
        ))
        .child(paragraph(
            &format!("{} ({})", reason(&certificate.error), certificate.error),
            12,
            Role::Muted,
            room,
        ))
        .child(
            node()
                .styles([flex(), flex_col(), gap(1), py(2)])
                .child(detail("Site", &certificate.url, false))
                .child(detail("Issued to", &certificate.subject, false))
                .child(detail("Issued by", &certificate.issuer, false))
                .child(detail("Expires", &certificate.expires, false))
                .child(detail("SHA-256", &certificate.fingerprint, true)),
        )
        .child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(Button::new("Go back", BROWSER_CERT_BACK).build())
                .child(if certificate.confirming {
                    Button::new(proceed, BROWSER_CERT_PROCEED)
                        .variant(Variant::Destructive)
                        .build()
                } else {
                    Button::new(proceed, BROWSER_CERT_PROCEED)
                        .variant(Variant::Outline)
                        .tone(Role::Danger)
                        .build()
                }),
        )
        .child(paragraph(
            if certificate.confirming {
                "Press again to load the site. This is remembered for this site until the browser closes."
            } else {
                "Going on is not recommended."
            },
            12,
            Role::Muted,
            room,
        ));
    node()
        .styles([
            flex(),
            items_center(),
            justify_center(),
            w_px(width),
            h_px(height),
            flex_none(),
            p(6),
            bg(Role::Background),
        ])
        .child(card)
}

#[cfg(test)]
mod tests;
