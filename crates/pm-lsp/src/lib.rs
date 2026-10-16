//! The language server client, and the languages a server installs.
//!
//! A language is a folder: a [`Manifest`] in `config.toml` that says how its
//! files are told apart, how they are edited and which language server reads
//! them, the tree-sitter queries beside it, and the grammar the manifest pins.
//! The [`Index`] lists the folders there are, with a checksum for each file;
//! it is read from the repository at the tag this build was made from, or
//! from the folder or address `PANDEMONIUM_LANGUAGES` names. [`install`]
//! fetches a language and its language server into the data folder, every
//! file checked against its checksum.
//!
//! A [`Hub`] runs the language servers of the projects the files open in a
//! window belong to: it keeps each open file's text as the window edits it,
//! starts a server the first time a file of its language is opened in a
//! project, and asks it for completions and hovers. What a server says is
//! wrong with a file comes back through the sink the hub was made with.

mod fetch;
mod hub;
mod index;
mod install;
mod manifest;
mod paths;
mod resolve;
mod rpc;
mod server;
mod snippet;
mod types;
mod uri;

pub use hub::{Hub, Published, Sink, rank, score};
pub use index::{Index, Listing, build_index};
pub use install::{add_extension, install, installed, installed_manifest, remove};
pub use manifest::{Archive, Download, Install, InstallKind, Manifest, Pinned, ServerSpec};
pub use paths::{grammar_file, language_folder, local_root};
pub use resolve::server_state;
pub use types::{Completion, CompletionKind, Diagnostic, Hover, Severity, TextChange};
