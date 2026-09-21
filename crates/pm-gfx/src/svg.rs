//! Vector icons: the artwork, and the rasterizer that turns it into coverage.
//!
//! An icon is drawn the way a glyph is — a coverage bitmap in the shared
//! atlas, tinted when it is drawn — so the colour lives in the caller's theme
//! rather than in the artwork, and one file serves every colour it is asked
//! for. The artwork is rasterized once per size, on the first frame that asks
//! for that size.

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

/// One piece of vector artwork, named so the atlas can key on it.
///
/// The name is the identity: two icons with the same name are the same icon,
/// and the source is only read the first time one is rasterized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Svg {
    /// What this icon is called, unique across the icons a build ships.
    pub name: &'static str,
    /// The document the icon is drawn from.
    pub source: &'static str,
}

impl Svg {
    /// The icon called `name`, drawn from `source`.
    pub const fn new(name: &'static str, source: &'static str) -> Self {
        Self { name, source }
    }
}

/// Renders `svg` into a square of `size` texels of coverage.
///
/// The artwork's own colours are thrown away: what comes back is how much of
/// each texel the drawing covers, which is exactly what the glyph pipeline
/// samples.
pub(crate) fn rasterize(svg: Svg, size: u32) -> Option<Vec<u8>> {
    if size == 0 {
        return None;
    }

    let tree = Tree::from_str(svg.source, &Options::default()).ok()?;
    let mut pixmap = Pixmap::new(size, size)?;
    let extent = tree.size();
    let scale = size as f32 / extent.width().max(extent.height()).max(f32::EPSILON);
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    Some(pixmap.pixels().iter().map(|texel| texel.alpha()).collect())
}
