//! What could be written where the cursor is, and the list that offers it.
//!
//! The list stays up while the reader goes on typing: every keystroke goes
//! into the buffer as it always would and narrows the list afterwards, which
//! is what makes completion something that happens beside the typing rather
//! than instead of it. It closes when nothing matches any more, or when the
//! cursor leaves the word it was offered for.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use pm_gfx::{Point, Rgba};
use pm_text::{Client, Completion, CompletionKind, Handle, Position};
use pm_ui::{Div, IconName, IconSize, Styled, Theme, h_flex, icon, text, v_flex};

use super::fuzzy;
use crate::message::Message;

/// Widest the list is drawn.
const WIDTH: f32 = 420.0;

/// How far from the list what it says about the selected completion is drawn.
const GAP: f32 = 4.0;

/// Height of one row of it.
const ROW_HEIGHT: f32 = 24.0;

/// Most rows drawn at once, however many the server offered.
const VISIBLE: usize = 10;

/// What identifies a completion across the lists a server sends as the
/// reader types: the same name of the same kind with the same signature is
/// the same thing, whichever list it arrives in.
type Key = (CompletionKind, String, String);

/// What a server filled a completion in with, kept for as long as the list
/// is up so that a list offered anew does not lose it.
struct Filled {
    /// The type or signature it said.
    detail: String,
    /// What it said at length.
    documentation: String,
    /// The edits elsewhere it brings along.
    extra: Vec<(std::ops::Range<Position>, String)>,
}

/// The identity of `item` across lists.
fn key(item: &Completion) -> Key {
    (item.kind, item.label.clone(), item.signature.clone())
}

/// Where a kind of completion stands among the others when nothing typed
/// ranks them apart: names first, then keywords, then what is called, then
/// the types, then the rest.
fn rank(kind: CompletionKind) -> u8 {
    match kind {
        CompletionKind::Variable
        | CompletionKind::Constant
        | CompletionKind::Value
        | CompletionKind::Field
        | CompletionKind::Property
        | CompletionKind::EnumMember => 0,
        CompletionKind::Keyword | CompletionKind::Snippet => 1,
        CompletionKind::Function | CompletionKind::Method | CompletionKind::Constructor => 2,
        CompletionKind::Class
        | CompletionKind::Struct
        | CompletionKind::Interface
        | CompletionKind::Enum
        | CompletionKind::TypeParameter => 3,
        _ => 4,
    }
}

/// The icon a kind of completion is drawn with.
fn kind_icon(kind: CompletionKind) -> IconName {
    match kind {
        CompletionKind::Function => IconName::SquareFunction,
        CompletionKind::Method | CompletionKind::Constructor => IconName::Box,
        CompletionKind::Field => IconName::SquareDot,
        CompletionKind::Variable => IconName::Variable,
        CompletionKind::Class => IconName::Component,
        CompletionKind::Interface => IconName::Plug,
        CompletionKind::Module | CompletionKind::Folder => IconName::Package,
        CompletionKind::Property => IconName::Wrench,
        CompletionKind::Unit => IconName::Ruler,
        CompletionKind::Value | CompletionKind::Constant => IconName::Hash,
        CompletionKind::Enum => IconName::List,
        CompletionKind::EnumMember => IconName::CircleDot,
        CompletionKind::Keyword => IconName::KeyRound,
        CompletionKind::Snippet => IconName::FileCode,
        CompletionKind::Color => IconName::Palette,
        CompletionKind::File | CompletionKind::Reference => IconName::File,
        CompletionKind::Struct => IconName::Blocks,
        CompletionKind::Event => IconName::Zap,
        CompletionKind::Operator => IconName::Percent,
        CompletionKind::TypeParameter => IconName::Type,
        CompletionKind::Other => IconName::Circle,
    }
}

