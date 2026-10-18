//! The editor: a file's text as it is written in, with its syntax tree, what its language server knows of it and says about it, and the rules editing it follows.
//!
//! A [`Document`] is the text of one open file. Every change goes through
//! [`Document::edit`], which keeps the syntax tree and the version its
//! language server was told of in step and says what to tell the server.
//! The rest of the editing rules are plain operations on a document that
//! return the edits they call for: the indentation of a new line, a closing
//! bracket going back a level, pairs of brackets and quotes, more than one caret, whole lines
//! commented, moved, duplicated and deleted, the selection grown along the
//! syntax tree, text found and replaced, the completion picked, the edits that format the file. Nothing here sends anything or draws anything; the window does.

mod complete;
mod cursors;
mod document;
mod edit;
mod find;
mod fold;
mod format;
mod gutter;
mod indent;
mod jumps;
mod keymap;
mod lines;
mod link;
mod marks;
mod pairs;
mod settings;
mod sticky;
mod wrap;

pub use document::{Completions, Document, HoverCard};
pub use edit::Edited;
pub use find::Query;
pub use fold::Fold;
pub use gutter::LineChange;
pub use jumps::{Jumps, Spot};
pub use keymap::{Action, Chord, Keymap, Preset, Pressed, shown};
pub use settings::{CursorShape, DiagnosticsShown, EditorSettings, SoftWrap};
pub use wrap::segments;
