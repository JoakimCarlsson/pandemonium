//! A diff laid out side by side or unified: the old lines on the left and
//! the new on the right, each with its line number, removed lines tinted in
//! the danger tone and added ones in the success tone, the words that
//! changed inside a line boxed, the code coloured by its syntax, hunk
//! headers between them, and notes left on lines under the lines they are on.
//! Long lines wrap when the editor's soft wrap is on, and otherwise scroll
//! across with a sideways wheel or Shift and the wheel.

use std::rc::Rc;
use std::sync::Arc;

use pm_treesitter::Grammar;
use rustle::layout_text;
use rustle::prelude::*;

use crate::app_theme::{ExtendedColors, extended_colors};
use crate::button::{Button, ButtonVariant};
use crate::code_look::code_look;
use crate::code_text::{column_of, expand_tabs, kind_colour, runs};
use crate::diff_syntax::{LineSpans, colour_rows};
use crate::icon_button::IconButton;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::text_input::TextInput;
use rustle::{ColorScheme, color_scheme};

/// The most rows a diff view draws, so a huge diff stays responsive.
const ROWS_AT_MOST: usize = 4000;

/// How strongly removed and added lines are tinted, out of 255.
const TINT: u8 = 40;

/// How strongly the words that changed inside a line are boxed, out of 255.
const WORD_TINT: u8 = 110;

/// How wide the column of line numbers is.
const GUTTER: f32 = 40.0;

/// The fewest characters a wrapped row holds; a narrower side does not wrap.
const MIN_WRAP_COLUMNS: usize = 16;

/// One row of a side-by-side diff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiffRow {
    /// The old side: its line number and text, when this row has one.
    pub left: Option<(usize, String)>,
    /// The new side: its line number and text, when this row has one.
    pub right: Option<(usize, String)>,
    /// The hunk header this row stands for, when it is one.
    pub hunk: Option<String>,
    /// The stretches of the old side's text that changed, as byte ranges.
    pub left_words: Vec<(usize, usize)>,
    /// The stretches of the new side's text that changed, as byte ranges.
    pub right_words: Vec<(usize, usize)>,
}

/// A note shown under the line it was left on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiffNote {
    /// Whether the line is of the new side rather than the old.
    pub new: bool,
    /// The line, counted from 1.
    pub line: usize,
    /// What the note says.
    pub text: String,
}

/// A note being written under a line, with what learns of its text and of it being added or dropped.
#[derive(Clone)]
pub struct DiffDraft {
    /// Whether the line is of the new side rather than the old.
    pub new: bool,
    /// The line, counted from 1.
    pub line: usize,
    /// What is typed.
    pub text: String,
    /// What learns the text changed.
    pub on_text: Rc<dyn Fn(String)>,
    /// What adds the note.
    pub on_save: Rc<dyn Fn()>,
    /// What drops the note.
    pub on_cancel: Rc<dyn Fn()>,
}

/// How a diff view is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiffViewVariant {
    /// Old and new side by side.
    #[default]
    Split,
    /// The new side alone, removed lines shown above the lines replacing them.
    Unified,
}

/// A diff view with a variant and a size.
#[derive(Clone)]
pub struct DiffView {
    /// Its rows, in order.
    rows: Vec<DiffRow>,
    /// How it is drawn.
    variant: DiffViewVariant,
    /// Its line height.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// The grammar its code is coloured with, when one is loaded.
    grammar: Option<Arc<Grammar>>,
    /// The notes shown under their lines.
    notes: Vec<DiffNote>,
    /// The note being written, when one is.
    draft: Option<DiffDraft>,
    /// Whether it scrolls by itself, rather than growing to its rows inside something that scrolls.
    scrolls: bool,
    /// What a press on a line number does, given whether it is of the new side and the line.
    on_line: Option<Rc<dyn Fn(bool, usize)>>,
    /// What the button on a note does, given its place among the notes.
    on_remove_note: Option<Rc<dyn Fn(usize)>>,
}

