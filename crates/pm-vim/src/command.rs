//! The grammar of normal and visual mode: which keys make a command.
//!
//! Keys are gathered until they say something: `d` says nothing yet, `d2`
//! still nothing, `d2w` a whole command. The parser is handed every key
//! gathered so far and answers that it needs more, that they make no
//! command, or what command they make — so a half-typed command is nothing
//! but the keys typed, and cancelling one is forgetting them.

use crate::key::{Key, Keystroke};
use crate::mode::Mode;
use crate::motion::{Find, Motion};
use crate::object::Object;
use crate::operator::Operator;

/// What the keys gathered so far amount to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Parsed {
    /// Not a command yet; more keys may make one.
    Incomplete,
    /// No command, however many keys follow.
    Invalid,
    /// A whole command.
    Done(Command),
}

/// A whole command: the register and count it was given, and what it does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Command {
    /// The register named with `"`, if one was.
    pub register: Option<char>,
    /// The count typed, before the command and after its operator multiplied.
    pub count: Option<usize>,
    /// What the command does.
    pub kind: Kind,
}

/// What a command does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    /// Moves the cursor, or the far end of the selection.
    Move(Motion),
    /// Applies an operator to a span in normal mode.
    Operate(Operator, Target),
    /// Applies an operator to the selection, as whole lines when `lines`.
    OperateSelection(Operator, bool),
    /// Selects a text object, its delimiters too when `around`.
    Select(Object, bool),
    /// Anything else.
    Act(Act),
}

/// What an operator in normal mode is given to act on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    /// The span a motion covers.
    Motion(Motion),
    /// A text object, its delimiters too when `around`.
    Object(Object, bool),
    /// Whole lines from the cursor's, as the operator doubled says.
    Lines,
}

/// Where insert mode begins, relative to the cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Entry {
    /// `i`: before the cursor.
    Before,
    /// `a`: after the cursor.
    After,
    /// `I`: before the first non-blank of the line.
    LineStart,
    /// `A`: at the end of the line.
    LineEnd,
    /// `o`: on a new line below.
    Below,
    /// `O`: on a new line above.
    Above,
}

/// Where `z` puts the cursor's line in the view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placement {
    /// `zt`: at the top.
    Top,
    /// `zz`: in the middle.
    Center,
    /// `zb`: at the bottom.
    Bottom,
}

/// A command aimed at the window's panes rather than the text: Ctrl-W.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Window {
    /// Focus the pane to the left.
    FocusLeft,
    /// Focus the pane to the right.
    FocusRight,
    /// Focus the pane above.
    FocusUp,
    /// Focus the pane below.
    FocusDown,
    /// Split the pane side by side.
    SplitRight,
    /// Split the pane one above the other.
    SplitDown,
    /// Close the pane's tab.
    Close,
}

/// A command that is neither a motion nor an operator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Act {
    /// Begins typing.
    Insert(Entry),
    /// `R`: begins typing over the text.
    ReplaceMode,
    /// `r`: replaces the character under the cursor, or the selection's.
    ReplaceChar(char),
    /// `J` or `gJ`: joins lines, with a space between for `J`.
    Join { spaces: bool },
    /// `p` or `P`: puts a register's text after the cursor, or before.
    Paste { before: bool },
    /// `u`: takes the last change back.
    Undo,
    /// Ctrl-R: puts the last change taken back again.
    Redo,
    /// `.`: makes the last change again.
    Repeat,
    /// `~`: swaps the case of the character under the cursor.
    ToggleCaseChar,
    /// `v` or `V`: selects, or switches or leaves the kind of selection.
    Visual(Mode),
    /// `gv`: selects what was last selected.
    VisualAgain,
    /// `o` in visual mode: moves the cursor to the selection's other end.
    SwapEnds,
    /// `m`: sets a mark at the cursor.
    Mark(char),
    /// `q` and a register: records the keys that follow.
    Record(char),
    /// `q` while recording: stops.
    StopRecording,
    /// `@`: plays the keys recorded in a register, `@` for the last one.
    Play(char),
    /// `z`: scrolls the cursor's line to a place in the view.
    Scroll(Placement),
    /// Ctrl-W: acts on the panes.
    Window(Window),
    /// `:`: runs a command line.
    Ex(String),
    /// `ZZ` or `ZQ`: closes the file, writing it first for `ZZ`.
    Quit { write: bool },
}

