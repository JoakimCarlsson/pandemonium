//! The blocks a markdown source is made of and the styled text inside them.

/// How a run of text is set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Inline {
    /// Set heavier, for `**strong**` text.
    pub strong: bool,
    /// Set leaning, for `*emphasised*` text.
    pub emphasis: bool,
    /// Set in the code font, for `` `code` ``.
    pub code: bool,
    /// Struck through, for `~~deleted~~` text.
    pub strike: bool,
    /// The text of a link.
    pub link: bool,
}

/// A run of text set one way. A hard line break is a span of `"\n"`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Span {
    /// What it says.
    pub text: String,
    /// How it is set.
    pub inline: Inline,
}

/// One block of a markdown source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Block {
    /// Running text.
    Paragraph(Vec<Span>),
    /// A heading of `level` 1 to 6, 1 the largest.
    Heading {
        /// How deep the heading is, from 1.
        level: u8,
        /// What it says.
        spans: Vec<Span>,
    },
    /// A list, numbered from `start` when it is ordered.
    List {
        /// The number of the first item of an ordered list.
        start: Option<u64>,
        /// The items, each its own blocks.
        items: Vec<Vec<Block>>,
    },
    /// A block of code, kept as written.
    Code {
        /// The language the fence named, empty when it named none.
        language: String,
        /// The code, without its last line break.
        text: String,
    },
    /// Quoted blocks.
    Quote(Vec<Block>),
    /// An HTML `<details>`: a summary that folds the blocks under it away.
    Details {
        /// The text of its `<summary>`, empty when it had none.
        summary: Vec<Span>,
        /// The blocks it folds.
        blocks: Vec<Block>,
    },
    /// A line across.
    Rule,
    /// A table: its heading cells and its rows of cells.
    Table {
        /// The cells of the heading row.
        head: Vec<Vec<Span>>,
        /// The rows below it, each a row of cells.
        rows: Vec<Vec<Vec<Span>>>,
    },
}
