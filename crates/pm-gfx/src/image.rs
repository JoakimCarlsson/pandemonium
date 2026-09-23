//! Pictures: pixels decoded once, drawn in their own colours.
//!
//! An icon is coverage tinted by the caller; a picture is the opposite, its
//! colours are the point. So a picture is not packed into the glyph atlas: it
//! is a texture of its own, uploaded the first frame it is drawn and kept for
//! as long as frames keep drawing it.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

/// Longest side a picture is kept at, larger ones being scaled down to it.
///
/// Every device wgpu runs on takes a texture this large, and a screen shows
/// nothing of a larger one that this does not.
const LONGEST_SIDE: u32 = 4096;

/// Side an SVG document is drawn at when it gives no size of its own worth
/// keeping, before it is scaled up to be looked at.
const VECTOR_SIDE: f32 = 1024.0;

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
        let decoded = image::load_from_memory(bytes).ok()?;
        let decoded = match decoded.width().max(decoded.height()) > LONGEST_SIDE {
            true => decoded.thumbnail(LONGEST_SIDE, LONGEST_SIDE),
            false => decoded,
        };
        let pixels = decoded.to_rgba8();
        Some(Self::of(pixels.width(), pixels.height(), pixels.into_raw()))
    }

    /// Draws an SVG document at the size it asks for, or at a size worth
    /// looking at when it asks for a small one.
    fn vector(bytes: &[u8]) -> Option<Self> {
        let tree = Tree::from_data(bytes, &Options::default()).ok()?;
        let extent = tree.size();
        let longest = extent.width().max(extent.height()).max(f32::EPSILON);
        let scale = (VECTOR_SIDE / longest).clamp(1.0, LONGEST_SIDE as f32 / longest);
        let width = (extent.width() * scale).ceil() as u32;
        let height = (extent.height() * scale).ceil() as u32;
        let mut pixmap = Pixmap::new(width.max(1), height.max(1))?;
        resvg::render(
            &tree,
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
