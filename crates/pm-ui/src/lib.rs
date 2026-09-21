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
mod glyphs;
mod icons;
mod measured;
mod overlay;
mod resize;
mod scroll;
mod split;
mod style;
mod text;
mod theme;
mod ui;
mod widgets;

pub use div::{Div, div, h_flex, v_flex};
pub use element::{
    Element, Input, Interaction, IntoElement, LayoutContext, PaintContext, Region, RegionAction,
};
pub use glyphs::Glyphs;
pub use icons::{Icon, IconName, IconSize, LayoutIcon, LayoutIconButton, icon, layout_icon_button};
pub use measured::{Bounds, Measured, measured};
pub use overlay::{Overlay, overlay};
pub use resize::{ResizeEdge, ResizeEvent, ResizePhase, ResizeState, Sash, sash};
pub use scroll::Scroll;
pub use split::{Split, split};
pub use style::{Align, Axis, Edges, Justify, Length, STEP, Style, Styled, space};
pub use text::{Text, text};
pub use theme::{
    Appearance, Colors, DEFAULT_FAMILY, Emphasis, FAMILIES, Font, Radii, Sizes, Syntax, Terminal,
    TextScale, TextSize, Theme, ThemeFamily, family,
};
pub use ui::{PointerCursor, Ui};
pub use widgets::{
    Button, ButtonVariant, MenuItem, Switch, Tab, ThemePreview, button, icon_button, menu,
    menu_entry, menu_separator, rule, section, switch, switch_field, tab, tab_bar, theme_preview,
    toggle_grid, toggle_row,
};
