//! The handles on the edges between panels, dragged to change how big a panel is, and the sizes they set.

use crate::areas::{BrowserEvent, GitEvent, WindowEvent};
use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::events::Interactive;
use crate::theme::Role;

use crate::state::WorkspaceState;
use crate::{graph, plan, review, sidebar, terminal};

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
    id: Target,
    /// Which way it lies.
    layout: Layout,
    /// Where its middle line starts in the window, and how long it is, when it floats over an edge.
    placed: Option<((f32, f32), f32)>,
    /// Whether it is built bare, to be put inside the element whose edge it is.
    bare: bool,
    /// What the caller puts around the handle's strip.
    modifier: Modifier,
}

impl Sash {
    /// An upright handle `id`, dragged left and right.
    pub fn column(id: Target) -> Self {
        Self::lying(id, Layout::Column)
    }

    /// A flat handle `id`, dragged up and down.
    pub fn row(id: Target) -> Self {
        Self::lying(id, Layout::Row)
    }

    /// A handle `id` lying as `layout` says, in the flow, with its own look.
    fn lying(id: Target, layout: Layout) -> Self {
        Self {
            id,
            layout,
            placed: None,
            bare: false,
            modifier: Modifier::new(),
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
        let (half, along) = (THICKNESS / 2.0, Modifier::new());
        let own = match (self.layout, self.placed) {
            (Layout::Column, Some(((x, y), length))) => {
                along.offset(x - half, y).size(THICKNESS, length)
            }
            (Layout::Row, Some(((x, y), length))) => {
                along.offset(x, y - half).size(length, THICKNESS)
            }
            (Layout::Column, None) => along.offset(-half, 0.0).width(THICKNESS).fill_max_height(),
            (Layout::Row, None) => along.offset(0.0, -half).height(THICKNESS).fill_max_width(),
        };
        let over = rustle::remember_mutable_interaction_source();
        let hovered = over.collect_is_hovered_as_state();
        let own = own
            .z_index(10)
            .hoverable(&over)
            .cursor(cursor)
            .control(self.id, send(Event::pressing(self.id)))
            .chain(crate::events::sash_drag(self.id));
        let modifier = self.modifier.chain(own);
        let handle = move || {
            row_arranged(
                modifier,
                Arrangement::Center,
                Alignment::Center,
                move || {
                    let line = if hovered.get() {
                        line.background(theme::color(Role::Accent))
                    } else {
                        line
                    };
                    rustle::spacer(line);
                },
            );
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

/// The sizes set by the handles inside panels rather than between them: the git graph's
/// height, the browser tools' width, and the dock's height and whether it is maximized. A
/// region of the screen keyed by its room holds these too, whichever panel it is, since a
/// drag of these handles changes no revision.
pub type InnerSizes = (f32, Option<f32>, f32, bool);

impl WorkspaceState {
    /// The sizes the handles inside panels set; see [`InnerSizes`].
    pub fn inner_sizes(&self) -> InnerSizes {
        (
            self.git.graph_height,
            self.browser.as_ref().map(|browser| browser.tools_width),
            self.terminal_height,
            self.terminal_maximized,
        )
    }

    /// How big the panel the handle `id` resizes is now, in a window `window` logical pixels wide and tall, and which way the handle is pulled to grow it.
    pub fn sash(&self, id: Target, window: (f32, f32)) -> Option<(f32, Grows)> {
        match id {
            Target::Window(WindowEvent::SidebarSash) => Some((self.sidebar_width, Grows::Right)),
            Target::Window(WindowEvent::ReviewSash) => Some((self.review_width, Grows::Left)),
            Target::Window(WindowEvent::PlanSash) => Some((
                plan::width_in(self, window.0 - self.sidebar_room() - self.review_width),
                Grows::Left,
            )),
            Target::Window(WindowEvent::TerminalSash) => Some((
                terminal::height(self, window.1 - crate::title_bar::TITLE_BAR_HEIGHT),
                Grows::Up,
            )),
            Target::Git(GitEvent::GraphSash) => Some((self.git.graph_height, Grows::Up)),
            Target::Browser(BrowserEvent::BrowserSash) => self
                .browser
                .as_ref()
                .map(|browser| (browser.tools_width, Grows::Left)),
            Target::Window(WindowEvent::SplitSash(index)) => self.split_sash(index, window),
            _ => None,
        }
    }

    /// The state with the panel the handle `id` resizes made `size` logical pixels big, within what that panel allows in a window `window` logical pixels wide and tall.
    pub fn with_sash_size(mut self, id: Target, size: f32, window: (f32, f32)) -> Self {
        match id {
            Target::Window(WindowEvent::SidebarSash) => {
                self.sidebar_width = size.clamp(sidebar::WIDTHS.0, sidebar::WIDTHS.1)
            }
            Target::Window(WindowEvent::ReviewSash) => {
                self.review_width = size.clamp(review::WIDTHS.0, review::WIDTHS.1)
            }
            Target::Window(WindowEvent::PlanSash) => {
                self.plan_width = size.clamp(plan::WIDTHS.0, plan::WIDTHS.1)
            }
            Target::Window(WindowEvent::TerminalSash) => {
                self.terminal_maximized = false;
                let page = window.1 - crate::title_bar::TITLE_BAR_HEIGHT;
                self.terminal_height = size.clamp(terminal::HEIGHTS.0, terminal::tallest(page));
            }
            Target::Git(GitEvent::GraphSash) => {
                self.git.graph_height = size.clamp(graph::HEIGHTS.0, graph::HEIGHTS.1)
            }
            Target::Browser(BrowserEvent::BrowserSash) => {
                if let Some(browser) = self.browser.as_mut() {
                    browser.tools_width = size.clamp(crate::browser::TOOLS_LEAST, window.0);
                }
            }
            Target::Window(WindowEvent::SplitSash(index)) => self.resize_split(index, size, window),
            _ => {}
        }
        self
    }
}
