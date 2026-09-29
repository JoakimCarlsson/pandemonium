//! What a server is told when a document changes.
//!
//! A server that asked for ranged changes is told the one span that differs
//! between the text it was last told and the text as it now stands, found by
//! walking in from both ends until the two part. That is the edit whatever
//! made it — a keystroke, a paste, a formatter, a reread from disk — so the
//! buffer is never asked to describe its own edits a second time, and the two
//! descriptions can never disagree.

use lsp_types::{Position, Range, TextDocumentContentChangeEvent};
use ropey::Rope;

use crate::lsp::encoding::Encoding;

/// The change that turns `before` into `after`, as one span of `before`
/// and what now stands in its place, counted the way `encoding` says.
pub(super) fn ranged(
    before: &Rope,
    after: &Rope,
    encoding: Encoding,
) -> TextDocumentContentChangeEvent {
    let prefix = common_prefix(before, after);
    let suffix = common_suffix(before, after, prefix);
    let start = char_floor(before, prefix);
    let before_end = char_ceil(before, before.len_bytes() - suffix);
    let after_end = char_ceil(after, after.len_bytes() - suffix);
    let after_start = char_floor(after, prefix);
    TextDocumentContentChangeEvent {
        range: Some(Range {
            start: position(before, start, encoding),
            end: position(before, before_end, encoding),
        }),
        range_length: None,
        text: after
            .slice(after_start..after_end.max(after_start))
            .to_string(),
    }
}

/// The whole of `text`, which replaces whatever the server held.
pub(super) fn whole(text: &Rope) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text: text.to_string(),
    }
}

/// How many bytes `before` and `after` begin with in common.
fn common_prefix(before: &Rope, after: &Rope) -> usize {
    let mut counted = 0;
    let mut theirs = after.chunks();
    let mut them: &[u8] = &[];
    for ours in before.chunks() {
        let mut ours = ours.as_bytes();
        while !ours.is_empty() {
            if them.is_empty() {
                match theirs.next() {
                    Some(chunk) => them = chunk.as_bytes(),
                    None => return counted,
                }
            }
            let length = ours.len().min(them.len());
            let same = ours[..length]
                .iter()
                .zip(&them[..length])
                .take_while(|(a, b)| a == b)
                .count();
            counted += same;
            if same < length {
                return counted;
            }
            ours = &ours[length..];
            them = &them[length..];
        }
    }
    counted
}

/// How many bytes `before` and `after` end with in common, without reaching
/// back into the `prefix` bytes they begin with in common.
fn common_suffix(before: &Rope, after: &Rope, prefix: usize) -> usize {
    let most = before.len_bytes().min(after.len_bytes()) - prefix;
    let mut ours = before.bytes_at(before.len_bytes());
    let mut theirs = after.bytes_at(after.len_bytes());
    let mut counted = 0;
    while counted < most {
        match (ours.prev(), theirs.prev()) {
            (Some(a), Some(b)) if a == b => counted += 1,
            _ => break,
        }
    }
    counted
}

/// The character the byte `at` of `text` falls in.
fn char_floor(text: &Rope, at: usize) -> usize {
    text.byte_to_char(at)
}

/// The first character that begins at or after the byte `at` of `text`.
fn char_ceil(text: &Rope, at: usize) -> usize {
    let char = text.byte_to_char(at);
    match text.char_to_byte(char) == at {
        true => char,
        false => char + 1,
    }
}

/// Where the character `at` of `text` begins, counted the way `encoding` says.
fn position(text: &Rope, at: usize, encoding: Encoding) -> Position {
    let line = text.char_to_line(at);
    let column = at - text.line_to_char(line);
    let written = text.line(line).to_string();
    Position::new(line as u32, encoding.outward(&written, column) as u32)
}
