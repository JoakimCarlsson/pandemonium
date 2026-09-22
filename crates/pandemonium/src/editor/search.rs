//! Finding text in one open file, and putting something else in its place.
//!
//! The search belongs to the document rather than to the pane: a file looked
//! for something in and then shown in a second pane is still being searched,
//! and the matches are the same in both. What the bar over the pane holds is
//! only how this is drawn.

use std::ops::Range;

use pm_text::{Buffer, Position};

use crate::field::Field;

/// Which of the search bar's two fields the keyboard is going to.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SearchField {
    /// The text being looked for.
    #[default]
    Query,
    /// The text it is being replaced with.
    Replacement,
}

/// What is being looked for in one file, and where it was found.
#[derive(Default)]
pub struct Search {
    /// The text being looked for.
    query: Field,
    /// The text it is replaced with.
    replacement: Field,
    /// Whether an upper-case letter in the query has to match one in the text.
    case_sensitive: bool,
    /// Whether a match has to be a whole word.
    whole_word: bool,
    /// Whether the bar is open over the pane.
    open: bool,
    /// Whether the bar is showing its replacement field.
    replacing: bool,
    /// Which field the keyboard is going to.
    field: SearchField,
    /// Every match, in the order they appear in the file.
    matches: Vec<Range<Position>>,
    /// Which of them is the one being looked at.
    current: Option<usize>,
}

impl Search {
    /// Whether the bar is open over the pane.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether the bar is showing its replacement field.
    pub fn is_replacing(&self) -> bool {
        self.replacing
    }

    /// Which field the keyboard is going to.
    pub fn field(&self) -> SearchField {
        self.field
    }

    /// Sends later keystrokes to `field`.
    pub fn focus(&mut self, field: SearchField) {
        self.field = field;
        if field == SearchField::Replacement {
            self.replacing = true;
        }
    }

    /// The field holding what is being looked for.
    pub fn query(&self) -> &Field {
        &self.query
    }

    /// The field holding what it is replaced with.
    pub fn replacement(&self) -> &Field {
        &self.replacement
    }

    /// Puts the caret `caret` characters into `field`.
    pub fn place(&mut self, field: SearchField, caret: usize) {
        self.focus(field);
        match field {
            SearchField::Query => self.query.place(caret),
            SearchField::Replacement => self.replacement.place(caret),
        }
    }

    /// Whether an upper-case letter in the query has to match one in the text.
    pub fn is_case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Whether a match has to be a whole word.
    pub fn is_whole_word(&self) -> bool {
        self.whole_word
    }

    /// Every match, in the order they appear in the file.
    pub fn matches(&self) -> &[Range<Position>] {
        &self.matches
    }

    /// Which match is the one being looked at, of how many there are.
    pub fn standing(&self) -> Option<(usize, usize)> {
        Some((self.current? + 1, self.matches.len()))
    }

    /// The match being looked at, if the query has found anything.
    pub fn current(&self) -> Option<Range<Position>> {
        self.matches.get(self.current?).cloned()
    }

    /// Whether `range` is the match being looked at.
    pub fn is_current(&self, range: &Range<Position>) -> bool {
        self.current() == Some(range.clone())
    }

    /// Opens the bar, taking `seeded` as the query when there is one.
    pub fn open(&mut self, seeded: Option<String>, replacing: bool, buffer: &Buffer) {
        self.open = true;
        self.replacing = replacing || self.replacing;
        self.field = SearchField::Query;
        if let Some(seeded) = seeded.filter(|text| !text.is_empty() && !text.contains('\n')) {
            self.query.set(seeded);
        }
        self.refresh(buffer);
    }

    /// Closes the bar, leaving what was being looked for for the next time.
    pub fn close(&mut self) {
        self.open = false;
        self.matches.clear();
        self.current = None;
    }

    /// Puts whatever the keyboard is aimed at through `edit`.
    pub fn edit_field(&mut self, edit: impl FnOnce(&mut Field), buffer: &Buffer) {
        match self.field {
            SearchField::Query => {
                edit(&mut self.query);
                self.refresh(buffer);
            }
            SearchField::Replacement => edit(&mut self.replacement),
        }
    }

    /// Turns matching upper case against upper case on or off.
    pub fn toggle_case(&mut self, buffer: &Buffer) {
        self.case_sensitive = !self.case_sensitive;
        self.refresh(buffer);
    }

    /// Turns matching whole words only on or off.
    pub fn toggle_whole_word(&mut self, buffer: &Buffer) {
        self.whole_word = !self.whole_word;
        self.refresh(buffer);
    }

    /// Shows or hides the replacement field.
    pub fn toggle_replacing(&mut self) {
        self.replacing = !self.replacing;
        if !self.replacing {
            self.field = SearchField::Query;
        }
    }

    /// Finds every match again, keeping the one nearest the cursor current.
    pub fn refresh(&mut self, buffer: &Buffer) {
        let head = buffer.selection().start();
        self.matches = self.find_all(buffer);
        self.current = self
            .matches
            .iter()
            .position(|found| found.start >= head)
            .or(if self.matches.is_empty() {
                None
            } else {
                Some(0)
            });
    }

    /// Moves to the match after the one being looked at, wrapping around.
    pub fn next(&mut self) -> Option<Range<Position>> {
        self.step(1)
    }

    /// Moves to the match before the one being looked at, wrapping around.
    pub fn previous(&mut self) -> Option<Range<Position>> {
        self.step(-1)
    }

    /// Moves `step` matches along, wrapping around at either end.
    fn step(&mut self, step: isize) -> Option<Range<Position>> {
        let count = self.matches.len();
        if count == 0 {
            return None;
        }
        let current = self.current.unwrap_or(0) as isize;
        self.current = Some((current + step).rem_euclid(count as isize) as usize);
        self.current()
    }

    /// Every place in `buffer` the query is found.
    ///
    /// The file is walked a line at a time: a query with a line break in it
    /// is not something this bar looks for, and a line is short enough that
    /// searching it is a scan rather than an index.
    fn find_all(&self, buffer: &Buffer) -> Vec<Range<Position>> {
        if self.query.is_empty() {
            return Vec::new();
        }
        let needle = self.folded(self.query.value());
        let mut found = Vec::new();

        for line in 0..buffer.line_count() {
            let text = self.folded(&buffer.line_text(line));
            let chars = text.chars().collect::<Vec<_>>();
            let pattern = needle.chars().collect::<Vec<_>>();
            if pattern.is_empty() || pattern.len() > chars.len() {
                continue;
            }
            for start in 0..=chars.len() - pattern.len() {
                if chars[start..start + pattern.len()] != pattern[..] {
                    continue;
                }
                let end = start + pattern.len();
                if self.whole_word && !is_word_bounded(&chars, start, end) {
                    continue;
                }
                found.push(Position::new(line, start)..Position::new(line, end));
            }
        }
        found
    }

    /// `text` in the case matching compares in.
    fn folded(&self, text: &str) -> String {
        if self.case_sensitive {
            text.to_owned()
        } else {
            text.to_lowercase()
        }
    }
}

/// Whether the run from `start` to `end` of `chars` is a whole word.
fn is_word_bounded(chars: &[char], start: usize, end: usize) -> bool {
    let word = |ch: &char| ch.is_alphanumeric() || *ch == '_';
    !start
        .checked_sub(1)
        .and_then(|before| chars.get(before))
        .is_some_and(word)
        && !chars.get(end).is_some_and(word)
}
