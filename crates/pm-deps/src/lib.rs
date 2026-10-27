//! Dependency awareness: which dependencies a manifest names and which of them have a newer version in their registry.
//!
//! An [`Ecosystem`] reads one kind of manifest, `package.json`, `Cargo.toml` or
//! `go.mod`, and says where each dependency's version stands in the text, and
//! how to ask the ecosystem's registry for the latest one. The [`Registry`]
//! asks, keeping what it learns for a while, spacing its requests out and
//! giving up on a lookup that fails without telling the caller why, so that
//! editing is never held up by it. [`Registry::outdated`] puts the two together:
//! the dependencies of a manifest whose version is behind, each with the text
//! that would bring it up to date.

mod cargo;
mod dependency;
mod ecosystem;
mod go;
mod npm;
mod registry;
mod version;

pub use dependency::{Dependency, Outdated};
pub use ecosystem::{Ecosystem, ecosystem_for};
pub use registry::Registry;
pub use version::Version;
