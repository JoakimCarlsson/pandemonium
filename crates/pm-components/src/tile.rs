//! The tile: a row of a list that holds its children side by side, answers to a press and fills under the pointer and while it is selected.

use pm_input::FocusId;
use pm_style::{Radius, Role, bg, cursor_pointer, flex, gap, h, items_center, rounded};
use pm_ui::{Node, node};

/// A tile being described: what it answers to, how tall it is and whether it is selected.
///
/// Build it with [`Tile::new`], change it with the methods that follow, and
/// turn it into a node with [`Tile::build`]; the caller adds what it holds
/// and the room around it.
#[derive(Clone, Copy, Debug)]
pub struct Tile {
    /// The name it answers to.
    id: FocusId,
    /// How tall it is, in steps of four logical pixels.
    height: f32,
    /// How far apart its children sit, in steps of four logical pixels.
    gap: f32,
    /// How round its corners are.
    radius: Radius,
    /// Whether it is filled.
    selected: bool,
}

impl Tile {
    /// An empty row 36 logical pixels tall answering to `id`.
    pub fn new(id: FocusId) -> Self {
        Self {
            id,
            height: 9.0,
            gap: 3.0,
            radius: Radius::Md,
            selected: false,
        }
    }

    /// The tile `height` steps tall.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The tile with its children `gap` steps apart.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// The tile with corners rounded by `radius`.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// The tile filled while `selected` is true.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The node that draws the tile: it fills under the pointer unless it is selected, and darker while pressed.
    pub fn build(self) -> Node {
        let mut styles = vec![
            flex(),
            items_center(),
            gap(self.gap),
            h(self.height),
            rounded(self.radius),
            cursor_pointer(),
            bg(Role::SurfaceActive).active(),
        ];
        styles.push(if self.selected {
            bg(Role::SurfaceSelected)
        } else {
            bg(Role::SurfaceHover).hover()
        });
        node().styles(styles).focusable(self.id)
    }
}

impl From<Tile> for Node {
    /// The node that draws the tile.
    fn from(tile: Tile) -> Self {
        tile.build()
    }
}
