//! The tile: a row of a list that fills while it is selected and answers to a press.

use pm_input::FocusId;
use pm_style::{Radius, Role, bg, cursor_pointer, flex, gap, h, items_center, rounded};
use pm_ui::{Node, node};

/// A tile being described: what it answers to and whether it is selected.
///
/// Build it with [`Tile::new`], change it with [`Tile::selected`], and turn
/// it into a node with [`Tile::build`]; the caller adds what it holds.
#[derive(Clone, Copy, Debug)]
pub struct Tile {
    /// The name it answers to.
    id: FocusId,
    /// Whether it is filled.
    selected: bool,
}

impl Tile {
    /// An empty tile answering to `id`.
    pub fn new(id: FocusId) -> Self {
        Self {
            id,
            selected: false,
        }
    }

    /// The tile filled while `selected` is true.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The node that draws the tile.
    pub fn build(self) -> Node {
        let mut styles = vec![
            flex(),
            items_center(),
            gap(3),
            h(9),
            rounded(Radius::Md),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
            bg(Role::SurfaceActive).active(),
        ];
        if self.selected {
            styles.push(bg(Role::SurfaceSelected));
        }
        node().styles(styles).focusable(self.id)
    }
}
