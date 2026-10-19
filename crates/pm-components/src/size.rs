//! The size every component is measured in: three steps, which each component turns into its own measures.

/// How big a component is.
///
/// A parent holds one `Size` and passes it to the components it builds; each
/// component says in its own documentation what the steps measure.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Size {
    /// The tightest step: sits inside a strip of controls or a line of text.
    Xs,
    /// The snug step: sits inside a row or a narrow column.
    Sm,
    /// The usual step.
    #[default]
    Md,
    /// The roomy step: closes a step or heads a page.
    Lg,
}
