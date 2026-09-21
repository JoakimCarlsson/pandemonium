//! The corner radii, in the Tailwind naming.

/// Corner radii, in the Tailwind naming.
#[derive(Clone, Copy, Debug)]
pub struct Radii {
    /// 2px: the roundest a 16px tile can be and still read as a square.
    pub sm: f32,
    /// 4px: buttons, fields and the boxes a label sits in.
    pub md: f32,
    /// 6px: cards, tiles and popovers.
    pub lg: f32,
    /// 10px: a panel that floats over the whole window.
    pub xl: f32,
    /// Large enough to round any bar into a pill.
    pub full: f32,
}

impl Radii {
    /// The radii every theme uses.
    pub const DEFAULT: Self = Self {
        sm: 2.0,
        md: 4.0,
        lg: 6.0,
        xl: 10.0,
        full: 9999.0,
    };
}
