//! What keeps the reader safe in the browser: the questions a page asks about the reader's machine, a site whose certificate is not trusted, and the files a page downloaded.

use std::time::{Duration, Instant};

use pm_browser::{DownloadState, Notice};

/// How long after a question first shows, or the question shown changes, pressing Allow does nothing, in case the reader was about to press something the page moved under the pointer.
pub const ALLOW_DELAY: Duration = Duration::from_millis(500);

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

impl Question {
    /// What the bar says of the question: which host wants what.
    pub fn words(&self) -> String {
        let host = self
            .origin
            .split_once("://")
            .map_or(self.origin.as_str(), |(_, host)| host);
        format!("{host} wants to use your {}", self.permission)
    }
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

impl Certificate {
    /// The host of the site, without scheme or path.
    pub fn host(&self) -> &str {
        self.url
            .split_once("://")
            .map_or(self.url.as_str(), |(_, rest)| rest)
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default()
    }

    /// What the engine's name for the problem means.
    pub fn reason(&self) -> &'static str {
        match self.error.as_str() {
            "ERR_CERT_AUTHORITY_INVALID" => {
                "The certificate was not issued by an authority this machine trusts. It may be self-signed."
            }
            "ERR_CERT_DATE_INVALID" => "The certificate has expired or is not valid yet.",
            "ERR_CERT_COMMON_NAME_INVALID" => {
                "The certificate was issued for a different site name."
            }
            "ERR_CERT_REVOKED" => "The certificate was revoked by the authority that issued it.",
            "ERR_CERT_WEAK_SIGNATURE_ALGORITHM" | "ERR_CERT_WEAK_KEY" => {
                "The certificate is protected too weakly to be trusted."
            }
            _ => "The certificate cannot be trusted.",
        }
    }
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

impl Download {
    /// What the bar says of the download.
    pub fn words(&self) -> String {
        match self.saving {
            Saving::Started => format!("Downloading {} to your Downloads folder", self.name),
            Saving::Finished => format!("Saved {} in your Downloads folder", self.name),
            Saving::Failed => format!("Could not save {}", self.name),
        }
    }
}

/// What the reader answered, for the engine.
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

/// What a tab shows for the reader's safety.
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
    /// The question showing and when it first showed, which is when [`ALLOW_DELAY`] starts.
    shown: Option<(u64, Instant)>,
    /// Whether the window was told that Allow of the question showing became ready.
    ready_told: bool,
}

impl Safety {
    /// Adds a question at the time `now`, unless it is already asked.
    pub fn ask_at(&mut self, question: Question, now: Instant) {
        if !self.questions.iter().any(|asked| asked.id == question.id) {
            self.questions.push(question);
        }
        self.track(now);
    }

    /// Takes away the question `id` at the time `now`, as when the page went away.
    pub fn cancel_at(&mut self, id: u64, now: Instant) {
        self.questions.retain(|asked| asked.id != id);
        self.track(now);
    }

    /// Starts the delay over when the question showing is not the one it was counted for, and ends it when none shows.
    fn track(&mut self, now: Instant) {
        let first = self.questions.first().map(|asked| asked.id);
        match (first, self.shown) {
            (None, _) => {
                self.shown = None;
                self.ready_told = false;
            }
            (Some(id), Some((counted, _))) if id == counted => {}
            (Some(id), _) => {
                self.shown = Some((id, now));
                self.ready_told = false;
            }
        }
    }

    /// Whether Allow of the question showing became ready at `now` since last asked, so the prompt must be drawn again.
    pub fn note_ready(&mut self, now: Instant) -> bool {
        if self.ready_told || !self.allow_ready(now) {
            return false;
        }
        self.ready_told = true;
        true
    }

    /// Whether pressing Allow does anything at the time `now`: the first question has been showing for [`ALLOW_DELAY`].
    pub fn allow_ready(&self, now: Instant) -> bool {
        match (self.questions.first(), self.shown) {
            (Some(asked), Some((counted, since))) => {
                asked.id == counted && now.saturating_duration_since(since) >= ALLOW_DELAY
            }
            _ => false,
        }
    }

    /// Answers the question `id`, allowing it when `allow`, at the time `now`; only the question showing can be answered, and Allow only once [`ALLOW_DELAY`] has passed.
    pub fn answer_at(&mut self, id: u64, allow: bool, now: Instant) {
        let showing = self.questions.first().map(|asked| asked.id);
        if showing == Some(id) && (!allow || self.allow_ready(now)) {
            self.questions.remove(0);
            self.answers.push(Answer::Permission { id, allow });
            self.track(now);
        }
    }

