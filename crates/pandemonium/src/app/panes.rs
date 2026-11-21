//! Dragging a tab of a pane to another pane, or to the edge of one to split it.

use pm_components::panes::{Item, Side};
use pm_components::pointing::Over;
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
        self.carry_moved(at, None);
    }

    /// Follows the pointer carrying a control to `at` in the window, over `over` when the
    /// place under it takes what is carried.
    pub(super) fn carry_moved(&mut self, at: (f32, f32), over: Option<Over>) {
        let at = self.framed_point(at);
        let project = match over {
            Some(Over::Project(project)) => Some(project),
            _ => None,
        };
        let pane = match over {
            Some(Over::Pane(pane, side)) => Some((pane, side)),
            _ => None,
        };
        let row = match over {
            Some(Over::Tree(row)) => Some(row),
            _ => None,
        };
        self.drag_project(at, project);
        self.drag_pane(at, pane);
        self.drag_tree(at, row);
    }

    /// Puts what was carried where the pointer let go of it, with `modifiers` held.
    pub(super) fn carry_ended(&mut self, modifiers: rustle::Modifiers) {
        self.drop_project();
        self.drop_pane();
        self.drop_tree(modifiers);
    }

    /// Opens the menu of panes where its button was clicked, at `at` in the window's frame.
    pub(super) fn open_pane_menu(&mut self, at: (f32, f32)) {
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.with_pane_menu(at))
        });
    }

    /// Follows the pointer to `at` while a tab of a pane is carried, over the pane and edge
    /// `over` when it is over one.
    pub(super) fn drag_pane(&mut self, at: (f32, f32), over: Option<(usize, Side)>) {
        let Some(item) = self.pane_press.clone() else {
            return;
        };
        self.pane_dragging = true;
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.pane_dragged(item, over, at))
        });
    }

    /// Ends a drag of a tab, moving it where it was let go, and says whether there was a drag.
    pub(super) fn drop_pane(&mut self) -> bool {
        let pressed: Option<Item> = self.pane_press.take();
        if pressed.is_none() || !std::mem::take(&mut self.pane_dragging) {
            return false;
        }
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.pane_dropped())
        });
        self.deliver();
        true
    }
}
