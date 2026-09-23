//! The first screen: pick a theme and a keymap, and set the editor's defaults.
//!
//! Onboarding is the first run and nothing more — it writes a handful of
//! preferences down and steps out of the way. Editing them afterwards belongs
//! to the settings pane, which edits the same [`Preferences`] rather than
//! keeping a second copy of the answers.
//!
//! The screen is a function of the preferences and nothing else. Input comes
//! back as a [`Message`], [`Preferences::apply`] folds it in, and the next
//! frame is built from the result; there is no widget state anywhere in
//! between.
//!
//! [`Preferences`]: crate::config::Preferences
//! [`Preferences::apply`]: crate::config::Preferences::apply
//! [`Message`]: crate::message::Message

mod basics;
mod page;

pub use page::page;
