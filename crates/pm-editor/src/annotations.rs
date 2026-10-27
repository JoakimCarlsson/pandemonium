//! What a language server says to show in and beside the text without being part of it: names coloured by what they are, hints written inside a line and lines of text kept above a stretch.

use std::ops::Range;

use pm_buffer::{Change, Position, Span};
use pm_protocol::{CodeLens, HintKind, InlayHint, SemanticToken, TokenKind};

use crate::document::Document;

/// What a server said for one version of the text: the version it is about, the version asked for last, and what it said.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Said<T> {
    /// The version of the text what is held is about, once the server answered.
    pub(crate) answered: Option<i32>,
    /// The version of the text the server was asked about last.
    pub(crate) asked: Option<i32>,
    /// What it said, kept in step with the text as it is edited.
    pub(crate) items: Vec<T>,
}

impl<T> Default for Said<T> {
    /// Nothing asked and nothing said.
    fn default() -> Self {
        Self {
            answered: None,
            asked: None,
            items: Vec::new(),
        }
    }
}

impl<T> Said<T> {
    /// Whether the server should be asked about the text at `version`: it was neither asked nor answered for it.
    pub(crate) fn wanted_at(&self, version: i32) -> bool {
        self.asked != Some(version) && self.answered != Some(version)
    }

    /// Takes in `items` as the answer about the text at `version`, when the text is still at that version and the answer is not older than the one held.
    fn take(&mut self, version: i32, current: i32, items: Vec<T>) -> bool {
        if version != current || self.answered.is_some_and(|held| held > version) {
            return false;
        }
        self.answered = Some(version);
        self.items = items;
        true
    }
}

/// A stretch of the text a server says is a name of a kind.
pub(crate) type Named = (Range<usize>, TokenKind);

/// Text a language server says to show inside a line that is not part of the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Hint {
    /// The byte of the text it stands before.
    pub offset: usize,
    /// What it shows.
    pub label: String,
    /// What it tells.
    pub kind: HintKind,
    /// Whether a space stands before it.
    pub pad_left: bool,
    /// Whether a space stands after it.
    pub pad_right: bool,
}

/// A line of text a language server says to show with a stretch of a line, such as how many times a function is used.
#[derive(Clone, Debug, PartialEq)]
pub struct Lens {
    /// The byte of the text the stretch it is about starts at.
    pub offset: usize,
    /// What it says.
    pub title: String,
    /// Whether pressing it does something.
    pub pressable: bool,
    /// Whether pressing it runs a command in a shell of the dock.
    pub runs: bool,
    /// Its place in the list the server last gave, which names it when it is pressed.
    pub index: usize,
}

/// What a language server said to show in and beside the text of a document.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Annotations {
    /// What each name is, in the order of the text.
    pub(crate) names: Said<Named>,
    /// The hints, in the order of the text.
    pub(crate) hints: Said<Hint>,
    /// The lenses, in the order of the text.
    pub(crate) lenses: Said<Lens>,
    /// Whether registries were allowed to be asked about dependencies when the lenses were last asked for.
    pub(crate) lenses_with_registry: bool,
    /// The values of names at the end of the lines, while a program is paused, in the order of the text.
    pub(crate) values: Vec<Hint>,
}

/// The place the byte `offset` moves to when `change` is made, or nothing when `change` replaced the text it stood in.
fn moved(offset: usize, change: &Change) -> Option<usize> {
    if offset < change.range.start {
        Some(offset)
    } else if offset >= change.range.end {
        Some(offset + change.text.len() - change.range.len())
    } else {
        None
    }
}

/// The stretch `range` after `change` is made, or nothing when `change` replaced part of it.
fn moved_range(range: &Range<usize>, change: &Change) -> Option<Range<usize>> {
    if range.end <= change.range.start {
        Some(range.clone())
    } else if range.start >= change.range.end {
        let end = range.end + change.text.len() - change.range.len();
        let start = range.start + change.text.len() - change.range.len();
        Some(start..end)
    } else {
        None
    }
}

