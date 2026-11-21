//! The panes of the main area: tab groups laid side by side or one above the other, each tab dragged to another group or to the edge of one to split it.

use crate::areas::{
    BrowserEvent, ChatEvent, DialogEvent, DiffEvent, EditorEvent, SearchEvent, WindowEvent,
};
use crate::target::Target;
use serde::{Deserialize, Serialize};

use crate::sash::Grows;
use crate::state::{Page, Tab, WorkspaceState};
use crate::terminal;

/// How tall the strip of tabs above a pane is, in logical pixels.
pub const TABS: f32 = 48.0;

/// How narrow or short a pane can be made by dragging the handle beside it, in logical pixels.
const SMALLEST: f32 = 160.0;

/// How far into a pane, as a share of its width or height, a tab let go of splits it rather than joining it.
const EDGE: f32 = 0.25;

/// What a tab of a pane shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    /// The page itself: the conversation of a session, the page for starting work or a project's overview.
    Page,
    /// The files the agent changed.
    Diff,
    /// The lines around the hits of a search.
    Excerpts,
    /// A page of the web.
    Browser,
    /// The notes of the newer release.
    Update,
    /// An open file, read from the last commit when `head`.
    File {
        /// The file, from the root of the repository.
        path: String,
        /// Whether it is read from the last commit.
        head: bool,
    },
}

/// A pane: the tabs it holds and the one it shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// Its tabs, in the order they are drawn.
    pub items: Vec<Item>,
    /// The tab it shows, by position.
    pub active: usize,
}

/// Which way the two sides of a split lie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    /// Side by side.
    Row,
    /// One above the other.
    Column,
}

/// How the main area is cut into panes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Layout {
    /// One pane over the whole of its space.
    Group(Group),
    /// Two parts of the space, the first left of or above the second.
    Split {
        /// Which way the parts lie.
        axis: Axis,
        /// The share of the space the first part takes, held as observable state: this
        /// layout's own, which a copy of the layout, such as one kept aside for another
        /// session, holds apart.
        ratio: crate::Held<f32>,
        /// The left or upper part.
        first: Box<Layout>,
        /// The right or lower part.
        second: Box<Layout>,
    },
}

/// The edge of a pane a dragged tab is over, or its middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The middle: the tab joins the pane.
    Center,
    /// The left edge: the pane is split with the tab on the left.
    Left,
    /// The right edge: the pane is split with the tab on the right.
    Right,
    /// The top edge: the pane is split with the tab above.
    Top,
    /// The bottom edge: the pane is split with the tab below.
    Bottom,
}

/// A place in the window, and how big it is, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Area {
    /// The left edge.
    pub x: f32,
    /// The top edge.
    pub y: f32,
    /// How wide it is.
    pub width: f32,
    /// How tall it is.
    pub height: f32,
}

impl Area {
    /// The area with its left edge at `x`, its top edge at `y`, `width` wide and `height` tall.
    fn at(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The part of the area the first or second side of a split along `axis` takes, the first taking `ratio` of it.
    pub fn halves(self, axis: Axis, ratio: f32) -> (Self, Self) {
        self.cut(axis, (self.extent(axis) * ratio).round())
    }

    /// How far the area reaches along `axis`: its width side by side, its height one above the other.
    fn extent(self, axis: Axis) -> f32 {
        match axis {
            Axis::Row => self.width,
            Axis::Column => self.height,
        }
    }

    /// The two parts of the area cut along `axis`, the first `first` logical pixels across.
    fn cut(self, axis: Axis, first: f32) -> (Self, Self) {
        let Self { x, y, .. } = self;
        match axis {
            Axis::Row => (
                Self::at(x, y, first, self.height),
                Self::at(x + first, y, self.width - first, self.height),
            ),
            Axis::Column => (
                Self::at(x, y, self.width, first),
                Self::at(x, y + first, self.width, self.height - first),
            ),
        }
    }

    /// The edge of the area, as a pane with its strip of tabs along the top, that `at` is over: the nearest edge within a share of the area from it, or else the middle.
    pub fn side_at(self, at: (f32, f32)) -> Side {
        let (across, down) = (
            (at.0 - self.x) / self.width.max(1.0),
            (at.1 - self.y - TABS) / (self.height - TABS).max(1.0),
        );
        if at.1 < self.y + TABS {
            return Side::Center;
        }
        [
            (across, Side::Left),
            (1.0 - across, Side::Right),
            (down, Side::Top),
            (1.0 - down, Side::Bottom),
        ]
        .into_iter()
        .filter(|(distance, _)| *distance < EDGE)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(Side::Center, |(_, side)| side)
    }
}

/// A pane laid out: its tabs and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed<'a> {
    /// The pane, by position in the order panes are laid out.
    pub index: usize,
    /// The pane.
    pub group: &'a Group,
    /// Where it is.
    pub area: Area,
}

