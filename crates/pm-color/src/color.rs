//! Operations on [`Color`], the colour a style names.

use crate::{Oklch, Role, Roles, Srgba};

pub use crate::palette::Color;

impl Color {
    /// An opaque colour from 8-bit channels.
    pub fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::Srgb(Srgba::from_bytes(red, green, blue))
    }

    /// An opaque colour from a packed `0xRRGGBB` value.
    pub fn hex(rgb: u32) -> Self {
        Self::Srgb(Srgba::hex(rgb))
    }

    /// A colour from a packed `0xRRGGBBAA` value.
    pub fn hexa(rgba: u32) -> Self {
        Self::Srgb(Srgba::hexa(rgba))
    }

    /// A colour from perceptual lightness, chroma and a hue in degrees.
    pub fn oklch(lightness: f32, chroma: f32, hue: f32) -> Self {
        Self::Srgb(Oklch::new(lightness, chroma, hue).to_srgba())
    }

    /// The colour as the sRGB value a renderer takes, with a role resolved
    /// against `roles`.
    pub fn resolve(self, roles: &impl Roles) -> Srgba {
        match self {
            Self::Role(role, opacity) => roles.role(role).alpha(opacity),
            other => other.srgba(),
        }
    }

    /// The colour as the sRGB value a renderer takes.
    ///
    /// A role has no colour without a theme: it reads as opaque magenta, so a
    /// role that was never resolved is easy to see. Use [`Color::resolve`].
    pub fn srgba(self) -> Srgba {
        match self {
            Self::Srgb(value) => value,
            Self::Role(..) => Srgba::new(1.0, 0.0, 1.0, 1.0),
            Self::Transparent => Srgba::TRANSPARENT,
            named => named
                .palette_oklch()
                .map_or(Srgba::TRANSPARENT, Oklch::to_srgba),
        }
    }

    /// The same colour with its opacity scaled by `factor`, from 0.0 to 1.0.
    pub fn alpha(self, factor: f32) -> Self {
        if let Self::Role(role, opacity) = self {
            return Self::Role(role, opacity * factor);
        }
        Self::Srgb(self.srgba().alpha(factor))
    }

    /// The same colour with its opacity scaled to `percent`, from 0 to 100.
    pub fn percent(self, percent: u8) -> Self {
        self.alpha(f32::from(percent.min(100)) / 100.0)
    }

    /// The point `amount` of the way from this colour to `other`, mixed in a
    /// perceptual space.
    pub fn mix(self, other: Self, amount: f32) -> Self {
        let (from, to) = (self.srgba(), other.srgba());
        let mixed = from.to_oklab().mix(to.to_oklab(), amount).to_srgba();
        Self::Srgb(mixed.with_alpha(from.a + (to.a - from.a) * amount))
    }

    /// The colour as `[red, green, blue, alpha]` floats.
    pub fn to_array(self) -> [f32; 4] {
        self.srgba().to_array()
    }
}

impl From<Color> for Srgba {
    /// The colour as the renderer takes it.
    fn from(color: Color) -> Self {
        color.srgba()
    }
}

impl From<Srgba> for Color {
    /// An explicit colour.
    fn from(value: Srgba) -> Self {
        Self::Srgb(value)
    }
}

impl Default for Color {
    /// [`Color::Transparent`].
    fn default() -> Self {
        Self::Transparent
    }
}

impl From<Role> for Color {
    /// The role, fully opaque.
    fn from(role: Role) -> Self {
        Self::Role(role, 1.0)
    }
}