impl Document {
    /// Keeps what the language server said in step with the text after `change` was made to it, letting go of what the change replaced.
    pub(crate) fn carry_annotations(&mut self, change: &Change) {
        let names = &mut self.annotations.names.items;
        names.retain_mut(|(range, _)| match moved_range(range, change) {
            Some(shifted) => {
                *range = shifted;
                true
            }
            None => false,
        });
        self.annotations.hints.items.retain_mut(|hint| {
            moved(hint.offset, change).is_some_and(|offset| {
                hint.offset = offset;
                true
            })
        });
        self.annotations.values.retain_mut(|hint| {
            moved(hint.offset, change).is_some_and(|offset| {
                hint.offset = offset;
                true
            })
        });
        self.annotations.lenses.items.retain_mut(|lens| {
            moved(lens.offset, change).is_some_and(|offset| {
                lens.offset = offset;
                true
            })
        });
        if self.annotations.names.answered.is_some() {
            self.repaint();
        }
    }

    /// Takes in `tokens`, what each name is as the language server said for the text at `version`.
    pub fn take_semantic_tokens(&mut self, version: i32, tokens: Vec<SemanticToken>) {
        let mut names: Vec<Named> = tokens
            .into_iter()
            .map(|token| {
                let span = Span::new(
                    Position::new(token.line, token.start),
                    Position::new(token.line, token.start + token.length),
                );
                (self.range_of(span), token.kind)
            })
            .filter(|(range, _)| !range.is_empty())
            .collect();
        names.sort_by_key(|(range, _)| range.start);
        if self.annotations.names.take(version, self.version, names) {
            self.repaint();
        }
    }

    /// Notes that the language server was asked what each name of the text at `version` is.
    pub fn semantic_tokens_asked(&mut self, version: i32) {
        self.annotations.names.asked = Some(version);
    }

    /// Whether the language server should be asked what each name of the text as it is now is.
    pub fn wants_semantic_tokens(&self) -> bool {
        self.synced
            && self.semantic_tokens_supported
            && self.annotations.names.wanted_at(self.version)
    }

    /// Lets go of what the language server said each name is, as when the reader turned semantic highlighting off.
    pub fn forget_semantic_tokens(&mut self) {
        if self.annotations.names.answered.is_some() || !self.annotations.names.items.is_empty() {
            self.annotations.names = Said::default();
            self.repaint();
        }
    }

    /// The names the language server said, as stretches of the text, that touch `window`, in order.
    pub(crate) fn names_in(&self, window: Range<usize>) -> &[Named] {
        let names = &self.annotations.names.items;
        let first = names.partition_point(|(range, _)| range.end <= window.start);
        let last = names.partition_point(|(range, _)| range.start < window.end);
        &names[first..last.max(first)]
    }

    /// Takes in `hints`, what to show inside the lines as the language server said for the text at `version`.
    pub fn take_inlay_hints(&mut self, version: i32, hints: Vec<InlayHint>) {
        let mut kept: Vec<Hint> = hints
            .into_iter()
            .map(|hint| Hint {
                offset: self.offset_of(hint.position),
                label: hint.label,
                kind: hint.kind,
                pad_left: hint.pad_left,
                pad_right: hint.pad_right,
            })
            .collect();
        kept.sort_by_key(|hint| hint.offset);
        self.annotations.hints.take(version, self.version, kept);
    }

    /// Notes that the language server was asked what to show inside the lines of the text at `version`.
    pub fn inlay_hints_asked(&mut self, version: i32) {
        self.annotations.hints.asked = Some(version);
    }

    /// Whether the language server should be asked what to show inside the lines of the text as it is now.
    pub fn wants_inlay_hints(&self) -> bool {
        self.synced && self.inlay_hints_supported && self.annotations.hints.wanted_at(self.version)
    }

    /// Lets go of the hints, as when the reader turned them off.
    pub fn forget_inlay_hints(&mut self) {
        self.annotations.hints = Said::default();
    }

    /// The span of the whole text, which is what hints are asked for.
    pub fn whole_span(&self) -> Span {
        Span::new(
            Position::new(0, 0),
            self.position_of(self.field.text().len()),
        )
    }

    /// The hints that stand in line `row`, in order.
    pub fn hints_on(&self, row: usize) -> &[Hint] {
        let line = self.line(row);
        let hints = &self.annotations.hints.items;
        let first = hints.partition_point(|hint| hint.offset < line.start);
        let last = hints.partition_point(|hint| hint.offset <= line.end);
        &hints[first..last.max(first)]
    }

    /// The text of the stretch `span` of the text.
    pub fn text_of(&self, span: Span) -> &str {
        &self.field.text()[self.range_of(span)]
    }

    /// The row of the text that the place `position` is in.
    pub fn row_of(&self, position: Position) -> usize {
        self.line_of(self.offset_of(position))
    }