/// A tab being dragged to another pane or the edge of one.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneDrag {
    /// The tab.
    pub item: Item,
    /// The pane, by position, and the edge of it the tab is over, if it is over one.
    pub over: Option<(usize, Side)>,
    /// Where the pointer is.
    pub at: (f32, f32),
}

impl Default for Layout {
    fn default() -> Self {
        Self::Group(Group::holding(vec![Item::Page]))
    }
}

impl Item {
    /// The tab of the open file `viewer`.
    fn of_viewer(viewer: &crate::open_files::Viewer) -> Self {
        Self::File {
            path: viewer.path.clone(),
            head: viewer.head,
        }
    }
}

impl Group {
    /// A pane holding `items`, showing the first.
    fn holding(items: Vec<Item>) -> Self {
        Self { items, active: 0 }
    }

    /// Shows `item`, when the pane holds it.
    fn show(&mut self, item: &Item) {
        if let Some(at) = self.items.iter().position(|held| held == item) {
            self.active = at;
        }
    }
}

impl Layout {
    /// The split along `axis` giving `first` `ratio` of the space and `second` the rest.
    fn split(axis: Axis, ratio: f32, first: Self, second: Self) -> Self {
        Self::Split {
            axis,
            ratio: crate::Held::new(ratio),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// The split along `axis` of the parts left, or the one part left when the other is gone.
    fn joined(axis: Axis, ratio: f32, first: Option<Self>, second: Option<Self>) -> Option<Self> {
        match (first, second) {
            (Some(first), Some(second)) => Some(Self::split(axis, ratio, first, second)),
            (one, two) => one.or(two),
        }
    }

    /// The panes in the order they are laid out: left to right, top to bottom.
    pub fn groups(&self) -> Vec<&Group> {
        match self {
            Self::Group(group) => vec![group],
            Self::Split { first, second, .. } => {
                let mut groups = first.groups();
                groups.extend(second.groups());
                groups
            }
        }
    }

    /// The panes, to change, in the order they are laid out.
    fn groups_mut(&mut self) -> Vec<&mut Group> {
        match self {
            Self::Group(group) => vec![group],
            Self::Split { first, second, .. } => {
                let mut groups = first.groups_mut();
                groups.extend(second.groups_mut());
                groups
            }
        }
    }

    /// The pane holding `item`, by position.
    pub fn group_of(&self, item: &Item) -> Option<usize> {
        self.groups()
            .iter()
            .position(|group| group.items.contains(item))
    }

    /// The panes laid out in `area`.
    pub fn placed(&self, area: Area) -> Vec<Placed<'_>> {
        let mut placed = Vec::new();
        self.place(area, &mut placed);
        placed
    }

    /// Lays the panes out in `area` after those already in `placed`.
    fn place<'a>(&'a self, area: Area, placed: &mut Vec<Placed<'a>>) {
        match self {
            Self::Group(group) => placed.push(Placed {
                index: placed.len(),
                group,
                area,
            }),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let (one, two) = area.halves(*axis, ratio.get());
                first.place(one, placed);
                second.place(two, placed);
            }
        }
    }

    /// The split whose handle is the `index`-th, laid out in `area`, and the space it cuts, counting in `count` the splits passed.
    fn split_mut(
        &mut self,
        index: usize,
        area: Area,
        count: &mut usize,
    ) -> Option<(&mut Self, Area)> {
        let Self::Split { axis, ratio, .. } = self else {
            return None;
        };
        let (one, two) = area.halves(*axis, ratio.get());
        if *count == index {
            return Some((self, area));
        }
        *count += 1;
        let Self::Split { first, second, .. } = self else {
            return None;
        };
        if let Some(found) = first.split_mut(index, one, count) {
            return Some(found);
        }
        second.split_mut(index, two, count)
    }

    /// The split `index` counts to, in the same order as [`split_mut`](Self::split_mut), with the
    /// part of `area` it cuts.
    fn split_ref(&self, index: usize, area: Area, count: &mut usize) -> Option<(&Self, Area)> {
        let Self::Split {
            axis,
            ratio,
            first,
            second,
        } = self
        else {
            return None;
        };
        let (one, two) = area.halves(*axis, ratio.get());
        if *count == index {
            return Some((self, area));
        }
        *count += 1;
        first
            .split_ref(index, one, count)
            .or_else(|| second.split_ref(index, two, count))
    }

    /// The share of the space the first side of each split takes, first side before second.
    pub fn ratios(&self) -> Vec<f32> {
        match self {
            Self::Group(_) => Vec::new(),
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                let mut ratios = vec![ratio.get()];
                ratios.extend(first.ratios());
                ratios.extend(second.ratios());
                ratios
            }
        }
    }

    /// The layout with `item` taken out of its pane, and the pane taken out with it when that leaves it empty.
    fn without(self, item: &Item) -> Option<Self> {
        match self {
            Self::Group(mut group) => {
                let Some(at) = group.items.iter().position(|held| held == item) else {
                    return Some(Self::Group(group));
                };
                group.items.remove(at);
                if group.active > at || group.active >= group.items.len() {
                    group.active = group.active.saturating_sub(1);
                }
                (!group.items.is_empty()).then_some(Self::Group(group))
            }
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => Self::joined(axis, ratio.get(), first.without(item), second.without(item)),
        }
    }

    /// The layout with the `target`-th pane cut in two along `side`, `leaf` on that side, counting in `count` the panes passed.
    fn split_at(self, target: usize, side: Side, leaf: Group, count: &mut usize) -> Self {
        match self {
            Self::Group(group) => {
                let here = *count == target;
                *count += 1;
                if !here {
                    return Self::Group(group);
                }
                let alone = Self::Group(leaf);
                let held = Self::Group(group);
                let (axis, first, second) = match side {
                    Side::Left => (Axis::Row, alone, held),
                    Side::Right => (Axis::Row, held, alone),
                    Side::Top => (Axis::Column, alone, held),
                    Side::Bottom | Side::Center => (Axis::Column, held, alone),
                };
                Self::split(axis, 0.5, first, second)
            }
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let first = first.split_at(target, side, leaf.clone(), count);
                let second = second.split_at(target, side, leaf, count);
                Self::split(axis, ratio.get(), first, second)
            }
        }
    }

    /// The layout with the `target`-th pane cut in two along `side`, an empty pane on that side, and the position of the empty pane.
    pub fn split_empty(self, target: usize, side: Side) -> (Self, usize) {
        let at = match side {
            Side::Left | Side::Top => target,
            _ => target + 1,
        };
        (
            self.split_at(target, side, Group::holding(Vec::new()), &mut 0),
            at,
        )
    }

    /// The layout with the `target`-th pane taken out, or nothing when it is the only one, counting in `count` the panes passed.
    pub fn without_group(self, target: usize, count: &mut usize) -> Option<Self> {
        match self {
            Self::Group(group) => {
                let here = *count == target;
                *count += 1;
                (!here).then_some(Self::Group(group))
            }
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => Self::joined(
                axis,
                ratio.get(),
                first.without_group(target, count),
                second.without_group(target, count),
            ),
        }
    }

    /// The layout with `item` moved to the `target`-th pane, or to a new pane on `side` of it, and the pane it left taken out when that leaves it empty; nothing changes when the tab would leave a pane only to come back to it.
    pub fn moved(self, item: &Item, target: usize, side: Side) -> Self {
        let Some(from) = self.group_of(item) else {
            return self;
        };
        let Some(alone) = self
            .groups()
            .get(target)
            .map(|group| group.items.iter().all(|held| held == item))
        else {
            return self;
        };
        if from == target && (side == Side::Center || alone) {
            return self;
        }
        let mut left = self;
        if let Some(group) = left.groups_mut().into_iter().nth(from) {
            group.items.retain(|held| held != item);
            group.active = group.active.min(group.items.len().saturating_sub(1));
        }
        let mut moved = match side {
            Side::Center => {
                if let Some(group) = left.groups_mut().into_iter().nth(target) {
                    group.items.push(item.clone());
                }
                left
            }
            side => left.split_at(target, side, Group::holding(vec![item.clone()]), &mut 0),
        };
        let source = if side != Side::Center && from > target {
            from + 1
        } else {
            from
        };
        if moved
            .groups()
            .get(source)
            .is_some_and(|group| group.items.is_empty())
            && let Some(pruned) = moved.clone().without_group(source, &mut 0)
        {
            moved = pruned;
        }
        if let Some(at) = moved.group_of(item)
            && let Some(group) = moved.groups_mut().into_iter().nth(at)
        {
            group.show(item);
        }
        moved
    }

    /// The layout with the tab `from` turned into `to`, in the same place.
    fn replaced(mut self, from: &Item, to: Item) -> Self {
        for group in self.groups_mut() {
            if let Some(held) = group.items.iter_mut().find(|held| *held == from) {
                *held = to;
                break;
            }
        }
        self
    }

    /// The layout holding exactly the tabs in `items`, in their order within each pane: the tabs no longer there taken out, and the missing ones put in the `focused`-th pane, the page and the diff at its front.
    fn reconciled(self, items: &[Item], focused: usize) -> Self {
        let gone: Vec<Item> = self
            .groups()
            .iter()
            .flat_map(|group| group.items.iter())
            .filter(|item| !items.contains(item))
            .cloned()
            .collect();
        let mut layout = gone
            .iter()
            .try_fold(self, |layout, item| layout.without(item))
            .unwrap_or_else(|| Self::Group(Group::holding(Vec::new())));
        let count = layout.groups().len();
        let missing: Vec<Item> = items
            .iter()
            .filter(|item| layout.group_of(item).is_none())
            .cloned()
            .collect();
        let home = layout.group_of(&Item::Page).unwrap_or(0);
        let focused = focused.min(count - 1);
        let holds_file = |group: &&Group| {
            group
                .items
                .iter()
                .any(|item| matches!(item, Item::File { .. }))
        };
        let files = match layout.groups().get(focused) {
            Some(group) if holds_file(group) || group.items.is_empty() => focused,
            _ => layout
                .groups()
                .iter()
                .position(holds_file)
                .unwrap_or(focused),
        };
        for item in missing {
            let to = match item {
                Item::Page | Item::Diff | Item::Excerpts | Item::Browser | Item::Update => home,
                Item::File { .. } => files,
            };
            if let Some(group) = layout.groups_mut().into_iter().nth(to) {
                group.items.push(item);
            }
        }
        for group in layout.groups_mut() {
            let shown = group.items.get(group.active).cloned();
            group
                .items
                .sort_by_key(|item| items.iter().position(|held| held == item));
            group.active = shown
                .and_then(|shown| group.items.iter().position(|held| *held == shown))
                .unwrap_or(0);
        }
        layout
    }
}

