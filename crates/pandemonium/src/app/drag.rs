//! Carrying a tab across the window, and working out where it would land.
//!
//! A drag is answered by the window rather than by the tab that started it:
//! the tab is let go of somewhere else entirely, over another pane or over
//! the edge of one, and only the window knows what is where. Panes and tabs
//! leave their bounds behind as they paint, and this is what reads them back.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use pm_gfx::{Point, Rect};
use pm_ui::Bounds;

use crate::panes::{Item, PaneId, SplitDirection};

/// How near an edge a tab must be let go of to divide the pane it is over.
///
/// A fifth of the pane's shorter side, as Zed has it: near enough to an edge
/// to be deliberate, far enough in that the middle of a pane is a large and
/// forgiving target.
const EDGE: f32 = 0.2;

/// How far a press may travel and still have been a click on the tab.
const SLIP: f32 = 4.0;

/// Where a tab being carried would land if it were let go of now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropPlace {
    /// Among the pane's tabs, at this place in the bar.
    Tab(usize),
    /// Into the pane, as the tab it is showing.
    Into,
    /// Into a new pane, dividing the one it is over.
    Split(SplitDirection),
}

/// A tab under the pointer, and where letting go of it would put it.
#[derive(Clone, Copy, Debug)]
pub struct TabDrag {
    /// The pane the tab was picked up from.
    pub from: PaneId,
    /// What that tab holds.
    pub item: Item,
    /// Where the pointer is now.
    pub at: Point,
    /// How far the pointer has travelled since the press.
    pub travelled: f32,
    /// The pane it is over and where in it, if it is over one at all.
    pub target: Option<(PaneId, DropPlace)>,
}

impl TabDrag {
    /// Whether the pointer has gone far enough to have carried the tab.
    ///
    /// A press that travels no further than a few pixels is the hand not
    /// being quite still, not a tab being moved: it selects the tab.
    pub fn is_carried(&self) -> bool {
        self.travelled > SLIP
    }
}

/// A cell for something that has not been painted yet.
pub fn unmeasured() -> Bounds {
    Rc::new(Cell::new(Rect::from_xywh(0.0, 0.0, 0.0, 0.0)))
}

/// Where the panes and their tabs came out in the last frame.
///
/// Layout is the element tree's, and it is over by the time a drop has to be
/// answered, so every pane and every tab leaves its bounds in a cell here as
/// it paints. The cells are handed out while the frame is built and read back
/// when the pointer is let go of.
#[derive(Default)]
pub struct Geometry {
    /// Where each pane was painted.
    panes: BTreeMap<PaneId, Bounds>,
    /// Where each tab of each pane was painted.
    tabs: BTreeMap<(PaneId, Item), Bounds>,
    /// Where each pane's bar of tabs was painted.
    bars: BTreeMap<PaneId, Bounds>,
}

impl Geometry {
    /// The cell pane `id` writes its bounds into.
    pub fn pane(&mut self, id: PaneId) -> Bounds {
        Self::cell(&mut self.panes, id)
    }

    /// The cell the bar of tabs of pane `id` writes its bounds into.
    pub fn bar(&mut self, id: PaneId) -> Bounds {
        Self::cell(&mut self.bars, id)
    }

    /// The cell the tab holding `item` in pane `id` writes its bounds into.
    pub fn tab(&mut self, id: PaneId, item: Item) -> Bounds {
        Self::cell(&mut self.tabs, (id, item))
    }

    /// The cell `key` writes into, starting one where there is none.
    fn cell<K: Ord>(cells: &mut BTreeMap<K, Bounds>, key: K) -> Bounds {
        cells.entry(key).or_insert_with(unmeasured).clone()
    }

    /// Forgets the panes and tabs that were not drawn in the last frame.
    pub fn keep(&mut self, panes: &[PaneId], tabs: &[(PaneId, Item)]) {
        self.panes.retain(|id, _| panes.contains(id));
        self.bars.retain(|id, _| panes.contains(id));
        self.tabs.retain(|key, _| tabs.contains(key));
    }

    /// Where the tab of `pane` holding `item` was painted.
    pub fn tab_bounds(&self, pane: PaneId, item: Item) -> Option<Rect> {
        self.tabs.get(&(pane, item)).map(|cell| cell.get())
    }

    /// Where `pane` was painted.
    pub fn pane_bounds(&self, pane: PaneId) -> Option<Rect> {
        self.panes.get(&pane).map(|cell| cell.get())
    }