    /// Shows `values`, a label for the end of each row, while a program is paused; an empty list takes them away.
    pub fn show_values(&mut self, values: Vec<(usize, String)>) {
        let hints: Vec<Hint> = values
            .into_iter()
            .map(|(row, label)| Hint {
                offset: self.line(row).end,
                label,
                kind: HintKind::Value,
                pad_left: true,
                pad_right: false,
            })
            .collect();
        if hints != self.annotations.values {
            self.annotations.values = hints;
        }
    }

    /// The values shown at the end of row `row`, in order.
    pub fn values_on(&self, row: usize) -> &[Hint] {
        let line = self.line(row);
        let values = &self.annotations.values;
        let first = values.partition_point(|hint| hint.offset < line.start);
        let last = values.partition_point(|hint| hint.offset <= line.end);
        &values[first..last.max(first)]
    }

    /// Takes in `lenses`, what to show with stretches of the text as the language server said for the text at `version`.
    pub fn take_code_lenses(&mut self, version: i32, lenses: Vec<CodeLens>) {
        let mut kept: Vec<Lens> = lenses
            .into_iter()
            .enumerate()
            .map(|(index, lens)| Lens {
                offset: self.offset_of(lens.span.start),
                title: lens.title,
                pressable: lens.pressable,
                runs: lens.runs,
                index,
            })
            .collect();
        let lines: Vec<(usize, String, bool)> = kept
            .iter()
            .map(|lens| {
                (
                    self.line_of(lens.offset),
                    lens.title.clone(),
                    lens.pressable,
                )
            })
            .collect();
        let mut at = 0;
        kept.retain(|_| {
            let (line, title, pressable) = &lines[at];
            at += 1;
            *pressable
                || !lines
                    .iter()
                    .any(|(other, said, press)| *press && other == line && said == title)
        });
        kept.sort_by_key(|lens| lens.offset);
        self.annotations.lenses.take(version, self.version, kept);
    }

    /// Notes that the server was asked what to show with stretches of the text at `version`, with registries allowed to be asked about dependencies or not.
    pub fn code_lenses_asked(&mut self, version: i32, registry: bool) {
        self.annotations.lenses.asked = Some(version);
        self.annotations.lenses_with_registry = registry;
    }

    /// Whether the server should be asked what to show with stretches of the text as it is now: it was not asked for this text, or registries were allowed or forbidden since.
    pub fn wants_code_lenses(&self, registry: bool) -> bool {
        self.synced
            && self.code_lens_supported
            && (self.annotations.lenses.wanted_at(self.version)
                || self.annotations.lenses_with_registry != registry)
    }

    /// Lets go of the lenses, as when the reader turned them off.
    pub fn forget_code_lenses(&mut self) {
        self.annotations.lenses = Said::default();
    }

    /// What the lens at `index` of the list the server gave says.
    pub fn lens_titled(&self, index: usize) -> Option<&str> {
        self.annotations
            .lenses
            .items
            .iter()
            .find(|lens| lens.index == index && lens.pressable)
            .map(|lens| lens.title.as_str())
    }

    /// Whether any lens is held, which gives the lines they stand above a row of their own.
    pub fn shows_lens_rows(&self) -> bool {
        !self.annotations.lenses.items.is_empty()
    }

    /// Whether the line at `index` has lenses, which stand in a row above it.
    pub fn has_lens_row(&self, index: usize) -> bool {
        self.shows_lens_rows() && !self.lenses_on(index).is_empty()
    }

    /// Whether pressing the lens at `index` of the list the server gave runs a command in a shell.
    pub fn lens_runs(&self, index: usize) -> bool {
        self.annotations
            .lenses
            .items
            .iter()
            .any(|lens| lens.index == index && lens.runs)
    }

    /// The lenses whose stretch starts in line `row`, in order.
    pub fn lenses_on(&self, row: usize) -> &[Lens] {
        let line = self.line(row);
        let lenses = &self.annotations.lenses.items;
        let first = lenses.partition_point(|lens| lens.offset < line.start);
        let last = lenses.partition_point(|lens| lens.offset <= line.end);
        &lenses[first..last.max(first)]
    }

    /// The byte of the text `position`, as a language server names it, stands at.
    fn offset_of(&self, position: Position) -> usize {
        self.range_of(Span::new(position, position)).start
    }
}