/// The panes of the main area of one page, and the one being worked in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panes {
    /// How the area is cut into panes.
    pub layout: Layout,
    /// The pane last worked in, by position, which a file opened goes to.
    pub focused: usize,
    /// While the pane worked in is empty, the tab that was shown when it was chosen; once another tab is shown, the pane holding it is the one worked in.
    pub parked: Option<Item>,
}

impl Panes {
    /// The panes cut as `layout`, the `focused`-th worked in, with the tab `parked` while it is empty.
    fn new(layout: Layout, focused: usize, parked: Option<Item>) -> Self {
        Self {
            layout,
            focused,
            parked,
        }
    }
}

/// The edge the row `id` of the menu of panes splits the pane worked in on, if it is one of its rows.
pub fn split_row_of(id: Target) -> Option<Side> {
    match id {
        Target::Window(WindowEvent::SplitRight) => Some(Side::Right),
        Target::Window(WindowEvent::SplitLeft) => Some(Side::Left),
        Target::Window(WindowEvent::SplitUp) => Some(Side::Top),
        Target::Window(WindowEvent::SplitDown) => Some(Side::Bottom),
        _ => None,
    }
}

impl WorkspaceState {
    /// Whether the page shows its views in panes: a session, a page for starting work or a project's overview.
    pub fn paned(&self) -> bool {
        match (self.page, self.open()) {
            (Page::Session(_), Some(_)) | (Page::Chat, _) => true,
            (Page::New(project) | Page::Project(project), _) => project < self.projects.len(),
            _ => false,
        }
    }