/// Why reading stopped short of a command.
enum Stop {
    /// The keys ran out.
    Incomplete,
    /// A key made the command impossible.
    Invalid,
}

/// Reading a command, which stops short with a [`Stop`].
type Step<T> = Result<T, Stop>;

/// The keys gathered, and how far into them reading has got.
struct Reader<'a> {
    /// The keys.
    keys: &'a [Keystroke],
    /// How many have been read.
    at: usize,
}

impl Reader<'_> {
    /// The next key.
    fn next(&mut self) -> Step<Keystroke> {
        let key = *self.keys.get(self.at).ok_or(Stop::Incomplete)?;
        self.at += 1;
        match key.is_escape() {
            true => Err(Stop::Invalid),
            false => Ok(key),
        }
    }

    /// The next key, which must type a character; Enter types a line break.
    fn char(&mut self) -> Step<char> {
        let key = self.next()?;
        match key.key {
            Key::Enter => Ok('\n'),
            _ => key.char().ok_or(Stop::Invalid),
        }
    }

    /// The character the next key types, without reading it.
    fn peek(&self) -> Option<char> {
        self.keys.get(self.at).and_then(|key| key.char())
    }

    /// A count, if one is typed next.
    fn count(&mut self) -> Option<usize> {
        let mut count = None::<usize>;
        while let Some(digit) = self.peek().and_then(|ch| ch.to_digit(10)) {
            if digit == 0 && count.is_none() {
                break;
            }
            self.at += 1;
            count = Some(
                count
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit as usize),
            );
        }
        count
    }

    /// Text typed up to Enter, as a search or a command line takes it.
    ///
    /// Backspace takes a character back, and taking back from nothing
    /// cancels, the way it does on vim's own command line.
    fn line(&mut self) -> Step<String> {
        let mut text = String::new();
        loop {
            let key = self.next()?;
            match key.key {
                Key::Enter => return Ok(text),
                Key::Backspace if text.pop().is_none() => return Err(Stop::Invalid),
                Key::Backspace => {}
                _ => text.extend(key.char()),
            }
        }
    }
}

/// What `keys` amount to in `mode`, `recording` saying whether `q` stops.
pub(crate) fn parse(keys: &[Keystroke], mode: Mode, recording: bool) -> Parsed {
    let mut reader = Reader { keys, at: 0 };
    match command(&mut reader, mode, recording) {
        Ok(command) => Parsed::Done(command),
        Err(Stop::Incomplete) => Parsed::Incomplete,
        Err(Stop::Invalid) => Parsed::Invalid,
    }
}

/// What is being typed on the command line or in a search, if anything.
///
/// Answers the line as the status bar shows it, its `:`, `/` or `?` first.
pub(crate) fn typed_line(keys: &[Keystroke]) -> Option<String> {
    let start = keys
        .iter()
        .position(|key| matches!(key.char(), Some(':' | '/' | '?')))?;
    let before = &keys[..start];
    let prefix_only = before.iter().all(|key| {
        key.char()
            .is_some_and(|ch| ch.is_ascii_digit() || "\"dcy<>gu~U".contains(ch))
    });
    if !prefix_only {
        return None;
    }
    let mut text = String::new();
    for key in &keys[start..] {
        match key.key {
            Key::Backspace => {
                text.pop();
            }
            _ => text.extend(key.char()),
        }
    }
    Some(text)
}

