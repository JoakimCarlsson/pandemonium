//! What could be written where the cursor is, and the list that offers it.
//!
//! The list stays up while the reader goes on typing: every keystroke goes
//! into the buffer as it always would and narrows the list afterwards, which
//! is what makes completion something that happens beside the typing rather
//! than instead of it. It closes when nothing matches any more, or when the
//! cursor leaves the word it was offered for.

use std::collections::HashSet;
use std::sync::Arc;

use pm_gfx::Point;
use pm_text::{Client, Completion, Handle, Position};
use pm_ui::{Div, Styled, Theme, h_flex, text, v_flex};

use crate::message::Message;

/// Widest the list is drawn.
const WIDTH: f32 = 420.0;

/// How far from the list what it says about the selected completion is drawn.
const GAP: f32 = 4.0;

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
    /// The server that offered them, which is the one that fills them in.
    client: Arc<Client>,
    /// Which of them it has been asked to fill in.
    asked: HashSet<usize>,
}

impl Completions {
    /// The list of `items` `client` offered for the word beginning at `start`.
    pub fn new(items: Vec<Completion>, start: Position, at: Point, client: Arc<Client>) -> Self {
        let mut list = Self {
            items,
            matched: Vec::new(),
            selected: 0,
            start,
            at,
            client,
            asked: HashSet::new(),
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

    /// Where on screen what is said about the selected completion hangs from:
    /// beside the list, level with its top.
    pub fn beside(&self) -> Point {
        Point::new(self.at.x + WIDTH + GAP, self.at.y)
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
            .filter(|(_, item)| item.filter.to_lowercase().starts_with(&typed))
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

    /// The server that offered the list.
    pub fn client(&self) -> &Arc<Client> {
        &self.client
    }

    /// The selected completion's record, the first time it is asked for, so
    /// that the server can be asked to fill it in once and only once.
    pub fn unasked(&mut self) -> Option<Handle> {
        let index = *self.matched.get(self.selected)?;
        self.asked
            .insert(index)
            .then(|| self.items[index].handle.clone())
    }

    /// Whether the completion `handle` names has been asked to be filled in.
    pub fn is_asked(&self, handle: &Handle) -> bool {
        self.items
            .iter()
            .position(|item| item.handle == *handle)
            .is_some_and(|index| self.asked.contains(&index))
    }

    /// Takes in what the server filled the completion `handle` names in with.
    ///
    /// What it inserts stays as it was offered: a server may only add to an
    /// item when it resolves it, and the list was narrowed by what it said.
    pub fn fill(&mut self, handle: &Handle, filled: Completion) {
        let Some(item) = self.items.iter_mut().find(|item| item.handle == *handle) else {
            return;
        };
        if !filled.detail.is_empty() {
            item.detail = filled.detail;
        }
        if !filled.documentation.is_empty() {
            item.documentation = filled.documentation;
        }
        if !filled.extra.is_empty() {
            item.extra = filled.extra;
        }
    }

    /// What the server says at length about the selected completion.
    pub fn documentation(&self) -> Option<&str> {
        let item = self.at_place(self.selected)?;
        (!item.documentation.is_empty()).then_some(item.documentation.as_str())
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
