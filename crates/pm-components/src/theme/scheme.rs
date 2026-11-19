//! One theme: a colour for every role.

use super::{Role, Roles, Srgba, Tone};

use super::Fonts;

/// The colour each role is painted in, and the font stacks, for one appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// One colour per role, in the order of [`Role::ALL`].
    colors: [Tone; Role::ALL.len()],
    /// The font stacks.
    pub fonts: Fonts,
}

impl Theme {
    /// A theme painting every role transparent, for a file to paint over.
    pub fn blank() -> Self {
        Self {
            colors: [Tone::Transparent; Role::ALL.len()],
            fonts: Fonts::DEFAULT,
        }
    }

    /// The colour `role` is painted in.
    pub fn color(&self, role: Role) -> Tone {
        self.colors[Self::index(role)]
    }

    /// This theme with `role` painted in `color`.
    ///
    /// A colour that is itself a role is not followed: it would never
    /// resolve, so name a palette entry or an explicit value.
    pub fn with(mut self, role: Role, color: Tone) -> Self {
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

impl Roles for Theme {
    /// The colour `role` is painted in.
    fn role(&self, role: Role) -> Srgba {
        self.color(role).srgba()
    }
}