impl DiffView {
    /// A split view of `rows`.
    pub fn new(rows: Vec<DiffRow>) -> Self {
        Self {
            rows,
            variant: DiffViewVariant::Split,
            size: Size::Sm,
            modifier: Modifier::new(),
            grammar: None,
            notes: Vec::new(),
            draft: None,
            scrolls: true,
            on_line: None,
            on_remove_note: None,
        }
    }

    /// The view in `variant`.
    pub fn variant(mut self, variant: DiffViewVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The view at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The view laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The view with its code coloured by `grammar`.
    pub fn grammar(mut self, grammar: Option<Arc<Grammar>>) -> Self {
        self.grammar = grammar;
        self
    }

    /// The view with `notes` under their lines.
    pub fn notes(mut self, notes: Vec<DiffNote>) -> Self {
        self.notes = notes;
        self
    }

    /// The view with `draft` being written under its line.
    pub fn draft(mut self, draft: Option<DiffDraft>) -> Self {
        self.draft = draft;
        self
    }

    /// The view scrolling by itself when `scrolls`, or growing to its rows.
    pub fn scrolls(mut self, scrolls: bool) -> Self {
        self.scrolls = scrolls;
        self
    }

    /// The view calling `on_line` when a line number is pressed.
    pub fn on_line(mut self, on_line: impl Fn(bool, usize) + 'static) -> Self {
        self.on_line = Some(Rc::new(on_line));
        self
    }

    /// The view calling `on_remove_note` when a note's button is pressed.
    pub fn on_remove_note(mut self, on_remove_note: impl Fn(usize) + 'static) -> Self {
        self.on_remove_note = Some(Rc::new(on_remove_note));
        self
    }

    /// Emits the view.
    #[track_caller]
    pub fn show(mut self) {
        let scroll = remember_scroll_state();
        let code = crate::code_font();
        let (drawn, drawn_height) = match self.size {
            Size::Xs => (10.0, 16.0),
            Size::Sm => (12.0, 18.0),
            Size::Md => (13.0, 20.0),
            Size::Lg => (14.0, 22.0),
        };
        let font_size = code.sized(drawn);
        let height = code.line_for(font_size, drawn_height / drawn);
        let style = code_style(font_size, height, code.family);
        let width = layout_text(&"0".repeat(64), &style, None).size().width / 64.0;
        let width = width.max(1.0);
        let shown = self.rows.len().min(ROWS_AT_MOST);
        let across = remember(|| 0.0_f32);
        let area = remember(|| 0.0_f32);
        let room = text_room(area.get(), self.variant, width);
        let wrap = wrap_columns(room, width);
        let widest = self.rows[..shown]
            .iter()
            .flat_map(|row| [&row.left, &row.right])
            .flatten()
            .map(|(_, text)| column_of(text, text.len()))
            .max()
            .unwrap_or(0);
        let limit = (widest as f32 * width + 2.0 * width - room).max(0.0);
        let look = Look {
            style,
            width,
            height,
            scheme: color_scheme(),
            extended: extended_colors(),
            wrap,
            across: if wrap.is_some() {
                0.0
            } else {
                across.get().min(limit)
            },
        };
        let wheel = move |input: &rustle::PointerInput| {
            if wrap.is_some() || input.phase != rustle::PointerInputPhase::Scroll {
                return false;
            }
            let delta = input.scroll_delta;
            let sideways = if input.modifiers.shift && delta.x == 0.0 {
                delta.y
            } else {
                delta.x
            };
            if sideways == 0.0 || delta.y.abs() > delta.x.abs() && !input.modifiers.shift {
                return false;
            }
            let to = (across.get().min(limit) - sideways).clamp(0.0, limit);
            if to != across.get() {
                across.set(to);
            }
            true
        };
        let colours = colour_rows(&self.rows[..shown], self.grammar.as_ref());
        let modifier = if self.scrolls {
            std::mem::take(&mut self.modifier).vertical_scroll(&scroll)
        } else {
            std::mem::take(&mut self.modifier)
        };
        let view = Rc::new(self);
        let sized = move |size: rustle::Size| {
            if area.get() != size.width {
                area.set(size.width);
            }
        };
        let rows = move || {
            for (place, row) in view.rows.iter().take(shown).enumerate() {
                let (left_spans, right_spans) = &colours[place];
                key(place, || {
                    let sides = Sides {
                        row,
                        left_spans,
                        right_spans,
                    };
                    match view.variant {
                        DiffViewVariant::Split => split_row(&sides, &view, &look),
                        DiffViewVariant::Unified => unified_row(&sides, &view, &look),
                    }
                    under_row(row, &view);
                });
            }
        };
        column_with(modifier, move || {
            column_with(
                Modifier::new()
                    .fill_max_width()
                    .on_size_changed(sized)
                    .pointer_input(wheel),
                rows,
            );
        });
    }
}

/// How wide the text of one side is in a view `area` wide drawn as
/// `variant`, with characters `width` wide: what is left past the line
/// numbers and the marker.
fn text_room(area: f32, variant: DiffViewVariant, width: f32) -> f32 {
    let side = match variant {
        DiffViewVariant::Split => (area - 1.0) / 2.0,
        DiffViewVariant::Unified => area,
    };
    side - GUTTER - width - 2.0 * crate::spacing(6.0)
}

/// How many characters a row holds when the editor's soft wrap is on, in
/// text `room` wide with characters `width` wide; nothing when lines do not
/// wrap or the view has not been laid out.
fn wrap_columns(room: f32, width: f32) -> Option<usize> {
    let edge = (room / width).floor().max(0.0) as usize;
    if room <= 0.0 || edge < MIN_WRAP_COLUMNS {
        return None;
    }
    let look = code_look();
    match look.soft_wrap {
        pm_editor::SoftWrap::Off => None,
        pm_editor::SoftWrap::Edge => Some(edge.saturating_sub(1)),
        pm_editor::SoftWrap::LineLength => Some(edge.saturating_sub(1).min(look.line_length)),
    }
}

/// Where each row of `text` starts when it wraps at `columns` characters:
/// the byte and the column of the first character of each row.
fn wrap_cuts(text: &str, columns: usize) -> Vec<(usize, usize)> {
    let mut cuts = vec![(0, 0)];
    let mut column = 0;
    for (at, character) in text.char_indices() {
        let next = column
            + expand_tabs(&text[at..at + character.len_utf8()], column)
                .chars()
                .count();
        let start = cuts.last().map_or(0, |(_, start)| *start);
        if next - start > columns && column > start {
            cuts.push((at, column));
        }
        column = next;
    }
    cuts
}

/// The rows `line` takes as `look` lays it out: more than one only when it wraps.
fn line_rows(line: Option<&(usize, String)>, look: &Look) -> usize {
    match (line, look.wrap) {
        (Some((_, text)), Some(columns)) => wrap_cuts(text, columns).len(),
        _ => 1,
    }
}

/// How the code of a diff is set, the same for every line.
#[derive(Clone, Copy)]
struct Look {
    /// The text style of the code.
    style: TextStyle,
    /// How wide a character is.
    width: f32,
    /// How tall a line is.
    height: f32,
    /// The Material colours.
    scheme: ColorScheme,
    /// The colours Material has no role for.
    extended: ExtendedColors,
    /// How many characters a row holds when long lines wrap.
    wrap: Option<usize>,
    /// How far the text is scrolled across when they do not.
    across: f32,
}

/// A row with the syntax colours of its sides.
struct Sides<'a> {
    /// The row.
    row: &'a DiffRow,
    /// The colours of its old side.
    left_spans: &'a LineSpans,
    /// The colours of its new side.
    right_spans: &'a LineSpans,
}