/// Reads one whole command.
fn command(reader: &mut Reader, mode: Mode, recording: bool) -> Step<Command> {
    let register = match reader.peek() {
        Some('"') => {
            reader.at += 1;
            Some(reader.char()?)
        }
        _ => None,
    };
    let count = reader.count();
    let key = reader.next()?;
    let (kind, extra) = match mode.is_visual() {
        true => (visual(reader, key)?, None),
        false => normal(reader, key, recording)?,
    };
    let count = match (count, extra) {
        (None, None) => None,
        (count, extra) => Some(count.unwrap_or(1).saturating_mul(extra.unwrap_or(1))),
    };
    Ok(Command {
        register,
        count,
        kind,
    })
}

/// Reads a normal-mode command from its first key, with any count typed
/// after its operator.
fn normal(reader: &mut Reader, key: Keystroke, recording: bool) -> Step<(Kind, Option<usize>)> {
    if let Some(motion) = motion(reader, key)? {
        return Ok((Kind::Move(motion), None));
    }
    if let Some(ch) = key.ctrl_char() {
        let act = match ch {
            'r' => Act::Redo,
            'w' => Act::Window(window(reader)?),
            _ => return Err(Stop::Invalid),
        };
        return Ok((Kind::Act(act), None));
    }
    let ch = match key.key {
        Key::Delete => 'x',
        _ => key.char().ok_or(Stop::Invalid)?,
    };
    if let Some(operator) = Operator::of(ch, false) {
        return operate(reader, operator, ch);
    }
    if let Some((operator, target)) = shorthand(ch) {
        return Ok((Kind::Operate(operator, target), None));
    }
    let act = match ch {
        'g' => {
            let next = reader.char()?;
            if let Some(operator) = Operator::of(next, true) {
                return operate(reader, operator, next);
            }
            match next {
                'J' => Act::Join { spaces: false },
                'v' => Act::VisualAgain,
                _ => return Err(Stop::Invalid),
            }
        }
        'i' => Act::Insert(Entry::Before),
        'a' => Act::Insert(Entry::After),
        'I' => Act::Insert(Entry::LineStart),
        'A' => Act::Insert(Entry::LineEnd),
        'o' => Act::Insert(Entry::Below),
        'O' => Act::Insert(Entry::Above),
        'R' => Act::ReplaceMode,
        'r' => Act::ReplaceChar(reader.char()?),
        'J' => Act::Join { spaces: true },
        'p' => Act::Paste { before: false },
        'P' => Act::Paste { before: true },
        'u' => Act::Undo,
        '.' => Act::Repeat,
        '~' => Act::ToggleCaseChar,
        'v' => Act::Visual(Mode::Visual),
        'V' => Act::Visual(Mode::VisualLine),
        'm' => Act::Mark(reader.char()?),
        'q' if recording => Act::StopRecording,
        'q' => Act::Record(reader.char()?),
        '@' => Act::Play(reader.char()?),
        'z' => Act::Scroll(match reader.char()? {
            't' | '\n' => Placement::Top,
            'z' | '.' => Placement::Center,
            'b' | '-' => Placement::Bottom,
            _ => return Err(Stop::Invalid),
        }),
        ':' => Act::Ex(reader.line()?),
        'Z' => match reader.char()? {
            'Z' => Act::Quit { write: true },
            'Q' => Act::Quit { write: false },
            _ => return Err(Stop::Invalid),
        },
        _ => return Err(Stop::Invalid),
    };
    Ok((Kind::Act(act), None))
}

/// The operator and target a one-key shorthand stands for: `x` is `dl`.
fn shorthand(ch: char) -> Option<(Operator, Target)> {
    Some(match ch {
        'x' => (Operator::Delete, Target::Motion(Motion::Right)),
        'X' => (Operator::Delete, Target::Motion(Motion::Left)),
        's' => (Operator::Change, Target::Motion(Motion::Right)),
        'S' => (Operator::Change, Target::Lines),
        'C' => (Operator::Change, Target::Motion(Motion::LineEnd)),
        'D' => (Operator::Delete, Target::Motion(Motion::LineEnd)),
        'Y' => (Operator::Yank, Target::Lines),
        _ => return None,
    })
}

