//! A markdown source written out as the text it shows, without its markup.

use crate::block::{Block, Span};
use crate::parse::parse;

/// The text `source` shows when it is laid out, without its markup: blocks
/// apart by a blank line, list items marked with a dash or their number and
/// set in under it, and table cells apart by tabs.
pub fn plain(source: &str) -> String {
    blocks(&parse(source))
}

/// `blocks` written out one after another, a blank line between each.
fn blocks(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(block)
        .filter(|written| !written.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// One block written out.
fn block(block: &Block) -> String {
    match block {
        Block::Paragraph(spans) | Block::Heading { spans, .. } => text(spans),
        Block::List { start, items } => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let marker = match start {
                    Some(first) => format!("{}. ", first + index as u64),
                    None => "- ".to_owned(),
                };
                indented(&marker, &blocks(item).replace("\n\n", "\n"))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Block::Code { text, .. } => text.clone(),
        Block::Quote(quoted) => blocks(quoted),
        Block::Details {
            summary,
            blocks: folded,
        } => [text(summary), blocks(folded)]
            .into_iter()
            .filter(|written| !written.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        Block::Rule => String::new(),
        Block::Math(text) => text.clone(),
        Block::Images { images, .. } => images
            .iter()
            .map(|image| image.alt.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        Block::Table { head, rows } => std::iter::once(head)
            .chain(rows)
            .map(|row| {
                row.iter()
                    .map(|cell| text(cell))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// `written` behind `marker` on its first line, its other lines set in as far.
fn indented(marker: &str, written: &str) -> String {
    let indent = " ".repeat(marker.len());
    written
        .lines()
        .enumerate()
        .map(|(at, line)| match at {
            0 => format!("{marker}{line}"),
            _ if line.is_empty() => String::new(),
            _ => format!("{indent}{line}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What `spans` say, end to end.
fn text(spans: &[Span]) -> String {
    spans.iter().map(|span| span.text.as_str()).collect()
}
