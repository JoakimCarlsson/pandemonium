//! Snippets: text with blanks to fill in, written out at the caret, after which Tab moves from blank to blank.
//!
//! A snippet body holds `$1`, `${1}` and `${1:default}` for the blanks, in the order Tab visits them, the
//! same number standing for blanks that are filled in together, `$0` for where the caret ends up,
//! `${1|one,two|}` for a blank that starts as its first choice, and `\$`, `\}` and `\\` for those
//! characters themselves. Words the editor has no value for, such as `$TM_FILENAME`, are written as their default or nothing.

use std::collections::HashMap;
use std::ops::Range;

use pm_text::Edit;
use serde::{Deserialize, Deserializer, Serialize};

use crate::document::Document;
use crate::emmet::{self, Dialect};
use crate::settings::EditorSettings;

/// A snippet the reader wrote: the word that names it and what it writes out.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UserSnippet {
    /// The word that, typed before the caret with Tab pressed, writes the snippet out.
    pub prefix: String,
    /// The snippet, a line to each item, or one string.
    #[serde(deserialize_with = "lines_of")]
    pub body: Vec<String>,
    /// What it is for.
    #[serde(default)]
    pub description: String,
}

/// A body written as one string or as a list of lines.
#[derive(Deserialize)]
#[serde(untagged)]
enum Body {
    /// One string.
    One(String),
    /// A line to each item.
    Many(Vec<String>),
}

/// The lines of a snippet body that is a string or a list of strings.
fn lines_of<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    Ok(match Body::deserialize(deserializer)? {
        Body::One(text) => text.lines().map(str::to_owned).collect(),
        Body::Many(lines) => lines,
    })
}

/// The blanks of one number: where each is in the text.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stop {
    /// The number the blanks are visited by; 0 is the end.
    pub(crate) number: u32,
    /// Where each blank is, as bytes of the text.
    pub(crate) ranges: Vec<Range<usize>>,
}

/// A snippet written out, its blanks counted in bytes from the start of its text.
#[derive(Debug, PartialEq)]
pub(crate) struct Layout {
    /// What is written.
    pub(crate) text: String,
    /// The blanks in the order they are visited, the end last.
    pub(crate) stops: Vec<Stop>,
}

/// A snippet being filled in: its blanks as bytes of the document, and the one the caret is in.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Session {
    /// The blanks in the order they are visited, the end last.
    pub(crate) stops: Vec<Stop>,
    /// The blank the caret is in, by its place in `stops`.
    pub(crate) current: usize,
}

/// A snippet to be written out: the edit that writes it, and the blanks to be filled in once it is written.
#[derive(Clone, Debug, PartialEq)]
pub struct Expansion {
    /// The edit that writes the snippet, selecting its first blank.
    pub edit: Edit,
    /// The blanks, when there are any besides the end.
    session: Option<Session>,
}

/// What the parser has gathered while reading a body.
struct Reading<'a> {
    /// The text written so far.
    text: String,
    /// Each blank as its number and where it is.
    found: Vec<(u32, Range<usize>)>,
    /// What each numbered blank was first written with, which the blanks of the same number are written with too.
    defaults: HashMap<u32, String>,
    /// The characters not read yet.
    rest: std::iter::Peekable<std::str::Chars<'a>>,
}

