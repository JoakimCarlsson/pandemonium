//! The editor: a file's text as it is written in, with its syntax tree, what its language server knows of it and says about it, and the rules editing it follows.
//!
//! A [`Document`] is the text of one open file. Every change goes through
//! [`Document::edit`], which keeps the syntax tree and the version its
//! language server was told of in step and says what to tell the server.
//! The rest of the editing rules are plain operations on a document that
//! return the edits they call for: the indentation of a new line, a closing
//! bracket going back a level, the completion picked, the edits that format
//! the file. Nothing here sends anything or draws anything; the window does.

mod complete;
mod document;
mod edit;
mod format;
mod indent;
mod jumps;

pub use document::{Completions, Document, HoverCard};
pub use edit::Edited;
pub use jumps::{Jumps, Spot};
