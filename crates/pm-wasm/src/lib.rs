//! The sandboxed runtime that loads plugins as WebAssembly components.
//!
//! A [`Sandbox`] holds a compiled module and runs one call of it at a time in
//! a fresh instance: the module gets no clock, no files and no network of its
//! own, only the single import the [`pm_plugin_api`] contract names, which the
//! [`Host`](pm_plugin_api::Host) it is run with answers. Each call is given a
//! budget of work and a ceiling of memory, so a module that never returns or
//! grows without end is stopped instead of stopping the server.

mod sandbox;

pub use sandbox::{Limits, Sandbox};
