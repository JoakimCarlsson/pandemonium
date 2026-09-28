//! Widgets extracted from real screens, and only once they repeated there.
//!
//! One component to a file: a widget that paints itself is an [`Element`],
//! a widget that arranges other widgets is a function returning a [`Div`].
//!
//! [`Element`]: crate::element::Element
//! [`Div`]: crate::div::Div

mod button;
mod checkbox;
mod field;
mod icon_button;
mod kbd;
mod menu;
mod rule;
mod scrollbar;
mod section;
mod switch;
mod switch_field;
mod tabs;
mod theme_gallery;
mod theme_preview;
mod toggle_grid;
mod toggle_option;
mod toggle_row;
mod view_tab;

pub use button::{Button, ButtonVariant, button};
pub use checkbox::{ToggleState, checkbox};
pub use field::{Field, field};
pub use icon_button::{icon_button, tinted_icon_button, turning_icon_button};
pub use kbd::kbd;
pub use menu::{MenuItem, menu, menu_entry, menu_separator, menu_submenu};
pub use rule::rule;
pub use scrollbar::{SCROLLBAR_GUTTER, Scrollbar, scrollbar};
pub use section::section;
pub use switch::{Switch, switch};
pub use switch_field::switch_field;
pub use tabs::{Tab, tab, tab_bar};
pub use theme_gallery::theme_gallery;
pub use theme_preview::{ThemePreview, theme_preview};
pub use toggle_grid::toggle_grid;
pub use toggle_row::toggle_row;
pub use view_tab::view_tab;
