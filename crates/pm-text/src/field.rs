//! A single line of editable text, the caret in it and what is selected.

use std::ops::Range;

use crate::reach::Reach;
use crate::word::Class;

/// Where a caret moves to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motion {
    /// One character left.
    Left,
    /// One character right.
    Right,
    /// To the start of the word before the caret.
    WordLeft,
    /// To the end of the word after the caret.
    WordRight,
    /// To the start of the line.
    Home,
    /// To the end of the line.
    End,
    /// To the start of the line of a multiline field the caret is on.
    LineStart,
    /// To the end of the line of a multiline field the caret is on.
    LineEnd,
    /// To the line above, as many characters in as the caret is.
    LineUp,
    /// To the line below, as many characters in as the caret is.
    LineDown,
}

/// One change to make to a [`Field`].
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// Type a character, replacing what is selected.
    Insert(char),
    /// Insert text, replacing what is selected; control characters are dropped.
    Paste(String),
    /// Delete what is selected, or else the character before the caret.
    Backspace,
    /// Delete what is selected, or else the character after the caret.
    Delete,
    /// Delete what is selected, or else the word before the caret.
    BackspaceWord,
    /// Delete what is selected, or else the word after the caret.
    DeleteWord,
    /// Move the caret, extending the selection when the flag is set and
    /// dropping it otherwise.
    Move(Motion, bool),
    /// Select the whole line.
    SelectAll,
    /// Put the caret at a byte offset of what is shown, extending the
    /// selection when the flag is set and dropping it otherwise.
    Place(usize, bool),
    /// Take back the last change.
    Undo,
    /// Make again the change that was taken back.
    Redo,
    /// Scroll the field by this many logical pixels: vertically for multiline
    /// inputs, horizontally otherwise; negative offsets are taken as none.
    ScrollTo(f32),
    /// Replace the bytes of the text in the range with the text, as one
    /// change, then select from the first offset of the new text to the
    /// second; control characters are dropped as when pasting.
    Replace(std::ops::Range<usize>, String, (usize, usize)),
    /// Select from the first byte offset of the text to the second, the
    /// caret at the second.
    Select(usize, usize),
    /// Keep what is selected and select from the first byte offset of the
    /// text to the second as well, the new selection the one the caret is
    /// in; edits are then made at every selection.
    AddSelection(usize, usize),
    /// Keep only the selection the caret is in.
    OnlySelection,
    /// Show text an input method is still composing at the caret, such as
    /// the accent of a dead key waiting for its letter, or stop showing it
    /// when empty. The text is not typed until the input method commits it.
    Compose(String),
}

/// Text with a caret between two of its characters and, when the
/// reader has selected some, the other end of the selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Field {
    /// The text being edited, which holds no control characters but line breaks and tabs, and those only in a multiline field.
    text: String,
    /// Counts changes to text without scanning or copying it to detect edits.
    revision: u64,
    /// The caret, as a byte offset on a character boundary.
    caret: usize,
    /// The end of the selection that does not move, as a byte offset on a
    /// character boundary; equal to the caret when nothing is selected.
    anchor: usize,
    /// The scroll offset in logical pixels: vertical for multiline inputs, horizontal otherwise.
    scroll: f32,
    /// Whether the next multiline layout should bring the caret into view.
    follow_caret: bool,
    /// How many characters the line may hold, if it is limited.
    limit: Option<usize>,
    /// Whether the line is shown as dots and cannot be copied from.
    masked: bool,
    /// Whether the line can be selected and copied but not changed.
    read_only: bool,
    /// Whether the text may hold line breaks, and so is more than one line.
    multiline: bool,
    /// The states the line was in before each change, oldest first.
    past: Vec<Snapshot>,
    /// The states that were taken back, most recently taken back last.
    future: Vec<Snapshot>,
    /// Whether the last change was typing, which the next typing joins.
    typing: bool,
    /// Text an input method is composing at the caret: shown, but not part
    /// of the text until it is committed.
    composing: String,
    /// The other selections, besides the one the caret is in, each as the
    /// end that does not move and the end that does.
    others: Vec<(usize, usize)>,
}

/// What a field held at one moment, for undoing back to it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    /// The line.
    text: String,
    /// The caret.
    caret: usize,
    /// The other end of the selection.
    anchor: usize,
    /// The other selections.
    others: Vec<(usize, usize)>,
}

/// What a masked line shows for each of its characters.
const MASK: char = '•';

/// How many changes undo remembers.
const HISTORY: usize = 200;

