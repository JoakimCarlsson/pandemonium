//! Font selection, shaping and the measurement layout asks for.

use std::collections::HashMap;
use std::sync::Arc;

use cosmic_text::fontdb::Weight;
use cosmic_text::{Attrs, Buffer, Family, FamilyOwned, FontSystem, LayoutGlyph, Metrics, Shaping};

use crate::geometry::Size;

/// Families tried in order before falling back to the platform sans-serif.
const PREFERRED_FAMILIES: [&str; 5] = [
    "Inter",
    "Inter Display",
    "SF Pro Text",
    "Noto Sans",
    "DejaVu Sans",
];

/// How a run of text is drawn: size, leading, weight and slant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontStyle {
    /// Em size in logical pixels.
    pub size: f32,
    /// Distance between baselines in logical pixels.
    pub line_height: f32,
    /// Weight on the usual 100..=900 scale.
    pub weight: u16,
    /// Whether the run is slanted.
    pub italic: bool,
}

impl FontStyle {
    /// Creates a style at `size` with a line height of 1.4 em and regular weight.
    pub const fn new(size: f32) -> Self {
        Self {
            size,
            line_height: size * 1.4,
            weight: 400,
            italic: false,
        }
    }

    /// Returns this style at `weight` on the usual 100..=900 scale.
    pub const fn weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }

    /// Returns this style with `line_height` logical pixels between baselines.
    pub const fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    /// Returns this style slanted.
    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
}

/// A shaped run of text, positioned relative to its own top-left corner.
pub struct ShapedRun {
    /// Advance width of the whole run.
    pub width: f32,
    /// Height of the line box the run occupies.
    pub height: f32,
    /// Baseline offset from the top of the line box.
    pub baseline: f32,
    /// The glyphs in visual order, still in logical units.
    pub(crate) glyphs: Vec<LayoutGlyph>,
}

impl ShapedRun {
    /// The extent this run occupies during layout.
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

/// Identifies a shaped run in the cache.
#[derive(Clone, PartialEq, Eq, Hash)]
struct RunKey {
    /// The text that was shaped.
    text: String,
    /// Em size, in raw bits so it can be hashed.
    size: u32,
    /// Line height, in raw bits so it can be hashed.
    line_height: u32,
    /// Weight on the usual 100..=900 scale.
    weight: u16,
    /// Whether the run is slanted.
    italic: bool,
}

impl RunKey {
    /// Builds the key identifying `text` drawn in `style`.
    fn new(text: &str, style: FontStyle) -> Self {
        Self {
            text: text.to_owned(),
            size: style.size.to_bits(),
            line_height: style.line_height.to_bits(),
            weight: style.weight,
            italic: style.italic,
        }
    }
}

/// Shapes text once and hands the same run to layout and to drawing.
pub struct TextSystem {
    /// The font database and shaping engine.
    fonts: FontSystem,
    /// The family every run is shaped with.
    family: FamilyOwned,
    /// Runs already shaped, keyed by their text and style.
    runs: HashMap<RunKey, Arc<ShapedRun>>,
}

impl TextSystem {
    /// Loads the system fonts and picks the UI family.
    pub fn new() -> Self {
        let fonts = FontSystem::new();
        let family = choose_family(&fonts);
        Self {
            fonts,
            family,
            runs: HashMap::new(),
        }
    }

    /// The font system, for the rasterizer that shares it.
    pub(crate) fn fonts_mut(&mut self) -> &mut FontSystem {
        &mut self.fonts
    }

    /// Shapes `text` in `style`, reusing the cached run when there is one.
    pub fn shape(&mut self, text: &str, style: FontStyle) -> Arc<ShapedRun> {
        let key = RunKey::new(text, style);
        if let Some(run) = self.runs.get(&key) {
            return run.clone();
        }

        let run = Arc::new(self.shape_uncached(text, style));
        self.runs.insert(key, run.clone());
        run
    }

    /// The extent `text` would occupy in `style`.
    pub fn measure(&mut self, text: &str, style: FontStyle) -> Size {
        self.shape(text, style).size()
    }

    /// Lays a single unwrapped line out and collects its glyphs.
    fn shape_uncached(&mut self, text: &str, style: FontStyle) -> ShapedRun {
        let metrics = Metrics::new(style.size, style.line_height);
        let mut buffer = Buffer::new(&mut self.fonts, metrics);
        let attrs = Attrs::new()
            .family(self.family.as_family())
            .weight(Weight(style.weight))
            .style(if style.italic {
                cosmic_text::Style::Italic
            } else {
                cosmic_text::Style::Normal
            });

        let mut buffer = buffer.borrow_with(&mut self.fonts);
        buffer.set_size(None, None);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);

        let Some(line) = buffer.layout_runs().next() else {
            return ShapedRun {
                width: 0.0,
                height: style.line_height,
                baseline: style.line_height,
                glyphs: Vec::new(),
            };
        };

        ShapedRun {
            width: line.line_w,
            height: style.line_height,
            baseline: line.line_y - line.line_top,
            glyphs: line.glyphs.to_vec(),
        }
    }
}

impl Default for TextSystem {
    /// Loads the system fonts and picks the UI family.
    fn default() -> Self {
        Self::new()
    }
}

/// Picks the first installed family from [`PREFERRED_FAMILIES`].
fn choose_family(fonts: &FontSystem) -> FamilyOwned {
    for wanted in PREFERRED_FAMILIES {
        let installed = fonts
            .db()
            .faces()
            .flat_map(|face| face.families.iter())
            .any(|(name, _)| name == wanted);
        if installed {
            return FamilyOwned::new(Family::Name(wanted));
        }
    }
    FamilyOwned::new(Family::SansSerif)
}