/// The style code is set in: `font_size` pixels big in `family`, for lines `height` tall.
fn code_style(font_size: f32, height: f32, family: FontFamily) -> TextStyle {
    TextStyle {
        color: color_scheme().on_surface,
        font_size,
        line_height: height,
        font_family: family,
        max_lines: Some(1),
        ligatures: false,
        ..TextStyle::default()
    }
}

/// What a side of a row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    /// A line both sides hold.
    Same,
    /// A line taken out.
    Removed,
    /// A line put in.
    Added,
    /// No line, beside one the other side has.
    Empty,
}

/// What `row` holds on each side.
fn sides(row: &DiffRow) -> (Side, Side) {
    let same =
        matches!((&row.left, &row.right), (Some((_, left)), Some((_, right))) if left == right);
    let side = |line: &Option<(usize, String)>, changed: Side| match line {
        Some(_) if same => Side::Same,
        Some(_) => changed,
        None => Side::Empty,
    };
    (
        side(&row.left, Side::Removed),
        side(&row.right, Side::Added),
    )
}

/// One side of a row as it is drawn.
struct Half<'a> {
    /// Its line number and text, when it has a line.
    line: Option<&'a (usize, String)>,
    /// What it is.
    side: Side,
    /// Whether it is of the new side.
    new: bool,
    /// Its syntax colours.
    spans: &'a LineSpans,
    /// The stretches of it that changed.
    words: &'a [(usize, usize)],
}

