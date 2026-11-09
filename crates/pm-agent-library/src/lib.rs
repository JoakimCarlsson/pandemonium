//! The instructions, skills, agent definitions and tools given to agents, by workspace, user and built-in scope.
//!
//! The harness, crab, is the one that reads what an agent is given from disk;
//! this crate writes what crab finds as the items the settings list, and says
//! how an agent is started so that what the reader turned off is left out,
//! without the files on disk being touched.

mod create;
mod found;
mod item;
mod product;
mod tool;

pub use create::{Draft, PromptFile, create, create_prompt_file, valid_name};
pub use found::{APPENDS_PROMPT, REPLACES_PROMPT, discover, discover_own, locations};
pub use item::{Item, Kind, Scope};
pub use product::Product;
pub use tool::{BUILT_IN, Source, ToolEntry, ToolSet, toggled_set, toggled_tool};
