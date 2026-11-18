//! The handles on the edges between panels, dragged to change how big a panel is, and the sizes they set.

use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::kit::{FocusId, Hovered, Interactive, Looks, Role, Styled};

use crate::git_controls::GRAPH_SASH;
use crate::state::WorkspaceState;
use crate::{graph, plan, review, sidebar, terminal};

/// The handle on the right edge of the sidebar.
pub const SIDEBAR_SASH: FocusId = FocusId(7090);

/// The handle on the left edge of the review panel.
pub const REVIEW_SASH: FocusId = FocusId(7091);

/// The handle on the left edge of the plan panel.
pub const PLAN_SASH: FocusId = FocusId(7092);

/// The handle on the top edge of the terminal dock.
pub const TERMINAL_SASH: FocusId = FocusId(7093);

/// How thick a handle is, in logical pixels.
const THICKNESS: f32 = 6.0;

/// Which way a handle is pulled to make its panel bigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grows {
    /// The panel grows as the handle moves right.
    Right,
    /// The panel grows as the handle moves left.
    Left,
    /// The panel grows as the handle moves up.
    Up,
    /// The panel grows as the handle moves down.
    Down,
}

impl Grows {
    /// How much bigger the panel gets when the pointer holding its handle moves from `from` to `to`, in logical pixels.
    pub fn by(self, from: (f32, f32), to: (f32, f32)) -> f32 {
        match self {
            Self::Right => to.0 - from.0,
            Self::Left => from.0 - to.0,
            Self::Up => from.1 - to.1,
            Self::Down => to.1 - from.1,
        }
    }
}

/// Which way a handle lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Upright, between panels side by side; dragged left and right.
    Column,
    /// Flat, between panels one above the other; dragged up and down.
    Row,
}

/// A handle on the edge between two panels, either floating over the edge at a place in the window or lying over the top edge of what follows it in the flow.
#[derive(Clone, Debug)]
pub struct Sash {
    /// The element the handle is.
    id: FocusId,
    /// Which way it lies.
    layout: Layout,
    /// Where its middle line starts in the window, and how long it is, when it floats over an edge.
    placed: Option<((f32, f32), f32)>,
    /// Whether it is built bare, to be put inside the element whose edge it is.
    bare: bool,
    /// What the caller puts around the handle's strip.
    modifier: Modifier,
    /// How the strip lays out the line it lights, when the caller replaces it.
    arranged: Option<(Arrangement, Alignment)>,
}

impl Sash {
    /// An upright handle `id`, dragged left and right.
    pub fn column(id: FocusId) -> Self {
        Self {
            id,
            layout: Layout::Column,
            placed: None,
            bare: false,
            modifier: Modifier::new(),
            arranged: None,
        }
    }

    /// A flat handle `id`, dragged up and down.
    pub fn row(id: FocusId) -> Self {
        Self {
            id,
            layout: Layout::Row,
            placed: None,
            bare: false,
            modifier: Modifier::new(),
            arranged: None,
        }
    }

    /// The handle floating over the edge whose line starts at `at` in the window and runs `length` logical pixels.
    pub fn at(mut self, at: (f32, f32), length: f32) -> Self {
        self.placed = Some((at, length));
        self
    }

    /// The handle without the zero-height element that holds it in a flow, for putting inside the element whose edge it lies over.
    pub fn bare(mut self) -> Self {
        self.bare = true;
        self
    }

    /// The handle with `modifier` chained around its strip, outside its own placement and size; calls add up.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The handle spreading its children by `arrangement` and placing them across by `alignment` in place of its own.
    pub fn arranged(mut self, arrangement: Arrangement, alignment: Alignment) -> Self {
        self.arranged = Some((arrangement, alignment));
        self
    }