/// The colour a kind of completion's icon is drawn in, from the syntax
/// colours the editor paints the same things with.
fn kind_color(theme: &Theme, kind: CompletionKind) -> Rgba {
    let syntax = &theme.syntax;
    match kind {
        CompletionKind::Variable => syntax.variable,
        CompletionKind::Field | CompletionKind::Property => syntax.property,
        CompletionKind::Method | CompletionKind::Function | CompletionKind::Constructor => {
            syntax.function
        }
        CompletionKind::Class
        | CompletionKind::Struct
        | CompletionKind::Interface
        | CompletionKind::Enum
        | CompletionKind::TypeParameter => syntax.type_name,
        CompletionKind::Keyword | CompletionKind::Operator => syntax.keyword,
        CompletionKind::Constant | CompletionKind::Value | CompletionKind::EnumMember => {
            syntax.constant
        }
        CompletionKind::Snippet | CompletionKind::Color => syntax.string,
        CompletionKind::Module | CompletionKind::Event => syntax.attribute,
        _ => theme.colors.text_muted,
    }
}

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
    /// What the servers have filled items in with, by what the items are.
    filled: HashMap<Key, Filled>,
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
            filled: HashMap::new(),
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
        for mut item in items {
            if let Some(filled) = self.filled.get(&key(&item)) {
                Self::apply(&mut item, filled);
                self.asked.insert(self.items.len());
            }
            self.items.push((client.clone(), item));
        }
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

    /// Keeps only what `typed` matches, best match first and, among equals,
    /// by kind and then by what the server ranked them by.
    pub fn narrow(&mut self, typed: &str) {
        self.typed = typed.to_owned();
        let mut scored = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, (_, item))| {
                fuzzy::score(typed, &item.filter).map(|score| (index, score))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|(a, a_score), (b, b_score)| {
            let (a, b) = (&self.items[*a].1, &self.items[*b].1);
            b_score
                .cmp(a_score)
                .then_with(|| rank(a.kind).cmp(&rank(b.kind)))
                .then_with(|| a.sort.cmp(&b.sort))
                .then_with(|| a.label.cmp(&b.label))
        });
        self.matched = scored.into_iter().map(|(index, _)| index).collect();
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

    /// The servers and records of the rows shown that have not been asked
    /// about yet, the selected one first, so that each is asked once and only
    /// once.
    pub fn unasked(&mut self) -> Vec<(Arc<Client>, Handle)> {
        let first = self.selected.saturating_sub(VISIBLE - 1);
        let mut wanted = self
            .matched
            .iter()
            .skip(first)
            .take(VISIBLE)
            .copied()
            .collect::<Vec<_>>();
        if let Some(selected) = self.selected_index() {
            wanted.retain(|index| *index != selected);
            wanted.insert(0, selected);
        }
        wanted
            .into_iter()
            .filter(|index| self.asked.insert(*index))
            .map(|index| {
                let (client, item) = &self.items[index];
                (client.clone(), item.handle.clone())
            })
            .collect()
    }

    /// Whether the completion `handle` names has been asked to be filled in.
    pub fn is_asked(&self, handle: &Handle) -> bool {
        self.items
            .iter()
            .position(|(_, item)| item.handle == *handle)
            .is_some_and(|index| self.asked.contains(&index))
    }

    /// Takes in what the server filled in the completion it resolved with.
    ///
    /// What it inserts stays as it was offered: a server may only add to an
    /// item when it resolves it, and the list was narrowed by what it said.
    /// What it said is kept by what the item is, so a list offered anew as
    /// the reader types shows it again without asking.
    pub fn fill(&mut self, filled: Completion) {
        let identity = key(&filled);
        let said = Filled {
            detail: filled.detail,
            documentation: filled.documentation,
            extra: filled.extra,
        };
        for (_, item) in self
            .items
            .iter_mut()
            .filter(|(_, item)| key(item) == identity)
        {
            Self::apply(item, &said);
        }
        self.filled.insert(identity, said);
    }

    /// Puts what a server filled in into `item`, keeping what it left empty.
    fn apply(item: &mut Completion, filled: &Filled) {
        if !filled.detail.is_empty() {
            item.detail = filled.detail.clone();
        }
        if !filled.documentation.is_empty() {
            item.documentation = filled.documentation.clone();
        }
        if !filled.extra.is_empty() {
            item.extra = filled.extra.clone();
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

/// Builds one row of the list, lit while it is the selected one: the icon of
/// its kind, its name, the signature straight after it and its type at the
/// far end.
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
        .child(
            icon(kind_icon(item.kind))
                .size(IconSize::Small)
                .color(kind_color(theme, item.kind)),
        )
        .child(
            h_flex()
                .items_center()
                .overflow_hidden()
                .child(text(item.label.clone()).text_sm().font_mono())
                .child(
                    text(item.signature.clone())
                        .text_xs()
                        .font_mono()
                        .color(theme.colors.text_muted),
                ),
        )
        .child(h_flex().flex_1())
        .child(
            text(item.detail.clone())
                .text_xs()
                .color(theme.colors.text_muted),
        )
}
