//! Motions: where a key sends the cursor, and how much of the text an
//! operator given that motion acts on.
//!
//! A motion resolves to a place and a kind. The kind is what turns the span
//! between the cursor and that place into a span of text: an exclusive motion
//! stops short of the place, an inclusive one takes the character there, and
//! a linewise one takes every line it touches whole.

use std::collections::HashMap;

use pm_text::{Buffer, Position};

use crate::search::{LastSearch, Pattern};
use crate::text::{self, first_non_blank, is_empty_line, last_column};

/// How much of the text between the cursor and a motion's place it covers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    /// Up to the place, not including the character there.
    Exclusive,
    /// Up to and including the character at the place.
    Inclusive,
    /// Every line from the cursor's to the place's, whole.
    Linewise,
}

/// A search for a character on the cursor's line: `f`, `F`, `t` or `T`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Find {
    /// The character looked for.
    pub ch: char,
    /// Whether it looks to the right.
    pub forward: bool,
    /// Whether it stops one short of the character.
    pub till: bool,
}

/// Where a key sends the cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Motion {
    /// `h`: one character left, on the same line.
    Left,
    /// `l`: one character right, on the same line.
    Right,
    /// Backspace: one character left, onto the line above at the start.
    WrappingLeft,
    /// Space: one character right, onto the line below at the end.
    WrappingRight,
    /// `k`: one line up.
    Up,
    /// `j`: one line down.
    Down,
    /// `w` or `W`: to the start of the next word.
    NextWordStart { big: bool },
    /// `e` or `E`: to the end of the next word.
    NextWordEnd { big: bool },
    /// `b` or `B`: to the start of the previous word.
    PreviousWordStart { big: bool },
    /// `ge` or `gE`: to the end of the previous word.
    PreviousWordEnd { big: bool },
    /// `0`: to the first column.
    LineStart,
    /// `^`: to the first character that is not blank.
    FirstNonBlank,
    /// `$`: to the last character of the line.
    LineEnd,
    /// `g_`: to the last character of the line that is not blank.
    LastNonBlank,
    /// `+` or Enter: to the first non-blank of the next line.
    NextLineStart,
    /// `-`: to the first non-blank of the previous line.
    PreviousLineStart,
    /// `_`: to the first non-blank of this line, or of a later one.
    CurrentLineStart,
    /// `|`: to a column of this line.
    Column,
    /// `gg`: to the first line, or the line counted.
    FirstLine,
    /// `G`: to the last line, or the line counted.
    LastLine,
    /// `f`, `F`, `t` or `T`: to a character on this line.
    Find(Find),
    /// `;` or `,`: the last character search again, reversed for `,`.
    RepeatFind { reverse: bool },
    /// `%`: to the bracket matching the next one on the line.
    Matching,
    /// `}`: to the next empty line.
    ParagraphForward,
    /// `{`: to the previous empty line.
    ParagraphBackward,
    /// `H`: to the top line of the view.
    ViewTop,
    /// `M`: to the middle line of the view.
    ViewMiddle,
    /// `L`: to the bottom line of the view.
    ViewBottom,
    /// Ctrl-D: half a view down.
    HalfPageDown,
    /// Ctrl-U: half a view up.
    HalfPageUp,
    /// Page Down: a view down.
    PageDown,
    /// Page Up: a view up.
    PageUp,
    /// `/` or `?`: to the next match of typed text.
    Search { text: String, forward: bool },
    /// `n` or `N`: the last search again, reversed for `N`.
    SearchNext { reverse: bool },
    /// `*` or `#`: to the next match of the word under the cursor.
    SearchWord { forward: bool },
    /// `'` or `` ` ``: to a mark, its line for `'`.
    Mark { name: char, line: bool },
}

/// The part of the buffer the pane is showing, for the motions that go by it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct View {
    /// The first line the pane shows.
    pub top: usize,
    /// How many lines the pane has room for.
    pub rows: usize,
}

