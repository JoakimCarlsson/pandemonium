//! What an editor asks of the language server reading its file: told of each
//! change, asked for completions, a hover card, the signature of a call, the
//! places a name is used, where it is defined, a new name, the changes offered
//! at a place, and to press a lens or take a change. The window sends these on;
//! nothing here sends anything.

use pm_buffer::{Position, Span};
use pm_lsp::{Seek, TextChange};

/// One thing an editor asks of the language server reading its file.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerAsk {
    /// The text reached this version by this change.
    Changed(i32, TextChange),
    /// The completions at the place, at the version, asked by the character typed when one did.
    Complete(i32, Position, Option<String>),
    /// The rest of the completion the server wrote this way.
    Resolve(String),
    /// What is said about the place at the version.
    Hover(i32, Position),
    /// What the call the place is in looks like, at the version.
    Signature(i32, Position),
    /// The places the name at the place is used, at the version.
    Highlights(i32, Position),
    /// Where the name at the place leads, as asked by the seek.
    Seek(Position, Seek),
    /// What can be renamed at the place.
    PrepareRename(Position),
    /// Renames the name at the place to this name.
    Rename(Position, String),
    /// The changes offered in the stretch, of the kinds named when any are.
    CodeActions(Span, Option<Vec<String>>),
    /// Takes the change at this place of the list offered last.
    TakeAction(usize),
    /// Presses the lens at this place of the list the server gave.
    PressLens(usize),
    /// Writes the colour as the language spells it over the stretch.
    Colour(Span, [u8; 4]),
    /// Formats the file, or the stretch of it when one is given.
    Format(Option<Span>),
    /// Formats what was just typed, at this version of the text, with the
    /// caret at this place after the character typed.
    FormatOnType(i32, Position, String),
}

/// The kind of change that sorts the imports of a file.
pub const ORGANIZE_IMPORTS: &str = "source.organizeImports";

/// The kind of change that fixes every problem of a file that can be fixed at once.
pub const FIX_ALL: &str = "source.fixAll";