/// Emits `sides` side by side.
#[track_caller]
fn split_row(sides_of: &Sides, view: &DiffView, look: &Look) {
    let row = sides_of.row;
    if let Some(hunk) = &row.hunk {
        hunk_row(hunk, look.height);
        return;
    }
    let (left, right) = sides(row);
    let lines = line_rows(row.left.as_ref(), look).max(line_rows(row.right.as_ref(), look));
    row_with(
        Modifier::new()
            .fill_max_width()
            .height(look.height * lines as f32),
        || {
            half(
                Half {
                    line: row.left.as_ref(),
                    side: left,
                    new: false,
                    spans: sides_of.left_spans,
                    words: &row.left_words,
                },
                view,
                look,
            );
            spacer(
                Modifier::new()
                    .width(1.0)
                    .fill_max_height()
                    .background(look.scheme.outline_variant),
            );
            half(
                Half {
                    line: row.right.as_ref(),
                    side: right,
                    new: true,
                    spans: sides_of.right_spans,
                    words: &row.right_words,
                },
                view,
                look,
            );
        },
    );
}

/// Emits `sides` as unified lines.
#[track_caller]
fn unified_row(sides_of: &Sides, view: &DiffView, look: &Look) {
    let row = sides_of.row;
    if let Some(hunk) = &row.hunk {
        hunk_row(hunk, look.height);
        return;
    }
    let line = |half_of: Half| {
        let lines = line_rows(half_of.line, look);
        row_with(
            Modifier::new()
                .fill_max_width()
                .height(look.height * lines as f32),
            || half(half_of, view, look),
        );
    };
    let old = || Half {
        line: row.left.as_ref(),
        side: Side::Removed,
        new: false,
        spans: sides_of.left_spans,
        words: &row.left_words,
    };
    let new = |side: Side| Half {
        line: row.right.as_ref(),
        side,
        new: true,
        spans: sides_of.right_spans,
        words: &row.right_words,
    };
    match sides(row) {
        (Side::Same, _) => line(new(Side::Same)),
        (left, right) => {
            if left == Side::Removed {
                key(0, || line(old()));
            }
            if right == Side::Added {
                key(1, || line(new(Side::Added)));
            }
        }
    }
}

/// Emits a hunk header reading `hunk`, `height` tall.
fn hunk_row(hunk: &str, height: f32) {
    let scheme = color_scheme();
    Label::new(hunk.to_owned())
        .variant(LabelVariant::Code)
        .colour(scheme.primary)
        .size(Size::Xs)
        .modifier(
            Modifier::new()
                .fill_max_width()
                .height(height + 4.0)
                .background(scheme.surface_container)
                .padding_xy(8.0, 2.0),
        )
        .show();
}

