//! The tile: a row of a list that holds its children side by side, answers to a press and fills under the pointer and while it is selected.

use crate::kit::{Flex, FocusId, Hovered, Interactive, Looks, Radius, Role, Styled, row};
use rustle::{CursorIcon, Modifier};

/// A tile being described: what it answers to, how tall it is and whether it is selected.
///
/// Build it with [`Tile::new`], change it with the methods that follow, and
/// emit it with [`Tile::build`] or [`Tile::build_with`]; the caller gives what
/// it holds and the room around it.
#[derive(Clone, Debug)]
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
    /// How its children are laid out, past the gap, when the caller chose it.
    flex: Option<Flex>,
    /// The padding the caller gave in place of the tile's own, which has none.
    inner: Modifier,
    /// What the caller puts around its box.
    modifier: Modifier,
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
            flex: None,
            inner: Modifier::new(),
            modifier: Modifier::new(),
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

    /// The tile with its children laid out by `flex` instead of centred across; the tile's gap still applies.
    pub fn flex(mut self, flex: Flex) -> Self {
        self.flex = Some(flex);
        self
    }

    /// The tile with `modifier` as its padding, in place of its own (none), inside its fill.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = modifier;
        self
    }

    /// The tile with `modifier` put around its box, outside its own: sizes, growth, margins, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the tile holding `content`: it fills under the pointer unless it is selected, and darker while pressed.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let looks = if self.selected {
            Looks::new().idle(Role::SurfaceSelected)
        } else {
            Looks::new().hover(Role::SurfaceHover)
        }
        .active(Role::SurfaceActive)
        .radius(self.radius);
        let modifier = self.modifier.chain(
            Modifier::new()
                .h(self.height)
                .looks_of(looks, self.id)
                .cursor(CursorIcon::Pointer)
                .target(self.id)
                .chain(self.inner),
        );
        let flex = self.flex.unwrap_or(Flex::new().center()).gap(self.gap);
        row(modifier, flex, content);
    }

    /// Emits the tile holding nothing.
    #[track_caller]
    pub fn build(self) {
        self.build_with(|| {});
    }
}