/// What a motion needs to know besides the buffer and where it starts.
pub(crate) struct Context<'a> {
    /// How many times to make the motion, and whether a count was typed.
    pub count: Option<usize>,
    /// The column vertical motions aim at, `usize::MAX` for line ends.
    pub goal: Option<usize>,
    /// The part of the buffer the pane is showing.
    pub view: View,
    /// The last character search, for `;` and `,`.
    pub last_find: &'a mut Option<Find>,
    /// The last search, for `n` and `N`.
    pub last_search: &'a mut Option<LastSearch>,
    /// The marks set in the buffer.
    pub marks: &'a HashMap<char, Position>,
}

/// Where a motion took the cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Moved {
    /// The place it arrived at.
    pub to: Position,
    /// How much of the text an operator given it covers.
    pub kind: Kind,
    /// The column vertical motions aim at from here.
    pub goal: Option<usize>,
}

impl Motion {
    /// Where the motion takes the cursor from `from`, if it goes anywhere.
    pub(crate) fn resolve(
        &self,
        buffer: &Buffer,
        from: Position,
        cx: &mut Context,
    ) -> Option<Moved> {
        let times = cx.count.unwrap_or(1).max(1);
        let last = buffer.line_count().saturating_sub(1);
        let at = |position: Position| buffer.char_of(position);
        let to_position = |offset: usize| buffer.position_of(offset.min(buffer.len_chars()));
        let line_start = |line: usize| Position::new(line, first_non_blank(buffer, line));
        let half = (cx.view.rows / 2).max(1);

        let (to, kind, goal) = match self {
            Self::Left => (
                Position::new(from.line, from.column.saturating_sub(times)),
                Kind::Exclusive,
                None,
            ),
            Self::Right => (
                Position::new(
                    from.line,
                    (from.column + times).min(buffer.line_len(from.line)),
                ),
                Kind::Exclusive,
                None,
            ),
            Self::WrappingLeft => (
                to_position(at(from).saturating_sub(times)),
                Kind::Exclusive,
                None,
            ),
            Self::WrappingRight => (to_position(at(from) + times), Kind::Exclusive, None),
            Self::Up => return vertical(buffer, from, -(times as isize), cx.goal),
            Self::Down => return vertical(buffer, from, times as isize, cx.goal),
            Self::HalfPageDown => return vertical(buffer, from, half as isize, cx.goal),
            Self::HalfPageUp => return vertical(buffer, from, -(half as isize), cx.goal),
            Self::PageDown => return vertical(buffer, from, cx.view.rows.max(1) as isize, cx.goal),
            Self::PageUp => {
                return vertical(buffer, from, -(cx.view.rows.max(1) as isize), cx.goal);
            }
            Self::NextWordStart { big } => {
                let offset = (0..times).fold(at(from), |offset, _| {
                    text::next_word_start(buffer, offset, *big)
                });
                (to_position(offset), Kind::Exclusive, None)
            }
            Self::NextWordEnd { big } => {
                let offset = (0..times).fold(at(from), |offset, _| {
                    text::next_word_end(buffer, offset, *big)
                });
                (to_position(offset), Kind::Inclusive, None)
            }
            Self::PreviousWordStart { big } => {
                let offset = (0..times).fold(at(from), |offset, _| {
                    text::previous_word_start(buffer, offset, *big)
                });
                (to_position(offset), Kind::Exclusive, None)
            }
            Self::PreviousWordEnd { big } => {
                let offset = (0..times).fold(at(from), |offset, _| {
                    text::previous_word_end(buffer, offset, *big)
                });
                (to_position(offset), Kind::Inclusive, None)
            }
            Self::LineStart => (Position::new(from.line, 0), Kind::Exclusive, None),
            Self::FirstNonBlank => (line_start(from.line), Kind::Exclusive, None),
            Self::LineEnd => {
                let line = (from.line + times - 1).min(last);
                (
                    Position::new(line, last_column(buffer, line)),
                    Kind::Inclusive,
                    Some(usize::MAX),
                )
            }
            Self::LastNonBlank => {
                let line = (from.line + times - 1).min(last);
                let kept = buffer.line_text(line).trim_end().chars().count();
                (
                    Position::new(line, kept.saturating_sub(1)),
                    Kind::Inclusive,
                    None,
                )
            }
            Self::NextLineStart => {
                if from.line + times > last {
                    return None;
                }
                (line_start(from.line + times), Kind::Linewise, None)
            }
            Self::PreviousLineStart => {
                let line = from.line.checked_sub(times)?;
                (line_start(line), Kind::Linewise, None)
            }
            Self::CurrentLineStart => (
                line_start((from.line + times - 1).min(last)),
                Kind::Linewise,
                None,
            ),
            Self::Column => (
                Position::new(from.line, (times - 1).min(last_column(buffer, from.line))),
                Kind::Exclusive,
                None,
            ),
            Self::FirstLine => {
                let line = cx
                    .count
                    .map_or(0, |count| count.saturating_sub(1))
                    .min(last);
                (line_start(line), Kind::Linewise, None)
            }
            Self::LastLine => {
                let line = cx
                    .count
                    .map_or(last, |count| count.saturating_sub(1))
                    .min(last);
                (line_start(line), Kind::Linewise, None)
            }
            Self::Find(find) => {
                *cx.last_find = Some(*find);
                (
                    find_on_line(buffer, from, *find, times, false)?,
                    find_kind(*find),
                    None,
                )
            }
            Self::RepeatFind { reverse } => {
                let mut find = (*cx.last_find)?;
                if *reverse {
                    find.forward = !find.forward;
                }
                (
                    find_on_line(buffer, from, find, times, true)?,
                    find_kind(find),
                    None,
                )
            }
            Self::Matching => {
                let line = buffer.line_chars(from.line).collect::<Vec<_>>();
                let column =
                    (from.column..line.len()).find(|column| "()[]{}".contains(line[*column]))?;
                let partner = text::matching_bracket(buffer, at(Position::new(from.line, column)))?;
                (to_position(partner), Kind::Inclusive, None)
            }
            Self::ParagraphForward => {
                let line = (0..times).fold(from.line, |line, _| paragraph_forward(buffer, line));
                let to = match is_empty_line(buffer, line) || line < last {
                    true => Position::new(line, 0),
                    false => Position::new(line, buffer.line_len(line)),
                };
                (to, Kind::Exclusive, None)
            }
            Self::ParagraphBackward => {
                let line = (0..times).fold(from.line, |line, _| paragraph_backward(buffer, line));
                (Position::new(line, 0), Kind::Exclusive, None)
            }
            Self::ViewTop => {
                let line = (cx.view.top + times - 1).min(last);
                (line_start(line), Kind::Linewise, None)
            }
            Self::ViewMiddle => {
                let bottom = (cx.view.top + cx.view.rows.max(1) - 1).min(last);
                (line_start((cx.view.top + bottom) / 2), Kind::Linewise, None)
            }
            Self::ViewBottom => {
                let bottom = (cx.view.top + cx.view.rows.max(1) - 1).min(last);
                (
                    line_start(bottom.saturating_sub(times - 1).max(cx.view.top)),
                    Kind::Linewise,
                    None,
                )
            }
            Self::Search { text, forward } => {
                let search = LastSearch {
                    pattern: Pattern {
                        text: text.clone(),
                        whole_word: false,
                    },
                    forward: *forward,
                };
                *cx.last_search = Some(search.clone());
                (
                    search_from(buffer, from, &search, times)?,
                    Kind::Exclusive,
                    None,
                )
            }
            Self::SearchNext { reverse } => {
                let mut search = cx.last_search.clone()?;
                search.forward ^= *reverse;
                (
                    search_from(buffer, from, &search, times)?,
                    Kind::Exclusive,
                    None,
                )
            }
            Self::SearchWord { forward } => {
                let word = buffer.word_at(from);
                let text = buffer.text_in(word.clone());
                if text.is_empty() {
                    return None;
                }
                let search = LastSearch {
                    pattern: Pattern {
                        text,
                        whole_word: true,
                    },
                    forward: *forward,
                };
                *cx.last_search = Some(search.clone());
                (
                    search_from(buffer, word.start, &search, times)?,
                    Kind::Exclusive,
                    None,
                )
            }
            Self::Mark { name, line } => {
                let mark = buffer.clamped(*cx.marks.get(name)?);
                match line {
                    true => (line_start(mark.line), Kind::Linewise, None),
                    false => (mark, Kind::Exclusive, None),
                }
            }
        };
        Some(Moved {
            to: buffer.clamped(to),
            kind,
            goal,
        })
    }

