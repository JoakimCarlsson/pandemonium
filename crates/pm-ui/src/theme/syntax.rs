//! The colours code is highlighted in.

use pm_gfx::Rgba;

/// The colours code is highlighted in.
#[derive(Clone, Copy, Debug, Default)]
pub struct Syntax {
    /// Keywords.
    pub keyword: Rgba,
    /// String and character literals.
    pub string: Rgba,
    /// Function and method names.
    pub function: Rgba,
    /// Comments and documentation.
    pub comment: Rgba,
    /// Numeric and boolean literals.
    pub number: Rgba,
    /// Types and traits.
    pub type_name: Rgba,
    /// Brackets, delimiters and other punctuation.
    pub punctuation: Rgba,
    /// Variables, parameters and plain identifiers.
    pub variable: Rgba,
    /// Fields, members and keys.
    pub property: Rgba,
    /// Named constants.
    pub constant: Rgba,
    /// Operators.
    pub operator: Rgba,
    /// Markup elements, such as HTML and JSX tags.
    pub tag: Rgba,
    /// Markup attributes, and annotations on declarations.
    pub attribute: Rgba,
}
