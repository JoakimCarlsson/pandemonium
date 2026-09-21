//! The colours code is highlighted in.

use pm_gfx::Rgba;

/// The colours code is highlighted in.
#[derive(Clone, Copy, Debug)]
pub struct Syntax {
    /// Keywords and operators.
    pub keyword: Rgba,
    /// String and character literals.
    pub string: Rgba,
    /// Function and method names.
    pub function: Rgba,
    /// Comments and documentation.
    pub comment: Rgba,
    /// Numeric literals.
    pub number: Rgba,
    /// Types, traits and named constants.
    pub type_name: Rgba,
    /// Brackets, delimiters and other punctuation.
    pub punctuation: Rgba,
}
