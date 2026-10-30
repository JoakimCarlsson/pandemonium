//! The shaper: fonts loaded once, and runs shaped and rasterized against them.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cosmic_text::fontdb::Weight;
use cosmic_text::{
    Attrs, Buffer, Family, FeatureTag, FontFeatures, FontSystem, Metrics, Shaping, Style,
    SwashCache, SwashContent,
};
use pm_typography::{FontFamily, FontSlant, TypographyStyle};

use crate::run::{Bitmap, GlyphKey, Run};

/// How many shaped runs each cache generation holds.
const SHAPED_LIMIT: usize = 32_768;

/// The estimated bytes of text and glyphs kept in each generation.
const SHAPED_BYTES: usize = 16 * 1024 * 1024;

/// Supplies unique font identities to dependent layout caches.
static NEXT_FONT_KEY: AtomicU64 = AtomicU64::new(1);

/// What a shaped run depends on besides the text: the family, the size
/// and line height as bits, the weight and the slant.
type Setting = (FontFamily, u32, u32, u16, FontSlant);

/// The room one run of text takes, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Extent {
    /// The width of the run.
    pub width: f32,
    /// The height of its line.
    pub height: f32,
}

/// Owns the system fonts and shapes and rasterizes text with them.
pub struct Shaper {
    /// The font database and shaping engine.
    fonts: FontSystem,
    /// The rasterizer for glyph outlines.
    swash: SwashCache,
    /// The installed family sans text is set in, when a stack named one.
    sans: Option<String>,
    /// The installed family monospaced text is set in, when a stack named one.
    mono: Option<String>,
    /// The runs shaped before, by their setting and then their text.
    shaped: HashMap<Setting, HashMap<String, Run>>,
    /// How many runs `shaped` holds across all settings.
    shaped_count: usize,
    /// The previous generation, promoted on use instead of flushing all hot runs at once.
    older: HashMap<Setting, HashMap<String, Run>>,
    /// Estimated retained bytes in the newer generation.
    shaped_bytes: usize,
    /// Identifies this shaper and its current font stacks across layout caches.
    font_key: u64,
    /// Whether monospaced text joins characters into the ligatures its font has for them.
    ligatures: bool,
}

impl Shaper {
    /// Loads the system fonts.
    pub fn new() -> Self {
        Self {
            fonts: FontSystem::new(),
            swash: SwashCache::new(),
            sans: None,
            mono: None,
            shaped: HashMap::new(),
            shaped_count: 0,
            older: HashMap::new(),
            shaped_bytes: 0,
            font_key: NEXT_FONT_KEY.fetch_add(1, Ordering::Relaxed),
            ligatures: true,
        }
    }

    /// Whether monospaced text joins characters into the ligatures its font has for them.
    pub fn ligatures(&self) -> bool {
        self.ligatures
    }

    /// Lets monospaced text join characters such as `->` into the ligatures its font has for them, or sets each character alone when `on` is not set; text shaped before is shaped again.
    pub fn set_ligatures(&mut self, on: bool) {
        if self.ligatures == on {
            return;
        }
        self.ligatures = on;
        self.shaped.clear();
        self.shaped_count = 0;
        self.older.clear();
        self.shaped_bytes = 0;
        self.font_key = NEXT_FONT_KEY.fetch_add(1, Ordering::Relaxed);
    }

    /// Sets sans and monospaced text in the first family of each stack that is
    /// installed, leaving the system's own choice where none is.
    pub fn set_stacks(&mut self, sans: &[&str], mono: &[&str]) {
        self.sans = self.installed(sans);
        self.mono = self.installed(mono);
        self.shaped.clear();
        self.shaped_count = 0;
        self.older.clear();
        self.shaped_bytes = 0;
        self.font_key = NEXT_FONT_KEY.fetch_add(1, Ordering::Relaxed);
    }

    /// Identifies the fonts used by this shaper; changes whenever its font stacks are reset.
    pub fn font_key(&self) -> u64 {
        self.font_key
    }

    /// The families of the fonts installed on the system, each once and in the order of their names, and those among them whose letters are all as wide as each other.
    pub fn families(&self) -> (Vec<String>, Vec<String>) {
        let mut all = std::collections::BTreeSet::new();
        let mut mono = std::collections::BTreeSet::new();
        for face in self.fonts.db().faces() {
            for (name, _) in &face.families {
                all.insert(name.clone());
                if face.monospaced {
                    mono.insert(name.clone());
                }
            }
        }
        (all.into_iter().collect(), mono.into_iter().collect())
    }

