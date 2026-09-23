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
use crate::syntax;
use crate::text::{self, first_non_blank, indent_width, is_empty_line, last_column};

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

/// Which way an indentation jump compares the lines it passes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Indentation {
    /// To a line indented less than this one.
    Lesser,
    /// To a line indented more.
    Greater,
    /// To a line indented the same.
    Same,
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
    /// `gM`: to the middle of the line.
    MiddleOfLine,
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
    /// `N%`: to the line that far through the file.
    Percent,
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
    /// `)`: to the start of the next sentence.
    SentenceForward,
    /// `(`: to the start of this sentence, or the one before.
    SentenceBackward,
    /// `]]`, `][`, `[[` or `[]`: to the start or end of a top-level block.
    Section { forward: bool, end: bool },
    /// `]m`, `]M`, `[m` or `[M`: to the start or end of a function.
    Method { forward: bool, end: bool },
    /// `]/` or `[/`: to the next or previous comment.
    Comment { forward: bool },
    /// `]-`, `]+`, `]=` and their `[` twins: to a line indented differently.
    Indent {
        forward: bool,
        indentation: Indentation,
    },
    /// `])`, `]}`, `[(` or `[{`: to the bracket that encloses the cursor.
    Unmatched { forward: bool, bracket: char },
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
    /// Ctrl-F or Page Down: a view down.
    PageDown,
    /// Ctrl-B or Page Up: a view up.
    PageUp,
    /// `/` or `?`: to the next match of typed text.
    Search { text: String, forward: bool },
    /// `n` or `N`: the last search again, reversed for `N`.
    SearchNext { reverse: bool },
    /// `*`, `#`, `g*` or `g#`: to the next match of the word under the
    /// cursor, as a whole word unless `partial`.
    SearchWord { forward: bool, partial: bool },
    /// `'` or `` ` ``: to a mark, its line for `'`.
    Mark { name: char, line: bool },
}

/// The part of the buffer the pane is showing, for the motions that go by it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct View<'a> {
    /// The first line the pane shows.
    pub top: usize,
    /// How many lines the pane has room for.
    pub rows: usize,
    /// How many lines the pane keeps between the cursor and its edges.
    pub margin: usize,
    /// The column `gq` wraps text at.
    pub wrap: usize,
    /// The runs of lines folded away, which up and down step over.
    pub folds: &'a [std::ops::Range<usize>],
}

