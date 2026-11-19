//! Dragging a tab of a pane to another pane, or to the edge of one to split it.

use pm_components::areas::WindowEvent;
use pm_components::panes::{Area, Item, Side};
use pm_components::target::Target;

use super::App;

impl App {
    /// Whether the control `id` can be carried elsewhere in the window: a project's header,
    /// a pane's tab, or a row of the project's files other than their root.
    pub(super) fn carries(&self, id: Target) -> bool {
        self.model.read(|screen| {
            screen.header_of(id).is_some()
                || screen.pane_tab_of(id).is_some()
                || screen
                    .tree_row_of(id)
                    .is_some_and(|target| !target.is_root())
        })
    }

    /// Starts carrying the control `id`, at `at` in the window: a project's header, a pane's
    /// tab or a row of the project's files, which then follows the pointer.
    pub(super) fn carry_started(&mut self, id: Target, at: (f32, f32)) {
        self.project_press = self.model.read(|screen| screen.header_of(id));
        self.project_dragging = false;
        self.pane_press = self.model.read(|screen| screen.pane_tab_of(id));
        self.pane_dragging = false;
        self.tree_press = self
            .model
            .read(|screen| screen.tree_row_of(id))
            .filter(|target| !target.is_root());
        self.carry_moved(at);
    }

    /// Follows the pointer carrying a control to `at` in the window.
    pub(super) fn carry_moved(&mut self, at: (f32, f32)) {
        let at = self.framed_point(at);
        self.drag_project(at);
        self.drag_pane(at);
        self.drag_tree(at);
    }

    /// Puts what was carried where the pointer let go of it, with `modifiers` held, so the
    /// press it began with clicks nothing.
    pub(super) fn carry_ended(&mut self, modifiers: rustle::Modifiers) {
        if self.drop_project() | self.drop_pane() | self.drop_tree(modifiers) {
            self.pressed_on = None;
        }
    }

    /// Opens the menu of panes just under the pointer, its right edge near it.
    pub(super) fn open_pane_menu(&mut self) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model
            .update(|state| *state = std::mem::take(state).with_pane_menu((x - 190.0, y + 14.0)));
    }

    /// The pane under `at`, by position, and the edge of it `at` is over.
    fn pane_under(&self, at: (f32, f32)) -> Option<(usize, Side)> {
        self.hovered.iter().find_map(|id| {
            let Target::Window(WindowEvent::Pane(index)) = *id else {
                return None;
            };
            let rect = self.place(*id)?;
            let area = Area {
                x: rect.origin.x,
                y: rect.origin.y,
                width: rect.size.width,
                height: rect.size.height,
            };
            let inside = at.0 >= area.x
                && at.1 >= area.y
                && at.0 < area.x + area.width
                && at.1 < area.y + area.height;
            inside.then(|| (index, area.side_at(at)))
        })
    }

    /// Follows the pointer to `at` while a tab of a pane is carried, over the pane and edge under the pointer.
    pub(super) fn drag_pane(&mut self, at: (f32, f32)) {
        let Some(item) = self.pane_press.clone() else {
            return;
        };
        self.pane_dragging = true;
        let over = self.pane_under(at);
        self.model
            .update(|state| *state = std::mem::take(state).pane_dragged(item, over, at));
    }

    /// Ends a drag of a tab, moving it where it was let go, and says whether there was a drag.
    pub(super) fn drop_pane(&mut self) -> bool {
        let pressed: Option<Item> = self.pane_press.take();
        if pressed.is_none() || !std::mem::take(&mut self.pane_dragging) {
            return false;
        }
        self.model
            .update(|state| *state = std::mem::take(state).pane_dropped());
        self.deliver();
        true
    }
}