    /// The first of `stack` the system has a font for.
    fn installed(&self, stack: &[&str]) -> Option<String> {
        stack.iter().find_map(|wanted| {
            self.fonts
                .db()
                .faces()
                .flat_map(|face| face.families.iter())
                .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
                .map(|(name, _)| name.clone())
        })
    }

    /// Shapes `text` on one line in `style`, remembered so the same text in
    /// the same setting is shaped once.
    pub fn shape(&mut self, text: &str, style: &TypographyStyle) -> Run {
        let setting = (
            style.family,
            style.size.to_bits(),
            style.line_height_px().to_bits(),
            style.weight,
            style.slant,
        );
        if let Some(run) = self.shaped.get(&setting).and_then(|runs| runs.get(text)) {
            return run.clone();
        }
        let run = self
            .older
            .get_mut(&setting)
            .and_then(|runs| runs.remove(text))
            .unwrap_or_else(|| self.shape_uncached(text, style));
        let bytes = text.len() + run.glyphs.len() * std::mem::size_of::<cosmic_text::LayoutGlyph>();
        if bytes <= SHAPED_BYTES {
            if self.shaped_count >= SHAPED_LIMIT || self.shaped_bytes + bytes > SHAPED_BYTES {
                self.older = std::mem::take(&mut self.shaped);
                self.shaped_count = 0;
                self.shaped_bytes = 0;
            }
            self.shaped
                .entry(setting)
                .or_default()
                .insert(text.to_owned(), run.clone());
            self.shaped_count += 1;
            self.shaped_bytes += bytes;
        }
        run
    }

    /// The extent `text` occupies on one line in `style`.
    pub fn measure(&mut self, text: &str, style: &TypographyStyle) -> Extent {
        let run = self.shape(text, style);
        Extent {
            width: run.width,
            height: run.height,
        }
    }

    /// Shapes `text` on one line in `style` with the shaping engine.
    fn shape_uncached(&mut self, text: &str, style: &TypographyStyle) -> Run {
        let line_height = style.line_height_px();
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(style.size, line_height));
        let family = match style.family {
            FontFamily::Sans => self.sans.as_deref().map_or(Family::SansSerif, Family::Name),
            FontFamily::Serif => Family::Serif,
            FontFamily::Mono => self.mono.as_deref().map_or(Family::Monospace, Family::Name),
        };
        let mut features = FontFeatures::new();
        if !self.ligatures && style.family == FontFamily::Mono {
            for tag in [
                FeatureTag::STANDARD_LIGATURES,
                FeatureTag::CONTEXTUAL_LIGATURES,
                FeatureTag::CONTEXTUAL_ALTERNATES,
            ] {
                features.disable(tag);
            }
        }
        let attrs = Attrs::new()
            .font_features(features)
            .family(family)
            .weight(Weight(style.weight))
            .style(match style.slant {
                FontSlant::Normal => Style::Normal,
                FontSlant::Italic => Style::Italic,
            });
        let mut buffer = buffer.borrow_with(&mut self.fonts);
        buffer.set_size(None, None);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        match buffer.layout_runs().next() {
            Some(line) => Run {
                width: line.line_w,
                height: line_height,
                baseline: line.line_y - line.line_top,
                glyphs: Arc::from(line.glyphs),
            },
            None => Run {
                width: 0.0,
                height: line_height,
                baseline: line_height,
                glyphs: Arc::from([]),
            },
        }
    }

    /// The coverage bitmap of one glyph, or nothing for a glyph with no ink
    /// or one that is not a plain mask.
    pub fn rasterize(&mut self, key: GlyphKey) -> Option<Bitmap> {
        let image = self.swash.get_image_uncached(&mut self.fonts, key.0)?;
        if image.content != SwashContent::Mask || image.placement.width == 0 {
            return None;
        }
        Some(Bitmap {
            width: image.placement.width,
            height: image.placement.height,
            left: image.placement.left,
            top: image.placement.top,
            coverage: image.data,
        })
    }
}

impl Default for Shaper {
    /// [`Shaper::new`].
    fn default() -> Self {
        Self::new()
    }
}
