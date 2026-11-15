//! The sizes every component comes in.

/// A component's size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    /// Extra small.
    Xs,
    /// Small, the default.
    #[default]
    Sm,
    /// Medium.
    Md,
    /// Large.
    Lg,
}
