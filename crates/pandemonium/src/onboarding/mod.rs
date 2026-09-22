//! The first screen: pick a theme and a keymap, and set the editor's defaults.
//!
//! Onboarding is the first run and nothing more — it writes a handful of
//! preferences down and steps out of the way. Editing them afterwards belongs
//! to a settings surface, which reads and writes the same [`crate::config`]
//! file rather than keeping a second copy of the answers.
//!
//! The screen is a function of [`Setup`] and nothing else. Input comes back as
//! a [`Message`], [`Setup::apply`] folds it in, and the next frame is built
//! from the result; there is no widget state anywhere in between.

mod basics;
mod page;
mod setup;

pub use page::page;
pub use setup::{Setup, ThemeMode};