impl Reading<'_> {
    /// Reads digits from the rest of the body as a number.
    fn number(&mut self) -> Option<u32> {
        let mut digits = String::new();
        while let Some(digit) = self.rest.next_if(char::is_ascii_digit) {
            digits.push(digit);
        }
        digits.parse().ok()
    }

    /// Reads a name of letters, digits and underscores from the rest of the body.
    fn name(&mut self) -> String {
        let mut name = String::new();
        while let Some(letter) = self
            .rest
            .next_if(|held| held.is_alphanumeric() || *held == '_')
        {
            name.push(letter);
        }
        name
    }

    /// Reads text up to the `}` that closes the blank being read, the text of nested blanks written as their defaults; the `}` is read too.
    fn until_close(&mut self) -> String {
        let mut inner = Reading {
            text: String::new(),
            found: Vec::new(),
            defaults: self.defaults.clone(),
            rest: std::mem::replace(&mut self.rest, "".chars().peekable()),
        };
        inner.read(true);
        self.rest = inner.rest;
        inner.text
    }

    /// Reads the rest of the body, or up to the `}` that ends a default when `inside`, writing the blanks it finds.
    fn read(&mut self, inside: bool) {
        while let Some(character) = self.rest.next() {
            match character {
                '\\' => {
                    if let Some(escaped) = self.rest.next() {
                        self.text.push(escaped);
                    }
                }
                '}' if inside => return,
                '$' => self.dollar(inside),
                other => self.text.push(other),
            }
        }
    }

    /// Reads what follows a `$`: a blank, a choice, a name or the `$` itself.
    fn dollar(&mut self, inside: bool) {
        match self.rest.peek().copied() {
            Some(digit) if digit.is_ascii_digit() => {
                if let Some(number) = self.number() {
                    self.blank(number, None, inside);
                }
            }
            Some('{') => {
                self.rest.next();
                self.braced(inside);
            }
            Some(letter) if letter.is_alphabetic() || letter == '_' => {
                self.name();
            }
            _ => self.text.push('$'),
        }
    }

    /// Reads the inside of `${...}` and the `}` after it: a blank with or without a default or choices, or a name with a default.
    fn braced(&mut self, inside: bool) {
        match self.rest.peek().copied() {
            Some(digit) if digit.is_ascii_digit() => {
                let Some(number) = self.number() else {
                    return;
                };
                match self.rest.peek().copied() {
                    Some(':') => {
                        self.rest.next();
                        let text = self.until_close();
                        self.blank(number, Some(text), inside);
                    }
                    Some('|') => {
                        self.rest.next();
                        let mut choices = String::new();
                        for held in self.rest.by_ref() {
                            if held == '|' {
                                break;
                            }
                            choices.push(held);
                        }
                        self.rest.next_if_eq(&'}');
                        let first = choices.split(',').next().unwrap_or_default().to_owned();
                        self.blank(number, Some(first), inside);
                    }
                    _ => {
                        self.rest.next_if_eq(&'}');
                        self.blank(number, None, inside);
                    }
                }
            }
            _ => {
                self.name();
                if self.rest.next_if_eq(&':').is_some() {
                    let text = self.until_close();
                    self.text.push_str(&text);
                } else {
                    self.rest.next_if_eq(&'}');
                }
            }
        }
    }

    /// Writes a blank numbered `number`, with `default` the first time it is written or what it was first written with after that.
    fn blank(&mut self, number: u32, default: Option<String>, inside: bool) {
        let text = match (self.defaults.get(&number), default) {
            (Some(held), _) => held.clone(),
            (None, Some(given)) => {
                self.defaults.insert(number, given.clone());
                given
            }
            (None, None) => String::new(),
        };
        let start = self.text.len();
        self.text.push_str(&text);
        if !inside {
            self.found.push((number, start..self.text.len()));
        }
    }
}

/// The text `body` writes and where its blanks are, the end made explicit as a blank 0 where `$0` was not written.
pub(crate) fn layout(body: &str) -> Layout {
    let mut reading = Reading {
        text: String::new(),
        found: Vec::new(),
        defaults: HashMap::new(),
        rest: body.chars().peekable(),
    };
    reading.read(false);
    let end = reading.text.len();
    let mut numbers: Vec<u32> = reading
        .found
        .iter()
        .map(|(number, _)| *number)
        .filter(|number| *number != 0)
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    let mut stops: Vec<Stop> = numbers
        .into_iter()
        .map(|number| Stop {
            number,
            ranges: reading
                .found
                .iter()
                .filter(|(held, _)| *held == number)
                .map(|(_, range)| range.clone())
                .collect(),
        })
        .collect();
    let finish: Vec<Range<usize>> = reading
        .found
        .iter()
        .filter(|(held, _)| *held == 0)
        .map(|(_, range)| range.clone())
        .collect();
    let mut ranges = finish;
    if ranges.is_empty() {
        ranges.push(Range { start: end, end });
    }
    stops.push(Stop { number: 0, ranges });
    Layout {
        text: reading.text,
        stops,
    }
}