/// Reads what `operator`, typed as `key`, is given to act on.
fn operate(reader: &mut Reader, operator: Operator, key: char) -> Step<(Kind, Option<usize>)> {
    let count = reader.count();
    let next = reader.next()?;
    let target = match next.char() {
        Some(ch) if ch == key => Target::Lines,
        Some('g') if matches!(key, 'u' | 'U' | '~') && reader.peek() == Some(key) => {
            reader.at += 1;
            Target::Lines
        }
        Some(side @ ('i' | 'a')) => {
            let object = Object::of(reader.char()?).ok_or(Stop::Invalid)?;
            Target::Object(object, side == 'a')
        }
        _ => Target::Motion(motion(reader, next)?.ok_or(Stop::Invalid)?),
    };
    Ok((Kind::Operate(operator, target), count))
}

/// Reads a visual-mode command from its first key.
fn visual(reader: &mut Reader, key: Keystroke) -> Step<Kind> {
    if let Some(motion) = motion(reader, key)? {
        return Ok(Kind::Move(motion));
    }
    if key.key == Key::Delete {
        return Ok(Kind::OperateSelection(Operator::Delete, false));
    }
    let ch = key.char().ok_or(Stop::Invalid)?;
    let selection = |operator: Operator, lines: bool| Ok(Kind::OperateSelection(operator, lines));
    match ch {
        'd' | 'x' => selection(Operator::Delete, false),
        'D' | 'X' => selection(Operator::Delete, true),
        'c' | 's' => selection(Operator::Change, false),
        'C' | 'S' | 'R' => selection(Operator::Change, true),
        'y' => selection(Operator::Yank, false),
        'Y' => selection(Operator::Yank, true),
        '>' => selection(Operator::Indent, false),
        '<' => selection(Operator::Outdent, false),
        '~' => selection(Operator::ToggleCase, false),
        'u' => selection(Operator::Lowercase, false),
        'U' => selection(Operator::Uppercase, false),
        'g' => match reader.char()? {
            'u' => selection(Operator::Lowercase, false),
            'U' => selection(Operator::Uppercase, false),
            '~' => selection(Operator::ToggleCase, false),
            'J' => Ok(Kind::Act(Act::Join { spaces: false })),
            'v' => Ok(Kind::Act(Act::VisualAgain)),
            _ => Err(Stop::Invalid),
        },
        'i' | 'a' => {
            let object = Object::of(reader.char()?).ok_or(Stop::Invalid)?;
            Ok(Kind::Select(object, ch == 'a'))
        }
        'J' => Ok(Kind::Act(Act::Join { spaces: true })),
        'p' | 'P' => Ok(Kind::Act(Act::Paste { before: ch == 'P' })),
        'r' => Ok(Kind::Act(Act::ReplaceChar(reader.char()?))),
        'o' | 'O' => Ok(Kind::Act(Act::SwapEnds)),
        'v' => Ok(Kind::Act(Act::Visual(Mode::Visual))),
        'V' => Ok(Kind::Act(Act::Visual(Mode::VisualLine))),
        ':' => Ok(Kind::Act(Act::Ex(reader.line()?))),
        _ => Err(Stop::Invalid),
    }
}

/// Reads the pane command after Ctrl-W.
fn window(reader: &mut Reader) -> Step<Window> {
    let key = reader.next()?;
    let ch = key.char().or(key.ctrl_char());
    Ok(match (key.key, ch) {
        (Key::Left, _) | (_, Some('h')) => Window::FocusLeft,
        (Key::Right, _) | (_, Some('l')) => Window::FocusRight,
        (Key::Up, _) | (_, Some('k')) => Window::FocusUp,
        (Key::Down, _) | (_, Some('j')) => Window::FocusDown,
        (_, Some('v')) => Window::SplitRight,
        (_, Some('s')) => Window::SplitDown,
        (_, Some('c' | 'q')) => Window::Close,
        _ => return Err(Stop::Invalid),
    })
}

