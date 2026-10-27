//! The versioned contract plugins are written against.
//!
//! An extension's code is a WebAssembly module. It exports [`EXPORT_MEMORY`],
//! [`EXPORT_ALLOC`] and [`EXPORT_COMMAND`], and imports [`HOST_IMPORT`] from
//! [`HOST_MODULE`]. Everything crossing the boundary is JSON in the guest's
//! memory, named by a pointer and a length packed into one `i64` by
//! [`pack`]: a [`CommandCall`] goes in and a [`CommandReply`] comes back, and
//! the module asks the host for things with a [`HostCall`] and is answered
//! with a [`HostReply`]. A module that carries languages may also export
//! [`EXPORT_LANGUAGE`], which answers a [`LanguageCall`] with a [`LanguageReply`].
//!
//! The contract is append-only, like the protocol: a version adds calls,
//! fields and replies and never removes or repurposes one, so an editor that
//! speaks a newer [`API_VERSION`] loads an extension written for an older one.
//! A new thing the host can do is a new variant of [`HostCall`], not a new
//! import.

mod call;
mod host;
mod language;
mod pack;

pub use call::{CommandCall, CommandReply};
pub use host::{Host, HostCall, HostReply, ProcessOutput};
pub use language::{LanguageCall, LanguageReply, Launch, TaskItem, ToolchainItem};
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

/// The export that answers what the host asks for a language: `(pointer, length) -> packed`, a [`LanguageCall`] in and a [`LanguageReply`] out. A module may leave it out; version 2 added it.
pub const EXPORT_LANGUAGE: &str = "pm_language";

/// The memory the module exports.
pub const EXPORT_MEMORY: &str = "memory";
