//! One theme: a colour for every role.

use rustle::Color;

use super::{Fonts, Role};

/// The colour each role is painted in, and the font stacks, for one appearance.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Theme {
    /// One colour per role, in the order of [`Role::ALL`].
    colors: [Color; Role::ALL.len()],
    /// The font stacks.
    pub fonts: Fonts,
}

impl Theme {
    /// A theme painting every role transparent, for a file to paint over.
    pub fn blank() -> Self {
        Self {
            colors: [Color::TRANSPARENT; Role::ALL.len()],
            fonts: Fonts::DEFAULT,
        }
    }

    /// The colour `role` is painted in.
    pub fn color(&self, role: Role) -> Color {
        self.colors[Self::index(role)]
    }

    /// This theme with `role` painted in `color`.
    pub fn with(mut self, role: Role, color: Color) -> Self {
        self.colors[Self::index(role)] = color;
        self
    }

    /// Where `role` sits in the colour table.
    fn index(role: Role) -> usize {
        Role::ALL
            .iter()
            .position(|candidate| *candidate == role)
            .unwrap_or_default()
    }
}
