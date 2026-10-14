//! The tile: a row that holds its children side by side, either a row of a list that fills while it is selected or a bordered strip.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, gap, h, items_center, px,
    rounded, w_full,
};
use pm_ui::{Node, node};

/// How a tile looks, which says what it is for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// A row of a list that answers to a press and fills while it is selected.
    #[default]
    Row,
    /// A rounded, bordered row that stands alone and does not answer to a press.
    Strip,
}

/// A tile being described: how it looks, what it answers to and whether it is selected.
///
/// Build it with [`Tile::new`] for a row or [`Tile::strip`] for a strip,
/// change it with [`Tile::selected`], and turn it into a node with
/// [`Tile::build`]; the caller adds what it holds.
#[derive(Clone, Copy, Debug)]
pub struct Tile {
    /// The name it answers to; a strip has none.
    id: Option<FocusId>,
    /// How it looks.
    variant: Variant,
    /// How tall it is, in steps of four logical pixels.
    height: u32,
    /// Whether it is filled.
    selected: bool,
}

impl Tile {
    /// An empty row answering to `id`.
    pub fn new(id: FocusId) -> Self {
        Self {
            id: Some(id),
            variant: Variant::Row,
            height: 9,
            selected: false,
        }
    }

    /// An empty strip `height` steps tall.
    pub fn strip(height: u32) -> Self {
        Self {
            id: None,
            variant: Variant::Strip,
            height,
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
        let mut styles = vec![flex(), items_center(), gap(3), h(self.height)];
        match self.variant {
            Variant::Row => styles.extend([
                rounded(Radius::Md),
                cursor_pointer(),
                bg(Role::SurfaceHover).hover(),
                bg(Role::SurfaceActive).active(),
            ]),
            Variant::Strip => styles.extend([
                px(4),
                w_full(),
                rounded(Radius::Xl),
                border(),
                border_color(Role::Border),
            ]),
        }
        if self.selected {
            styles.push(bg(Role::SurfaceSelected));
        }
        let tile = node().styles(styles);
        match self.id {
            Some(id) => tile.focusable(id),
            None => tile,
        }
    }
}
