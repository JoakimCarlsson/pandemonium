//! The perceptual colour spaces the palette is defined in.

use crate::Srgba;
use crate::srgba::encode;

/// A colour as lightness and two opponent axes: the rectangular form of
/// [`Oklch`], the one in which mixing is a straight interpolation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklab {
    /// Perceived lightness, 0.0 black to 1.0 white.
    pub lightness: f32,
    /// Green-to-red axis.
    pub a: f32,
    /// Blue-to-yellow axis.
    pub b: f32,
}

impl Oklab {
    /// The point `amount` of the way from `self` to `other`.
    pub fn mix(self, other: Self, amount: f32) -> Self {
        Self {
            lightness: self.lightness + (other.lightness - self.lightness) * amount,
            a: self.a + (other.a - self.a) * amount,
            b: self.b + (other.b - self.b) * amount,
        }
    }

    /// The nearest representable sRGB colour, opaque.
    ///
    /// Out-of-gamut colours lose chroma at constant lightness and hue until
    /// they fit, instead of clipping channel by channel.
    pub fn to_srgba(self) -> Srgba {
        let lightness = self.lightness.clamp(0.0, 1.0);
        let chroma = self.a.hypot(self.b);
        let direction = if chroma > 0.0 {
            (self.a / chroma, self.b / chroma)
        } else {
            (0.0, 0.0)
        };
        let at = |scale: f32| linear_channels(lightness, direction.0 * scale, direction.1 * scale);
        let mut channels = at(chroma);
        if !in_gamut(channels) {
            let (mut inside, mut outside) = (0.0, chroma);
            for _ in 0..24 {
                let middle = (inside + outside) / 2.0;
                if in_gamut(at(middle)) {
                    inside = middle;
                } else {
                    outside = middle;
                }
            }
            channels = at(inside);
        }
        let [red, green, blue] = channels.map(|channel| encode(channel.clamp(0.0, 1.0)));
        Srgba::new(red, green, blue, 1.0)
    }
}

/// Linear-light RGB of an Oklab coordinate, unclamped.
fn linear_channels(lightness: f32, a: f32, b: f32) -> [f32; 3] {
    let long = (lightness + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let medium = (lightness - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let short = (lightness - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * long - 3.307_711_6 * medium + 0.230_969_93 * short,
        -1.268_438 * long + 2.609_757_4 * medium - 0.341_319_4 * short,
        -0.004_196_086_3 * long - 0.703_418_6 * medium + 1.707_614_7 * short,
    ]
}

/// Whether every linear channel lies inside the sRGB gamut.
fn in_gamut(channels: [f32; 3]) -> bool {
    channels
        .iter()
        .all(|channel| (-0.000_5..=1.000_5).contains(channel))
}

/// A colour as lightness, chroma and hue angle: the form in which a palette
/// shade is written down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    /// Perceived lightness, 0.0 black to 1.0 white.
    pub lightness: f32,
    /// How far from grey, 0.0 up to about 0.4.
    pub chroma: f32,
    /// The hue angle in degrees.
    pub hue: f32,
}

impl Oklch {
    /// A colour from lightness, chroma and a hue in degrees.
    pub const fn new(lightness: f32, chroma: f32, hue: f32) -> Self {
        Self {
            lightness,
            chroma,
            hue,
        }
    }

    /// The rectangular form of this colour.
    pub fn to_oklab(self) -> Oklab {
        let radians = self.hue.to_radians();
        Oklab {
            lightness: self.lightness,
            a: self.chroma * radians.cos(),
            b: self.chroma * radians.sin(),
        }
    }

    /// The nearest representable sRGB colour, opaque.
    pub fn to_srgba(self) -> Srgba {
        self.to_oklab().to_srgba()
    }
}