    /// Whether the terminal dock shows under the panes.
    pub fn docking(&self) -> bool {
        self.terminal && self.paned() && self.docked().is_some()
    }

    /// How wide the review panel is, in logical pixels: nothing while it is closed.
    fn review_room(&self) -> f32 {
        if self.review_session().is_some() && self.review {
            self.sizes.review_width.get()
        } else {
            0.0
        }
    }

    /// How wide the plan panel is in a window `width` logical pixels wide: what it was dragged to, or its share of the room the sidebar and the review panel leave; nothing while it is closed.
    pub fn plan_room(&self, width: f32) -> f32 {
        if self.plan_shown().is_some() {
            crate::plan::width_in(self, width - self.sidebar_room() - self.review_room())
        } else {
            0.0
        }
    }

    /// Where the panes are in a window `window` logical pixels wide and tall: right of the sidebar, left of the plan and review panels and above the terminal dock.
    pub fn pane_area(&self, window: (f32, f32)) -> Area {
        let beside = window.0 - self.sidebar_room() - self.review_room();
        let plan = self.plan_room(window.0);
        let page = window.1 - crate::title_bar::TITLE_BAR_HEIGHT;
        let dock = if self.docking() {
            terminal::height(self, page)
        } else {
            0.0
        };
        Area {
            x: self.sidebar_room(),
            y: crate::title_bar::TITLE_BAR_HEIGHT,
            width: beside - plan,
            height: page - dock,
        }
    }

