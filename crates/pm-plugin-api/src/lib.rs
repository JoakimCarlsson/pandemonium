//! The versioned contract plugins are written against.
//!
//! An extension's code is a WebAssembly module. It exports [`EXPORT_MEMORY`],
//! [`EXPORT_ALLOC`] and [`EXPORT_COMMAND`], and imports [`HOST_IMPORT`] from
//! [`HOST_MODULE`]. Everything crossing the boundary is JSON in the guest's
//! memory, named by a pointer and a length packed into one `i64` by
//! [`pack`]: a [`CommandCall`] goes in and a [`CommandReply`] comes back, and
//! the module asks the host for things with a [`HostCall`] and is answered
//! with a [`HostReply`].
//!
//! The contract is append-only, like the protocol: a version adds calls,
//! fields and replies and never removes or repurposes one, so an editor that
//! speaks a newer [`API_VERSION`] loads an extension written for an older one.
//! A new thing the host can do is a new variant of [`HostCall`], not a new
//! import. Version 2 added the [`HookCall`]s the host asks a module while it
//! runs a language server or a debug adapter, answered by the optional
//! [`EXPORT_HOOK`], and the host calls that let a module find programs, fetch
//! a release into its own storage and tell the reader what it is doing.

mod call;
mod hook;
mod host;
mod pack;

pub use call::{CommandCall, CommandReply};
pub use hook::{Command, CompletionItem, HookCall, HookReply, SymbolItem};
pub use host::{Host, HostCall, HostReply, ProcessOutput, Release, ReleaseAsset, StorageEntry};
pub use pack::{pack, unpack};

/// The version of this contract that this build speaks.
pub const API_VERSION: u32 = 2;

/// The module the host's import lives in.
pub const HOST_MODULE: &str = "pandemonium";

/// The import a module calls to ask the host for something: `(pointer, length) -> packed`.
pub const HOST_IMPORT: &str = "host";

/// The export that hands the host room in the module's memory: `(length) -> pointer`.
pub const EXPORT_ALLOC: &str = "pm_alloc";

/// The export that runs a command: `(pointer, length) -> packed`.
pub const EXPORT_COMMAND: &str = "pm_command";

/// The memory the module exports.
pub const EXPORT_MEMORY: &str = "memory";

/// The export, added by version 2, that answers a [`HookCall`]: `(pointer, length) -> packed`. A module without it answers no hook.
pub const EXPORT_HOOK: &str = "pm_hook";
