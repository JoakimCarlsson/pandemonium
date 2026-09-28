//! Pictures: pixels decoded once, drawn in their own colours.
//!
//! An icon is coverage tinted by the caller; a picture is the opposite, its
//! colours are the point. So a picture is not packed into the glyph atlas: it
//! is a texture of its own, uploaded the first frame it is drawn and kept for
//! as long as frames keep drawing it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree, fontdb};

/// Longest side a picture is kept at, larger ones being scaled down to it.
///
/// Every device wgpu runs on takes a texture this large, and a screen shows
/// nothing of a larger one that this does not.
const LONGEST_SIDE: u32 = 4096;

/// Side an SVG document is drawn at when it gives no size of its own worth
/// keeping, before it is scaled up to be looked at.
const VECTOR_SIDE: f32 = 1024.0;

/// Families an SVG's `sans-serif` is drawn in, the first one installed
/// winning, led by the one the editor's own prose is shaped in.
const SANS_SERIF: &[&str] = &["Open Sans", "Noto Sans", "DejaVu Sans", "Liberation Sans"];

/// Families an SVG's `monospace` is drawn in, the first one installed winning,
/// led by the one the editor's own code is shaped in.
const MONOSPACE: &[&str] = &["Noto Sans Mono", "DejaVu Sans Mono", "Liberation Mono"];

/// The system's fonts, loaded once for the text SVG documents draw, with the
/// generic families named after faces that are installed.
static FONTS: LazyLock<Arc<fontdb::Database>> = LazyLock::new(|| {
    let mut fonts = fontdb::Database::new();
    fonts.load_system_fonts();
    if let Some(family) = installed(&fonts, SANS_SERIF) {
        fonts.set_sans_serif_family(family);
    }
    if let Some(family) = installed(&fonts, MONOSPACE) {
        fonts.set_monospace_family(family);
    }
    Arc::new(fonts)
});

/// The identity the next picture decoded is given.
static NEXT: AtomicU64 = AtomicU64::new(1);

/// One decoded picture: its size and its pixels, shared however often it is
/// drawn.
///
/// The identity is what the renderer keys the texture on, so a picture that
/// is drawn every frame is uploaded once, and two decodes of one file are two
/// pictures.
#[derive(Clone)]
pub struct Image {
    /// What the renderer knows this picture's texture by.
    id: u64,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// The pixels, row by row, four sRGB-encoded bytes each with the alpha
    /// not multiplied in.
    pixels: Arc<[u8]>,
}

impl std::fmt::Debug for Image {
    /// Names the picture by its identity and size, not by its pixels.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Image")
            .field("id", &self.id)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl Image {
    /// Decodes `bytes` as a PNG, JPEG, GIF, WebP, BMP or ICO file, or as an
    /// SVG document when it is none of those.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        Self::raster(bytes).or_else(|| Self::vector(bytes))
    }

    /// Rasterizes a full-colour SVG at `scale` physical pixels per logical pixel.
    pub fn from_svg(source: &str, scale: f32) -> Option<Self> {
        let tree = Tree::from_str(source, &svg_options()).ok()?;
        Self::render_svg(&tree, scale)
    }

    /// A picture of `width` by `height` straight-alpha RGBA `pixels`, scaled
    /// down to what a texture can hold, or none when the pixels do not fill
    /// that size.
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let decoded =
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(width, height, pixels)?);
        Some(Self::fitted(decoded))
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// What the renderer knows this picture's texture by.
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// The pixels, four bytes each, row by row.
    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Decodes one of the raster formats, scaling it down to what a texture
    /// can hold.
    fn raster(bytes: &[u8]) -> Option<Self> {
        Some(Self::fitted(image::load_from_memory(bytes).ok()?))
    }

    /// `decoded`, scaled down to what a texture can hold.
    fn fitted(decoded: image::DynamicImage) -> Self {
        let decoded = match decoded.width().max(decoded.height()) > LONGEST_SIDE {
            true => decoded.thumbnail(LONGEST_SIDE, LONGEST_SIDE),
            false => decoded,
        };
        let pixels = decoded.into_rgba8();
        Self::of(pixels.width(), pixels.height(), pixels.into_raw())
    }

    /// Draws an SVG document at the size it asks for, or at a size worth
    /// looking at when it asks for a small one.
    fn vector(bytes: &[u8]) -> Option<Self> {
        let tree = Tree::from_data(bytes, &svg_options()).ok()?;
        let extent = tree.size();
        let longest = extent.width().max(extent.height()).max(f32::EPSILON);
        let scale = (VECTOR_SIDE / longest).clamp(1.0, LONGEST_SIDE as f32 / longest);
        Self::render_svg(&tree, scale)
    }

    /// Rasterizes `tree` at `scale`, bounded by the maximum texture side.
    fn render_svg(tree: &Tree, scale: f32) -> Option<Self> {
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let extent = tree.size();
        let longest = extent.width().max(extent.height()).max(f32::EPSILON);
        let scale = scale.min(LONGEST_SIDE as f32 / longest);
        let width = (extent.width() * scale).ceil() as u32;
        let height = (extent.height() * scale).ceil() as u32;
        let mut pixmap = Pixmap::new(width.max(1), height.max(1))?;
        resvg::render(
            tree,
            Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        let pixels = pixmap
            .pixels()
            .iter()
            .flat_map(|texel| {
                let straight = texel.demultiply();
                [
                    straight.red(),
                    straight.green(),
                    straight.blue(),
                    straight.alpha(),
                ]
            })
            .collect::<Vec<_>>();
        Some(Self::of(pixmap.width(), pixmap.height(), pixels))
    }

    /// A picture of `width` by `height` holding `pixels`, given the next
    /// identity.
    fn of(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            pixels: pixels.into(),
        }
    }
}

/// How an SVG document is read: with the system's fonts, so its text draws.
fn svg_options() -> Options<'static> {
    Options {
        fontdb: FONTS.clone(),
        ..Options::default()
    }
}

/// The first of `families` that `fonts` holds a face of, or else the family
/// of whichever face it holds first.
fn installed(fonts: &fontdb::Database, families: &[&str]) -> Option<String> {
    let has = |family: &str| {
        fonts
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == family))
    };
    families
        .iter()
        .find(|family| has(family))
        .map(|family| (*family).to_owned())
        .or_else(|| {
            fonts
                .faces()
                .find_map(|face| face.families.first().map(|(name, _)| name.clone()))
        })
}