    /// Every tab the page has, in order: the page, the diff while the session changed files, and the open files. A chat has no diff.
    fn pane_items(&self) -> Vec<Item> {
        let diff = self
            .review_session()
            .is_some_and(|session| !session.is_chat());
        let shown = [
            (true, Item::Page),
            (diff, Item::Diff),
            (self.excerpts.is_some(), Item::Excerpts),
            (self.browser.is_some(), Item::Browser),
            (self.update_open, Item::Update),
        ];
        let viewers = self.git.viewers.iter().filter(|viewer| !viewer.hidden);
        let shown = shown
            .into_iter()
            .filter_map(|(on, item)| on.then_some(item));
        shown.chain(viewers.map(Item::of_viewer)).collect()
    }

    /// The tab shown in the pane being worked in.
    pub fn current_item(&self) -> Item {
        match self.tab {
            Tab::File => self.shown_viewer().map_or(Item::Page, Item::of_viewer),
            Tab::Diff if self.pane_items().contains(&Item::Diff) => Item::Diff,
            Tab::Excerpts if self.excerpts.is_some() => Item::Excerpts,
            Tab::Browser if self.browser.is_some() => Item::Browser,
            Tab::Update if self.update_open => Item::Update,
            _ => Item::Page,
        }
    }

    /// The panes as the page stands: every tab it has in one of them, and the tab shown in the pane being worked in shown there.
    pub fn panes(&self) -> Panes {
        let mut layout = self
            .panes
            .layout
            .clone()
            .reconciled(&self.pane_items(), self.panes.focused);
        let current = self.current_item();
        let held = layout.group_of(&current).unwrap_or(0);
        if let Some(group) = layout.groups_mut().into_iter().nth(held) {
            group.show(&current);
        }
        let empty = layout
            .groups()
            .get(self.panes.focused)
            .is_some_and(|group| group.items.is_empty());
        match self.panes.parked.as_ref() {
            Some(parked) if empty && *parked == current => {
                Panes::new(layout, self.panes.focused, Some(current))
            }
            _ => Panes::new(layout, held, None),
        }
    }

