//! Colours as the rest of the codebase writes them: sRGB with straight alpha.

/// An sRGB colour with straight (non-premultiplied) alpha, each channel in `0..=1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba {
    /// Red channel.
    pub r: f32,
    /// Green channel.
    pub g: f32,
    /// Blue channel.
    pub b: f32,
    /// Opacity.
    pub a: f32,
}

impl Rgba {
    /// A fully transparent colour.
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    /// Creates a colour from channels already in `0..=1`.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates an opaque colour from `0xRRGGBB`.
    pub const fn hex(rgb: u32) -> Self {
        Self::hexa((rgb << 8) | 0xff)
    }

    /// Creates a colour from `0xRRGGBBAA`.
    pub const fn hexa(rgba: u32) -> Self {
        Self::new(
            ((rgba >> 24) & 0xff) as f32 / 255.0,
            ((rgba >> 16) & 0xff) as f32 / 255.0,
            ((rgba >> 8) & 0xff) as f32 / 255.0,
            (rgba & 0xff) as f32 / 255.0,
        )
    }

    /// Returns this colour at `alpha` opacity, scaling the alpha it already has.
    pub const fn alpha(self, alpha: f32) -> Self {
        Self::new(self.r, self.g, self.b, self.a * alpha)
    }

    /// Returns this colour blended `amount` of the way towards `other`.
    pub fn mix(self, other: Self, amount: f32) -> Self {
        let lerp = |a: f32, b: f32| a + (b - a) * amount;
        Self::new(
            lerp(self.r, other.r),
            lerp(self.g, other.g),
            lerp(self.b, other.b),
            lerp(self.a, other.a),
        )
    }

    /// Whether this colour would draw nothing.
    pub fn is_transparent(&self) -> bool {
        self.a <= f32::EPSILON
    }

    /// The channels as the shaders take them, in the order they are declared.
    ///
    /// The values stay sRGB-encoded: the surface is a plain `Unorm` one, so
    /// blending happens in the same gamma space the colours were written in.
    /// That is what keeps antialiased text the weight the font intends —
    /// blending coverage in linear light thins dark text on a light surface
    /// until it looks ragged.
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}