/// Reads a motion from its first key, or answers `None` when `key` does
/// not begin one.
fn motion(reader: &mut Reader, key: Keystroke) -> Step<Option<Motion>> {
    let named = match (key.key, key.ctrl) {
        (Key::Left, false) => Some(Motion::Left),
        (Key::Right, false) => Some(Motion::Right),
        (Key::Up, false) => Some(Motion::Up),
        (Key::Down, false) => Some(Motion::Down),
        (Key::Backspace, false) => Some(Motion::WrappingLeft),
        (Key::Enter, false) => Some(Motion::NextLineStart),
        (Key::Home, false) => Some(Motion::LineStart),
        (Key::End, false) => Some(Motion::LineEnd),
        (Key::PageUp, false) => Some(Motion::PageUp),
        (Key::PageDown, false) => Some(Motion::PageDown),
        _ => None,
    };
    if named.is_some() {
        return Ok(named);
    }
    if let Some(ch) = key.ctrl_char() {
        return Ok(match ch {
            'd' => Some(Motion::HalfPageDown),
            'u' => Some(Motion::HalfPageUp),
            _ => None,
        });
    }
    let Some(ch) = key.char() else {
        return Ok(None);
    };
    let find = |reader: &mut Reader, forward: bool, till: bool| -> Step<Option<Motion>> {
        let ch = reader.char()?;
        Ok(Some(Motion::Find(Find { ch, forward, till })))
    };
    Ok(Some(match ch {
        'h' => Motion::Left,
        'l' => Motion::Right,
        ' ' => Motion::WrappingRight,
        'k' => Motion::Up,
        'j' => Motion::Down,
        'w' => Motion::NextWordStart { big: false },
        'W' => Motion::NextWordStart { big: true },
        'e' => Motion::NextWordEnd { big: false },
        'E' => Motion::NextWordEnd { big: true },
        'b' => Motion::PreviousWordStart { big: false },
        'B' => Motion::PreviousWordStart { big: true },
        '0' => Motion::LineStart,
        '^' => Motion::FirstNonBlank,
        '$' => Motion::LineEnd,
        '+' => Motion::NextLineStart,
        '-' => Motion::PreviousLineStart,
        '_' => Motion::CurrentLineStart,
        '|' => Motion::Column,
        'G' => Motion::LastLine,
        'f' => return find(reader, true, false),
        'F' => return find(reader, false, false),
        't' => return find(reader, true, true),
        'T' => return find(reader, false, true),
        ';' => Motion::RepeatFind { reverse: false },
        ',' => Motion::RepeatFind { reverse: true },
        '%' => Motion::Matching,
        '}' => Motion::ParagraphForward,
        '{' => Motion::ParagraphBackward,
        'H' => Motion::ViewTop,
        'M' => Motion::ViewMiddle,
        'L' => Motion::ViewBottom,
        'n' => Motion::SearchNext { reverse: false },
        'N' => Motion::SearchNext { reverse: true },
        '*' => Motion::SearchWord { forward: true },
        '#' => Motion::SearchWord { forward: false },
        '/' | '?' => Motion::Search {
            text: reader.line()?,
            forward: ch == '/',
        },
        '\'' | '`' => Motion::Mark {
            name: reader.char()?,
            line: ch == '\'',
        },
        'g' => match reader.peek() {
            Some(next @ ('g' | 'e' | 'E' | '_' | 'j' | 'k')) => {
                reader.at += 1;
                match next {
                    'g' => Motion::FirstLine,
                    'e' => Motion::PreviousWordEnd { big: false },
                    'E' => Motion::PreviousWordEnd { big: true },
                    '_' => Motion::LastNonBlank,
                    'j' => Motion::Down,
                    _ => Motion::Up,
                }
            }
            Some(_) => return Ok(None),
            None if reader.at >= reader.keys.len() => return Err(Stop::Incomplete),
            None => return Ok(None),
        },
        _ => return Ok(None),
    }))
}
