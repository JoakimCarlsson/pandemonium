//! Parsing markdown into blocks of styled text, for rendering agent output.
//!
//! [`parse`] turns a markdown source into [`Block`]s: paragraphs, headings,
//! lists, code, quotes, rules and tables, whose text is a run of [`Span`]s
//! each set one [`Inline`] way. Laying them out is the caller's; [`plain`]
//! writes a source out as the text it shows, without its markup.

mod block;
mod parse;
mod plain;

pub use block::{Block, Inline, Span};
pub use parse::{bare_address, links, parse};
pub use plain::plain;