    /// Keeps the panes as the page stands, so the pane being worked in is the one a file opened next goes to.
    pub(crate) fn settle_panes(&mut self) {
        if self.paned() {
            self.panes = self.panes();
        }
    }

    /// Puts the open file `to` in the place of the tab of `from`, which it takes over, unless an empty pane is the one worked in, which it goes to instead.
    pub(crate) fn replace_pane_item(&mut self, from: Item, to: Item) {
        if self.panes.parked.is_some() {
            return;
        }
        self.panes.layout = std::mem::take(&mut self.panes.layout).replaced(&from, to);
    }

    /// The tab `id` is in a pane's strip, if it is one.
    pub fn pane_tab_of(&self, id: Target) -> Option<Item> {
        match id {
            Target::Chat(ChatEvent::TabChat) => Some(Item::Page),
            Target::Diff(DiffEvent::TabDiff) => Some(Item::Diff),
            Target::Search(SearchEvent::TabExcerpts) => Some(Item::Excerpts),
            Target::Browser(BrowserEvent::TabBrowser) => Some(Item::Browser),
            Target::Dialog(DialogEvent::TabUpdate) => Some(Item::Update),
            Target::Editor(EditorEvent::FileTab(index)) => {
                Some(Item::of_viewer(self.git.viewers.get(index)?))
            }
            _ => None,
        }
        .filter(|item| self.pane_items().contains(item))
    }

    /// Shows `item` in its pane and makes that pane the one being worked in.
    pub(crate) fn show_item(&mut self, item: &Item) {
        match item {
            Item::Page => self.tab = Tab::Chat,
            Item::Diff => self.tab = Tab::Diff,
            Item::Excerpts => self.tab = Tab::Excerpts,
            Item::Browser => self.tab = Tab::Browser,
            Item::Update => self.tab = Tab::Update,
            Item::File { .. } => {
                if let Some(index) = self
                    .git
                    .viewers
                    .iter()
                    .position(|viewer| Item::of_viewer(viewer) == *item)
                {
                    self.git.shown = Some(index);
                    self.tab = Tab::File;
                    self.ask_shown_figures();
                }
            }
        }
    }

    /// Makes the `index`-th pane the one being worked in, showing the tab it shows, or, when it is empty, keeping it the one a file opened next goes to.
    pub(crate) fn focus_pane(&mut self, index: usize) {
        let panes = self.panes();
        let Some(group) = panes.layout.groups().get(index).copied().cloned() else {
            return;
        };
        match group.items.get(group.active) {
            Some(item) => self.show_item(item),
            None => self.panes = Panes::new(panes.layout, index, Some(self.current_item())),
        }
    }

    /// Carries out the button `id` of an empty pane, if it is one: brings the page, the diff or the browser into it, shows the files to open one in it, or closes it.
    pub(crate) fn press_empty_pane(&mut self, id: Target) -> bool {
        let Target::Window(WindowEvent::PanePut { pane, put }) = id else {
            return false;
        };
        match put {
            0 | 1 | 4 => {
                let item = match put {
                    0 => Item::Page,
                    1 => Item::Diff,
                    _ => Item::Browser,
                };
                if item == Item::Browser && self.browser.is_none() {
                    self.open_browser(crate::browser::HOME);
                }
                let panes = self.panes();
                self.panes = Panes::new(panes.layout.moved(&item, pane, Side::Center), pane, None);
                self.show_item(&item);
                self.settle_panes();
            }
            2 => {
                self.focus_pane(pane);
                self.review = true;
                self.show_review_tab(crate::state::ReviewTab::Files);
            }
            _ => {
                let panes = self.panes();
                if let Some(layout) = panes.layout.without_group(pane, &mut 0) {
                    self.panes = Panes::new(layout, 0, None);
                    self.settle_panes();
                }
            }
        }
        true
    }

