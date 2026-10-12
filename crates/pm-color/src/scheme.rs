//! The roles a colour plays in a window: interface, syntax and terminal.
//!
//! A role is named for what it means rather than for the hue a theme gives
//! it, so one screen is painted in any theme.

use crate::Srgba;

/// The semantic colours elements are painted in.
#[derive(Clone, Copy, Debug, Default)]
pub struct Interface {
    /// The window behind everything.
    pub background: Srgba,
    /// A panel or card sitting on the background.
    pub surface: Srgba,
    /// A surface under the pointer.
    pub surface_hover: Srgba,
    /// A surface being pressed.
    pub surface_active: Srgba,
    /// A surface that is the selected one of a set.
    pub surface_selected: Srgba,
    /// The ordinary divider and outline colour.
    pub border: Srgba,
    /// A quieter divider, for rules inside a panel.
    pub border_variant: Srgba,
    /// The outline of the element holding keyboard focus.
    pub border_focused: Srgba,
    /// The outline of a selected element.
    pub border_selected: Srgba,
    /// The wash over where something being carried would land if let go of.
    ///
    /// A drop target is read against whatever it covers — a pane of text, a
    /// bar of tabs, an empty pane — so it is a translucent neutral rather
    /// than the accent, which a theme is free to make as quiet as it likes.
    pub drop_target: Srgba,
    /// Body text.
    pub text: Srgba,
    /// Secondary text: descriptions and captions.
    pub text_muted: Srgba,
    /// Text that is barely there: hints and disabled labels.
    pub text_subtle: Srgba,
    /// Text drawn on top of `accent`.
    pub text_on_accent: Srgba,
    /// The caret in a pane of text.
    ///
    /// Not the accent: a theme is free to have no accent colour at all, and
    /// a caret that cannot be found is a caret that is not there.
    pub cursor: Srgba,
    /// The wash over selected text.
    pub selection: Srgba,
    /// A name that can be followed, while the pointer is over it.
    ///
    /// Not the accent: the accent is a surface a control is drawn on, and a
    /// link is a line drawn on the background under a name, which has to be
    /// legible against it in a theme whose accent is nearly the background.
    pub link: Srgba,
    /// The one colour that means "this is the action".
    pub accent: Srgba,
    /// The accent under the pointer.
    pub accent_hover: Srgba,
    /// The accent being pressed.
    pub accent_active: Srgba,
    /// Something finished or is healthy.
    pub success: Srgba,
    /// Something needs attention.
    pub warning: Srgba,
    /// Something failed or is destructive.
    pub danger: Srgba,
}

/// The colours code is highlighted in.
#[derive(Clone, Copy, Debug, Default)]
pub struct Syntax {
    /// Keywords.
    pub keyword: Srgba,
    /// String and character literals.
    pub string: Srgba,
    /// Function and method names.
    pub function: Srgba,
    /// Comments and documentation.
    pub comment: Srgba,
    /// Numeric and boolean literals.
    pub number: Srgba,
    /// Types and traits.
    pub type_name: Srgba,
    /// Brackets, delimiters and other punctuation.
    pub punctuation: Srgba,
    /// Variables, parameters and plain identifiers.
    pub variable: Srgba,
    /// Fields, members and keys.
    pub property: Srgba,
    /// Named constants.
    pub constant: Srgba,
    /// Operators.
    pub operator: Srgba,
    /// Markup elements, such as HTML and JSX tags.
    pub tag: Srgba,
    /// Markup attributes, and annotations on declarations.
    pub attribute: Srgba,
}

/// The sixteen colours a program can name, and the cursor drawn over them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Terminal {
    /// The eight ordinary colours, then the eight bright ones.
    pub ansi: [Srgba; 16],
    /// The block drawn where the next character will go.
    pub cursor: Srgba,
    /// A selected run of cells.
    pub selection: Srgba,
}
