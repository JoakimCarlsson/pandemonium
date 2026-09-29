//! How a server counts along a line, and the editor's count beside it.
//!
//! The editor counts a column in characters; the protocol counts it in code
//! units, of whichever width the server chose when it was started. The two
//! agree on a line that is plain ASCII and on no other, so one dash in one
//! comment is enough to send every question about the rest of that line to
//! the wrong place. Every position that crosses the pipe is translated
//! here: out in the server's units, back in the editor's.

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use lsp_types::PositionEncodingKind;
use ropey::Rope;

use crate::cursor::Position;

/// How a server counts one column of a line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum Encoding {
    /// Bytes of UTF-8.
    Utf8,
    /// Code units of UTF-16, which every server understands.
    #[default]
    Utf16,
    /// Characters, which is what the editor itself counts in.
    Utf32,
}

impl Encoding {
    /// What the server said in its answer to the handshake it counts in.
    ///
    /// A server that says nothing is taken to count in UTF-16, which is what
    /// the protocol falls back to when nothing was agreed.
    pub(super) fn of(stated: Option<&PositionEncodingKind>) -> Self {
        match stated.map(PositionEncodingKind::as_str) {
            Some("utf-8") => Self::Utf8,
            Some("utf-32") => Self::Utf32,
            _ => Self::Utf16,
        }
    }

    /// How much of a column `ch` takes up, counted the server's way.
    fn width(self, ch: char) -> usize {
        match self {
            Self::Utf8 => ch.len_utf8(),
            Self::Utf16 => ch.len_utf16(),
            Self::Utf32 => 1,
        }
    }

    /// How far into `line` its first `column` characters reach.
    pub(super) fn outward(self, line: &str, column: usize) -> usize {
        if self == Self::Utf32 || line.is_ascii() {
            return column;
        }
        line.chars().take(column).map(|ch| self.width(ch)).sum()
    }

    /// How many characters into `line` the server's `column` falls.
    fn inward(self, line: &str, column: usize) -> usize {
        if self == Self::Utf32 || line.is_ascii() {
            return column;
        }
        let mut counted = 0;
        for (characters, ch) in line.chars().enumerate() {
            if counted >= column {
                return characters;
            }
            counted += self.width(ch);
        }
        line.chars().count()
    }
}

/// The text every position being translated is counted against.
///
/// An answer may name a file the editor never sent — the definition of a
/// symbol two crates away — so a file that was not sent is read from disk
/// once and kept for as long as that answer is being read.
pub(super) struct Files {
    /// How the server counts a column.
    encoding: Encoding,
    /// The text of each file, as it was sent or as the disk holds it.
    texts: HashMap<PathBuf, Option<Rope>>,
    /// The last line taken out, which the next position is usually on too.
    last: Option<(PathBuf, usize, String)>,
}

impl Files {
    /// The files `open` were sent as, counted the way `encoding` says.
    pub(super) fn new(encoding: Encoding, open: HashMap<PathBuf, Rope>) -> Self {
        Self {
            encoding,
            texts: open
                .into_iter()
                .map(|(path, text)| (path, Some(text)))
                .collect(),
            last: None,
        }
    }

    /// `at` in the file at `path`, counted the server's way.
    pub(super) fn encode(&mut self, path: &Path, at: Position) -> Position {
        if self.encoding == Encoding::Utf32 {
            return at;
        }
        let encoding = self.encoding;
        let column = self.with_line(path, at.line, |line| encoding.outward(line, at.column));
        Position::new(at.line, column)
    }

    /// `at` in the file at `path`, counted the editor's way.
    pub(super) fn decode(&mut self, path: &Path, at: Position) -> Position {
        if self.encoding == Encoding::Utf32 {
            return at;
        }
        let encoding = self.encoding;
        let column = self.with_line(path, at.line, |line| encoding.inward(line, at.column));
        Position::new(at.line, column)
    }

    /// `span` in the file at `path`, counted the editor's way.
    pub(super) fn decode_span(&mut self, path: &Path, span: Range<Position>) -> Range<Position> {
        self.decode(path, span.start)..self.decode(path, span.end)
    }

    /// Reads the `line`-th line of the file at `path` out to `read`.
    fn with_line<T>(&mut self, path: &Path, line: usize, read: impl FnOnce(&str) -> T) -> T {
        let held = matches!(
            self.last.as_ref(),
            Some((cached, number, _)) if cached == path && *number == line,
        );
        if !held {
            let text = self.text(path).map(|rope| line_text(rope, line));
            self.last = Some((path.to_path_buf(), line, text.unwrap_or_default()));
        }
        read(self.last.as_ref().map_or("", |(_, _, text)| text.as_str()))
    }

    /// The text of the file at `path`, read from disk if it was never sent.
    fn text(&mut self, path: &Path) -> Option<&Rope> {
        self.texts
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                std::fs::read_to_string(path)
                    .ok()
                    .map(|text| Rope::from_str(&text))
            })
            .as_ref()
    }
}

/// The `line`-th line of `text`, or nothing where the file ends short of it.
fn line_text(text: &Rope, line: usize) -> String {
    if line >= text.len_lines() {
        return String::new();
    }
    text.line(line).to_string()
}
