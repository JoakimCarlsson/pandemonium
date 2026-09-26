//! Conflict blocks read from a merged file and the choices that replace them.

use std::ops::Range;

/// One complete set of Git conflict markers and the text between them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conflict {
    /// The byte range occupied by the markers and both sides.
    pub range: Range<usize>,
    /// The zero-based line of the current side's marker.
    pub start_line: usize,
    /// The zero-based line separating the current and incoming sides.
    pub divider_line: usize,
    /// The zero-based line of the incoming side's closing marker.
    pub end_line: usize,
    /// The zero-based line of the common ancestor marker, when present.
    pub base_line: Option<usize>,
    /// The current branch's text.
    pub current: String,
    /// The incoming branch's text.
    pub incoming: String,
}

/// The side or sides to keep when resolving a conflict block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    /// Keep the current branch's text.
    Current,
    /// Keep the incoming branch's text.
    Incoming,
    /// Keep both sides in their displayed order.
    Both,
}

/// An inline action offered above a conflict in the file editor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Replace the conflict with the chosen content.
    Accept(Choice),
    /// Compare the two versions in a separate pane.
    Compare,
}

impl Conflict {
    /// Replaces this block in `source` with the selected text.
    pub fn resolve(&self, source: &str, choice: Choice) -> String {
        let replacement = match choice {
            Choice::Current => self.current.clone(),
            Choice::Incoming => self.incoming.clone(),
            Choice::Both => format!("{}{}", self.current, self.incoming),
        };
        let mut resolved = source.to_owned();
        resolved.replace_range(self.range.clone(), &replacement);
        resolved
    }
}

/// Reads complete two-sided or diff3 conflict blocks from `source`.
pub fn conflicts(source: &str) -> Vec<Conflict> {
    let mut found = Vec::new();
    let mut start = None;
    let mut middle = None;
    let mut base = None;
    let mut at = 0;

    for (line_number, line) in source.split_inclusive('\n').enumerate() {
        if line.starts_with("<<<<<<< ") {
            start = Some((at, at + line.len(), line_number));
            middle = None;
            base = None;
        } else if start.is_some() && line.starts_with("||||||| ") {
            base = Some((at, line_number));
        } else if start.is_some() && line.trim_end_matches(['\r', '\n']) == "=======" {
            middle = Some((at, at + line.len(), line_number));
        } else if line.starts_with(">>>>>>> ") {
            if let (
                Some((begin, current_start, start_line)),
                Some((current_end, incoming_start, divider_line)),
            ) = (start, middle)
            {
                found.push(Conflict {
                    range: begin..at + line.len(),
                    start_line,
                    divider_line,
                    end_line: line_number,
                    base_line: base.map(|(_, line)| line),
                    current: source[current_start..base.map_or(current_end, |(at, _)| at)]
                        .to_owned(),
                    incoming: source[incoming_start..at].to_owned(),
                });
            }
            start = None;
            middle = None;
            base = None;
        }
        at += line.len();
    }
    found
}
