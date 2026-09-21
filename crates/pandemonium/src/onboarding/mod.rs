//! The first screen: pick a theme, a keymap and the agents sessions will run.
//!
//! The screen is a function of [`Setup`] and nothing else. Input comes back as
//! a [`Message`], [`Setup::apply`] folds it in, and the next frame is built
//! from the result; there is no widget state anywhere in between. One
//! component to a file, the way the widgets under it are laid out.

mod agent_card;
mod agent_section;
mod basics;
mod header;
mod keymap_section;
mod mark;
mod page;
mod ready;
mod rule;
mod section;
mod setup;
mod theme_section;

pub use page::page;
pub use setup::{Message, Setup};
