//! The element tree, the layout pass, hit testing, focus and input routing.
//!
//! `pm-ui` is ours: there is no UI framework under it, only [`pm_gfx`]'s draw
//! list. A screen is a function that builds an element tree from the caller's
//! own state; [`Ui::draw`] measures it, paints it and remembers where its
//! interactive regions ended up, so the next click or keypress comes back as
//! one of the caller's messages.
//!
//! Styling follows Tailwind: a fixed spacing scale, one utility setter per
//! step, and design tokens on a [`Theme`] instead of colours written at the
//! call site.

mod div;
mod element;
mod scroll;
mod style;
mod text;
mod theme;
mod ui;
mod widgets;

pub use div::{Div, div, h_flex, v_flex};
pub use element::{Element, Input, Interaction, IntoElement, LayoutContext, PaintContext, Region};
pub use scroll::Scroll;
pub use style::{Align, Axis, Edges, Justify, Length, STEP, Style, Styled, space};
pub use text::{Text, text};
pub use theme::{
    Appearance, Colors, DEFAULT_FAMILY, FAMILIES, Radii, Syntax, TextScale, Theme, ThemeFamily,
    family,
};
pub use ui::Ui;
pub use widgets::{
    Button, ButtonVariant, Switch, ThemePreview, button, rule, section, switch, switch_field,
    theme_preview, toggle_grid, toggle_row,
};
