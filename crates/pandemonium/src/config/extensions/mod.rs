//! User language packages, their maintained catalogue and validated activation.

mod catalogue;
mod loader;

pub(crate) use catalogue::recover;
pub use catalogue::{Entry, catalogue, import, install, remove};
pub use loader::installed;
pub(crate) use loader::{keymaps, reload, take_errors, themes};