impl Field {
    /// A field holding `text`, with the caret at its end and nothing selected.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            follow_caret: true,
            caret: text.len(),
            anchor: text.len(),
            text,
            ..Self::default()
        }
    }

    /// The field limited to `characters` characters.
    pub fn limited(mut self, characters: usize) -> Self {
        self.limit = Some(characters);
        self
    }

    /// The field shown as dots, whose text cannot be copied.
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// The field that can be selected and copied from but not changed.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// The field that may hold line breaks: Enter's partner, a line break typed or pasted, is kept.
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    /// Whether the field may hold line breaks.
    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// Whether the field cannot be changed.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Whether there is a change to take back.
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    /// Whether there is a change that was taken back to make again.
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Whether the line can be copied from: something is selected and the line is not masked.
    pub fn can_copy(&self) -> bool {
        !self.selected().is_empty()
    }

    /// The scroll offset in logical pixels: vertical for multiline inputs, horizontal otherwise.
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// Whether an edit still needs its multiline caret brought into view.
    pub fn follows_caret(&self) -> bool {
        self.follow_caret
    }

    /// The vertical offset for a multiline viewport, following the caret only after an edit.
    pub fn scroll_for_lines(&self, caret: usize, lines: usize, visible: usize, height: f32) -> f32 {
        let room = lines.saturating_sub(visible) as f32 * height;
        let offset = self.scroll.clamp(0.0, room);
        if !self.follow_caret {
            return offset;
        }
        let top = caret as f32 * height;
        offset
            .min(top)
            .max(top + height - visible as f32 * height)
            .clamp(0.0, room)
    }

    /// What the line looks like: its text with what is being composed at the
    /// caret, or a dot for each character of that when masked.
    pub fn shown(&self) -> std::borrow::Cow<'_, str> {
        if self.composing.is_empty() && !self.masked {
            return std::borrow::Cow::Borrowed(&self.text);
        }
        let mut composed = self.text.clone();
        composed.insert_str(self.caret, &self.composing);
        if self.masked {
            std::borrow::Cow::Owned(MASK.to_string().repeat(composed.chars().count()))
        } else {
            std::borrow::Cow::Owned(composed)
        }
    }

    /// The offset in what is shown that matches the byte offset `byte` of the
    /// text; offsets from the caret on fall after what is being composed.
    pub fn to_shown(&self, byte: usize) -> usize {
        let after_composing = !self.composing.is_empty() && byte >= self.caret;
        self.to_shown_uncomposed(byte)
            + if after_composing {
                self.composing_width()
            } else {
                0
            }
    }

    /// The range of what is shown that an input method is still composing,
    /// or nothing while it composes nothing.
    pub fn marked(&self) -> Option<Range<usize>> {
        (!self.composing.is_empty()).then(|| {
            let start = self.to_shown_uncomposed(self.caret);
            start..start + self.composing_width()
        })
    }

    /// The offset in what is shown that matches the byte offset `byte` of the
    /// text, leaving out what is being composed.
    fn to_shown_uncomposed(&self, byte: usize) -> usize {
        if self.masked {
            self.text[..byte.min(self.text.len())].chars().count() * MASK.len_utf8()
        } else {
            byte
        }
    }

    /// How many bytes of what is shown the text being composed takes.
    fn composing_width(&self) -> usize {
        if self.masked {
            self.composing.chars().count() * MASK.len_utf8()
        } else {
            self.composing.len()
        }
    }

    /// The byte offset of the text that matches the offset `shown` in what is
    /// shown; an offset inside what is being composed is the caret's.
    pub fn to_text(&self, shown: usize) -> usize {
        let start = self.to_shown_uncomposed(self.caret);
        let width = self.composing_width();
        let shown = if shown >= start + width {
            shown - width
        } else {
            shown.min(start)
        };
        if !self.masked {
            return self.boundary(shown);
        }
        let characters = shown / MASK.len_utf8();
        self.text
            .char_indices()
            .nth(characters)
            .map_or(self.text.len(), |(at, _)| at)
    }

    /// Where the line should scroll to so the caret, `caret_x` logical pixels
    /// from the start of the line, stays inside a view `view` wide.
    pub fn scroll_for(&self, caret_x: f32, line: f32, view: f32) -> f32 {
        let margin = 2.0;
        let mut scroll = self.scroll;
        if caret_x - scroll > view - margin {
            scroll = caret_x - view + margin;
        }
        if caret_x - scroll < 0.0 {
            scroll = caret_x;
        }
        scroll.clamp(0.0, (line - view + margin).max(0.0))
    }

    /// The line as typed so far, without the caret.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How many times the text has changed; caret moves and composition do not change it.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The caret, as a byte offset into [`Field::text`].
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The byte of [`Field::text`] where the selection stays put while the caret moves; the caret when nothing is selected.
    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// The selected bytes of [`Field::text`], or nothing when none are.
    pub fn selection(&self) -> Option<Range<usize>> {
        (self.anchor != self.caret)
            .then(|| self.anchor.min(self.caret)..self.anchor.max(self.caret))
    }

    /// Every selection, the one the caret is in among them, as the end that does not move and the end that does, in the order they come in the text.
    pub fn selections(&self) -> Vec<(usize, usize)> {
        let mut all: Vec<(usize, usize)> = std::iter::once((self.anchor, self.caret))
            .chain(self.others.iter().copied())
            .collect();
        all.sort_by_key(|(anchor, caret)| (*anchor).min(*caret));
        all
    }

    /// The text of every selection that holds some, in the order they come, one to a line; empty when the line is masked.
    pub fn selected_all(&self) -> String {
        if self.masked {
            return String::new();
        }
        self.selections()
            .into_iter()
            .filter(|(anchor, caret)| anchor != caret)
            .map(|(anchor, caret)| &self.text[anchor.min(caret)..anchor.max(caret)])
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Whether there are selections besides the one the caret is in.
    pub fn has_others(&self) -> bool {
        !self.others.is_empty()
    }

    /// The selected text, empty when nothing is selected or the line is masked.
    pub fn selected(&self) -> &str {
        match self.selection() {
            Some(range) if !self.masked => &self.text[range],
            _ => "",
        }
    }

    /// Makes `edit`.
    pub fn apply(&mut self, edit: Edit) {
        self.follow_caret = !matches!(edit, Edit::ScrollTo(_));
        match &edit {
            Edit::Undo => return self.undo(),
            Edit::Redo => return self.redo(),
            Edit::ScrollTo(offset) => {
                self.scroll = offset.max(0.0);
                return;
            }
            Edit::Compose(text) => {
                self.composing = if self.read_only {
                    String::new()
                } else {
                    text.chars().filter(|c| !c.is_control()).collect()
                };
                return;
            }
            _ => {}
        }
        let changes = matches!(
            edit,
            Edit::Insert(_)
                | Edit::Paste(_)
                | Edit::Replace(..)
                | Edit::Backspace
                | Edit::Delete
                | Edit::BackspaceWord
                | Edit::DeleteWord
        );
        if changes && self.read_only {
            return;
        }
        let typed = matches!(&edit, Edit::Insert(c) if !c.is_whitespace());
        let before = (changes && !(typed && self.typing)).then(|| self.snapshot());
        let previous = (self.revision, self.caret, self.anchor, self.others.clone());
        let everywhere = matches!(
            edit,
            Edit::Insert(_)
                | Edit::Paste(_)
                | Edit::Backspace
                | Edit::Delete
                | Edit::BackspaceWord
                | Edit::DeleteWord
                | Edit::Move(..)
        );
        if everywhere && !self.others.is_empty() {
            self.perform_everywhere(&edit);
        } else {
            if !matches!(edit, Edit::AddSelection(..)) {
                self.others.clear();
            }
            self.perform(edit);
        }
        if changes && previous != (self.revision, self.caret, self.anchor, self.others.clone()) {
            if let Some(before) = before {
                self.past.push(before);
                if self.past.len() > HISTORY {
                    self.past.remove(0);
                }
            }
            self.future.clear();
            self.typing = typed;
        } else if !changes {
            self.typing = false;
        }
    }

    /// Makes `edit` at every selection, the last in the text first so the earlier ones stay where they are, and joins selections that come to overlap.
    fn perform_everywhere(&mut self, edit: &Edit) {
        let mut all: Vec<(usize, usize)> = std::iter::once((self.anchor, self.caret))
            .chain(self.others.iter().copied())
            .collect();
        let mut order: Vec<usize> = (0..all.len()).collect();
        order.sort_by_key(|index| std::cmp::Reverse(all[*index].0.min(all[*index].1)));
        for (done, index) in order.iter().enumerate() {
            (self.anchor, self.caret) = all[*index];
            let length = self.text.len() as isize;
            self.perform(edit.clone());
            let grown = self.text.len() as isize - length;
            all[*index] = (self.anchor, self.caret);
            for later in &order[..done] {
                let (anchor, caret) = all[*later];
                all[*later] = (
                    (anchor as isize + grown).max(0) as usize,
                    (caret as isize + grown).max(0) as usize,
                );
            }
        }
        let primary = all[0];
        let mut sorted = all.clone();
        sorted.sort_by_key(|(anchor, caret)| (*anchor).min(*caret));
        let mut joined: Vec<(usize, usize)> = Vec::new();
        for (anchor, caret) in sorted {
            let (start, end) = (anchor.min(caret), anchor.max(caret));
            if let Some(last) = joined.last_mut() {
                let (held_start, held_end) = (last.0.min(last.1), last.0.max(last.1));
                if start < held_end || (start == held_start && end == held_end) {
                    let reach = held_end.max(end);
                    *last = if last.1 >= last.0 {
                        (held_start, reach)
                    } else {
                        (reach, held_start)
                    };
                    continue;
                }
            }
            joined.push((anchor, caret));
        }
        let at = joined
            .iter()
            .position(|(anchor, caret)| {
                let (start, end) = ((*anchor).min(*caret), (*anchor).max(*caret));
                start <= primary.1 && primary.1 <= end
            })
            .unwrap_or(0);
        (self.anchor, self.caret) = joined.remove(at);
        self.others = joined;
    }

    /// Makes `edit` at the selection the caret is in.
    fn perform(&mut self, edit: Edit) {
        match edit {
            Edit::Insert(character) => {
                if !character.is_control() || (matches!(character, '\n' | '\t') && self.multiline) {
                    self.replace_selection(character.encode_utf8(&mut [0; 4]));
                }
            }
            Edit::Paste(text) => {
                let clean = self.cleaned(&text);
                self.replace_selection(&clean);
            }
            Edit::Replace(range, text, (anchor, caret)) => {
                let clean = self.cleaned(&text);
                let start = self.boundary(range.start);
                let end = self.boundary(range.end.max(range.start));
                self.replace_text(start..end, &clean);
                self.anchor = self.boundary(anchor);
                self.caret = self.boundary(caret);
            }
            Edit::Backspace => self.delete_or(|field| field.before(field.caret)),
            Edit::Delete => self.delete_or(|field| field.after(field.caret)),
            Edit::BackspaceWord => self.delete_or(|field| Some(field.word_before(field.caret))),
            Edit::DeleteWord => self.delete_or(|field| Some(field.word_after(field.caret))),
            Edit::Move(motion, extend) => self.travel(motion, extend),
            Edit::SelectAll => {
                self.anchor = 0;
                self.caret = self.text.len();
            }
            Edit::Place(shown, extend) => {
                self.caret = self.to_text(shown);
                if !extend {
                    self.anchor = self.caret;
                }
            }
            Edit::Select(anchor, caret) => {
                self.anchor = self.boundary(anchor);
                self.caret = self.boundary(caret);
            }
            Edit::AddSelection(anchor, caret) => {
                let anchor = self.boundary(anchor);
                let caret = self.boundary(caret);
                let (start, end) = (anchor.min(caret), anchor.max(caret));
                let taken = std::iter::once((self.anchor, self.caret))
                    .chain(self.others.iter().copied())
                    .any(|(held_anchor, held_caret)| {
                        held_anchor.min(held_caret) == start && held_anchor.max(held_caret) == end
                    });
                if !taken {
                    self.others.push((self.anchor, self.caret));
                    self.anchor = anchor;
                    self.caret = caret;
                }
            }
            Edit::OnlySelection => self.others.clear(),
            Edit::Undo | Edit::Redo | Edit::ScrollTo(_) | Edit::Compose(_) => {}
        }
    }

    /// `text` as this field holds it: line endings made plain, and control characters dropped but for line breaks and tabs in a multiline field.
    fn cleaned(&self, text: &str) -> String {
        let multiline = self.multiline;
        text.replace("\r\n", "\n")
            .chars()
            .filter(|c| !c.is_control() || (multiline && matches!(*c, '\n' | '\t')))
            .collect()
    }

    /// The line, caret and selection as they are now.
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            caret: self.caret,
            anchor: self.anchor,
            others: self.others.clone(),
        }
    }

    /// Takes in a state the line is put back to.
    fn restore(&mut self, snapshot: Snapshot) {
        if self.text != snapshot.text {
            self.revision = self.revision.wrapping_add(1);
        }
        self.text = snapshot.text;
        self.caret = snapshot.caret;
        self.anchor = snapshot.anchor;
        self.others = snapshot.others;
        self.typing = false;
    }

    /// Takes back the last change.
    fn undo(&mut self) {
        if let Some(previous) = self.past.pop() {
            self.future.push(self.snapshot());
            self.restore(previous);
        }
    }

    /// Makes again the last change that was taken back.
    fn redo(&mut self) {
        if let Some(next) = self.future.pop() {
            self.past.push(self.snapshot());
            self.restore(next);
        }
    }

    /// Replaces what is selected, or else nothing at the caret, with `text`.
    fn replace_selection(&mut self, text: &str) {
        let range = self.selection().unwrap_or(self.caret..self.caret);
        let limited = self.limit.map(|limit| {
            let kept = self.text.chars().count() - self.text[range.clone()].chars().count();
            text.chars()
                .take(limit.saturating_sub(kept))
                .collect::<String>()
        });
        let text = limited.as_deref().unwrap_or(text);
        self.caret = range.start + text.len();
        self.replace_text(range, text);
        self.anchor = self.caret;
    }

    /// Replaces bytes only when their contents differ, updating the cheap text revision.
    fn replace_text(&mut self, range: Range<usize>, text: &str) {
        if &self.text[range.clone()] != text {
            self.text.replace_range(range, text);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// Deletes what is selected, or else what lies between the caret and the
    /// offset `other` finds.
    fn delete_or(&mut self, other: impl Fn(&Self) -> Option<usize>) {
        let range = self.selection().or_else(|| {
            let other = other(self)?;
            Some(other.min(self.caret)..other.max(self.caret))
        });
        if let Some(range) = range {
            self.caret = range.start;
            self.replace_text(range, "");
            self.anchor = self.caret;
        }
    }

    /// Moves the caret by `motion`.
    fn travel(&mut self, motion: Motion, extend: bool) {
        let selection = self.selection();
        let target = match (motion, &selection) {
            (Motion::Left, Some(range)) if !extend => range.start,
            (Motion::Right, Some(range)) if !extend => range.end,
            (Motion::Left, _) => self.before(self.caret).unwrap_or(self.caret),
            (Motion::Right, _) => self.after(self.caret).unwrap_or(self.caret),
            (Motion::WordLeft, _) => self.word_before(self.caret),
            (Motion::WordRight, _) => self.word_after(self.caret),
            (Motion::Home, _) => 0,
            (Motion::End, _) => self.text.len(),
            (Motion::LineStart, _) => self.line_start(self.caret),
            (Motion::LineEnd, _) => self.line_end(self.caret),
            (Motion::LineUp, _) => self.line_step(false),
            (Motion::LineDown, _) => self.line_step(true),
        };
        self.caret = target;
        if !extend {
            self.anchor = target;
        }
    }

    /// The start and end of the word or line `reach` names around the byte `offset`.
    pub fn reach_around(&self, reach: Reach, offset: usize) -> (usize, usize) {
        match reach {
            Reach::Point => (offset, offset),
            Reach::Word => (
                self.word_before(self.after(offset).unwrap_or(offset)),
                self.word_after(self.before(offset).unwrap_or(offset)),
            ),
            Reach::Line => (self.line_start(offset), self.line_end(offset)),
        }
    }

    /// Where the line holding the byte `offset` starts.
    fn line_start(&self, offset: usize) -> usize {
        self.text[..offset].rfind('\n').map_or(0, |at| at + 1)
    }

    /// Where the line holding the byte `offset` ends, before its line break.
    fn line_end(&self, offset: usize) -> usize {
        self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |at| offset + at)
    }

    /// Where the caret goes on the line below, or above unless `down`, as many characters in as it is now or at the end of a shorter line; the start or end of the text when there is no such line.
    fn line_step(&self, down: bool) -> usize {
        let start = self.line_start(self.caret);
        let column = self.text[start..self.caret].chars().count();
        let target = if down {
            let end = self.line_end(self.caret);
            if end >= self.text.len() {
                return self.text.len();
            }
            end + 1
        } else {
            if start == 0 {
                return 0;
            }
            self.line_start(start - 1)
        };
        let end = self.line_end(target);
        self.text[target..end]
            .char_indices()
            .nth(column)
            .map_or(end, |(at, _)| target + at)
    }

    /// The nearest character boundary at or before `byte`, within the text.
    fn boundary(&self, byte: usize) -> usize {
        let mut byte = byte.min(self.text.len());
        while !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        byte
    }

    /// The offset of the character boundary before `from`.
    fn before(&self, from: usize) -> Option<usize> {
        self.text[..from]
            .char_indices()
            .next_back()
            .map(|(start, _)| start)
    }

    /// The offset of the character boundary after `from`.
    fn after(&self, from: usize) -> Option<usize> {
        self.text[from..]
            .chars()
            .next()
            .map(|character| from + character.len_utf8())
    }

    /// The start of the word before `from`: past any space, then past the
    /// run of characters of one class.
    fn word_before(&self, from: usize) -> usize {
        let mut at = from;
        let mut class = None;
        for (start, character) in self.text[..from].char_indices().rev() {
            let this = Class::of(character);
            match class {
                None if this == Class::Space => {}
                None => class = Some(this),
                Some(class) if class != this => break,
                Some(_) => {}
            }
            at = start;
        }
        at
    }

    /// The end of the word after `from`: past any space, then past the run
    /// of characters of one class.
    fn word_after(&self, from: usize) -> usize {
        let mut at = from;
        let mut class = None;
        for (start, character) in self.text[from..].char_indices() {
            let this = Class::of(character);
            match class {
                None if this == Class::Space => {}
                None => class = Some(this),
                Some(class) if class != this => break,
                Some(_) => {}
            }
            at = from + start + character.len_utf8();
        }
        at
    }
}

