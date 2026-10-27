//! Dependency awareness: which of the dependencies a manifest names have a newer version in their registry.
//!
//! Nothing here knows a kind of manifest or a registry. A language folder
//! declares both: where a manifest names its dependencies, and a [`Provider`],
//! the address that answers with a dependency's latest version, where in the
//! answer it stands, and the settings of the machine that make a dependency
//! private. The [`Registry`] asks, keeping what it learns for a while, spacing
//! its requests out and giving up on a lookup that fails without telling the
//! caller why, so that editing is never held up by it. [`Registry::outdated`]
//! returns the dependencies whose version is behind, each with the text that
//! would bring it up to date.

mod dependency;
mod provider;
mod registry;
mod version;

pub use dependency::{Dependency, Outdated};
pub use provider::{Private, Provider};
pub use registry::Registry;
pub use version::Version;
