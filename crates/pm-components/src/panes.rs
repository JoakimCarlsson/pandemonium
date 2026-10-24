//! The panes of the main area: tab groups laid side by side or one above the other, each tab dragged to another group or to the edge of one to split it.

use pm_input::FocusId;

use crate::open_files::FILE_TAB_BASE;
use crate::sash::Grows;
use crate::state::{Page, TAB_CHAT, TAB_DIFF, Tab, WorkspaceState};
use crate::terminal;

/// The first of the boxes that are the panes; the rest follow it in the order the panes are laid out.
pub const PANE_BASE: u64 = 4_500_000;

/// The first of the handles between two panes; the rest follow it in the order the splits are laid out.
pub const SPLIT_SASH_BASE: u64 = 4_600_000;

/// The first of the scrolling boxes of files shown in a pane other than the focused one; the rest follow it by the file's position.
pub const PEEK_SCROLL_BASE: u64 = 4_700_000;

/// The button in the strip of tabs that opens the menu of panes.
pub const PANE_MENU: FocusId = FocusId(7096);

/// The row of the menu of panes that splits the pane worked in with its tab on the right.
pub const SPLIT_RIGHT: FocusId = FocusId(7097);

/// The row of the menu of panes that splits the pane worked in with its tab on the left.
pub const SPLIT_LEFT: FocusId = FocusId(7098);

/// The row of the menu of panes that splits the pane worked in with its tab above.
pub const SPLIT_UP: FocusId = FocusId(7099);

/// The row of the menu of panes that splits the pane worked in with its tab below.
pub const SPLIT_DOWN: FocusId = FocusId(7100);

/// The first of the buttons of empty panes; each pane has four, by its position: bring the page in, bring the diff in, show the files, close it.
pub const PANE_PUT_BASE: u64 = 4_800_000;

/// How many ids each range above holds.
const RANGE: u64 = 100_000;

/// How tall the strip of tabs above a pane is, in logical pixels.
pub const TABS: f32 = 48.0;

/// How narrow or short a pane can be made by dragging the handle beside it, in logical pixels.
const SMALLEST: f32 = 160.0;

/// How far into a pane, as a share of its width or height, a tab let go of splits it rather than joining it.
const EDGE: f32 = 0.25;

/// What a tab of a pane shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// The page itself: the conversation of a session, the page for starting work or a project's overview.
    Page,
    /// The files the agent changed.
    Diff,
    /// The lines around the hits of a search.
    Excerpts,
    /// An open file, read from the last commit when `head`.
    File {
        /// The file, from the root of the repository.
        path: String,
        /// Whether it is read from the last commit.
        head: bool,
    },
}

/// A pane: the tabs it holds and the one it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Its tabs, in the order they are drawn.
    pub items: Vec<Item>,
    /// The tab it shows, by position.
    pub active: usize,
}

/// Which way the two sides of a split lie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Side by side.
    Row,
    /// One above the other.
    Column,
}

