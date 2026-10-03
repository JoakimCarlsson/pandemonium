//! Persistent selections shared by scroll areas and the window's input router.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use pm_gfx::Rect;

use super::{Selection, SelectionRow, Spot, spot_at};
use crate::{Placed, Scroll, Scrolled};

/// The content and placements of one selectable scroll area.
#[derive(Default)]
pub(crate) struct SelectionSurface {
    /// The scroll identity, held weakly so closed panes can be retired.
    pub scroll: Weak<Cell<Scroll>>,
    /// The visible area from the latest paint.
    pub bounds: Rect,
    /// The selected content boundaries.
    pub selection: Selection,
    /// Whether this area currently owns text focus.
    pub focused: bool,
    /// Whether this area is present in the current frame.
    pub visible: bool,
    /// Logical rows, independent of their line wrapping.
    pub rows: Vec<SelectionRow>,
    /// Carets of every drawn run, including content outside the viewport.
    pub placements: Vec<Placed>,
    /// Logical starts corresponding to placement keys.
    pub starts: Vec<Spot>,
    /// The last run's line box, used to separate ordinary labels.
    pub last_bounds: Option<Rect>,
}

impl SelectionSurface {
    /// Resolves a point through the frame's recorded character boundaries.
    pub fn spot_at(&self, point: pm_gfx::Point) -> Option<Spot> {
        spot_at(&self.placements, &self.starts, point)
    }

    /// Reads the current range from the logical rows.
    pub fn text(&self) -> Option<String> {
        self.selection.text(self.rows.len(), |at| SelectionRow {
            text: self.rows[at].text.clone(),
            lead: self.rows[at].lead,
            separator: self.rows[at].separator,
        })
    }

    /// Selects every logical row in this reading surface.
    pub fn select_all(&mut self) {
        if let Some((anchor, head)) = Selection::everything(self.rows.len(), |at| SelectionRow {
            text: self.rows[at].text.clone(),
            lead: self.rows[at].lead,
            separator: self.rows[at].separator,
        }) {
            self.selection.select(anchor, head);
        }
    }
}

/// The selectable scroll areas belonging to one UI instance.
#[derive(Default)]
pub(crate) struct SelectionRegistry {
    /// Live surfaces indexed by the identity of their shared scroll state.
    pub surfaces: Vec<Rc<RefCell<SelectionSurface>>>,
}

impl SelectionRegistry {
    /// Marks all areas absent until a scroll area paints them again.
    pub fn begin_frame(&mut self) {
        for surface in &self.surfaces {
            surface.borrow_mut().visible = false;
        }
    }

    /// Returns the state belonging to `scroll`, retiring closed areas.
    pub fn surface(&mut self, scroll: &Scrolled) -> Rc<RefCell<SelectionSurface>> {
        self.surfaces
            .retain(|surface| surface.borrow().scroll.strong_count() > 0);
        if let Some(surface) = self.surfaces.iter().find(|surface| {
            let state = surface.borrow();
            state.scroll.as_ptr() == Rc::as_ptr(scroll) && !state.visible
        }) {
            return surface.clone();
        }
        let surface = Rc::new(RefCell::new(SelectionSurface {
            scroll: Rc::downgrade(scroll),
            ..SelectionSurface::default()
        }));
        self.surfaces.push(surface.clone());
        surface
    }

    /// Removes focus and selection from every reading surface.
    pub fn clear(&mut self) {
        for surface in &self.surfaces {
            let mut surface = surface.borrow_mut();
            surface.focused = false;
            surface.selection.clear();
        }
    }

    /// Returns the area owning text focus.
    pub fn focused(&self) -> Option<Rc<RefCell<SelectionSurface>>> {
        self.surfaces
            .iter()
            .find(|surface| {
                let state = surface.borrow();
                state.focused && state.visible
            })
            .cloned()
    }
}

/// A text run that can start selection, clipped to its scroll area.
pub(crate) struct SelectionFrame {
    /// The visible portion of the run's line box.
    pub bounds: Rect,
    /// The surface whose logical positions the run refers to.
    pub surface: Rc<RefCell<SelectionSurface>>,
    /// Number of ordinary input regions painted before this run.
    pub regions: usize,
}