/// `colour` with its opacity set to `alpha` of 255.
fn tinted(colour: Color, alpha: u8) -> Color {
    Color::rgba(colour.r, colour.g, colour.b, alpha)
}

/// Emits one side of a row filling the row's height: its number, which
/// leaves a note when pressed, its marker and its coloured text.
fn half(half: Half, view: &DiffView, look: &Look) {
    let scheme = look.scheme;
    let extended = look.extended;
    let fill = match half.side {
        Side::Same => Color::TRANSPARENT,
        Side::Removed => tinted(scheme.error, TINT),
        Side::Added => tinted(extended.success, TINT),
        Side::Empty => scheme.surface_container,
    };
    let number = half.line.map(|(number, _)| *number);
    let new = half.new;
    let on_line = view.on_line.clone();
    let mut gutter = Modifier::new().width(GUTTER).padding_xy(4.0, 0.0);
    if let (Some(number), Some(on_line)) = (number, on_line) {
        gutter = gutter
            .clickable(move || on_line(new, number))
            .cursor(CursorIcon::Pointer);
    }
    let marker = match half.side {
        Side::Removed => "-",
        Side::Added => "+",
        Side::Same | Side::Empty => " ",
    };
    let text = half.line.map(|(_, text)| text.clone()).unwrap_or_default();
    let painted = Painted {
        cuts: match look.wrap {
            Some(columns) => wrap_cuts(&text, columns),
            None => vec![(0, 0)],
        },
        runs: runs(text.len(), half.spans),
        words: half
            .words
            .iter()
            .map(|(start, end)| (*start).min(text.len())..(*end).min(text.len()))
            .collect(),
        word_colour: match half.side {
            Side::Removed => tinted(scheme.error, WORD_TINT),
            _ => tinted(extended.success, WORD_TINT),
        },
        text,
    };
    let look = *look;
    row_arranged(
        Modifier::new()
            .weight(1.0)
            .fill_max_height()
            .background(fill)
            .clip(),
        Arrangement::SpacedBy(crate::spacing(6.0)),
        Alignment::Start,
        move || {
            row_arranged(
                Modifier::new().height(look.height),
                Arrangement::SpacedBy(crate::spacing(6.0)),
                Alignment::Center,
                || {
                    Label::new(number.map(|number| number.to_string()).unwrap_or_default())
                        .variant(LabelVariant::Code)
                        .colour(scheme.on_surface_variant)
                        .size(Size::Xs)
                        .modifier(gutter.clone())
                        .show();
                    Label::new(marker)
                        .variant(LabelVariant::Code)
                        .colour(match half.side {
                            Side::Removed => scheme.error,
                            Side::Added => extended.success,
                            Side::Same | Side::Empty => scheme.on_surface_variant,
                        })
                        .size(Size::Xs)
                        .modifier(Modifier::new().width(look.width))
                        .show();
                },
            );
            let painted = Rc::new(painted.clone());
            canvas(
                Modifier::new().weight(1.0).fill_max_height().clip(),
                move |canvas| painted.paint(canvas, &look),
            );
        },
    );
}

/// The text of one side of a row as it is painted.
#[derive(Clone)]
struct Painted {
    /// Its text.
    text: String,
    /// Where each of its rows starts, as a byte and a column: one row unless it wraps.
    cuts: Vec<(usize, usize)>,
    /// The runs of one syntax colour it is painted in.
    runs: Vec<(std::ops::Range<usize>, Option<pm_treesitter::Kind>)>,
    /// The stretches of it that changed.
    words: Vec<std::ops::Range<usize>>,
    /// The colour of the boxes behind the words that changed.
    word_colour: Color,
}