/// What a motion needs to know besides the buffer and where it starts.
pub(crate) struct Context<'a> {
    /// How many times to make the motion, and whether a count was typed.
    pub count: Option<usize>,
    /// The column vertical motions aim at, `usize::MAX` for line ends.
    pub goal: Option<usize>,
    /// The part of the buffer the pane is showing.
    pub view: View<'a>,
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
        let half = (cx.view.rows / 2).max(1) as isize;
        let page = cx.view.rows.max(1) as isize;
        let repeat =
            |step: &dyn Fn(usize) -> usize| (0..times).fold(at(from), |offset, _| step(offset));

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
            Self::Up => return vertical(buffer, from, -(times as isize), cx),
            Self::Down => return vertical(buffer, from, times as isize, cx),
            Self::HalfPageDown => return vertical(buffer, from, half, cx),
            Self::HalfPageUp => return vertical(buffer, from, -half, cx),
            Self::PageDown => return vertical(buffer, from, page * times as isize, cx),
            Self::PageUp => return vertical(buffer, from, -page * times as isize, cx),
            Self::NextWordStart { big } => (
                to_position(repeat(&|offset| {
                    text::next_word_start(buffer, offset, *big)
                })),
                Kind::Exclusive,
                None,
            ),
            Self::NextWordEnd { big } => (
                to_position(repeat(&|offset| text::next_word_end(buffer, offset, *big))),
                Kind::Inclusive,
                None,
            ),
            Self::PreviousWordStart { big } => (
                to_position(repeat(&|offset| {
                    text::previous_word_start(buffer, offset, *big)
                })),
                Kind::Exclusive,
                None,
            ),
            Self::PreviousWordEnd { big } => (
                to_position(repeat(&|offset| {
                    text::previous_word_end(buffer, offset, *big)
                })),
                Kind::Inclusive,
                None,
            ),
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
            Self::MiddleOfLine => {
                let len = buffer.line_len(from.line);
                let percent = cx.count.unwrap_or(50).min(100);
                (
                    Position::new(
                        from.line,
                        (len * percent / 100).min(last_column(buffer, from.line)),
                    ),
                    Kind::Exclusive,
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
            Self::Percent => {
                let lines = buffer.line_count();
                let line = (times.min(100) * lines)
                    .div_ceil(100)
                    .saturating_sub(1)
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
            Self::SentenceForward => (
                to_position(repeat(&|offset| text::next_sentence(buffer, offset))),
                Kind::Exclusive,
                None,
            ),
            Self::SentenceBackward => (
                to_position(repeat(&|offset| text::previous_sentence(buffer, offset))),
                Kind::Exclusive,
                None,
            ),
            Self::Section { forward, end } => {
                let places = syntax::sections(buffer, *end);
                (
                    step_through(&places, from, *forward, times)?,
                    Kind::Exclusive,
                    None,
                )
            }
            Self::Method { forward, end } => {
                let places = syntax::functions(buffer, *end);
                (
                    step_through(&places, from, *forward, times)?,
                    Kind::Exclusive,
                    None,
                )
            }
            Self::Comment { forward } => {
                let places = syntax::comments(buffer);
                (
                    step_through(&places, from, *forward, times)?,
                    Kind::Exclusive,
                    None,
                )
            }
            Self::Indent {
                forward,
                indentation,
            } => {
                let line = (0..times).try_fold(from.line, |line, _| {
                    indent_jump(buffer, line, *forward, *indentation)
                })?;
                (line_start(line), Kind::Linewise, None)
            }
            Self::Unmatched { forward, bracket } => {
                let (open, close) = match bracket {
                    '(' | ')' => ('(', ')'),
                    _ => ('{', '}'),
                };
                let offset = (0..times).try_fold(at(from), |offset, _| match forward {
                    true => text::enclosing_close(buffer, offset + 1, open, close),
                    false => text::enclosing_open(buffer, offset, open, close),
                })?;
                (to_position(offset), Kind::Exclusive, None)
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
                    pattern: Pattern::typed(text),
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
            Self::SearchWord { forward, partial } => {
                let word = buffer.word_at(from);
                let text = buffer.text_in(word.clone());
                if text.is_empty() {
                    return None;
                }
                let pattern = match partial {
                    true => Pattern::typed(&regex::escape(&text)),
                    false => Pattern::word(&text),
                };
                let search = LastSearch {
                    pattern,
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
            Self::FirstLine
                | Self::LastLine
                | Self::LineEnd
                | Self::CurrentLineStart
                | Self::Percent
                | Self::Find(_)
                | Self::RepeatFind { .. }
        )
    }

    /// Whether the motion is a jump, which the jump list remembers the
    /// place it left.
    pub(crate) fn is_jump(&self) -> bool {
        matches!(
            self,
            Self::FirstLine
                | Self::LastLine
                | Self::Percent
                | Self::Matching
                | Self::ParagraphForward
                | Self::ParagraphBackward
                | Self::SentenceForward
                | Self::SentenceBackward
                | Self::Section { .. }
                | Self::ViewTop
                | Self::ViewMiddle
                | Self::ViewBottom
                | Self::Search { .. }
                | Self::SearchNext { .. }
                | Self::SearchWord { .. }
                | Self::Mark { .. }
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

/// Where `lines` lines below `from` is, or above, keeping the aimed column
/// and counting a closed fold as the one line it shows.
fn vertical(buffer: &Buffer, from: Position, lines: isize, cx: &Context) -> Option<Moved> {
    let last = buffer.line_count().saturating_sub(1);
    let folded = |line: usize| cx.view.folds.iter().any(|fold| fold.contains(&line));
    let step = if lines >= 0 { 1 } else { -1 };
    let mut line = from.line;
    let mut left = lines.unsigned_abs();
    while left > 0 {
        let Some(next) = line.checked_add_signed(step).filter(|next| *next <= last) else {
            break;
        };
        line = next;
        if !folded(line) {
            left -= 1;
        }
    }
    while line > 0 && folded(line) {
        line -= 1;
    }
    if line == from.line {
        return None;
    }
    let goal = cx.goal.unwrap_or(from.column);
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

/// The `times`-th of `places` after `from`, or before it.
fn step_through(
    places: &[Position],
    from: Position,
    forward: bool,
    times: usize,
) -> Option<Position> {
    match forward {
        true => places
            .iter()
            .filter(|place| **place > from)
            .nth(times - 1)
            .copied(),
        false => places
            .iter()
            .rev()
            .filter(|place| **place < from)
            .nth(times - 1)
            .copied(),
    }
}

/// The next line from `line` in `forward`'s direction, skipping empty ones,
/// whose indentation compares with `line`'s as `indentation` asks.
fn indent_jump(
    buffer: &Buffer,
    line: usize,
    forward: bool,
    indentation: Indentation,
) -> Option<usize> {
    let width = indent_width(buffer, line);
    let lines: Box<dyn Iterator<Item = usize>> = match forward {
        true => Box::new(line + 1..buffer.line_count()),
        false => Box::new((0..line).rev()),
    };
    let mut lines = lines.filter(|candidate| !buffer.line_text(*candidate).trim().is_empty());
    lines.find(|candidate| {
        let other = indent_width(buffer, *candidate);
        match indentation {
            Indentation::Lesser => other < width,
            Indentation::Greater => other > width,
            Indentation::Same => other == width,
        }
    })
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
