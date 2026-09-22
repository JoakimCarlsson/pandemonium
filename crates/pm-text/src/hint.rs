//! What a language server writes into a line that is not in the file.
//!
//! A hint is drawn between two characters and is not one: the file does not
//! hold it, the cursor does not stand in it, and saving does not write it
//! out. What it does do is take room on the line, which is why the buffer
//! counts it when it works out which column a character is drawn at.

use crate::cursor::Position;

/// One thing a server has written into a line that the file does not hold.
#[derive(Clone, Debug)]
pub struct Hint {
    /// The character it is drawn in front of.
    pub position: Position,
    /// What it says.
    pub text: String,
}

impl Hint {
    /// How many columns the hint takes on the line it is drawn in.
    pub fn width(&self) -> usize {
        self.text.chars().count()
    }
}
