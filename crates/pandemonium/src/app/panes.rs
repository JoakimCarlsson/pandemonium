//! Dragging a tab of a pane to another pane, or to the edge of one to split it.

use pm_components::panes::{Area, Item, Side, pane_of};
use pm_input::FocusId;

use super::{App, DRAG_DISTANCE};

impl App {
    /// Notes the tab of a pane a press began on, if it began on one, so moving far enough from it drags the tab.
    pub(super) fn pane_press(&mut self, on: Option<FocusId>) {
        self.pane_press = on
            .and_then(|id| self.model.read(|screen| screen.pane_tab_of(id)))
            .zip(self.pointer.position);
        self.pane_dragging = false;
    }

    /// The pane under `at`, by position, and the edge of it `at` is over.
    fn pane_under(&self, at: (f32, f32)) -> Option<(usize, Side)> {
        self.output.regions.iter().find_map(|region| {
            let index = pane_of(region.id)?;
            region.rect.contains(at.0, at.1).then(|| {
                let area = Area {
                    x: region.rect.x,
                    y: region.rect.y,
                    width: region.rect.width,
                    height: region.rect.height,
                };
                (index, area.side_at(at))
            })
        })
    }

    /// Follows the pointer to `at` while a tab of a pane is held down: past a few pixels it is a drag, over the pane and edge under the pointer.
    pub(super) fn drag_pane(&mut self, at: (f32, f32)) {
        let Some((item, origin)) = self.pane_press.clone() else {
            return;
        };
        if !self.pane_dragging && (at.0 - origin.0).hypot(at.1 - origin.1) < DRAG_DISTANCE {
            return;
        }
        self.pane_dragging = true;
        let over = self.pane_under(at);
        self.model
            .update(|state| *state = std::mem::take(state).pane_dragged(item, over, at));
    }

    /// Ends a drag of a tab, moving it where it was let go, and says whether there was a drag.
    pub(super) fn drop_pane(&mut self) -> bool {
        let pressed: Option<(Item, (f32, f32))> = self.pane_press.take();
        if pressed.is_none() || !std::mem::take(&mut self.pane_dragging) {
            return false;
        }
        self.model
            .update(|state| *state = std::mem::take(state).pane_dropped());
        self.deliver();
        true
    }
}
