//! Gamma-encoded sRGB with straight alpha.

use super::Oklab;

/// An sRGB colour with each channel from 0.0 to 1.0 and straight
/// (non-premultiplied) alpha.
///
/// The channels stay sRGB-encoded as they reach the renderer: the surface is a
/// plain `Unorm` one, so blending happens in the same gamma space the colours
/// were written in, which keeps antialiased text the weight the font intends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Srgba {
    /// The red channel.
    pub r: f32,
    /// The green channel.
    pub g: f32,
    /// The blue channel.
    pub b: f32,
    /// The opacity, 0.0 transparent to 1.0 opaque.
    pub a: f32,
}

impl Srgba {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    /// A colour from its four channels, each already in `0..=1`.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// An opaque colour from 8-bit channels.
    pub const fn from_bytes(red: u8, green: u8, blue: u8) -> Self {
        Self::new(
            red as f32 / 255.0,
            green as f32 / 255.0,
            blue as f32 / 255.0,
            1.0,
        )
    }

    /// An opaque colour from a packed `0xRRGGBB` value.
    pub const fn hex(rgb: u32) -> Self {
        Self::hexa((rgb << 8) | 0xff)
    }

    /// A colour from a packed `0xRRGGBBAA` value.
    pub const fn hexa(rgba: u32) -> Self {
        Self::new(
            ((rgba >> 24) & 0xff) as f32 / 255.0,
            ((rgba >> 16) & 0xff) as f32 / 255.0,
            ((rgba >> 8) & 0xff) as f32 / 255.0,
            (rgba & 0xff) as f32 / 255.0,
        )
    }

    /// The same colour with its opacity scaled by `factor`.
    pub const fn alpha(self, factor: f32) -> Self {
        Self::new(self.r, self.g, self.b, self.a * factor)
    }

    /// The same colour with exactly this opacity.
    pub const fn with_alpha(self, alpha: f32) -> Self {
        Self { a: alpha, ..self }
    }

    /// The colour `amount` of the way towards `other`, channel by channel in
    /// sRGB; [`Tone::mix`](super::Tone::mix) blends perceptually instead.
    pub fn mix(self, other: Self, amount: f32) -> Self {
        let lerp = |from: f32, to: f32| from + (to - from) * amount;
        Self::new(
            lerp(self.r, other.r),
            lerp(self.g, other.g),
            lerp(self.b, other.b),
            lerp(self.a, other.a),
        )
    }

    /// The channels as `[red, green, blue, alpha]`, the order the shaders take them.
    pub const fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// The channels as 8-bit values, rounded and clamped.
    pub fn to_bytes(self) -> [u8; 4] {
        self.to_array()
            .map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    /// The colour in the perceptual Oklab space, dropping alpha.
    pub fn to_oklab(self) -> Oklab {
        let red = decode(self.r);
        let green = decode(self.g);
        let blue = decode(self.b);
        let long = (0.412_221_47 * red + 0.536_332_55 * green + 0.051_445_995 * blue).cbrt();
        let medium = (0.211_903_5 * red + 0.680_699_5 * green + 0.107_396_96 * blue).cbrt();
        let short = (0.088_302_46 * red + 0.281_718_85 * green + 0.629_978_7 * blue).cbrt();
        Oklab {
            lightness: 0.210_454_26 * long + 0.793_617_8 * medium - 0.004_072_047 * short,
            a: 1.977_998_5 * long - 2.428_592_2 * medium + 0.450_593_7 * short,
            b: 0.025_904_037 * long + 0.782_771_77 * medium - 0.808_675_77 * short,
        }
    }
}

/// Undoes the sRGB transfer curve of one channel.
pub(crate) fn decode(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Applies the sRGB transfer curve to one linear channel.
pub(crate) fn encode(channel: f32) -> f32 {
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A colour taken to the perceptual space and back is the colour it was.
    #[test]
    fn colours_survive_the_perceptual_round_trip() {
        for rgb in [0x000000, 0xffffff, 0x3b82f6, 0xef4444, 0x7f7f7f] {
            let colour = Srgba::hex(rgb);
            let back = colour.to_oklab().to_srgba();
            for (a, b) in colour.to_array().iter().zip(back.to_array()) {
                assert!((a - b).abs() < 1.0e-3, "{rgb:06x}: {a} against {b}");
            }
        }
    }

    /// Mixing halfway lands between the two colours, and the ends are the colours themselves.
    #[test]
    fn mixing_runs_from_one_colour_to_the_other() {
        let (black, white) = (Srgba::hex(0x000000), Srgba::hex(0xffffff));
        assert_eq!(black.mix(white, 0.0).to_bytes(), black.to_bytes());
        assert_eq!(black.mix(white, 1.0).to_bytes(), white.to_bytes());
        let middle = black.mix(white, 0.5).to_array()[0];
        assert!(middle > 0.1 && middle < 0.9);
    }
}
