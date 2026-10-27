//! Syntax trees and highlighting.
//!
//! A [`Grammar`] is a language's parser, compiled to WebAssembly and loaded at
//! run time, together with the queries that say what its trees mean: which
//! nodes are keywords or strings, where a block opens a level of indentation,
//! which nodes are the items an outline lists, which stretches hold another
//! language and which tokens are brackets that pair up. A [`Syntax`] is one
//! text parsed with a grammar, kept up to date edit by edit; it answers what
//! colour each stretch of a window of the text is, how deep a new line is
//! indented, which bracket pairs with the one at the caret, which items
//! enclose it, which places can be run and which carry a lens. The [`registry`] holds the grammars loaded so far by name, so
//! that a stretch written in another language can be highlighted in it.

mod brackets;
mod dependencies;
mod engine;
mod grammar;
mod highlight;
mod indent;
mod kind;
mod lenses;
mod outline;
pub mod registry;
mod runnables;
mod scope;
mod select;
mod syntax;
mod tags;

pub use dependencies::DependencyPlace;
pub use grammar::{Editing, Grammar, Queries, Scope, Tags, default_pairs};
pub use highlight::Highlight;
pub use indent::Indent;
pub use kind::Kind;
pub use lenses::LensPlace;
pub use outline::Item;
pub use runnables::Runnable;
pub use syntax::Syntax;
