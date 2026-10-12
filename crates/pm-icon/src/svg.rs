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
}
