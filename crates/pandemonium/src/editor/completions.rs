//! What could be written where the cursor is, and the list that offers it.
//!
//! The list stays up while the reader goes on typing: every keystroke goes
//! into the buffer as it always would and narrows the list afterwards, which
//! is what makes completion something that happens beside the typing rather
//! than instead of it. It closes when nothing matches any more, or when the
//! cursor leaves the word it was offered for.

use pm_gfx::Point;
use pm_text::{Completion, Position};
use pm_ui::{Div, Styled, Theme, h_flex, text, v_flex};

use crate::message::Message;

/// Widest the list is drawn.
const WIDTH: f32 = 420.0;

/// Height of one row of it.
const ROW_HEIGHT: f32 = 24.0;

/// Most rows drawn at once, however many the server offered.
const VISIBLE: usize = 10;

/// What could be written where the cursor is.
pub struct Completions {
    /// Everything the server offered.
    items: Vec<Completion>,
    /// Which of them what has been typed since leaves, best first.
    matched: Vec<usize>,
    /// Which of those is selected.
    selected: usize,
    /// Where the word being completed begins.
    start: Position,
    /// Where on screen the list hangs from.
    at: Point,
}

impl Completions {
    /// The list of `items`, offered for the word beginning at `start`.
    pub fn new(items: Vec<Completion>, start: Position, at: Point) -> Self {
        let mut list = Self {
            items,
            matched: Vec::new(),
            selected: 0,
            start,
            at,
        };
        list.narrow("");
        list
    }

    /// Where the word being completed begins.
    pub fn start(&self) -> Position {
        self.start
    }

    /// Where on screen the list hangs from.
    pub fn at(&self) -> Point {
        self.at
    }

    /// Whether the list has anything left to offer.
    pub fn is_empty(&self) -> bool {
        self.matched.is_empty()
    }

    /// Keeps only what still begins with `typed`, in the order offered.
    pub fn narrow(&mut self, typed: &str) {
        let typed = typed.to_lowercase();
        self.matched = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.label.to_lowercase().starts_with(&typed))
            .map(|(index, _)| index)
            .collect();
        self.selected = 0;
    }

    /// Moves the selection `step` rows along, wrapping around at either end.
    pub fn step(&mut self, step: isize) {
        let count = self.matched.len();
        if count == 0 {
            return;
        }
        self.selected = (self.selected as isize + step).rem_euclid(count as isize) as usize;
    }

    /// Which of the rows shown is selected.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The `place`-th completion shown, if there is one.
    pub fn at_place(&self, place: usize) -> Option<&Completion> {
        self.items.get(*self.matched.get(place)?)
    }

    /// The rows shown, and which of them is selected.
    fn shown(&self) -> (usize, Vec<(usize, &Completion)>) {
        let first = self.selected.saturating_sub(VISIBLE - 1);
        let rows = self
            .matched
            .iter()
            .enumerate()
            .skip(first)
            .take(VISIBLE)
            .map(|(place, index)| (place, &self.items[*index]))
            .collect();
        (self.selected, rows)
    }
}

/// Builds the list as it hangs under the word being completed.
pub fn completion_list(theme: &Theme, completions: &Completions) -> Div<Message> {
    let (selected, rows) = completions.shown();

    v_flex()
        .w_px(WIDTH)
        .py(0.5)
        .items_stretch()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.md)
        .children(
            rows.into_iter()
                .map(|(place, item)| row(theme, place, item, place == selected)),
        )
}

/// Builds one row of the list, lit while it is the selected one.
fn row(theme: &Theme, place: usize, item: &Completion, selected: bool) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(ROW_HEIGHT)
        .px(1.5)
        .gap(1)
        .items_center()
        .overflow_hidden()
        .when(selected, |line| line.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ChooseCompletion(place))
        .child(text(item.label.clone()).text_sm().font_mono())
        .child(
            text(item.kind.to_owned())
                .text_xs()
                .font_light()
                .color(theme.colors.accent),
        )
        .child(h_flex().flex_1())
        .child(
            text(item.detail.clone())
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}