/// How the main area is cut into panes.
#[derive(Clone, Debug, PartialEq)]
pub enum Layout {
    /// One pane over the whole of its space.
    Group(Group),
    /// Two parts of the space, the first left of or above the second.
    Split {
        /// Which way the parts lie.
        axis: Axis,
        /// The share of the space the first part takes.
        ratio: f32,
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
    /// The part of the area the first or second side of a split along `axis` takes, the first taking `ratio` of it.
    pub fn halves(self, axis: Axis, ratio: f32) -> (Self, Self) {
        match axis {
            Axis::Row => {
                let first = (self.width * ratio).round();
                (
                    Self {
                        width: first,
                        ..self
                    },
                    Self {
                        x: self.x + first,
                        width: self.width - first,
                        ..self
                    },
                )
            }
            Axis::Column => {
                let first = (self.height * ratio).round();
                (
                    Self {
                        height: first,
                        ..self
                    },
                    Self {
                        y: self.y + first,
                        height: self.height - first,
                        ..self
                    },
                )
            }
        }
    }

    /// The part of the area a tab let go of on `side` would take.
    pub fn of_side(self, side: Side) -> Self {
        let (wide, tall) = (self.width / 2.0, self.height / 2.0);
        match side {
            Side::Center => self,
            Side::Left => Self {
                width: wide,
                ..self
            },
            Side::Right => Self {
                x: self.x + wide,
                width: wide,
                ..self
            },
            Side::Top => Self {
                height: tall,
                ..self
            },
            Side::Bottom => Self {
                y: self.y + tall,
                height: tall,
                ..self
            },
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

/// A handle between the two sides of a split, laid out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seam {
    /// The handle.
    pub id: FocusId,
    /// Which way the sides lie.
    pub axis: Axis,
    /// Where its line starts.
    pub at: (f32, f32),
    /// How long it is.
    pub length: f32,
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
        Self::Group(Group {
            items: vec![Item::Page],
            active: 0,
        })
    }
}

impl Layout {
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
                let (one, two) = area.halves(*axis, *ratio);
                first.place(one, placed);
                second.place(two, placed);
            }
        }
    }

    /// The handles between the sides of every split, laid out in `area`.
    pub fn seams(&self, area: Area) -> Vec<Seam> {
        let mut seams = Vec::new();
        self.seam(area, &mut seams);
        seams
    }

    /// Lays out the handles of the splits in `area` after those already in `seams`.
    fn seam(&self, area: Area, seams: &mut Vec<Seam>) {
        if let Self::Split {
            axis,
            ratio,
            first,
            second,
        } = self
        {
            let (one, two) = area.halves(*axis, *ratio);
            let id = FocusId(SPLIT_SASH_BASE + seams.len() as u64);
            seams.push(match axis {
                Axis::Row => Seam {
                    id,
                    axis: *axis,
                    at: (two.x, two.y),
                    length: two.height,
                },
                Axis::Column => Seam {
                    id,
                    axis: *axis,
                    at: (two.x, two.y),
                    length: two.width,
                },
            });
            first.seam(one, seams);
            second.seam(two, seams);
        }
    }

    /// The split whose handle is the `index`-th, laid out in `area`, and the space it cuts.
    fn split_mut(&mut self, index: usize, area: Area) -> Option<(&mut Self, Area)> {
        let mut count = 0;
        self.find_split(index, area, &mut count)
    }

    /// Looks for the `index`-th split in `area`, counting in `count` the splits passed.
    fn find_split(
        &mut self,
        index: usize,
        area: Area,
        count: &mut usize,
    ) -> Option<(&mut Self, Area)> {
        let Self::Split { axis, ratio, .. } = self else {
            return None;
        };
        let (one, two) = area.halves(*axis, *ratio);
        if *count == index {
            return Some((self, area));
        }
        *count += 1;
        let Self::Split { first, second, .. } = self else {
            return None;
        };
        if let Some(found) = first.find_split(index, one, count) {
            return Some(found);
        }
        second.find_split(index, two, count)
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
            } => match (first.without(item), second.without(item)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (one, two) => one.or(two),
            },
        }
    }

    /// The layout with the `target`-th pane cut in two along `side`, `leaf` on that side.
    fn split_at(self, target: usize, side: Side, leaf: Group) -> Self {
        let mut count = 0;
        self.split_counting(target, side, leaf, &mut count)
    }

    /// Cuts the `target`-th pane, counting in `count` the panes passed.
    fn split_counting(self, target: usize, side: Side, leaf: Group, count: &mut usize) -> Self {
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
                Self::Split {
                    axis,
                    ratio: 0.5,
                    first: Box::new(first),
                    second: Box::new(second),
                }
            }
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let first = first.split_counting(target, side, leaf.clone(), count);
                let second = second.split_counting(target, side, leaf, count);
                Self::Split {
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }
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
            self.split_at(
                target,
                side,
                Group {
                    items: Vec::new(),
                    active: 0,
                },
            ),
            at,
        )
    }

    /// The layout with the `target`-th pane taken out, or nothing when it is the only one.
    pub fn without_group(self, target: usize) -> Option<Self> {
        let mut count = 0;
        self.without_counting(target, &mut count)
    }

    /// Takes the `target`-th pane out, counting in `count` the panes passed.
    fn without_counting(self, target: usize, count: &mut usize) -> Option<Self> {
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
            } => match (
                first.without_counting(target, count),
                second.without_counting(target, count),
            ) {
                (Some(first), Some(second)) => Some(Self::Split {
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (one, two) => one.or(two),
            },
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
            side => left.split_at(
                target,
                side,
                Group {
                    items: vec![item.clone()],
                    active: 0,
                },
            ),
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
            && let Some(pruned) = moved.clone().without_group(source)
        {
            moved = pruned;
        }
        if let Some(at) = moved.group_of(item)
            && let Some(group) = moved.groups_mut().into_iter().nth(at)
        {
            group.active = group
                .items
                .iter()
                .position(|held| held == item)
                .unwrap_or(0);
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
            .unwrap_or_else(|| {
                Self::Group(Group {
                    items: Vec::new(),
                    active: 0,
                })
            });
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
                Item::Page | Item::Diff | Item::Excerpts => home,
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

/// The edge the row `id` of the menu of panes splits the pane worked in on, if it is one of its rows.
pub fn split_row_of(id: FocusId) -> Option<Side> {
    match id {
        SPLIT_RIGHT => Some(Side::Right),
        SPLIT_LEFT => Some(Side::Left),
        SPLIT_UP => Some(Side::Top),
        SPLIT_DOWN => Some(Side::Bottom),
        _ => None,
    }
}

/// Which handle between panes `id` is, if it is one.
pub fn split_sash_of(id: FocusId) -> Option<usize> {
    id.0.checked_sub(SPLIT_SASH_BASE)
        .filter(|offset| *offset < RANGE)
        .map(|offset| offset as usize)
}

/// Which pane `id` is the box of, if it is one.
pub fn pane_of(id: FocusId) -> Option<usize> {
    id.0.checked_sub(PANE_BASE)
        .filter(|offset| *offset < RANGE)
        .map(|offset| offset as usize)
}

/// Which open file `id` scrolls while it is shown in a pane other than the focused one, by position.
pub fn peek_of(id: FocusId) -> Option<usize> {
    id.0.checked_sub(PEEK_SCROLL_BASE)
        .filter(|offset| *offset < RANGE)
        .map(|offset| offset as usize)
}

impl WorkspaceState {
    /// Whether the page shows its views in panes: a session, the page for starting work or a project's overview.
    pub fn paned(&self) -> bool {
        !self.projects.is_empty()
            && matches!(
                (self.page, self.open()),
                (Page::Session(_), Some(_)) | (Page::New(_) | Page::Project(_), _)
            )
    }

    /// Whether the terminal dock shows under the panes.
    pub fn docking(&self) -> bool {
        self.terminal && self.paned() && self.docked().is_some()
    }

    /// Where the panes are in a window `window` logical pixels wide and tall: right of the sidebar, left of the plan and review panels and above the terminal dock.
    pub fn pane_area(&self, window: (f32, f32)) -> Area {
        let review = if self.review_session().is_some() && self.review {
            self.review_width
        } else {
            0.0
        };
        let beside = window.0 - self.sidebar_room() - review;
        let plan = if self.plan_shown().is_some() {
            crate::plan::width_in(self, beside)
        } else {
            0.0
        };
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

    /// Every tab the page has, in order: the page, the diff while the session changed files, and the open files.
    fn pane_items(&self) -> Vec<Item> {
        let mut items = vec![Item::Page];
        if self.review_session().is_some() {
            items.push(Item::Diff);
        }
        if self.excerpts.is_some() {
            items.push(Item::Excerpts);
        }
        items.extend(
            self.git
                .viewers
                .iter()
                .filter(|viewer| !viewer.hidden)
                .map(|viewer| Item::File {
                    path: viewer.path.clone(),
                    head: viewer.head,
                }),
        );
        items
    }

    /// The tab shown in the pane being worked in.
    pub fn current_item(&self) -> Item {
        match self.tab {
            Tab::File => self.shown_viewer().map_or(Item::Page, |viewer| Item::File {
                path: viewer.path.clone(),
                head: viewer.head,
            }),
            Tab::Diff if self.pane_items().contains(&Item::Diff) => Item::Diff,
            Tab::Excerpts if self.excerpts.is_some() => Item::Excerpts,
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
            group.active = group
                .items
                .iter()
                .position(|item| *item == current)
                .unwrap_or(group.active);
        }
        let empty = layout
            .groups()
            .get(self.panes.focused)
            .is_some_and(|group| group.items.is_empty());
        match self.panes.parked.as_ref() {
            Some(parked) if empty && *parked == current => Panes {
                layout,
                focused: self.panes.focused,
                parked: Some(current),
            },
            _ => Panes {
                layout,
                focused: held,
                parked: None,
            },
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

    /// The position of the open file `item` names, if it names one.
    fn viewer_index(&self, item: &Item) -> Option<usize> {
        let Item::File { path, head } = item else {
            return None;
        };
        self.git
            .viewers
            .iter()
            .position(|viewer| viewer.path == *path && viewer.head == *head)
    }

    /// The tab `id` is in a pane's strip, if it is one.
    pub fn pane_tab_of(&self, id: FocusId) -> Option<Item> {
        match id {
            TAB_CHAT => Some(Item::Page),
            TAB_DIFF => Some(Item::Diff),
            crate::excerpts::TAB_EXCERPTS => Some(Item::Excerpts),
            _ => {
                let index = id.0.checked_sub(FILE_TAB_BASE)? as usize;
                let viewer = self.git.viewers.get(index)?;
                Some(Item::File {
                    path: viewer.path.clone(),
                    head: viewer.head,
                })
            }
        }
        .filter(|item| self.pane_items().contains(item))
    }

    /// Shows `item` in its pane and makes that pane the one being worked in.
    fn show_item(&mut self, item: &Item) {
        match item {
            Item::Page => self.tab = Tab::Chat,
            Item::Diff => self.tab = Tab::Diff,
            Item::Excerpts => self.tab = Tab::Excerpts,
            Item::File { .. } => {
                if let Some(index) = self.viewer_index(item) {
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
            None => {
                self.panes = Panes {
                    focused: index,
                    parked: Some(self.current_item()),
                    ..panes
                }
            }
        }
    }

    /// Carries out the button `id` of an empty pane, if it is one: brings the page or the diff into it, shows the files to open one in it, or closes it.
    pub(crate) fn press_empty_pane(&mut self, id: FocusId) -> bool {
        let Some(offset) =
            id.0.checked_sub(PANE_PUT_BASE)
                .filter(|offset| *offset < RANGE)
        else {
            return false;
        };
        let (pane, put) = ((offset / 4) as usize, offset % 4);
        match put {
            0 | 1 => {
                let item = if put == 0 { Item::Page } else { Item::Diff };
                let panes = self.panes();
                self.panes = Panes {
                    layout: panes.layout.moved(&item, pane, Side::Center),
                    focused: pane,
                    parked: None,
                };
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
                if let Some(layout) = panes.layout.without_group(pane) {
                    self.panes = Panes {
                        layout,
                        focused: 0,
                        parked: None,
                    };
                    self.settle_panes();
                }
            }
        }
        true
    }

    /// The state with the menu of panes open with its corner at `at`.
    pub fn with_pane_menu(mut self, at: (f32, f32)) -> Self {
        self = self.without_menu();
        self.pane_menu = Some(at);
        self
    }

    /// Cuts the pane being worked in, an empty pane on `side` of it that becomes the one worked in.
    pub(crate) fn split_focused(&mut self, side: Side) {
        let panes = self.panes();
        let (layout, empty) = panes.layout.split_empty(panes.focused, side);
        self.panes = Panes {
            layout,
            focused: empty,
            parked: Some(self.current_item()),
        };
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
        self.panes = Panes {
            layout: panes.layout.moved(&drag.item, target, side),
            focused: panes.focused,
            parked: None,
        };
        self.show_item(&drag.item);
        self.settle_panes();
        self
    }

    /// The share of its split the handle `index` gives the first side, how big that side is in a window `window` logical pixels wide and tall, and which way the handle is pulled to grow it.
    pub(crate) fn split_sash(&self, index: usize, window: (f32, f32)) -> Option<(f32, Grows)> {
        let mut layout = self.panes().layout;
        let (split, area) = layout.split_mut(index, self.pane_area(window))?;
        let Layout::Split { axis, ratio, .. } = split else {
            return None;
        };
        Some(match axis {
            Axis::Row => (area.width * *ratio, Grows::Right),
            Axis::Column => (area.height * *ratio, Grows::Down),
        })
    }

    /// Gives the first side of the split whose handle is `index` `size` logical pixels, within what leaves both sides room, in a window `window` logical pixels wide and tall.
    pub(crate) fn resize_split(&mut self, index: usize, size: f32, window: (f32, f32)) {
        let area = self.pane_area(window);
        let mut panes = self.panes();
        if let Some((Layout::Split { axis, ratio, .. }, cut)) = panes.layout.split_mut(index, area)
        {
            let extent = match axis {
                Axis::Row => cut.width,
                Axis::Column => cut.height,
            };
            let room = SMALLEST.min(extent / 2.0);
            *ratio = size.clamp(room, extent - room) / extent.max(1.0);
        }
        self.panes = panes;
    }

    /// The state after the file at `index` in a pane other than the focused one is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_peek(mut self, index: usize, delta: f32, room: f32) -> Self {
        if let Some(viewer) = self.git.viewers.get_mut(index) {
            viewer.scroll = (viewer.scroll + delta).clamp(0.0, room);
        }
        self
    }
}
