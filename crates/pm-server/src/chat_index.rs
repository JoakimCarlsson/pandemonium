//! The index the server searches its sessions' conversations by: each session's title and the messages of its log, matched against the words of a query and ranked.

use pm_harness::Event;
use pm_protocol::{ChatField, ChatHit};

/// How many sessions a search answers when the client names no limit.
pub(crate) const DEFAULT_LIMIT: usize = 50;

/// How many characters of the text before a match a preview keeps.
const PREVIEW_BEFORE: usize = 40;

/// How many characters a preview holds at most.
const PREVIEW_LENGTH: usize = 140;

/// What a match in each field is worth before anything else counts.
const fn weight(field: ChatField) -> u32 {
    match field {
        ChatField::Title => 3000,
        ChatField::Prompt => 2000,
        ChatField::Reply => 1000,
    }
}

/// What finding the query whole, as typed, adds to a match.
const PHRASE_BONUS: u32 = 500;

/// What each word found at the start of a word adds to a match, at most `STARTS_CAP` in all.
const START_BONUS: u32 = 100;

/// The most that words found at their start add to a match.
const STARTS_CAP: u32 = 400;

/// One text of a session that can be found: its title or one of its messages.
struct Text {
    /// Where in the session it is.
    field: ChatField,
    /// The text, its runs of white space made single spaces, so that it reads as one line.
    line: String,
    /// `line` in lower case, which the words of a query are looked for in.
    lower: String,
}

impl Text {
    /// The text `raw` found in `field`.
    fn new(field: ChatField, raw: &str) -> Self {
        let line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        let lower = line.to_lowercase();
        Self { field, line, lower }
    }
}

/// One session as the index holds it.
pub(crate) struct Document {
    /// The session, by the server's number.
    session: u64,
    /// When it last changed, in milliseconds since the epoch, which settles a tie.
    updated: u64,
    /// Its title, then its messages in order.
    texts: Vec<Text>,
}

impl Document {
    /// The session numbered `session`, called `title`, last changed at `updated`, whose log is `log`: its title and every prompt and reply its conversation holds.
    pub(crate) fn new(session: u64, title: &str, updated: u64, log: &[(u64, Event)]) -> Self {
        let mut texts = vec![Text::new(ChatField::Title, title)];
        texts.extend(
            messages(log)
                .into_iter()
                .map(|(field, text)| Text::new(field, &text)),
        );
        Self {
            session,
            updated,
            texts,
        }
    }

    /// The best match of `query` in this session, when every one of its words is found in one of its texts.
    fn best(&self, query: &Query) -> Option<(u32, &Text, usize)> {
        self.texts
            .iter()
            .filter_map(|text| query.score(text).map(|(score, at)| (score, text, at)))
            .max_by_key(|(score, ..)| *score)
    }
}

/// The messages `log` holds, in order: each prompt the reader sent, and what the agent said after it as one reply.
fn messages(log: &[(u64, Event)]) -> Vec<(ChatField, String)> {
    let mut found: Vec<(ChatField, String)> = Vec::new();
    for (_, event) in log {
        match event {
            Event::Prompted { text, .. } => found.push((ChatField::Prompt, text.clone())),
            Event::Said(text) => match found.last_mut() {
                Some((ChatField::Reply, reply)) => reply.push_str(text),
                _ => found.push((ChatField::Reply, text.clone())),
            },
            _ => {}
        }
    }
    found.retain(|(_, text)| !text.trim().is_empty());
    found
}

/// A query broken into what is looked for.
struct Query {
    /// The whole query in lower case, its runs of white space made single spaces.
    phrase: String,
    /// Its words in lower case.
    words: Vec<String>,
}

impl Query {
    /// The query `typed`, or nothing when it has no word.
    fn new(typed: &str) -> Option<Self> {
        let words: Vec<String> = typed.split_whitespace().map(str::to_lowercase).collect();
        (!words.is_empty()).then(|| Self {
            phrase: words.join(" "),
            words,
        })
    }

    /// How well `text` matches, with the byte of its lower case line the match starts at, when every word is found in it.
    fn score(&self, text: &Text) -> Option<(u32, usize)> {
        let places: Vec<usize> = self
            .words
            .iter()
            .map(|word| text.lower.find(word.as_str()))
            .collect::<Option<_>>()?;
        let phrase = text.lower.find(self.phrase.as_str());
        let starts = self
            .words
            .iter()
            .filter(|word| starts_a_word(&text.lower, word))
            .count() as u32;
        let score = weight(text.field)
            + phrase.map_or(0, |_| PHRASE_BONUS)
            + (starts * START_BONUS).min(STARTS_CAP);
        let at = phrase.unwrap_or_else(|| places.into_iter().min().unwrap_or(0));
        Some((score, at))
    }
}

/// Whether `word` is found in `lower` where a word starts: at its start or after a character that is not a letter or a digit.
fn starts_a_word(lower: &str, word: &str) -> bool {
    lower.match_indices(word).any(|(at, _)| {
        lower[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !before.is_alphanumeric())
    })
}

/// The stretch of `text` around the match starting at byte `at` of its lower case line, on one line, with an ellipsis where it was cut.
fn preview(text: &Text, at: usize) -> String {
    let matched_char = text
        .lower
        .get(..at)
        .map_or(0, |before| before.chars().count());
    let first = matched_char.saturating_sub(PREVIEW_BEFORE);
    let total = text.line.chars().count();
    let kept: String = text.line.chars().skip(first).take(PREVIEW_LENGTH).collect();
    let lead = if first > 0 { "…" } else { "" };
    let tail = if first + PREVIEW_LENGTH < total {
        "…"
    } else {
        ""
    };
    format!("{lead}{}{tail}", kept.trim())
}

/// The sessions of `documents` that `typed` finds, best first and at most `limit` of them: each session once, by its best match, a newer session first where they match as well.
pub(crate) fn search(documents: &[Document], typed: &str, limit: usize) -> Vec<ChatHit> {
    let Some(query) = Query::new(typed) else {
        return Vec::new();
    };
    let mut found: Vec<(ChatHit, u64)> = documents
        .iter()
        .filter_map(|document| {
            let (score, text, at) = document.best(&query)?;
            Some((
                ChatHit {
                    session: document.session,
                    field: text.field,
                    preview: preview(text, at),
                    score,
                },
                document.updated,
            ))
        })
        .collect();
    found.sort_by(|(left, left_updated), (right, right_updated)| {
        right
            .score
            .cmp(&left.score)
            .then(right_updated.cmp(left_updated))
    });
    found.truncate(limit);
    found.into_iter().map(|(hit, _)| hit).collect()
}

/// The sessions `core` holds, closed ones included, that `typed` finds, best first and at most `limit` of them; nothing when the sessions cannot be looked at.
pub(crate) fn search_core(core: &crate::core::Core, typed: &str, limit: usize) -> Vec<ChatHit> {
    let documents: Vec<Document> = match core.sessions.lock() {
        Ok(sessions) => sessions
            .iter()
            .map(|(number, session)| {
                Document::new(
                    *number,
                    &session.meta.title,
                    session.meta.updated,
                    &session.log,
                )
            })
            .collect(),
        Err(_) => return Vec::new(),
    };
    search(&documents, typed, limit)
}

#[cfg(test)]
mod tests;