    /// The state with the menu of panes open, hanging from its button.
    pub fn with_pane_menu(mut self) -> Self {
        self = self.without_menu();
        self.pane_menu = true;
        self
    }

    /// Cuts the pane being worked in, an empty pane on `side` of it that becomes the one worked in.
    pub(crate) fn split_focused(&mut self, side: Side) {
        let panes = self.panes();
        let (layout, empty) = panes.layout.split_empty(panes.focused, side);
        self.panes = Panes::new(layout, empty, Some(self.current_item()));
    }

    /// The state with the tab `item` being dragged, over the `over` pane and edge, if any, with the pointer at `at`.
    pub fn pane_dragged(mut self, item: Item, over: Option<(usize, Side)>, at: (f32, f32)) -> Self {
        self.pane_drag = Some(PaneDrag { item, over, at });
        self
    }

    /// The state after a dragged tab was let go: moved to the pane or the edge of one it is over, and shown there.
    pub fn pane_dropped(mut self) -> Self {
        let Some(drag) = self.pane_drag.take() else {
            return self;
        };
        let Some((target, side)) = drag.over else {
            return self;
        };
        let panes = self.panes();
        self.panes = Panes::new(
            panes.layout.moved(&drag.item, target, side),
            panes.focused,
            None,
        );
        self.show_item(&drag.item);
        self.settle_panes();
        self
    }

    /// The share of its split the handle `index` gives the first side, how big that side is in a window `window` logical pixels wide and tall, and which way the handle is pulled to grow it.
    pub(crate) fn split_sash(&self, index: usize, window: (f32, f32)) -> Option<(f32, Grows)> {
        let mut layout = self.panes().layout;
        let (split, area) = layout.split_mut(index, self.pane_area(window), &mut 0)?;
        let Layout::Split { axis, ratio, .. } = split else {
            return None;
        };
        Some(match axis {
            Axis::Row => (area.width * ratio.get(), Grows::Right),
            Axis::Column => (area.height * ratio.get(), Grows::Down),
        })
    }

    /// Gives the first side of the split whose handle is `index` `size` logical pixels, within
    /// what leaves both sides room, in a window `window` logical pixels wide and tall, in the
    /// share the held layout keeps as observable state; says whether it could, which it can
    /// while the panes shown are laid out as the ones held.
    pub(crate) fn hold_split(&self, index: usize, size: f32, window: (f32, f32)) -> bool {
        if self.panes() != self.panes {
            return false;
        }
        let area = self.pane_area(window);
        let Some((Layout::Split { axis, ratio, .. }, cut)) =
            self.panes.layout.split_ref(index, area, &mut 0)
        else {
            return false;
        };
        let extent = cut.extent(*axis);
        let room = SMALLEST.min(extent / 2.0);
        ratio.set(size.clamp(room, extent - room) / extent.max(1.0));
        true
    }

    /// Gives the first side of the split whose handle is `index` `size` logical pixels, within what leaves both sides room, in a window `window` logical pixels wide and tall.
    pub(crate) fn resize_split(&mut self, index: usize, size: f32, window: (f32, f32)) {
        let area = self.pane_area(window);
        let mut panes = self.panes();
        if let Some((Layout::Split { axis, ratio, .. }, cut)) =
            panes.layout.split_mut(index, area, &mut 0)
        {
            let extent = cut.extent(*axis);
            let room = SMALLEST.min(extent / 2.0);
            ratio.set(size.clamp(room, extent - room) / extent.max(1.0));
        }
        self.panes = panes;
    }
}