    /// Emits the handle: a strip the pointer takes hold of, clear until the pointer is on it, when a line as thin as a border lights up along the edge.
    #[track_caller]
    pub fn build(self) {
        let (cursor, line) = match self.layout {
            Layout::Column => (
                CursorIcon::ColResize,
                Modifier::new().width(1.0).fill_max_height(),
            ),
            Layout::Row => (
                CursorIcon::RowResize,
                Modifier::new().height(1.0).fill_max_width(),
            ),
        };
        let own = match (self.layout, self.placed) {
            (Layout::Column, Some(((x, y), length))) => Modifier::new()
                .at(x - THICKNESS / 2.0, y)
                .width(THICKNESS)
                .height(length),
            (Layout::Row, Some(((x, y), length))) => Modifier::new()
                .at(x, y - THICKNESS / 2.0)
                .width(length)
                .height(THICKNESS),
            (Layout::Column, None) => Modifier::new()
                .at(-THICKNESS / 2.0, 0.0)
                .width(THICKNESS)
                .fill_max_height(),
            (Layout::Row, None) => Modifier::new()
                .at(0.0, -THICKNESS / 2.0)
                .height(THICKNESS)
                .fill_max_width(),
        };
        let own = own
            .z_index(10)
            .group()
            .cursor(cursor)
            .target(self.id)
            .draggable_id(self.id);
        let (arrangement, alignment) = self
            .arranged
            .unwrap_or((Arrangement::Center, Alignment::Center));
        let modifier = self.modifier.chain(own);
        let handle = move || {
            row_arranged(modifier, arrangement, alignment, move || {
                rustle::spacer(line.looks(Looks::new().group(Role::Accent)));
            });
        };
        match self.placed {
            Some(_) => handle(),
            None if self.bare => rustle::stack_with(Modifier::new().match_parent_size(), handle),
            None => rustle::stack_with(Modifier::new().fill_max_width().height(0.0), move || {
                rustle::stack_with(Modifier::new().match_parent_size(), handle);
            }),
        }
    }
}

impl WorkspaceState {
    /// How big the panel the handle `id` resizes is now, in a window `window` logical pixels wide and tall, and which way the handle is pulled to grow it.
    pub fn sash(&self, id: FocusId, window: (f32, f32)) -> Option<(f32, Grows)> {
        match id {
            SIDEBAR_SASH => Some((self.sidebar_width, Grows::Right)),
            REVIEW_SASH => Some((self.review_width, Grows::Left)),
            PLAN_SASH => Some((
                plan::width_in(self, window.0 - self.sidebar_room() - self.review_width),
                Grows::Left,
            )),
            TERMINAL_SASH => Some((
                terminal::height(self, window.1 - crate::title_bar::TITLE_BAR_HEIGHT),
                Grows::Up,
            )),
            GRAPH_SASH => Some((self.git.graph_height, Grows::Up)),
            crate::browser::BROWSER_SASH => self
                .browser
                .as_ref()
                .map(|browser| (browser.tools_width, Grows::Left)),
            _ => crate::panes::split_sash_of(id).and_then(|index| self.split_sash(index, window)),
        }
    }

    /// The state with the panel the handle `id` resizes made `size` logical pixels big, within what that panel allows in a window `window` logical pixels wide and tall.
    pub fn with_sash_size(mut self, id: FocusId, size: f32, window: (f32, f32)) -> Self {
        match id {
            SIDEBAR_SASH => self.sidebar_width = size.clamp(sidebar::WIDTHS.0, sidebar::WIDTHS.1),
            REVIEW_SASH => self.review_width = size.clamp(review::WIDTHS.0, review::WIDTHS.1),
            PLAN_SASH => self.plan_width = size.clamp(plan::WIDTHS.0, plan::WIDTHS.1),
            TERMINAL_SASH => {
                self.terminal_maximized = false;
                let page = window.1 - crate::title_bar::TITLE_BAR_HEIGHT;
                self.terminal_height = size.clamp(terminal::HEIGHTS.0, terminal::tallest(page));
            }
            GRAPH_SASH => self.git.graph_height = size.clamp(graph::HEIGHTS.0, graph::HEIGHTS.1),
            crate::browser::BROWSER_SASH => {
                if let Some(browser) = self.browser.as_mut() {
                    browser.tools_width = size.clamp(crate::browser::TOOLS_LEAST, window.0);
                }
            }
            _ => {
                if let Some(index) = crate::panes::split_sash_of(id) {
                    self.resize_split(index, size, window);
                }
            }
        }
        self
    }
}
