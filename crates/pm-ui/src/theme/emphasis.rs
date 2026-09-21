//! How much of a colour a wash, a thumb or a halo carries.
//!
//! A translucent overlay is read against whatever it covers, so how strong it
//! has to be to register is a property of the theme and not of the screen
//! drawing it: the wash that marks the current line is barely there over the
//! near-black of a dark theme and would be a smear over a light one.

/// The alphas the translucent parts of the window are drawn at.
#[derive(Clone, Copy, Debug)]
pub struct Emphasis {
    /// The wash over selected text.
    pub selection: f32,
    /// The wash over the line the caret is on.
    pub current_line: f32,
    /// A scrollbar thumb at rest.
    pub scrollbar: f32,
    /// A scrollbar thumb under the pointer or being dragged.
    pub scrollbar_active: f32,
    /// The halo around a dot that is lit.
    pub halo: f32,
    /// How much of its colour a terminal cell marked faint keeps.
    pub dim: f32,
    /// How far a filled control is lifted towards the text colour on hover.
    pub hover_lift: f32,
}

impl Emphasis {
    /// The emphases every theme uses.
    pub const DEFAULT: Self = Self {
        selection: 0.3,
        current_line: 0.05,
        scrollbar: 0.35,
        scrollbar_active: 0.6,
        halo: 0.22,
        dim: 0.6,
        hover_lift: 0.16,
    };
}
