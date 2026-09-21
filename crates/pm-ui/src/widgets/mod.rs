//! Widgets extracted from real screens, and only once they repeated there.
//!
//! One component to a file: a widget that paints itself is an [`Element`],
//! a widget that arranges other widgets is a function returning a [`Div`].
//!
//! [`Element`]: crate::element::Element
//! [`Div`]: crate::div::Div

mod button;
mod switch;
mod switch_field;
mod theme_preview;
mod toggle_grid;
mod toggle_option;
mod toggle_row;

pub use button::{Button, ButtonVariant, button};
pub use switch::{Switch, switch};
pub use switch_field::switch_field;
pub use theme_preview::{ThemePreview, theme_preview};
pub use toggle_grid::toggle_grid;
pub use toggle_row::toggle_row;