    /// Where the bar of tabs of `pane` was painted.
    pub fn bar_bounds(&self, pane: PaneId) -> Option<Rect> {
        self.bars.get(&pane).map(|cell| cell.get())
    }

    /// The pane `point` falls inside.
    pub fn pane_at(&self, point: Point) -> Option<PaneId> {
        self.panes
            .iter()
            .find(|(_, cell)| cell.get().contains(point))
            .map(|(pane, _)| *pane)
    }

    /// The pane under `point`, and where in it a tab let go of there lands.
    ///
    /// A bar of tabs answers first, because a tab dropped on the bar belongs
    /// among its tabs rather than in the pane behind it; everything else is
    /// the pane itself, divided when the pointer is near one of its edges.
    pub fn target_at(
        &self,
        point: Point,
        order: &dyn Fn(PaneId) -> Vec<Item>,
    ) -> Option<(PaneId, DropPlace)> {
        if let Some((pane, place)) = self.tab_place(point, order) {
            return Some((pane, place));
        }
        let pane = self.pane_at(point)?;
        Some((pane, place_in(self.pane_bounds(pane)?, point)))
    }

    /// The line marking place `index` in the bar of tabs of `pane`.
    pub fn caret(&self, pane: PaneId, index: usize, tabs: &[Item]) -> Option<Rect> {
        let bar = self.bar_bounds(pane)?;
        let x = match tabs.get(index) {
            Some(item) => self.tab_bounds(pane, *item)?.left(),
            None => match tabs.last() {
                Some(item) => {
                    let bounds = self.tab_bounds(pane, *item)?;
                    bounds.left() + bounds.size.width
                }
                None => bar.left(),
            },
        };
        Some(Rect::from_xywh(x, bar.top(), CARET, bar.size.height))
    }

    /// Where in a bar of tabs `point` falls, if it falls in one at all.
    fn tab_place(
        &self,
        point: Point,
        order: &dyn Fn(PaneId) -> Vec<Item>,
    ) -> Option<(PaneId, DropPlace)> {
        let (pane, _) = self
            .bars
            .iter()
            .map(|(pane, cell)| (*pane, cell.get()))
            .find(|(_, bounds)| bounds.contains(point))?;

        let tabs = order(pane);
        let place = tabs
            .iter()
            .position(|item| {
                self.tab_bounds(pane, *item)
                    .is_some_and(|bounds| point.x < bounds.left() + bounds.size.width / 2.0)
            })
            .unwrap_or(tabs.len());
        Some((pane, DropPlace::Tab(place)))
    }
}

/// Width of the line marking where a carried tab would be inserted.
const CARET: f32 = 2.0;

/// Where in `bounds` the point falls: into the pane, or against an edge.
fn place_in(bounds: Rect, point: Point) -> DropPlace {
    let size = bounds.size;
    let margin = size.width.min(size.height) * EDGE;
    let left = point.x - bounds.left();
    let right = bounds.left() + size.width - point.x;
    let top = point.y - bounds.top();
    let bottom = bounds.top() + size.height - point.y;

    let nearest = [
        (left, SplitDirection::Left),
        (right, SplitDirection::Right),
        (top, SplitDirection::Up),
        (bottom, SplitDirection::Down),
    ]
    .into_iter()
    .min_by(|one, two| one.0.total_cmp(&two.0));

    match nearest {
        Some((distance, direction)) if distance < margin => DropPlace::Split(direction),
        _ => DropPlace::Into,
    }
}

/// The part of `bounds` a drop in `place` would take over.
///
/// The window paints this over the pane the pointer is over, which is how a
/// drag says where it would land before it lands: the whole pane for a tab
/// joining it, the half it would take for a split, a line for a place in a
/// bar of tabs.
pub fn highlight(bounds: Rect, place: DropPlace) -> Rect {
    let (width, height) = (bounds.size.width, bounds.size.height);
    match place {
        DropPlace::Tab(_) | DropPlace::Into => bounds,
        DropPlace::Split(SplitDirection::Left) => {
            Rect::from_xywh(bounds.left(), bounds.top(), width / 2.0, height)
        }
        DropPlace::Split(SplitDirection::Right) => Rect::from_xywh(
            bounds.left() + width / 2.0,
            bounds.top(),
            width / 2.0,
            height,
        ),
        DropPlace::Split(SplitDirection::Up) => {
            Rect::from_xywh(bounds.left(), bounds.top(), width, height / 2.0)
        }
        DropPlace::Split(SplitDirection::Down) => Rect::from_xywh(
            bounds.left(),
            bounds.top() + height / 2.0,
            width,
            height / 2.0,
        ),
    }
}
