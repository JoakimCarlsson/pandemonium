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

mod candidates;
mod choice;
mod editing;
mod fetch;
mod hub;
mod index;
mod inherit;
mod install;
mod legacy;
mod lens;
mod manifest;
mod paths;
mod preferences;
mod provider;
mod resolve;
mod rpc;
mod semantic;
mod server;
mod snippet;
mod types;
mod uri;

pub use candidates::WILDCARD;
pub use choice::ServerChoice;
pub use editing::{editing_of, scopes_of, tags_of};
pub use hub::{Environment, Hub, Published, Sink, rank, score};
pub use index::{Index, Listing, build_index};
pub use inherit::language_file;
pub use install::{
    add_extension, add_folder, install, installed, installed_manifest, language_of,
    language_of_text, remove, remove_folder,
};
pub use lens::{RunLens, run_lenses_of};
pub use manifest::{
    AdapterSpec, Archive, BracketSpec, Companion, DecreaseRule, Detect, Download, Install,
    InstallKind, Manifest, Pinned, ScopeSpec, ServerSpec, TagSpec, glob,
};
pub use paths::{grammar_file, language_folder, local_root, platform};
pub use preferences::ServerOverride;
pub use provider::{Provided, ProvidedCommand, Provider, SymbolLabel};
pub use resolve::{server_ready, server_state};
pub use semantic::{TokenRule, kind_by_rules, parse_rules, rules_for};
pub use types::{
    CodeAction, CodeLens, Completion, CompletionKind, Diagnostic, FileEdit, FoldRange, HintKind,
    Hover, InlayHint, InlineValue, InlineValueKind, Location, Problem, Seek, SemanticToken,
    Severity, Signature, TextChange, TokenKind, Triggers,
};
