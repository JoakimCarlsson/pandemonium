//! One piece of vector artwork.

/// One piece of vector artwork, named so a renderer can key a cache on it.
///
/// The name is the identity: two icons with the same name are the same icon,
/// and the source only needs reading the first time one is rasterized.
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

    /// The artwork `source` that arrived while the app runs, such as the icon of an extension, called `name`; artwork of one name is kept once, the first source given for it standing for every later one.
    pub fn arrived(name: &str, source: &str) -> Self {
        let mut held = ARRIVED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(svg) = held.get(name) {
            return *svg;
        }
        let svg = Self {
            name: Box::leak(name.to_owned().into_boxed_str()),
            source: Box::leak(source.to_owned().into_boxed_str()),
        };
        held.insert(name.to_owned(), svg);
        svg
    }
}

/// The artwork that arrived while the app runs, by name, each kept for as long as the app runs.
static ARRIVED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, Svg>>> =
    std::sync::LazyLock::new(Default::default);

/// An icon's coverage at one size: how much ink each pixel holds, row by row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mask {
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
    /// One byte per pixel, 0 for none and 255 for full.
    pub coverage: Vec<u8>,
}

impl Svg {
    /// The icon drawn into a square of `pixels` on a side, or nothing when the
    /// size is zero or the source is not a document that can be drawn.
    pub fn rasterize(&self, pixels: u32) -> Option<Mask> {
        let tree =
            resvg::usvg::Tree::from_str(self.source, &resvg::usvg::Options::default()).ok()?;
        let mut canvas = resvg::tiny_skia::Pixmap::new(pixels, pixels)?;
        let scale = pixels as f32 / tree.size().width().max(tree.size().height());
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut canvas.as_mut(),
        );
        let coverage = canvas.pixels().iter().map(|pixel| pixel.alpha()).collect();
        Some(Mask {
            width: pixels,
            height: pixels,
            coverage,
        })
    }
}
