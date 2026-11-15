//! A diff laid out side by side or unified: the old lines on the left and
//! the new on the right, each with its line number, removed lines tinted in
//! the danger tone and added ones in the success tone, the words that
//! changed inside a line boxed, the code coloured by its syntax, hunk
//! headers between them, and notes left on lines under the lines they are on.

use std::rc::Rc;
use std::sync::Arc;

use pm_treesitter::Grammar;
use rustle::layout_text;
use rustle::prelude::*;

use crate::app_theme::{ExtendedColors, extended_colors};
use crate::button::{Button, ButtonVariant};
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
        let height = match self.size {
            Size::Xs => 16.0,
            Size::Sm => 18.0,
            Size::Md => 20.0,
            Size::Lg => 22.0,
        };
        let style = code_style(height);
        let width = layout_text(&"0".repeat(64), &style, None).size().width / 64.0;
        let look = Look {
            style,
            width: width.max(1.0),
            height,
            scheme: color_scheme(),
            extended: extended_colors(),
        };
        let shown = self.rows.len().min(ROWS_AT_MOST);
        let colours = colour_rows(&self.rows[..shown], self.grammar.as_ref());
        let modifier = if self.scrolls {
            std::mem::take(&mut self.modifier).vertical_scroll(&scroll)
        } else {
            std::mem::take(&mut self.modifier)
        };
        let view = Rc::new(self);
        column_with(modifier, move || {
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
        });
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

/// The style code is set in, for lines `height` tall.
fn code_style(height: f32) -> TextStyle {
    TextStyle {
        color: color_scheme().on_surface,
        font_size: (height * 0.64).round(),
        line_height: height,
        font_family: FontFamily::Monospace,
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
    row_with(Modifier::new().fill_max_width().height(look.height), || {
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
    });
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
        row_with(Modifier::new().fill_max_width().height(look.height), || {
            half(half_of, view, look)
        });
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
        Arrangement::SpacedBy(6.0),
        Alignment::Center,
        move || {
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
                .show();
            let painted = Rc::new(painted.clone());
            canvas(
                Modifier::new().weight(1.0).fill_max_height(),
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
    /// The runs of one syntax colour it is painted in.
    runs: Vec<(std::ops::Range<usize>, Option<pm_treesitter::Kind>)>,
    /// The stretches of it that changed.
    words: Vec<std::ops::Range<usize>>,
    /// The colour of the boxes behind the words that changed.
    word_colour: Color,
}

impl Painted {
    /// Paints the boxes behind the words that changed, then the text in its colours.
    fn paint(&self, canvas: &mut Canvas, look: &Look) {
        let x_of = |byte: usize| column_of(&self.text, byte) as f32 * look.width;
        for range in &self.words {
            let left = x_of(range.start);
            let right = x_of(range.end);
            canvas.rect(
                left,
                1.0,
                (right - left).max(2.0),
                look.height - 2.0,
                self.word_colour,
            );
        }
        for (range, kind) in &self.runs {
            let piece = &self.text[range.clone()];
            if piece.trim().is_empty() {
                continue;
            }
            let column = column_of(&self.text, range.start);
            let style = TextStyle {
                color: kind.map_or(look.scheme.on_surface, |kind| {
                    kind_colour(kind, &look.scheme, &look.extended)
                }),
                ..look.style
            };
            canvas.draw_text(
                column as f32 * look.width,
                0.0,
                &expand_tabs(piece, column),
                &style,
                None,
            );
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
        Arrangement::SpacedBy(8.0),
        Alignment::Center,
        || {
            icon(phosphor::CHAT_CIRCLE_TEXT, 13.0, scheme.primary);
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
        Arrangement::SpacedBy(6.0),
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
