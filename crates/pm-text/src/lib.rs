//! Text buffers: rope storage, syntax trees and the language-server client.
//!
//! This layer is text as data. A [`Buffer`] holds one file — the rope it is
//! stored in, the syntax tree that follows the rope, where it is being
//! edited and what a server has said about it — and answers questions in
//! lines and columns. It neither lays out nor draws: how wide a character
//! comes out and which colour a [`Highlight`] is drawn in are decided a
//! layer up, against a font and a theme this crate never sees.

mod buffer;
mod cursor;
mod diagnostic;
pub mod frame;
mod hint;
mod history;
mod indent;
mod language;
mod lsp;
pub mod program;
mod syntax;

pub use buffer::Buffer;
pub use cursor::{Motion, Position, Selection};
pub use diagnostic::{Diagnostic, Severity};
pub use hint::Hint;
pub use indent::Indent;
pub use language::{Language, NO_OPTIONS, Server};
pub use lsp::{
    Answer, Asked, Calls, Client, CodeAction, Completion, FileEdit, Handle, Lens, Location,
    NamedLocation, Request, Servers, Symbol, Watched,
};
pub use syntax::{Highlight, Highlights, SyntaxNode, highlight};