    /// Takes the page back from a site whose certificate is not trusted.
    pub fn certificate_back(&mut self) {
        if self.certificate.take().is_some() {
            self.answers.push(Answer::Certificate { proceed: false });
        }
    }

    /// Asks again on the first press, and on the second loads the site whose certificate is not trusted.
    pub fn certificate_proceed(&mut self) {
        match self.certificate.as_mut() {
            Some(certificate) if !certificate.confirming => certificate.confirming = true,
            Some(_) => {
                self.certificate = None;
                self.answers.push(Answer::Certificate { proceed: true });
            }
            None => {}
        }
    }

    /// Takes in one thing the engine reported that the reader is to see or answer, at the time `now`.
    pub fn take_notice(&mut self, notice: Notice, now: Instant) {
        match notice {
            Notice::Permission {
                id,
                origin,
                permission,
            } => self.ask_at(
                Question {
                    id,
                    origin,
                    permission,
                },
                now,
            ),
            Notice::PermissionCancelled { id } => self.cancel_at(id, now),
            Notice::Certificate(info) => {
                self.certificate = Some(Certificate {
                    url: info.url,
                    error: info.error,
                    subject: info.subject,
                    issuer: info.issuer,
                    fingerprint: info.fingerprint,
                    expires: info.expires,
                    confirming: false,
                });
            }
            Notice::CertificateCleared => self.certificate = None,
            Notice::Download { name, state } => {
                self.download = Some(Download {
                    name,
                    saving: match state {
                        DownloadState::Started => Saving::Started,
                        DownloadState::Finished => Saving::Finished,
                        DownloadState::Failed => Saving::Failed,
                    },
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A question about the camera of `origin`, numbered `id`.
    fn asked(id: u64, origin: &str) -> Question {
        Question {
            id,
            origin: origin.to_owned(),
            permission: "camera".to_owned(),
        }
    }

    /// Questions are answered oldest first, and one asked twice is held once.
    #[test]
    fn questions_are_answered_in_the_order_they_came() {
        let start = Instant::now();
        let mut safety = Safety::default();
        safety.ask_at(asked(1, "https://a.example"), start);
        safety.ask_at(asked(1, "https://a.example"), start);
        safety.ask_at(asked(2, "https://b.example"), start);
        assert_eq!(safety.questions.len(), 2);
        safety.answer_at(2, false, start);
        assert_eq!(safety.questions.len(), 2);
        let later = start + ALLOW_DELAY;
        safety.answer_at(1, false, later);
        safety.answer_at(2, true, later + ALLOW_DELAY);
        assert!(safety.questions.is_empty());
        assert_eq!(
            safety.answers,
            vec![
                Answer::Permission {
                    id: 1,
                    allow: false
                },
                Answer::Permission { id: 2, allow: true },
            ]
        );
    }

    /// Allow waits half a second after the question showed, and Block does not.
    #[test]
    fn allow_waits_and_block_does_not() {
        let start = Instant::now();
        let mut safety = Safety::default();
        safety.ask_at(asked(1, "https://a.example"), start);
        safety.answer_at(1, true, start + Duration::from_millis(100));
        assert!(safety.answers.is_empty());
        safety.answer_at(1, true, start + ALLOW_DELAY);
        assert_eq!(
            safety.answers,
            vec![Answer::Permission { id: 1, allow: true }]
        );
        assert_eq!(
            asked(3, "https://c.example").words(),
            "c.example wants to use your camera"
        );
    }

    /// Proceeding past an untrusted certificate takes two presses; going back one.
    #[test]
    fn proceeding_asks_twice() {
        let mut safety = Safety {
            certificate: Some(Certificate::default()),
            ..Safety::default()
        };
        safety.certificate_proceed();
        assert!(safety.certificate.as_ref().is_some_and(|c| c.confirming));
        safety.certificate_proceed();
        assert!(safety.certificate.is_none());
        safety.certificate = Some(Certificate::default());
        safety.certificate_back();
        assert_eq!(
            safety.answers,
            vec![
                Answer::Certificate { proceed: true },
                Answer::Certificate { proceed: false }
            ]
        );
    }
}