/// Tests for multiline viewport offsets.
#[cfg(test)]
mod tests {
    use super::{Edit, Field, Motion};

    /// Wheel offsets survive layout until an edit or caret motion requires following again.
    #[test]
    fn multiline_scroll_follows_edits_only() {
        let mut field = Field::new("line\n".repeat(40)).multiline(true);
        assert_eq!(field.scroll_for_lines(40, 41, 8, 22.0), 726.0);
        field.apply(Edit::ScrollTo(220.0));
        assert_eq!(field.scroll_for_lines(40, 41, 8, 22.0), 220.0);
        field.apply(Edit::Insert('x'));
        assert_eq!(field.scroll_for_lines(40, 41, 8, 22.0), 726.0);
        field.apply(Edit::ScrollTo(726.0));
        field.apply(Edit::Move(Motion::Home, false));
        assert_eq!(field.scroll_for_lines(0, 41, 8, 22.0), 0.0);
        field.apply(Edit::ScrollTo(220.0));
        assert_eq!(field.scroll_for_lines(12, 41, 8, 22.0), 220.0);
        assert_eq!(field.scroll_for_lines(0, 0, 8, 22.0), 0.0);
        assert_eq!(field.scroll_for_lines(7, 8, 8, 22.0), 0.0);
    }

    /// Grouped Unicode typing remains one undo step, and revisions ignore caret moves and no-op deletes.
    #[test]
    fn revisions_preserve_undo_and_no_op_edits() {
        let mut field = Field::new("seed");
        for character in "café🦀".chars() {
            field.apply(Edit::Insert(character));
        }
        let revision = field.revision();
        assert_eq!(field.text(), "seedcafé🦀");
        assert_eq!(field.past.len(), 1);
        field.apply(Edit::Delete);
        assert_eq!(field.revision(), revision);
        field.apply(Edit::Undo);
        assert_eq!(field.text(), "seed");
        field.apply(Edit::Redo);
        assert_eq!(field.text(), "seedcafé🦀");
        let revision = field.revision();
        field.apply(Edit::Move(Motion::Home, false));
        assert_eq!(field.revision(), revision);
        field.apply(Edit::SelectAll);
        field.apply(Edit::Paste("replaced".to_owned()));
        field.apply(Edit::Undo);
        assert_eq!(field.text(), "seedcafé🦀");
    }

    /// Character limits still count Unicode characters, and composed and masked offsets round-trip.
    #[test]
    fn limits_and_display_offsets() {
        let mut field = Field::new("é").limited(3).masked(true);
        field.apply(Edit::Paste("🦀abc".to_owned()));
        assert_eq!(field.text(), "é🦀a");
        for byte in [0, 2, 6, 7] {
            assert_eq!(field.to_text(field.to_shown(byte)), byte);
        }
        field.apply(Edit::Compose("字".to_owned()));
        assert_eq!(field.to_text(field.to_shown(7)), 7);
        field.apply(Edit::Compose(String::new()));
        field.apply(Edit::SelectAll);
        field.apply(Edit::Paste("🙂🙂🙂🙂".to_owned()));
        assert_eq!(field.text(), "🙂🙂🙂");
    }
}
