//! The roles a colour plays in a window, named for what they mean.
//!
//! A style that says `bg(Role::Surface)` does not say which colour that is:
//! whoever paints it resolves the role against the theme in hand, so changing
//! the theme changes every box without rebuilding any of them.

use crate::Srgba;

/// What a colour is for.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Role {
    /// The window behind everything.
    Background,
    /// The side panel listing projects and sessions.
    Sidebar,
    /// A panel or card sitting on the background.
    Surface,
    /// A surface under the pointer.
    SurfaceHover,
    /// A surface being pressed.
    SurfaceActive,
    /// A surface that is the selected one of a set.
    SurfaceSelected,
    /// Dividers and outlines.
    Border,
    /// The outline of the element holding keyboard focus.
    Focused,
    /// Body text.
    Text,
    /// Secondary text: descriptions and captions.
    Muted,
    /// Text that is barely there: hints and disabled labels.
    Subtle,
    /// The one colour that means "this is the action".
    Accent,
    /// The accent under the pointer.
    AccentHover,
    /// Something finished or is healthy.
    Success,
    /// Something failed or is destructive.
    Danger,
    /// Text and icons drawn on the accent.
    OnAccent,
    /// Text and icons drawn on the success colour.
    OnSuccess,
    /// Text and icons drawn on the danger colour.
    OnDanger,
    /// Something that may be wrong.
    Warning,
    /// The words of a programming language in code.
    SyntaxKeyword,
    /// Functions, methods and macros in code.
    SyntaxFunction,
    /// Types and modules in code.
    SyntaxType,
    /// Strings and characters in code.
    SyntaxString,
    /// Numbers and constants in code.
    SyntaxConstant,
    /// Comments in code.
    SyntaxComment,
    /// Fields, properties and attributes in code.
    SyntaxProperty,
    /// Operators and punctuation in code.
    SyntaxPunctuation,
}

impl Role {
    /// Every role, in declaration order.
    pub const ALL: [Self; 27] = [
        Self::Background,
        Self::Sidebar,
        Self::Surface,
        Self::SurfaceHover,
        Self::SurfaceActive,
        Self::SurfaceSelected,
        Self::Border,
        Self::Focused,
        Self::Text,
        Self::Muted,
        Self::Subtle,
        Self::Accent,
        Self::AccentHover,
        Self::Success,
        Self::Danger,
        Self::OnAccent,
        Self::OnSuccess,
        Self::OnDanger,
        Self::Warning,
        Self::SyntaxKeyword,
        Self::SyntaxFunction,
        Self::SyntaxType,
        Self::SyntaxString,
        Self::SyntaxConstant,
        Self::SyntaxComment,
        Self::SyntaxProperty,
        Self::SyntaxPunctuation,
    ];

    /// The role's name as a theme file writes it, such as `surface-hover`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Sidebar => "sidebar",
            Self::Surface => "surface",
            Self::SurfaceHover => "surface-hover",
            Self::SurfaceActive => "surface-active",
            Self::SurfaceSelected => "surface-selected",
            Self::Border => "border",
            Self::Focused => "focused",
            Self::Text => "text",
            Self::Muted => "muted",
            Self::Subtle => "subtle",
            Self::Accent => "accent",
            Self::AccentHover => "accent-hover",
            Self::Success => "success",
            Self::Danger => "danger",
            Self::OnAccent => "on-accent",
            Self::OnSuccess => "on-success",
            Self::OnDanger => "on-danger",
            Self::Warning => "warning",
            Self::SyntaxKeyword => "syntax-keyword",
            Self::SyntaxFunction => "syntax-function",
            Self::SyntaxType => "syntax-type",
            Self::SyntaxString => "syntax-string",
            Self::SyntaxConstant => "syntax-constant",
            Self::SyntaxComment => "syntax-comment",
            Self::SyntaxProperty => "syntax-property",
            Self::SyntaxPunctuation => "syntax-punctuation",
        }
    }
}

/// Something that knows which colour each role is.
pub trait Roles {
    /// The colour `role` is painted in.
    fn role(&self, role: Role) -> Srgba;
}
