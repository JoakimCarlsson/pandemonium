//! What the window does with the panes that show a file as something other
//! than its text: a picture, and markdown read as it renders.

use std::path::Path;

use pm_core::Scope;

use crate::app::App;
use crate::image::Images;
use crate::panes::{Item, PaneId, SplitDirection};

impl App {
    /// Opens `path` of `scope` as a picture in `pane`, when it is one,
    /// saying whether it was.
    ///
    /// Every way a file is opened asks this first, so a picture is opened as
    /// one whether it was clicked in the tree, chosen from the palette or
    /// gone back to along the trail.
    pub(super) fn open_picture(
        &mut self,
        pane: PaneId,
        scope: Scope,
        path: &Path,
        preview: bool,
    ) -> bool {
        if !Images::is_picture(path) {
            return false;
        }
        let image = self.images.open(scope, path, preview);
        self.show_item(pane, scope, Item::Image(image), preview);
        true
    }

    /// Opens the focused markdown file rendered, in a pane beside it.
    ///
    /// A file already rendered somewhere is brought forward there rather
    /// than rendered twice.
    pub(super) fn open_rendered(&mut self) {
        let Some(file) = self.active_file_id() else {
            return;
        };
        let markdown = self
            .editor
            .path(file)
            .is_some_and(|path| crate::markdown::is_markdown(&path));
        if !markdown {
            return;
        }
        let item = Item::Rendered(file);
        let holder = self.panes.panes().into_iter().find(|pane| {
            self.panes
                .pane(*pane)
                .is_some_and(|pane| pane.items().any(|held| held == item))
        });
        match holder {
            Some(pane) => self.activate_tab(pane, item),
            None => {
                let pane = self.panes.focus();
                self.split_pane(pane, Some(item), SplitDirection::Right);
                self.sweep();
            }
        }
    }

    /// Scrolls the rendered markdown under the pointer by `delta` logical
    /// pixels, saying whether the pointer was over any.
    pub(super) fn scroll_rendered(&mut self, delta: f32) -> bool {
        let Some(Item::Rendered(file)) = self.item_under() else {
            return false;
        };
        let scroll = self.renders.scroll(file);
        let mut moved = scroll.get();
        moved.by(delta);
        scroll.set(moved);
        true
    }

    /// What the pane under the pointer is showing, or the focused pane's
    /// when the pointer is over none.
    pub(super) fn item_under(&self) -> Option<Item> {
        let pane = self
            .pointer
            .and_then(|at| self.geometry.pane_at(at))
            .unwrap_or_else(|| self.panes.focus());
        self.panes.pane(pane)?.active(self.scope())
    }
}
