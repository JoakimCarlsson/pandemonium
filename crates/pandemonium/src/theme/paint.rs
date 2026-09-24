//! What a theme file paints over the family it builds on, in one appearance.

use std::collections::BTreeMap;

use pm_gfx::Rgba;
use pm_ui::Theme;

use crate::theme::tokens::TOKENS;
use crate::theme::weights::Weight;

/// How many colours an ANSI palette has.
pub const ANSI: usize = 16;

/// The colours, the palette and the emphases one appearance of a theme file
/// names; whatever it leaves out is left to the family beneath.
#[derive(Clone, Debug, Default)]
pub struct Paint {
    /// The colours it repaints, by index into [`TOKENS`].
    pub colors: BTreeMap<usize, Rgba>,
    /// The terminal palette, when it names a whole one.
    pub palette: Option<[Rgba; ANSI]>,
    /// The emphases it sets.
    pub weights: Vec<(&'static Weight, f32)>,
}

impl Paint {
    /// Lays this over `theme`.
    pub fn apply(&self, theme: &mut Theme) {
        for (token, color) in &self.colors {
            TOKENS[*token].write(theme, *color);
        }
        if let Some(palette) = self.palette {
            theme.terminal.ansi = palette;
        }
        for (weight, value) in &self.weights {
            weight.write(theme, *value);
        }
    }
}