/// The lines of `body` after the first started with `indentation`, and each tab written as `unit`.
fn indented(body: &str, indentation: &str, unit: &str) -> String {
    body.replace('\t', unit)
        .replace('\n', &format!("\n{indentation}"))
}

/// Whether `character` can be part of the name of a snippet written as a word.
fn is_prefix_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | '!' | '#' | '@')
}

/// The HTML tags a plain word before the caret is taken to be an abbreviation for.
const TAGS: [&str; 62] = [
    "a",
    "abbr",
    "article",
    "aside",
    "audio",
    "b",
    "blockquote",
    "body",
    "br",
    "button",
    "canvas",
    "code",
    "dd",
    "details",
    "div",
    "dl",
    "dt",
    "em",
    "fieldset",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "i",
    "iframe",
    "img",
    "input",
    "label",
    "legend",
    "li",
    "link",
    "main",
    "meta",
    "nav",
    "ol",
    "option",
    "p",
    "pre",
    "section",
    "select",
    "small",
    "span",
    "strong",
    "style",
    "script",
    "table",
    "td",
    "textarea",
    "th",
    "tr",
    "ul",
    "video",
    "title",
];

/// What the language `language` an Emmet abbreviation is written in, when it takes them.
fn dialect_of(language: &str) -> Option<Dialect> {
    match language {
        "html" | "svelte" | "tsx" | "xml" => Some(Dialect::Markup),
        "css" | "scss" => Some(Dialect::Style),
        _ => None,
    }
}

