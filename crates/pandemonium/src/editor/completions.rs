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
///
/// Every server behind the file is asked, and what each offers joins the
/// list as it arrives: a type checker has the names and a linter the
/// fixes, and neither's answer replaces the other's. What each item came
/// from is kept beside it, since only that server can fill it in.
pub struct Completions {
    /// Everything the servers offered, with the server that offered each.
    items: Vec<(Arc<Client>, Completion)>,
    /// Which of them what has been typed since leaves, best first.
    matched: Vec<usize>,
    /// Which of those is selected.
    selected: usize,
    /// Where the word being completed begins.
    start: Position,
    /// Where on screen the list hangs from.
    at: Point,
    /// Which of them their servers have been asked to fill in.
    asked: HashSet<usize>,
    /// The servers whose lists said they were not all there was.
    incomplete: Vec<Arc<Client>>,
    /// What had been typed when the list was last narrowed.
    typed: String,
}

impl Completions {
    /// An empty list for the word beginning at `start`, hung from `at`.
    pub fn new(start: Position, at: Point) -> Self {
        Self {
            items: Vec::new(),
            matched: Vec::new(),
            selected: 0,
            start,
            at,
            asked: HashSet::new(),
            incomplete: Vec::new(),
            typed: String::new(),
        }
    }

    /// Takes in what `client` offered, in place of whatever it offered before.
    ///
    /// The selection stays on the item it was on when that item is still
    /// there, so a second server answering does not move the reader's place.
    pub fn offer(&mut self, client: &Arc<Client>, items: Vec<Completion>, incomplete: bool) {
        let selected = self.selected_index();
        let kept = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, (offered, _))| !Arc::ptr_eq(offered, client))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let selected = selected.and_then(|index| kept.iter().position(|kept| *kept == index));
        self.asked = self
            .asked
            .iter()
            .filter_map(|index| kept.iter().position(|kept| kept == index))
            .collect();
        let mut index = 0;
        self.items.retain(|_| {
            let keep = kept.contains(&index);
            index += 1;
            keep
        });
        self.items
            .extend(items.into_iter().map(|item| (client.clone(), item)));
        self.incomplete
            .retain(|offered| !Arc::ptr_eq(offered, client));
        if incomplete {
            self.incomplete.push(client.clone());
        }
        let typed = std::mem::take(&mut self.typed);
        self.narrow(&typed);
        if let Some(place) =
            selected.and_then(|selected| self.matched.iter().position(|index| *index == selected))
        {
            self.selected = place;
        }
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

    /// The servers whose lists are not all there is, to be asked again as
    /// the reader types on.
    pub fn incomplete(&self) -> &[Arc<Client>] {
        &self.incomplete
    }

    /// Keeps only what still begins with `typed`, in the order offered.
    pub fn narrow(&mut self, typed: &str) {
        self.typed = typed.to_owned();
        let typed = typed.to_lowercase();
        self.matched = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, (_, item))| item.filter.to_lowercase().starts_with(&typed))
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

    /// Which item the selection is on, among everything offered.
    fn selected_index(&self) -> Option<usize> {
        self.matched.get(self.selected).copied()
    }

    /// The `place`-th completion shown, and the server that offered it.
    pub fn at_place(&self, place: usize) -> Option<(&Arc<Client>, &Completion)> {
        let (client, item) = self.items.get(*self.matched.get(place)?)?;
        Some((client, item))
    }

    /// The selected completion's server and record, the first time it is
    /// asked for, so that the server is asked to fill it in once and only once.
    pub fn unasked(&mut self) -> Option<(Arc<Client>, Handle)> {
        let index = self.selected_index()?;
        self.asked.insert(index).then(|| {
            let (client, item) = &self.items[index];
            (client.clone(), item.handle.clone())
        })
    }

    /// Whether the completion `handle` names has been asked to be filled in.
    pub fn is_asked(&self, handle: &Handle) -> bool {
        self.items
            .iter()
            .position(|(_, item)| item.handle == *handle)
            .is_some_and(|index| self.asked.contains(&index))
    }

    /// Takes in what the server filled the completion `handle` names in with.
    ///
    /// What it inserts stays as it was offered: a server may only add to an
    /// item when it resolves it, and the list was narrowed by what it said.
    pub fn fill(&mut self, handle: &Handle, filled: Completion) {
        let Some((_, item)) = self
            .items
            .iter_mut()
            .find(|(_, item)| item.handle == *handle)
        else {
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
        let (_, item) = self.at_place(self.selected)?;
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
            .map(|(place, index)| (place, &self.items[*index].1))
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
