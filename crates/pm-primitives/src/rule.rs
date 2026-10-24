//! The unit every style crate is built from.

/// One declaration that changes one part of a style.
///
/// A rule is a small `Copy` value, so a list of them costs nothing to build
/// for every frame; applying it writes straight into the style it targets.
pub trait Rule: Copy {
    /// The style this rule writes into.
    type Target;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut Self::Target);
}
