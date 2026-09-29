//! Snippets: text with places in it to fill in, one after another.
//!
//! A server offers a completion as a snippet when what it inserts has parts
//! the reader is expected to write — the arguments of a call, the body of a
//! loop. The snippet is written in the protocol's own small language: `$1`
//! and `${1:default}` are the places, visited in the order of their numbers,
//! `$0` is where the cursor ends up, and a place that appears twice is one
//! place written in two spots at once. [`parse`] turns that into the text it
//! inserts and the places in it; the buffer then keeps the places in step
//! with the typing, and Tab moves on to the next one.

use std::collections::BTreeMap;
use std::ops::Range;

/// What a snippet inserts, and the places in it to fill in.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Snippet {
    /// The text it inserts, placeholders written out as their defaults.
    pub text: String,
    /// The places to visit, in order, each as every span of `text` it
    /// covers, counted in characters; where the cursor ends up comes last.
    pub stops: Vec<Vec<Range<usize>>>,
}

/// What `written` inserts and where its places are.
///
/// Anything the snippet language does not describe is taken as it is
/// written: a `$` with no number or name after it is a dollar sign.
/// Variables — `$TM_FILENAME` and the like — come out as their default or
/// as nothing, since the editor has no values of its own to put there.
pub fn parse(written: &str) -> Snippet {
    let chars = written.chars().collect::<Vec<_>>();
    let mut parser = Parser {
        chars: &chars,
        at: 0,
        text: String::new(),
        length: 0,
        places: BTreeMap::new(),
    };
    parser.body(false);
    let end = parser.length;
    let mut places = parser.places;
    let last = places
        .remove(&0)
        .unwrap_or_else(|| std::iter::once(end..end).collect());
    let mut stops = places.into_values().collect::<Vec<_>>();
    stops.push(last);
    Snippet {
        text: parser.text,
        stops,
    }
}

/// A walk over a snippet's characters, writing out what it inserts.
struct Parser<'a> {
    /// The snippet, as characters.
    chars: &'a [char],
    /// How far into it the walk has come.
    at: usize,
    /// What it inserts, so far.
    text: String,
    /// How many characters that is.
    length: usize,
    /// Every span each numbered place covers, by its number.
    places: BTreeMap<usize, Vec<Range<usize>>>,
}

impl Parser<'_> {
    /// Walks text until it ends, or until the `}` that closes a placeholder
    /// when `nested`.
    fn body(&mut self, nested: bool) {
        while let Some(&ch) = self.chars.get(self.at) {
            match ch {
                '\\' if self
                    .chars
                    .get(self.at + 1)
                    .is_some_and(|next| "$}\\".contains(*next)) =>
                {
                    self.push(self.chars[self.at + 1]);
                    self.at += 2;
                }
                '}' if nested => return,
                '$' => self.dollar(),
                ch => {
                    self.push(ch);
                    self.at += 1;
                }
            }
        }
    }

    /// Reads what follows a `$`: a place, a placeholder, a choice, a variable
    /// or nothing at all.
    fn dollar(&mut self) {
        let start = self.at;
        self.at += 1;
        if let Some(number) = self.number() {
            return self.place(number, self.length..self.length);
        }
        if self.chars.get(self.at) != Some(&'{') {
            if self.name().is_none() {
                self.at = start + 1;
                self.push('$');
            }
            return;
        }
        self.at += 1;
        if let Some(number) = self.number() {
            let begun = self.length;
            match self.chars.get(self.at) {
                Some(':') => {
                    self.at += 1;
                    self.body(true);
                }
                Some('|') => {
                    self.at += 1;
                    self.choice();
                }
                _ => {}
            }
            self.close();
            return self.place(number, begun..self.length);
        }
        if self.name().is_some() {
            if self.chars.get(self.at) == Some(&':') {
                self.at += 1;
                self.body(true);
            } else {
                self.skip_to_close();
            }
            self.close();
            return;
        }
        self.at = start + 1;
        self.push('$');
    }

    /// Writes out the first option of a choice, `${1|one,two|}`, and passes
    /// over the rest.
    fn choice(&mut self) {
        let mut first = true;
        while let Some(&ch) = self.chars.get(self.at) {
            self.at += 1;
            match ch {
                '|' => return,
                ',' => first = false,
                '\\' => {
                    if let Some(&escaped) = self.chars.get(self.at) {
                        self.at += 1;
                        if first {
                            self.push(escaped);
                        }
                    }
                }
                ch if first => self.push(ch),
                _ => {}
            }
        }
    }

    /// Takes down that place `number` covers `span`.
    fn place(&mut self, number: usize, span: Range<usize>) {
        self.places.entry(number).or_default().push(span);
    }

    /// Passes over the `}` that closes a placeholder, if it is there.
    fn close(&mut self) {
        if self.chars.get(self.at) == Some(&'}') {
            self.at += 1;
        }
    }

    /// Passes over whatever a variable's transform says, up to its `}`.
    fn skip_to_close(&mut self) {
        while self.chars.get(self.at).is_some_and(|ch| *ch != '}') {
            self.at += 1;
        }
    }

    /// The number written here, if one is.
    fn number(&mut self) -> Option<usize> {
        let start = self.at;
        while self.chars.get(self.at).is_some_and(char::is_ascii_digit) {
            self.at += 1;
        }
        (self.at > start).then(|| {
            self.chars[start..self.at]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or_default()
        })
    }

    /// The variable name written here, if one is.
    fn name(&mut self) -> Option<()> {
        let start = self.at;
        if !self
            .chars
            .get(self.at)
            .is_some_and(|ch| ch.is_ascii_alphabetic() || *ch == '_')
        {
            return None;
        }
        while self
            .chars
            .get(self.at)
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        {
            self.at += 1;
        }
        (self.at > start).then_some(())
    }

    /// Writes out one character of what the snippet inserts.
    fn push(&mut self, ch: char) {
        self.text.push(ch);
        self.length += 1;
    }
}
