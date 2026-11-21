//! How much a press takes at a time.

/// How much of a text a press takes: one click places a point, two take a
/// word and three or more take a line, or the paragraph where text is laid
/// out in blocks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Reach {
    /// A point between two characters.
    #[default]
    Point,
    /// A word, or a run of space or of other characters.
    Word,
    /// A line, or the rows that run on without a blank line between them.
    Line,
}

impl Reach {
    /// What the `clicks`th press in a row takes.
    pub fn of_clicks(clicks: u8) -> Self {
        match clicks {
            0 | 1 => Self::Point,
            2 => Self::Word,
            _ => Self::Line,
        }
    }
}
