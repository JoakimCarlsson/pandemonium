//! What the window does with the panes that show a file as something other
//! than its text: a picture, and markdown read as it renders.

use std::path::Path;

use pm_core::Scope;

use crate::app::App;
use crate::image::Images;
use crate::message::Message;
use crate::panes::{Item, PaneId, SplitDirection};

/// Logical pixels of wheel travel that magnify a diagram by a factor of e.
const WHEEL_ZOOM: f32 = 240.0;

impl App {
    /// Opens the worktree's outline beside the focused pane or brings it forward.
    pub(super) fn open_outline(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        self.open_beside(self.panes.focus(), Item::Outline(scope));
        self.refresh_annotations();
    }
    /// Opens the matching rendered view or SVG source beside `pane`.
    pub(super) fn open_file_preview(&mut self, pane: PaneId) {
        let active = self
            .panes
            .pane(pane)
            .and_then(|pane| pane.active(self.scope()));
        let item = match active {
            Some(Item::File(file)) => {
                let Some(path) = self.editor.path(file) else {
                    return;
                };
                if crate::markdown::is_markdown(&path) {
                    Item::Rendered(file)
                } else if is_svg(&path) {
                    let Some(scope) = self.editor.scope_of(file) else {
                        return;
                    };
                    let Some(root) = self.root_of(scope) else {
                        return;
                    };
                    Item::Image(self.images.open(scope, &root.at(&path), false))
                } else {
                    return;
                }
            }
            Some(Item::Image(image)) => {
                let Some(scope) = self.images.scope_of(image) else {
                    return;
                };
                let Some(path) = self.images.path_of(image).filter(|path| is_svg(path)) else {
                    return;
                };
                let path = path.to_path_buf();
                let Some(root) = self.root_of(scope) else {
                    return;
                };
                let Some(file) = self.editor.open(scope, &root, &path, false) else {
                    return;
                };
                Item::File(file)
            }
            _ => return,
        };
        self.open_beside(pane, item);
    }

    /// Brings an existing view forward or opens it beside `pane`.
    fn open_beside(&mut self, pane: PaneId, item: Item) {
        let holder = self.panes.panes().into_iter().find(|pane| {
            self.panes
                .pane(*pane)
                .is_some_and(|pane| pane.items().any(|held| held == item))
        });
        match holder {
            Some(holder) => self.activate_tab(holder, item),
            None => {
                self.split_pane(pane, Some(item), SplitDirection::Right);
                self.sweep();
            }
        }
    }

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
        if !Images::is_picture(path) || is_svg(path) {
            return false;
        }
        let Some(root) = self.root_of(scope) else {
            return false;
        };
        let image = self.images.open(scope, &root.at(path), preview);
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
        self.open_beside(self.panes.focus(), Item::Rendered(file));
    }

    /// Scrolls the rendered markdown under the pointer by `delta` logical
    /// pixels, or zooms the diagram under it when a zoom modifier is held,
    /// saying whether the pointer was over any.
    pub(super) fn scroll_rendered(&mut self, delta: f32) -> bool {
        let Some(Item::Rendered(file)) = self.item_under() else {
            return false;
        };
        let zooming = self.modifiers.control_key() || self.modifiers.super_key();
        if zooming
            && let Some(pointer) = self.pointer
            && self
                .renders
                .zoom_under(file, pointer, (delta / WHEEL_ZOOM).exp())
        {
            return true;
        }
        let scroll = self.renders.scroll(file);
        let mut moved = scroll.get();
        moved.by(delta);
        scroll.set(moved);
        true
    }

    /// Carries out `message` when it zooms or pans a rendered diagram,
    /// saying whether it did.
    pub(super) fn diagram_command(&mut self, message: Message) -> bool {
        match message {
            Message::PanDiagram(file, index, event) => self.renders.pan(file, index, event),
            Message::ZoomDiagram(file, index, step) => self.renders.step_zoom(file, index, step),
            _ => return false,
        }
        self.request_redraw();
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

/// Whether `path` is an SVG whose source can be edited beside its image.
fn is_svg(path: &Path) -> bool {
    path.extension()
        .and_then(|ending| ending.to_str())
        .is_some_and(|ending| ending.eq_ignore_ascii_case("svg"))
}
