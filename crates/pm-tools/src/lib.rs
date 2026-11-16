//! Finding the programs a machine has installed, for a process that was not started from a shell, and confining the processes started to memory-capped scopes.

mod confine;
mod find;
mod shield;

pub use confine::{Confinement, confined, helper_limit};
pub use find::{installed, path_beside};
pub use shield::{OWN_SCOPE, shield_from_children};