    /// Whether the motion counts as made even when the cursor stays put.
    pub(crate) fn always_moves(&self) -> bool {
        matches!(
            self,
            Self::FirstLine | Self::LastLine | Self::LineEnd | Self::CurrentLineStart
        )
    }
}

/// The kind a character search is, which depends on its direction.
fn find_kind(find: Find) -> Kind {
    match find.forward {
        true => Kind::Inclusive,
        false => Kind::Exclusive,
    }
}

/// Where `lines` lines below `from` is, or above, keeping the aimed column.
fn vertical(buffer: &Buffer, from: Position, lines: isize, goal: Option<usize>) -> Option<Moved> {
    let last = buffer.line_count().saturating_sub(1);
    let line = from.line.saturating_add_signed(lines).min(last);
    if line == from.line {
        return None;
    }
    let goal = goal.unwrap_or(from.column);
    Some(Moved {
        to: Position::new(line, goal.min(last_column(buffer, line))),
        kind: Kind::Linewise,
        goal: Some(goal),
    })
}

/// Where the `times`-th `find.ch` on `from`'s line is, in `find`'s direction.
///
/// A repeated till search skips the character it is already stopped
/// against, or `;` after `t` would never get anywhere.
fn find_on_line(
    buffer: &Buffer,
    from: Position,
    find: Find,
    times: usize,
    repeat: bool,
) -> Option<Position> {
    let line = buffer.line_chars(from.line).collect::<Vec<_>>();
    let skip = usize::from(repeat && find.till);
    let hits = line
        .iter()
        .enumerate()
        .filter(|(_, ch)| **ch == find.ch)
        .map(|(column, _)| column);
    let column = match find.forward {
        true => hits
            .filter(|column| *column > from.column + skip)
            .nth(times - 1)?,
        false => hits
            .rev()
            .filter(|column| *column + skip < from.column)
            .nth(times - 1)?,
    };
    let column = match (find.till, find.forward) {
        (true, true) => column - 1,
        (true, false) => column + 1,
        _ => column,
    };
    Some(Position::new(from.line, column))
}

/// The next empty line below `line` that follows one that is not.
fn paragraph_forward(buffer: &Buffer, line: usize) -> usize {
    let last = buffer.line_count().saturating_sub(1);
    let mut line = line;
    while line < last && is_empty_line(buffer, line) {
        line += 1;
    }
    while line < last && !is_empty_line(buffer, line) {
        line += 1;
    }
    line
}

/// The next empty line above `line` that precedes one that is not.
fn paragraph_backward(buffer: &Buffer, line: usize) -> usize {
    let mut line = line;
    while line > 0 && is_empty_line(buffer, line) {
        line -= 1;
    }
    while line > 0 && !is_empty_line(buffer, line) {
        line -= 1;
    }
    line
}

/// Where the `times`-th match of `search` from `from` is.
fn search_from(
    buffer: &Buffer,
    from: Position,
    search: &LastSearch,
    times: usize,
) -> Option<Position> {
    let mut offset = buffer.char_of(from);
    for _ in 0..times {
        offset = search.pattern.find(buffer, offset, search.forward)?;
    }
    Some(buffer.position_of(offset))
}