impl Painted {
    /// Paints the boxes behind the words that changed, then the text in
    /// its colours, row by row when it wraps and shifted by how far the
    /// view is scrolled across.
    fn paint(&self, canvas: &mut Canvas, look: &Look) {
        for (row, (start, base)) in self.cuts.iter().enumerate() {
            let end = self
                .cuts
                .get(row + 1)
                .map_or(self.text.len(), |(next, _)| *next);
            let top = row as f32 * look.height;
            let x_of = |byte: usize| {
                (column_of(&self.text, byte) - base) as f32 * look.width - look.across
            };
            for range in &self.words {
                let (from, to) = (range.start.max(*start), range.end.min(end));
                if from > to || from == to && range.start != range.end {
                    continue;
                }
                let left = x_of(from);
                let right = x_of(to);
                canvas.rect(
                    left,
                    top + 1.0,
                    (right - left).max(2.0),
                    look.height - 2.0,
                    self.word_colour,
                );
            }
            for (range, kind) in &self.runs {
                let (from, to) = (range.start.max(*start), range.end.min(end));
                if from >= to {
                    continue;
                }
                let piece = &self.text[from..to];
                if piece.trim().is_empty() {
                    continue;
                }
                let style = TextStyle {
                    color: kind.map_or(look.scheme.on_surface, |kind| {
                        kind_colour(kind, &look.scheme, &look.extended)
                    }),
                    ..look.style
                };
                canvas.draw_text(
                    x_of(from),
                    top,
                    &expand_tabs(piece, column_of(&self.text, from)),
                    &style,
                    None,
                );
            }
        }
    }
}

/// Whether `row` holds the line `line` of the new side when `new`, or of the old.
fn holds(row: &DiffRow, new: bool, line: usize) -> bool {
    let side = if new { &row.right } else { &row.left };
    side.as_ref().is_some_and(|(number, _)| *number == line)
}

/// Emits under `row` the notes left on its lines and the note being written on one of them.
fn under_row(row: &DiffRow, view: &DiffView) {
    for (place, note) in view.notes.iter().enumerate() {
        if holds(row, note.new, note.line) {
            key(("note", place), || note_box(note, place, view));
        }
    }
    if let Some(draft) = view
        .draft
        .as_ref()
        .filter(|draft| holds(row, draft.new, draft.line))
    {
        key("draft", || draft_box(draft));
    }
}

/// Emits `note`, at `place` among the notes, in a box with a button taking it away.
fn note_box(note: &DiffNote, place: usize, view: &DiffView) {
    let scheme = color_scheme();
    let remove = view.on_remove_note.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(GUTTER, 4.0)
            .background(scheme.surface_container),
        Arrangement::SpacedBy(crate::spacing(8.0)),
        Alignment::Center,
        || {
            icon(phosphor::CHAT_CIRCLE_TEXT, 14.0, scheme.primary);
            Label::new(note.text.clone())
                .size(Size::Xs)
                .modifier(Modifier::new().weight(1.0))
                .show();
            if let Some(remove) = remove {
                IconButton::new(phosphor::X)
                    .size(Size::Xs)
                    .on_click(move || remove(place))
                    .show();
            }
        },
    );
}

/// Emits the note being written: a field with buttons to add and to drop it.
fn draft_box(draft: &DiffDraft) {
    let scheme = color_scheme();
    let on_text = draft.on_text.clone();
    let on_submit = draft.on_save.clone();
    let on_save = draft.on_save.clone();
    let on_cancel = draft.on_cancel.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(GUTTER, 6.0)
            .background(scheme.surface_container),
        Arrangement::SpacedBy(crate::spacing(6.0)),
        Alignment::Center,
        || {
            TextInput::new(draft.text.clone())
                .placeholder("Leave a note for the agent")
                .autofocus()
                .size(Size::Sm)
                .modifier(Modifier::new().weight(1.0))
                .on_change(move |text| on_text(text))
                .on_submit(move |_| on_submit())
                .show();
            Button::new("Add note")
                .variant(ButtonVariant::Primary)
                .size(Size::Xs)
                .on_click(move || on_save())
                .show();
            Button::new("Cancel")
                .size(Size::Xs)
                .on_click(move || on_cancel())
                .show();
        },
    );
}
