//! The semantic colours: what a surface, a border or a run of text is painted
//! in, named for what it means rather than for the hue a theme gives it.

use pm_gfx::Rgba;

/// The semantic colours elements are painted in.
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    /// The window behind everything.
    pub background: Rgba,
    /// A panel or card sitting on the background.
    pub surface: Rgba,
    /// A surface under the pointer.
    pub surface_hover: Rgba,
    /// A surface being pressed.
    pub surface_active: Rgba,
    /// A surface that is the selected one of a set.
    pub surface_selected: Rgba,
    /// The ordinary divider and outline colour.
    pub border: Rgba,
    /// A quieter divider, for rules inside a panel.
    pub border_variant: Rgba,
    /// The outline of the element holding keyboard focus.
    pub border_focused: Rgba,
    /// The outline of a selected element.
    pub border_selected: Rgba,
    /// The wash over where something being carried would land if let go of.
    ///
    /// A drop target is read against whatever it covers — a pane of text, a
    /// bar of tabs, an empty pane — so it is a translucent neutral rather
    /// than the accent, which a theme is free to make as quiet as it likes.
    pub drop_target: Rgba,
    /// Body text.
    pub text: Rgba,
    /// Secondary text: descriptions and captions.
    pub text_muted: Rgba,
    /// Text that is barely there: hints and disabled labels.
    pub text_subtle: Rgba,
    /// Text drawn on top of `accent`.
    pub text_on_accent: Rgba,
    /// The caret in a pane of text.
    ///
    /// Not the accent: a theme is free to have no accent colour at all, and
    /// a caret that cannot be found is a caret that is not there.
    pub cursor: Rgba,
    /// The wash over selected text.
    pub selection: Rgba,
    /// A name that can be followed, while the pointer is over it.
    ///
    /// Not the accent: the accent is a surface a control is drawn on, and a
    /// link is a line drawn on the background under a name, which has to be
    /// legible against it in a theme whose accent is nearly the background.
    pub link: Rgba,
    /// The one colour that means "this is the action".
    pub accent: Rgba,
    /// The accent under the pointer.
    pub accent_hover: Rgba,
    /// The accent being pressed.
    pub accent_active: Rgba,
    /// Something finished or is healthy.
    pub success: Rgba,
    /// Something needs attention.
    pub warning: Rgba,
    /// Something failed or is destructive.
    pub danger: Rgba,
}