impl Document {
    /// The snippets the reader wrote that apply to this document, the language's own ahead of those for every language.
    fn own_snippets<'a>(&self, settings: &'a EditorSettings) -> Vec<&'a UserSnippet> {
        let own = self
            .language
            .as_ref()
            .and_then(|language| settings.snippets.get(language));
        own.into_iter()
            .chain(settings.snippets.get("*"))
            .flatten()
            .collect()
    }

    /// The snippet `body` written over the bytes `range`, indented as the line `range` is on is.
    pub(crate) fn snippet_edit(&self, range: Range<usize>, body: &str) -> Option<Expansion> {
        let indentation = self.indentation_at(range.start);
        let unit = self.editing.unit(1);
        let written = layout(&indented(body, &indentation, &unit));
        let base = range.start;
        let stops: Vec<Stop> = written
            .stops
            .iter()
            .map(|stop| Stop {
                number: stop.number,
                ranges: stop
                    .ranges
                    .iter()
                    .map(|held| base + held.start..base + held.end)
                    .collect(),
            })
            .collect();
        let first = stops.first()?.ranges.first()?.clone();
        let session = (stops.len() > 1).then_some(Session { stops, current: 0 });
        Some(Expansion {
            edit: Edit::Replace(range, written.text, (first.start, first.end)),
            session,
        })
    }

    /// The snippet that the word before the caret names, or the Emmet abbreviation written before it; nothing when Tab should indent instead.
    pub fn expansion(&self, settings: &EditorSettings) -> Option<Expansion> {
        if self.field.is_read_only() || self.field.selection().is_some() || self.field.has_others()
        {
            return None;
        }
        let caret = self.field.caret();
        let line = self.line(self.line_of(caret));
        let before = &self.field.text()[line.start..caret];
        if settings.snippets_on_tab
            && let Some((range, body)) = self.named_snippet(settings, line.start, before)
        {
            return self.snippet_edit(range, &body);
        }
        if settings.emmet
            && let Some((range, body)) = self.abbreviation(line.start, before)
        {
            return self.snippet_edit(range, &body);
        }
        None
    }

    /// The bytes of `before`, the text of a line from its start at `line_start` to the caret, that a snippet's name is written as, and the body of the snippet.
    fn named_snippet(
        &self,
        settings: &EditorSettings,
        line_start: usize,
        before: &str,
    ) -> Option<(Range<usize>, String)> {
        let snippets = self.own_snippets(settings);
        let word_start = before
            .char_indices()
            .rev()
            .take_while(|(_, character)| is_prefix_character(*character))
            .last()
            .map_or(before.len(), |(at, _)| at);
        (word_start..before.len())
            .filter(|at| before.is_char_boundary(*at))
            .find_map(|at| {
                let typed = &before[at..];
                let found = snippets.iter().find(|held| held.prefix == typed)?;
                Some((
                    line_start + at..line_start + before.len(),
                    found.body.join("\n"),
                ))
            })
    }

    /// The bytes of `before`, the text of a line from its start at `line_start` to the caret, that an Emmet abbreviation is written as, and what it writes out.
    fn abbreviation(&self, line_start: usize, before: &str) -> Option<(Range<usize>, String)> {
        let dialect = dialect_of(self.language.as_deref()?)?;
        let unit = self.editing.unit(1);
        let trimmed = before.trim_start();
        let begin = before.len() - trimmed.len();
        let after_space = trimmed.rfind(char::is_whitespace).map_or(0, |at| {
            at + trimmed[at..].chars().next().map_or(1, char::len_utf8)
        });
        [0, after_space].into_iter().find_map(|skip| {
            let candidate = &trimmed[skip..];
            let plain = candidate
                .chars()
                .all(|character| character.is_alphanumeric());
            let tag = dialect != Dialect::Markup || !plain || TAGS.contains(&candidate);
            if candidate.is_empty() || candidate.contains('<') || !tag {
                return None;
            }
            let expanded = emmet::expand(candidate, dialect, &unit)?;
            let start = line_start + begin + skip;
            Some((start..line_start + before.len(), expanded))
        })
    }

    /// Whether the blanks of a snippet are being filled in.
    pub fn snippet_active(&self) -> bool {
        self.snippet.is_some()
    }

    /// Starts filling in the blanks of `expansion`, which was written out and has its first blank selected.
    pub fn begin_snippet(&mut self, expansion: Expansion) {
        let Some(session) = expansion.session else {
            return;
        };
        self.linked = Some(session.stops[0].ranges.clone());
        self.snippet = Some(session);
    }

    /// Stops filling in the blanks of a snippet, leaving what was written.
    pub fn end_snippet(&mut self) {
        self.snippet = None;
        self.linked = None;
    }

    /// The edit that moves the caret to the next blank of the snippet, or to the previous one when not `forward`; the end of the snippet ends it. Nothing when none is being filled in.
    pub fn snippet_step(&mut self, forward: bool) -> Option<Edit> {
        let session = self.snippet.as_mut()?;
        if let Some(linked) = &self.linked {
            session.stops[session.current].ranges = linked.clone();
        }
        let target = if forward {
            (session.current + 1).min(session.stops.len() - 1)
        } else {
            session.current.saturating_sub(1)
        };
        session.current = target;
        let stop = session.stops[target].clone();
        let first = stop.ranges.first()?.clone();
        if stop.number == 0 {
            self.snippet = None;
            self.linked = None;
        } else {
            self.linked = Some(stop.ranges);
        }
        Some(Edit::Select(first.start, first.end))
    }

    /// Moves the blanks other than the one being filled in after the text of the blanks filled in together grew by `growth` bytes in each of them, they having stood at `before`.
    pub(crate) fn carry_blanks(&mut self, before: &[Range<usize>], growth: isize) {
        let Some(session) = &mut self.snippet else {
            return;
        };
        let current = session.current;
        let number = session.stops[current].number;
        for (place, stop) in session.stops.iter_mut().enumerate() {
            if place == current {
                continue;
            }
            for range in &mut stop.ranges {
                let passed = before
                    .iter()
                    .filter(|held| {
                        held.start < range.start
                            || (held.start == range.start && number < stop.number)
                    })
                    .count() as isize;
                let shift = growth * passed;
                range.start = (range.start as isize + shift).max(0) as usize;
                range.end = (range.end as isize + shift).max(0) as usize;
            }
        }
    }
}
