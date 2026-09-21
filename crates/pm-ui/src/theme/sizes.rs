//! The heights a control or a bar is drawn at.
//!
//! A size is here because more than one screen reaches for it: the row of a
//! file tree and the row of a list are one decision, and a theme that wants a
//! denser window moves them together. A measurement only one screen has —
//! how wide a sidebar opens, how far a tree indents per level — stays with
//! that screen.

/// The heights a control or a bar is drawn at, in logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct Sizes {
    /// 20px: a square control carrying nothing but an icon.
    pub icon_control: f32,
    /// 24px: a bar of status along an edge of the window.
    pub bar: f32,
    /// 26px: one row of a list or a tree.
    pub row: f32,
    /// 28px: a button, and anything else a label sits in a box of.
    pub control: f32,
    /// 32px: a field, and the row a labelled control shares with its label.
    pub field: f32,
    /// 32px: the bar of tabs above a pane, hairline included.
    pub tab_bar: f32,
    /// 40px: the content-backed window title bar.
    pub titlebar: f32,
}

impl Sizes {
    /// The sizes every theme uses.
    pub const DEFAULT: Self = Self {
        icon_control: 20.0,
        bar: 24.0,
        row: 26.0,
        control: 28.0,
        field: 32.0,
        tab_bar: 32.0,
        titlebar: 40.0,
    };
}
