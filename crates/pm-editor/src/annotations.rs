//! What a language server says to show in and beside the text without being part of it: names coloured by what they are, hints written inside a line and lines of text kept above a stretch.

use std::ops::Range;

use pm_buffer::{Change, Position, Span};
use pm_protocol::{
    CodeLens, ColorSwatch, DocumentLink, HintKind, InlayHint, LinkTarget, SemanticToken, TokenKind,
};

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
    /// The colour it shows as a small square instead of text, when it stands for one.
    pub swatch: Option<Swatch>,
}

/// A colour shown as a small square before the stretch that spells it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Swatch {
    /// How much red, green and blue, and how opaque, each from 0 to 255.
    pub color: [u8; 4],
    /// Its place in the list of colours of the file, which names it when it is pressed.
    pub index: usize,
}

/// A stretch of the text a language server says spells a colour, and the colour.
pub(crate) type Painted = (Range<usize>, [u8; 4]);

/// A stretch of the text a language server says leads somewhere when pressed.
pub(crate) type Linked = (Range<usize>, LinkTarget);

/// A line of text a language server says to show with a stretch of a line, such as how many times a function is used.
#[derive(Clone, Debug, PartialEq)]
pub struct Lens {
    /// The byte of the text the stretch it is about starts at.
    pub offset: usize,
    /// What it says.
    pub title: String,
    /// Whether pressing it does something.
    pub pressable: bool,
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
    /// The values of names at the end of the lines, while a program is paused, in the order of the text.
    pub(crate) values: Vec<Hint>,
    /// The stretches that lead somewhere when pressed, in the order of the text.
    pub(crate) links: Said<Linked>,
    /// The colours, in the order of the text.
    pub(crate) colors: Said<Painted>,
    /// The stretch of the text the names held come from alone, while the server has not said what every name of the file is: what the answers about stretches covered.
    pub(crate) covered: Option<Range<usize>>,
    /// The version of the text and the lines the server was last asked about alone, so the same stretch is not asked twice.
    pub(crate) range_asked: Option<(i32, Range<usize>)>,
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
        self.annotations
            .links
            .items
            .retain_mut(|(range, _)| match moved_range(range, change) {
                Some(shifted) => {
                    *range = shifted;
                    true
                }
                None => false,
            });
        self.annotations
            .colors
            .items
            .retain_mut(|(range, _)| match moved_range(range, change) {
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
        if let Some(covered) = &mut self.annotations.covered {
            let start = moved(covered.start, change).unwrap_or(change.range.start);
            let end = moved(covered.end, change).unwrap_or(change.range.start + change.text.len());
            *covered = start..end.max(start);
        }
        if self.annotations.names.answered.is_some() || self.annotations.covered.is_some() {
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
            self.annotations.covered = None;
            self.annotations.range_asked = None;
            self.repaint();
        }
    }

    /// Takes in `tokens`, what each name within the stretch `span` is as the language server said for the text at `version`, keeping the names held outside it; it is ignored once the server has said what every name of the file is.
    pub fn take_semantic_range(&mut self, version: i32, span: Span, tokens: Vec<SemanticToken>) {
        if version != self.version || self.annotations.names.answered.is_some() {
            return;
        }
        let stretch = self.range_of(span);
        let fresh: Vec<Named> = tokens
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
        let names = &mut self.annotations.names.items;
        names.retain(|(range, _)| range.end <= stretch.start || range.start >= stretch.end);
        names.extend(fresh);
        names.sort_by_key(|(range, _)| range.start);
        self.annotations.covered = Some(match self.annotations.covered.take() {
            Some(held) if held.start <= stretch.end && stretch.start <= held.end => {
                held.start.min(stretch.start)..held.end.max(stretch.end)
            }
            _ => stretch,
        });
        self.repaint();
    }

    /// The stretch to ask the language server about alone, `first` to `last` lines of the text, when it was not asked about it already, the names of the text as it is are not all held and the server says the names of a stretch.
    pub fn semantic_range_wanted(&self, lines: Range<usize>) -> Option<Span> {
        let annotations = &self.annotations;
        if !self.synced
            || !self.semantic_tokens_supported
            || !self.semantic_ranges_supported
            || annotations.names.answered.is_some()
            || self.lines() == 0
        {
            return None;
        }
        let last = lines.end.min(self.lines());
        let first = lines.start.min(last.saturating_sub(1));
        if first >= last {
            return None;
        }
        let bytes = self.line(first).start..self.line(last - 1).end;
        let held = annotations
            .covered
            .as_ref()
            .is_some_and(|covered| covered.start <= bytes.start && bytes.end <= covered.end);
        let asked = annotations
            .range_asked
            .as_ref()
            .is_some_and(|(version, asked)| {
                *version == self.version && asked.start <= first && last <= asked.end
            });
        if held || asked {
            return None;
        }
        Some(Span::new(
            Position::new(first as u32, 0),
            self.position_of(bytes.end),
        ))
    }

    /// Notes that the language server was asked what each name within the lines `lines` of the text at `version` is.
    pub fn semantic_range_asked(&mut self, version: i32, lines: Range<usize>) {
        self.annotations.range_asked = Some((version, lines));
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
            self.annotations.covered = None;
            self.annotations.range_asked = None;
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
                swatch: None,
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
                swatch: None,
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

    /// Takes in `links`, the stretches that lead somewhere when pressed as the language server said for the text at `version`.
    pub fn take_document_links(&mut self, version: i32, links: Vec<DocumentLink>) {
        let mut kept: Vec<Linked> = links
            .into_iter()
            .map(|link| (self.range_of(link.span), link.target))
            .filter(|(range, _)| !range.is_empty())
            .collect();
        kept.sort_by_key(|(range, _)| range.start);
        self.annotations.links.take(version, self.version, kept);
    }

    /// Notes that the language server was asked which stretches of the text at `version` lead somewhere.
    pub fn document_links_asked(&mut self, version: i32) {
        self.annotations.links.asked = Some(version);
    }

    /// Whether the language server should be asked which stretches of the text as it is now lead somewhere.
    pub fn wants_document_links(&self) -> bool {
        self.synced
            && self.document_links_supported
            && self.annotations.links.wanted_at(self.version)
    }

    /// Where the link the byte `offset` is in leads, with the stretch the link covers.
    pub fn link_at(&self, offset: usize) -> Option<(Range<usize>, &LinkTarget)> {
        let links = &self.annotations.links.items;
        let next = links.partition_point(|(range, _)| range.start <= offset);
        let (range, target) = links.get(next.checked_sub(1)?)?;
        (offset < range.end).then(|| (range.clone(), target))
    }

    /// Takes in `colors`, the stretches that spell a colour as the language server said for the text at `version`.
    pub fn take_colors(&mut self, version: i32, colors: Vec<ColorSwatch>) {
        let mut kept: Vec<Painted> = colors
            .into_iter()
            .map(|swatch| {
                (
                    self.range_of(swatch.span),
                    [swatch.red, swatch.green, swatch.blue, swatch.alpha],
                )
            })
            .filter(|(range, _)| !range.is_empty())
            .collect();
        kept.sort_by_key(|(range, _)| range.start);
        self.annotations.colors.take(version, self.version, kept);
    }

    /// Notes that the language server was asked which stretches of the text at `version` spell a colour.
    pub fn colors_asked(&mut self, version: i32) {
        self.annotations.colors.asked = Some(version);
    }

    /// Whether the language server should be asked which stretches of the text as it is now spell a colour.
    pub fn wants_colors(&self) -> bool {
        self.synced && self.colors_supported && self.annotations.colors.wanted_at(self.version)
    }

    /// The colours that stand in line `row`, each as a hint before its stretch, in order.
    pub fn colors_on(&self, row: usize) -> Vec<Hint> {
        let line = self.line(row);
        let colors = &self.annotations.colors.items;
        let first = colors.partition_point(|(range, _)| range.start < line.start);
        let last = colors.partition_point(|(range, _)| range.start <= line.end);
        colors[first..last.max(first)]
            .iter()
            .enumerate()
            .map(|(place, (range, color))| Hint {
                offset: range.start,
                label: String::new(),
                kind: HintKind::Other,
                pad_left: false,
                pad_right: true,
                swatch: Some(Swatch {
                    color: *color,
                    index: first + place,
                }),
            })
            .collect()
    }

    /// The stretch of the colour at `index` of the list of colours of the file, and the colour.
    pub fn color_at(&self, index: usize) -> Option<(Range<usize>, [u8; 4])> {
        self.annotations.colors.items.get(index).cloned()
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
                index,
            })
            .collect();
        kept.sort_by_key(|lens| lens.offset);
        self.annotations.lenses.take(version, self.version, kept);
    }

    /// Notes that the language server was asked what to show with stretches of the text at `version`.
    pub fn code_lenses_asked(&mut self, version: i32) {
        self.annotations.lenses.asked = Some(version);
    }

    /// Whether the language server should be asked what to show with stretches of the text as it is now.
    pub fn wants_code_lenses(&self) -> bool {
        self.synced && self.code_lens_supported && self.annotations.lenses.wanted_at(self.version)
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

    /// Whether any code lens is held for the file.
    pub fn has_code_lenses(&self) -> bool {
        !self.annotations.lenses.items.is_empty()
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
