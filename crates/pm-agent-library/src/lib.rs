//! The instructions, skills, agent definitions, hooks and tools given to agents, by workspace, user and built-in scope.
//!
//! Each product the app knows reads its customizations from folders of its
//! own; this crate knows those folders, reads what they hold for a project,
//! and says how an agent is started so that what the reader turned off is
//! left out, without the files on disk being touched.

mod discover;
mod front_matter;
mod item;
mod launch;
mod place;
mod product;
mod tool;

pub use discover::{discover, skills};
pub use item::{Item, Kind, Scope};
pub use launch::Launch;
pub use place::locations;
pub use product::Product;
pub use tool::{BUILT_IN, Source, ToolEntry, ToolSet, toggled_set, toggled_tool};
