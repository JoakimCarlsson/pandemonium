//! Plugin manifests, the registry, lifecycle and declared capabilities.
//!
//! An extension is a folder with an `extension.toml`: a [`Manifest`] naming
//! what the folder carries and the [capabilities](pm_policy::Capability) it
//! asks for. It carries any of languages (a language folder each, which
//! [`install`] hands to the language index like any other, so a language an
//! extension ships and a built-in one go through one code path), themes, icon
//! themes, debug adapters, tool servers and commands, and may carry a
//! WebAssembly module that runs the commands, in [`pm_wasm`] against the
//! contract of [`pm_plugin_api`].
//!
//! Extensions are kept in the data folder of the machine they were installed
//! on, so what one adds applies to the projects of that machine's server.
//! What an extension is allowed to do is [`pm_policy`]'s: nothing, until the
//! reader grants it, checked where the extension uses it.

mod command;
mod gate;
mod hook;
mod install;
mod manifest;
mod package;
mod style;

pub use command::run_command;
pub use gate::Status;
pub use hook::{answers_hooks, run_hook};
pub use install::{Installed, inspect, install, remove};
pub use manifest::{
    AdapterEntry, CommandEntry, FileEntry, IconEntry, LanguageEntry, Manifest, ServerEntry,
};
pub use package::{Package, installed, language_id, named};
pub use style::{IconTheme, Icons, Theme};

/// The file that makes a folder an extension.
pub const MANIFEST: &str = "extension.toml";
