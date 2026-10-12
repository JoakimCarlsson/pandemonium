//! The components the app is built from.
//!
//! A component is a function that returns a [`pm_ui::Node`] styled with
//! utilities, so the caller changes how it looks with `.styles([...])` and
//! never reaches into it. Each component lives in a module named for what it
//! is and is re-exported here.

mod activity;
mod card;
mod controls;
mod field;
mod progress;
mod showcase;
mod swatches;

pub use activity::{bouncing_dots, ping_dot, pulse_dot, spinner};
pub use card::card;
pub use controls::{avatar_stack, badge, button, switch};
pub use field::text_input;
pub use progress::progress_bar;
pub use showcase::{INCREMENT, NAME, NOTES, RESET, ShowcaseState, TOGGLE, showcase};
pub use swatches::{opacity_steps, palette, radius_steps, shadow_steps, transforms};
