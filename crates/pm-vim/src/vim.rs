//! The engine: keys in, a buffer changed, and what the window must do.
//!
//! [`State`] is what one buffer is in the middle of — its mode, the keys
//! gathered towards a command, its selection and marks. [`Vim`] is what the
//! whole window shares: registers, the last change, the last search, the
//! macros. Every key goes through [`Vim::press`], which answers whether the
//! key was taken and what the window has to do about it.

use std::collections::HashMap;

use pm_text::{Buffer, Motion as Step, Position, Selection};

use crate::command::{self, Act, Command, Entry, Kind, Parsed, Placement, Target, Window};
use crate::key::{Key, Keystroke};
use crate::mode::{Mode, Shape};
use crate::motion::{Context, Find, Motion, View};
use crate::object::Object;
use crate::operator::{self, Operator, Span, Store};
use crate::register::{Clipboard, Registers};
use crate::search::LastSearch;
use crate::text::{self, Class, class, first_non_blank, on_char};

/// Something the window has to do because of a key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Write the file.
    Save,
    /// Write every file.
    SaveAll,
    /// Close the file's tab.
    Close,
    /// Act on the panes.
    Window(Window),
    /// Scroll the cursor's line to a place in the view.
    Scroll(Placement),
    /// Scroll the view by this many lines, down when positive.
    ScrollBy(isize),
}

/// What became of a key.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Outcome {
    /// Whether the key was taken; one that was not is the window's to handle.
    pub handled: bool,
    /// What the window has to do.
    pub effects: Vec<Effect>,
}

/// A stretch of typing begun by a command, and what ending it must do.
#[derive(Clone, Debug)]
struct Typing {
    /// How deep the undo history was before the command, to join it into one step.
    depth: usize,
    /// How many times the typing is made in all.
    count: usize,
    /// How the typing was begun, when it was by an insert command.
    entry: Option<Entry>,
    /// The keys typed so far.
    keys: Vec<Keystroke>,
    /// What each character typed over in replace mode stood on, for Backspace.
    replaced: Vec<Option<char>>,
}

/// What one buffer is in the middle of.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The mode it is in.
    mode: Mode,
    /// The keys gathered towards a command.
    pending: Vec<Keystroke>,
    /// The end of the selection that stays put, in visual mode.
    anchor: Position,
    /// The end of the selection the cursor is on, in visual mode.
    head: Position,
    /// The selection last put on the buffer, to notice the pointer changing it.
    shown: Option<Selection>,
    /// The column vertical motions aim at.
    goal: Option<usize>,
    /// The marks set in the buffer.
    marks: HashMap<char, Position>,
    /// The last selection, for `gv`.
    last_visual: Option<(Mode, Position, Position)>,
    /// The typing under way, in insert or replace mode.
    typing: Option<Typing>,
}

impl State {
    /// The mode the buffer is in.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// How the cursor is drawn.
    pub fn shape(&self) -> Shape {
        match self.mode {
            Mode::Insert => Shape::Typing,
            Mode::Replace => Shape::Underline,
            _ if !self.pending.is_empty() => Shape::Underline,
            _ => Shape::Block,
        }
    }

    /// What has been typed towards a command, as the status bar shows it.
    pub fn pending(&self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        command::typed_line(&self.pending)
            .or_else(|| Some(self.pending.iter().map(|key| key.label()).collect()))
    }
}

/// What the window shares between buffers.
#[derive(Debug, Default)]
pub struct Vim {
    /// Every register.
    registers: Registers,
    /// The last character search, for `;` and `,`.
    last_find: Option<Find>,
    /// The last search, for `n` and `N`.
    last_search: Option<LastSearch>,
    /// The keys of the last change, for `.`.
    last_change: Option<Vec<Keystroke>>,
    /// The keys of the change being made, until it is finished.
    change: Option<Vec<Keystroke>>,
    /// The register being recorded into and the keys recorded so far.
    recording: Option<(char, Vec<Keystroke>)>,
    /// The keys recorded into each register.
    macros: HashMap<char, Vec<Keystroke>>,
    /// The register last played, for `@@`.
    last_macro: Option<char>,
    /// Whether `.` is replaying a change.
    repeating: bool,
    /// How many macros deep the keys being fed come from.
    playing: usize,
}

/// Everything a key is carried out against.
struct Stage<'a> {
    /// The buffer's state.
    state: &'a mut State,
    /// The buffer.
    buffer: &'a mut Buffer,
    /// The part of the buffer the pane shows.
    view: View,
    /// The system clipboard.
    clipboard: &'a mut dyn Clipboard,
    /// What the window has to do.
    effects: &'a mut Vec<Effect>,
}

impl Vim {
    /// The register being recorded into, if one is.
    pub fn recording(&self) -> Option<char> {
        self.recording.as_ref().map(|(name, _)| *name)
    }

    /// Carries `key` out on `buffer`, whose state is `state`.
    pub fn press(
        &mut self,
        state: &mut State,
        buffer: &mut Buffer,
        view: View,
        clipboard: &mut dyn Clipboard,
        key: Keystroke,
    ) -> Outcome {
        if let Some((_, keys)) = self.recording.as_mut() {
            keys.push(key);
        }
        let mut effects = Vec::new();
        let mut target = Stage {
            state,
            buffer,
            view,
            clipboard,
            effects: &mut effects,
        };
        let handled = self.feed(&mut target, key);
        Outcome { handled, effects }
    }

    /// Whether `.` or a macro is feeding the keys, rather than the reader.
    fn replaying(&self) -> bool {
        self.repeating || self.playing > 0
    }

    /// Sends `key` to whatever the buffer's mode does with keys.
    fn feed(&mut self, target: &mut Stage, key: Keystroke) -> bool {
        match target.state.mode {
            Mode::Insert => self.insert_key(target, key),
            Mode::Replace => self.replace_key(target, key),
            _ => self.command_key(target, key),
        }
    }

    /// Writes down a key typed as part of the change being made.
    fn note_typed(&mut self, target: &mut Stage, key: Keystroke) {
        if !self.repeating
            && let Some(change) = self.change.as_mut()
        {
            change.push(key);
        }
        if let Some(typing) = target.state.typing.as_mut()
            && !key.is_escape()
        {
            typing.keys.push(key);
        }
    }

    /// A key typed in insert mode: Escape leaves it, anything else types.
    ///
    /// The window types the key itself, auto-pairs and completions and all;
    /// only keys being replayed are typed here, since then there is no
    /// window behind them.
    fn insert_key(&mut self, target: &mut Stage, key: Keystroke) -> bool {
        self.note_typed(target, key);
        if key.is_escape() {
            self.finish_typing(target);
            return true;
        }
        if self.replaying() {
            type_key(target.buffer, key);
            return true;
        }
        false
    }

    /// A key typed in replace mode: it types over the character under the
    /// cursor, and Backspace puts back what it typed over.
    fn replace_key(&mut self, target: &mut Stage, key: Keystroke) -> bool {
        if key.ctrl && !key.is_escape() {
            return false;
        }
        self.note_typed(target, key);
        if key.is_escape() {
            self.finish_typing(target);
            return true;
        }
        let buffer = &mut *target.buffer;
        let head = buffer.selection().head;
        let Some(typing) = target.state.typing.as_mut() else {
            return true;
        };
        match key.key {
            Key::Char(ch) => {
                let over = buffer.char_at(head);
                let end = Position::new(head.line, head.column + usize::from(over.is_some()));
                buffer.replace(head..end, &ch.to_string());
                typing.replaced.push(over);
            }
            Key::Backspace => match typing.replaced.pop() {
                Some(Some(over)) => {
                    let start = Position::new(head.line, head.column.saturating_sub(1));
                    buffer.replace(start..head, &over.to_string());
                    buffer.place(start, false);
                }
                Some(None) => buffer.backspace(),
                None => buffer.move_cursor(Step::Left, false),
            },
            _ => type_key(buffer, key),
        }
        true
    }

    /// Ends the typing under way: makes it as many times as counted, joins
    /// it into one undo step and steps back onto the last character typed.
    fn finish_typing(&mut self, target: &mut Stage) {
        let buffer = &mut *target.buffer;
        if let Some(typing) = target.state.typing.take() {
            for _ in 1..typing.count {
                match typing.entry {
                    Some(Entry::Below) => buffer.insert_line_below(),
                    Some(Entry::Above) => buffer.insert_line_above(),
                    _ => {}
                }
                for key in &typing.keys {
                    type_key(buffer, *key);
                }
            }
            buffer.squash_since(typing.depth);
        }
        let head = buffer.selection().head;
        let back = Position::new(head.line, head.column.saturating_sub(1));
        buffer.set_selection(Selection::at(on_char(buffer, back)));
        target.state.mode = Mode::Normal;
        target.state.goal = None;
        if !self.repeating
            && let Some(change) = self.change.take()
        {
            self.last_change = Some(change);
        }
    }

    /// A key typed in normal or visual mode, gathered towards a command.
    fn command_key(&mut self, target: &mut Stage, key: Keystroke) -> bool {
        self.follow_pointer(target);
        let state = &mut *target.state;
        if key.is_escape() {
            if !state.pending.is_empty() {
                state.pending.clear();
                return true;
            }
            if state.mode.is_visual() {
                self.leave_visual(target);
                return true;
            }
            target.buffer.collapse_cursors();
            return false;
        }
        let claimed = matches!(key.ctrl_char(), Some('r' | 'w' | 'd' | 'u'));
        if key.ctrl && !claimed && state.pending.is_empty() {
            return false;
        }

        state.pending.push(key);
        match command::parse(&state.pending, state.mode, self.recording.is_some()) {
            Parsed::Incomplete => {}
            Parsed::Invalid => state.pending.clear(),
            Parsed::Done(command) => {
                let keys = std::mem::take(&mut state.pending);
                self.run(target, command, keys);
            }
        }
        true
    }

    /// Takes up a selection the pointer made, or a click that ended one.
    fn follow_pointer(&mut self, target: &mut Stage) {
        let selection = target.buffer.selection();
        let state = &mut *target.state;
        match state.mode {
            Mode::Normal if !selection.is_empty() && !target.buffer.has_many_cursors() => {
                state.mode = Mode::Visual;
                state.anchor = selection.anchor;
                state.head = before_end(target.buffer, selection);
                show_visual(state, target.buffer);
            }
            mode if mode.is_visual() && state.shown != Some(selection) => {
                if selection.is_empty() {
                    state.mode = Mode::Normal;
                    return;
                }
                state.anchor = selection.anchor;
                state.head = before_end(target.buffer, selection);
                show_visual(state, target.buffer);
            }
            _ => {}
        }
    }

    /// Carries out a whole command, gathered from `keys`.
    fn run(&mut self, target: &mut Stage, command: Command, keys: Vec<Keystroke>) {
        let changes = changes_text(&command);
        if changes && !self.repeating {
            self.change = Some(keys);
        }
        let before = target.state.mode;
        match command.kind.clone() {
            Kind::Move(motion) => self.move_by(target, &motion, command.count),
            Kind::Operate(operator, what) => self.operate(target, &command, operator, what),
            Kind::OperateSelection(operator, lines) => {
                self.operate_selection(target, &command, operator, lines)
            }
            Kind::Select(object, around) => select(target, object, around),
            Kind::Act(act) => self.act(target, &command, act),
        }
        if before.is_visual() && !target.state.mode.is_visual() {
            let state = &mut *target.state;
            state.last_visual = Some((before, state.anchor, state.head));
        }
        if target.state.mode.is_typing() {
            return;
        }
        if !self.repeating
            && let Some(change) = self.change.take()
        {
            self.last_change = Some(change);
        }
        settle(target.state, target.buffer);
    }

    /// Moves the cursor, or the far end of the selection, by `motion`.
    fn move_by(&mut self, target: &mut Stage, motion: &Motion, count: Option<usize>) {
        let visual = target.state.mode.is_visual();
        let from = match visual {
            true => target.state.head,
            false => target.buffer.selection().head,
        };
        let Some(moved) = self.resolve(target, motion, from, count) else {
            return;
        };
        target.state.goal = moved.goal;
        let half = (target.view.rows / 2).max(1) as isize;
        match motion {
            Motion::HalfPageDown => target.effects.push(Effect::ScrollBy(half)),
            Motion::HalfPageUp => target.effects.push(Effect::ScrollBy(-half)),
            _ => {}
        }
        match visual {
            true => {
                target.state.head = moved.to;
                show_visual(target.state, target.buffer);
            }
            false => target.buffer.set_selection(Selection::at(moved.to)),
        }
    }

    /// Where `motion` takes a cursor at `from`, if it takes it anywhere.
    fn resolve(
        &mut self,
        target: &mut Stage,
        motion: &Motion,
        from: Position,
        count: Option<usize>,
    ) -> Option<crate::motion::Moved> {
        let mut cx = Context {
            count,
            goal: target.state.goal,
            view: target.view,
            last_find: &mut self.last_find,
            last_search: &mut self.last_search,
            marks: &target.state.marks,
        };
        let moved = motion.resolve(target.buffer, from, &mut cx)?;
        (moved.to != from || motion.always_moves()).then_some(moved)
    }

    /// Applies `operator` to what `what` names, from the cursor.
    fn operate(&mut self, target: &mut Stage, command: &Command, operator: Operator, what: Target) {
        let from = target.buffer.selection().head;
        let times = command.count.unwrap_or(1).max(1);
        let last = target.buffer.line_count().saturating_sub(1);
        let depth = target.buffer.undo_depth();
        let span = match what {
            Target::Lines => Some(Span::lines(from.line, (from.line + times - 1).min(last))),
            Target::Object(object, around) => object.span(target.buffer, from, around),
            Target::Motion(motion) => {
                self.motion_span(target, operator, &motion, from, command.count)
            }
        };
        let Some(span) = span else {
            if operator == Operator::Change {
                self.begin_typing(target, Mode::Insert, depth, 1, None);
            } else {
                self.change = None;
            }
            return;
        };
        let mut store = Store {
            registers: &mut self.registers,
            register: command.register,
            clipboard: &mut *target.clipboard,
        };
        let to = operator.apply(target.buffer, span, from, &mut store);
        target.buffer.set_selection(Selection::at(to));
        if operator == Operator::Change {
            self.begin_typing(target, Mode::Insert, depth, 1, None);
        }
    }

    /// The span `motion` covers from `from` for `operator`.
    ///
    /// `cw` on a word changes to its end rather than to the next word's
    /// start, and `dw` on the last word of a line stops at the line's end
    /// rather than joining the next: vim's two exceptions to its own rule.
    fn motion_span(
        &mut self,
        target: &mut Stage,
        operator: Operator,
        motion: &Motion,
        from: Position,
        count: Option<usize>,
    ) -> Option<Span> {
        let buffer = &*target.buffer;
        if let Motion::NextWordStart { big } = motion
            && operator == Operator::Change
            && buffer.char_at(from).is_some_and(|ch| !ch.is_whitespace())
        {
            let times = count.unwrap_or(1).max(1);
            let start = buffer.char_of(from);
            let same = |offset: usize| {
                let kind = |at: usize| buffer.char_at_offset(at).map(|ch| class(ch, *big));
                kind(offset) == kind(offset + 1) && kind(offset) != Some(Class::Blank)
            };
            let mut end = match same(start) {
                true => text::next_word_end(buffer, start, *big),
                false => start,
            };
            for _ in 1..times {
                end = text::next_word_end(buffer, end, *big);
            }
            return Some(Span::chars(
                from,
                text::after(buffer, buffer.position_of(end)),
            ));
        }
        let moved = self.resolve(target, motion, from, count)?;
        let buffer = &*target.buffer;
        let to = moved.to;
        if matches!(motion, Motion::NextWordStart { .. })
            && to.line > from.line
            && to.column <= first_non_blank(buffer, to.line)
        {
            let end = Position::new(to.line - 1, buffer.line_len(to.line - 1));
            return Some(match end <= from {
                true => Span::lines(from.line, from.line),
                false => Span::chars(from, end),
            });
        }
        Some(Span::of_motion(buffer, from, moved))
    }

    /// Applies `operator` to the selection, as whole lines when `lines`.
    fn operate_selection(
        &mut self,
        target: &mut Stage,
        command: &Command,
        operator: Operator,
        lines: bool,
    ) {
        let span = visual_span(target.state, target.buffer, lines);
        let depth = target.buffer.undo_depth();
        target.state.mode = Mode::Normal;
        if matches!(operator, Operator::Indent | Operator::Outdent) {
            let times = command.count.unwrap_or(1).max(1);
            operator::shift(target.buffer, span, operator == Operator::Indent, times);
            let line = span.start.line;
            let to = Position::new(line, first_non_blank(target.buffer, line));
            target.buffer.set_selection(Selection::at(to));
            return;
        }
        let mut store = Store {
            registers: &mut self.registers,
            register: command.register,
            clipboard: &mut *target.clipboard,
        };
        let to = operator.apply(target.buffer, span, span.start, &mut store);
        target.buffer.set_selection(Selection::at(to));
        if operator == Operator::Change {
            self.begin_typing(target, Mode::Insert, depth, 1, None);
        }
    }

    /// Enters `mode` to type, joining what is typed onto the undo step that
    /// began at `depth`.
    fn begin_typing(
        &mut self,
        target: &mut Stage,
        mode: Mode,
        depth: usize,
        count: usize,
        entry: Option<Entry>,
    ) {
        target.state.mode = mode;
        target.state.typing = Some(Typing {
            depth,
            count,
            entry,
            keys: Vec::new(),
            replaced: Vec::new(),
        });
    }

    /// Carries out a command that is neither a motion nor an operator.
    fn act(&mut self, target: &mut Stage, command: &Command, act: Act) {
        let times = command.count.unwrap_or(1).max(1);
        let head = target.buffer.selection().head;
        match act {
            Act::Insert(entry) => {
                let depth = target.buffer.undo_depth();
                let buffer = &mut *target.buffer;
                let line = head.line;
                match entry {
                    Entry::Before => {}
                    Entry::After => {
                        let column = (head.column + 1).min(buffer.line_len(line));
                        buffer.place(Position::new(line, column), false);
                    }
                    Entry::LineStart => {
                        buffer.place(Position::new(line, first_non_blank(buffer, line)), false)
                    }
                    Entry::LineEnd => {
                        buffer.place(Position::new(line, buffer.line_len(line)), false)
                    }
                    Entry::Below => buffer.insert_line_below(),
                    Entry::Above => buffer.insert_line_above(),
                }
                self.begin_typing(target, Mode::Insert, depth, times, Some(entry));
            }
            Act::ReplaceMode => {
                let depth = target.buffer.undo_depth();
                self.begin_typing(target, Mode::Replace, depth, 1, None);
            }
            Act::ReplaceChar(ch) => self.replace_chars(target, ch, times),
            Act::Join { spaces } => join(target, times, spaces),
            Act::Paste { before } => self.paste(target, command.register, before, times),
            Act::Undo | Act::Redo => {
                for _ in 0..times {
                    let done = match act == Act::Undo {
                        true => target.buffer.undo(),
                        false => target.buffer.redo(),
                    };
                    if !done {
                        break;
                    }
                }
                let start = target.buffer.selection().start();
                target.buffer.set_selection(Selection::at(start));
                target.state.mode = Mode::Normal;
            }
            Act::Repeat => self.repeat(target, command.count),
            Act::ToggleCaseChar => {
                let buffer = &mut *target.buffer;
                let len = buffer.line_len(head.line);
                if len == 0 {
                    return;
                }
                let end = Position::new(head.line, (head.column + times).min(len));
                let converted = operator::convert(&buffer.text_in(head..end), Operator::ToggleCase);
                buffer.grouped(|buffer| buffer.replace(head..end, &converted));
                buffer.set_selection(Selection::at(end));
            }
            Act::Visual(mode) => {
                let state = &mut *target.state;
                match state.mode {
                    current if current == mode => return self.leave_visual(target),
                    Mode::Normal => {
                        state.anchor = head;
                        state.head = head;
                    }
                    _ => {}
                }
                state.mode = mode;
                show_visual(state, target.buffer);
            }
            Act::VisualAgain => {
                let Some((mode, anchor, head)) = target.state.last_visual else {
                    return;
                };
                let state = &mut *target.state;
                state.mode = mode;
                state.anchor = target.buffer.clamped(anchor);
                state.head = target.buffer.clamped(head);
                show_visual(state, target.buffer);
            }
            Act::SwapEnds => {
                let state = &mut *target.state;
                std::mem::swap(&mut state.anchor, &mut state.head);
                show_visual(state, target.buffer);
            }
            Act::Mark(name) => {
                target.state.marks.insert(name, head);
            }
            Act::Record(name) if name.is_ascii_alphanumeric() => {
                self.recording = Some((name.to_ascii_lowercase(), Vec::new()));
            }
            Act::Record(_) => {}
            Act::StopRecording => {
                if let Some((name, mut keys)) = self.recording.take() {
                    keys.pop();
                    self.macros.insert(name, keys);
                }
            }
            Act::Play(name) => {
                let name = match name {
                    '@' => self.last_macro,
                    name => Some(name.to_ascii_lowercase()),
                };
                let Some(keys) = name.and_then(|name| self.macros.get(&name).cloned()) else {
                    return;
                };
                self.last_macro = name;
                self.playing += 1;
                for _ in 0..times {
                    for key in &keys {
                        self.feed(target, *key);
                    }
                }
                self.playing -= 1;
            }
            Act::Scroll(placement) => target.effects.push(Effect::Scroll(placement)),
            Act::Window(window) => target.effects.push(Effect::Window(window)),
            Act::Ex(line) => self.ex(target, &line),
            Act::Quit { write } => {
                if write {
                    target.effects.push(Effect::Save);
                }
                target.effects.push(Effect::Close);
            }
        }
    }

    /// Replaces `times` characters from the cursor with `ch`, or every
    /// character of the selection.
    fn replace_chars(&mut self, target: &mut Stage, ch: char, times: usize) {
        let buffer = &mut *target.buffer;
        if target.state.mode.is_visual() {
            let span = visual_span(target.state, buffer, false);
            let range = match span.linewise {
                true => {
                    Position::new(span.start.line, 0)
                        ..Position::new(span.end.line, buffer.line_len(span.end.line))
                }
                false => span.start..span.end,
            };
            let replaced = buffer
                .text_in(range.clone())
                .chars()
                .map(|old| if old == '\n' { old } else { ch })
                .collect::<String>();
            buffer.grouped(|buffer| buffer.replace(range.clone(), &replaced));
            buffer.set_selection(Selection::at(range.start));
            target.state.mode = Mode::Normal;
            return;
        }
        let head = buffer.selection().head;
        if head.column + times > buffer.line_len(head.line) {
            self.change = None;
            return;
        }
        let end = Position::new(head.line, head.column + times);
        match ch {
            '\n' => {
                buffer.grouped(|buffer| buffer.replace(head..end, "\n"));
            }
            _ => {
                let text = ch.to_string().repeat(times);
                buffer.grouped(|buffer| buffer.replace(head..end, &text));
                buffer.set_selection(Selection::at(Position::new(head.line, end.column - 1)));
            }
        }
    }

    /// Puts a register's text in after the cursor, or before it, or in
    /// place of the selection.
    fn paste(&mut self, target: &mut Stage, register: Option<char>, before: bool, times: usize) {
        let Some(text) = self.registers.read(register, &mut *target.clipboard) else {
            return;
        };
        let linewise = text.ends_with('\n');
        let repeated = text.repeat(times);
        let buffer = &mut *target.buffer;

        if target.state.mode.is_visual() {
            let span = visual_span(target.state, buffer, false);
            target.state.mode = Mode::Normal;
            let mut store = Store {
                registers: &mut self.registers,
                register: None,
                clipboard: &mut *target.clipboard,
            };
            let depth = buffer.undo_depth();
            let at = operator::delete(buffer, span, &mut store);
            let (at, inserted) = match (span.linewise, linewise) {
                (true, _) => (Position::new(span.start.line, 0), with_break(&repeated)),
                (false, true) => (
                    at,
                    format!("\n{}", repeated.strip_suffix('\n').unwrap_or(&repeated)),
                ),
                (false, false) => (at, repeated),
            };
            let at = match span.linewise && span.start.line >= buffer.line_count() {
                true => Position::new(buffer.line_count().saturating_sub(1), 0),
                false => at,
            };
            buffer.grouped(|buffer| buffer.replace(at..at, &inserted));
            buffer.squash_since(depth);
            buffer.set_selection(Selection::at(at));
            return;
        }

        let head = buffer.selection().head;
        if linewise {
            let (at, inserted, line) = match (before, head.line + 1 < buffer.line_count()) {
                (true, _) => (Position::new(head.line, 0), repeated, head.line),
                (false, true) => (Position::new(head.line + 1, 0), repeated, head.line + 1),
                (false, false) => (
                    Position::new(head.line, buffer.line_len(head.line)),
                    format!("\n{}", repeated.strip_suffix('\n').unwrap_or(&repeated)),
                    head.line + 1,
                ),
            };
            buffer.grouped(|buffer| buffer.replace(at..at, &inserted));
            let to = Position::new(line, first_non_blank(buffer, line));
            buffer.set_selection(Selection::at(to));
            return;
        }
        let at = match before || buffer.line_len(head.line) == 0 {
            true => head,
            false => Position::new(head.line, head.column + 1),
        };
        buffer.grouped(|buffer| buffer.replace(at..at, &repeated));
        let end = at.after(&repeated);
        let to = match repeated.contains('\n') {
            true => at,
            false => Position::new(end.line, end.column.saturating_sub(1)),
        };
        buffer.set_selection(Selection::at(to));
    }

    /// Makes the last change again, `count` times over when one is given.
    fn repeat(&mut self, target: &mut Stage, count: Option<usize>) {
        let Some(keys) = self.last_change.clone() else {
            return;
        };
        let keys = recounted(keys, count);
        self.repeating = true;
        for key in keys {
            self.feed(target, key);
        }
        self.repeating = false;
    }

    /// Runs a command line: writing, closing, splitting, a line to go to or
    /// a substitution.
    fn ex(&mut self, target: &mut Stage, line: &str) {
        let line = line.trim();
        let effects: &[Effect] = match line {
            "w" | "write" => &[Effect::Save],
            "wa" | "wall" => &[Effect::SaveAll],
            "q" | "q!" | "quit" | "quit!" | "clo" | "close" => &[Effect::Close],
            "wq" | "wq!" | "x" | "xit" => &[Effect::Save, Effect::Close],
            "sp" | "split" => &[Effect::Window(Window::SplitDown)],
            "vs" | "vsp" | "vsplit" => &[Effect::Window(Window::SplitRight)],
            _ => &[],
        };
        if !effects.is_empty() {
            target.effects.extend_from_slice(effects);
            target.state.mode = Mode::Normal;
            return;
        }
        if let Ok(number) = line.parse::<usize>() {
            let buffer = &mut *target.buffer;
            let line = number
                .saturating_sub(1)
                .min(buffer.line_count().saturating_sub(1));
            buffer.set_selection(Selection::at(Position::new(
                line,
                first_non_blank(buffer, line),
            )));
            target.state.mode = Mode::Normal;
            return;
        }
        let visual = target
            .state
            .mode
            .is_visual()
            .then(|| visual_span(target.state, target.buffer, true));
        target.state.mode = Mode::Normal;
        substitute(target.buffer, line, visual);
    }

    /// Leaves visual mode, with the cursor where the selection's far end was.
    fn leave_visual(&mut self, target: &mut Stage) {
        let state = &mut *target.state;
        state.last_visual = Some((state.mode, state.anchor, state.head));
        state.mode = Mode::Normal;
        let head = on_char(target.buffer, state.head);
        target.buffer.set_selection(Selection::at(head));
    }
}

/// Whether `command` changes the text, and so is what `.` repeats.
fn changes_text(command: &Command) -> bool {
    match &command.kind {
        Kind::Operate(operator, _) => operator.changes(),
        Kind::Act(act) => matches!(
            act,
            Act::Insert(_)
                | Act::ReplaceMode
                | Act::ReplaceChar(_)
                | Act::Join { .. }
                | Act::Paste { .. }
                | Act::ToggleCaseChar
        ),
        _ => false,
    }
}

/// `keys` with their count replaced by `count`, when one is given.
fn recounted(keys: Vec<Keystroke>, count: Option<usize>) -> Vec<Keystroke> {
    let Some(count) = count else {
        return keys;
    };
    let register = match keys.first().and_then(|key| key.char()) {
        Some('"') => 2.min(keys.len()),
        _ => 0,
    };
    let digits = keys[register..]
        .iter()
        .take_while(|key| key.char().is_some_and(|ch| ch.is_ascii_digit()))
        .count();
    let digits_typed = count.to_string();
    let counted = digits_typed
        .chars()
        .map(|ch| Keystroke::plain(Key::Char(ch)));
    keys[..register]
        .iter()
        .copied()
        .chain(counted)
        .chain(keys[register + digits..].iter().copied())
        .collect()
}

/// Types `key` into `buffer` the way the window would have, for keys that
/// are replayed with no window behind them.
fn type_key(buffer: &mut Buffer, key: Keystroke) {
    if key.ctrl {
        return;
    }
    match key.key {
        Key::Char(ch) => buffer.insert_typed(ch),
        Key::Enter => buffer.insert_newline(),
        Key::Backspace => buffer.backspace(),
        Key::Delete => buffer.delete(),
        Key::Tab => buffer.insert_indent(),
        Key::Left => buffer.move_cursor(Step::Left, false),
        Key::Right => buffer.move_cursor(Step::Right, false),
        Key::Up => buffer.move_cursor(Step::Up, false),
        Key::Down => buffer.move_cursor(Step::Down, false),
        Key::Home => buffer.move_cursor(Step::LineStart, false),
        Key::End => buffer.move_cursor(Step::LineEnd, false),
        Key::Escape | Key::PageUp | Key::PageDown => {}
    }
}

/// Brings the cursor back onto a character after a command, as normal
/// mode keeps it, or shows the selection in visual mode.
fn settle(state: &mut State, buffer: &mut Buffer) {
    match state.mode {
        Mode::Normal => {
            let head = on_char(buffer, buffer.selection().head);
            if buffer.selection() != Selection::at(head) {
                buffer.set_selection(Selection::at(head));
            }
            state.shown = None;
        }
        mode if mode.is_visual() => show_visual(state, buffer),
        _ => {}
    }
}

/// The last character a selection covers, which is where the cursor is.
fn before_end(buffer: &Buffer, selection: Selection) -> Position {
    if selection.head <= selection.anchor {
        return selection.head;
    }
    let offset = buffer.char_of(selection.head).saturating_sub(1);
    buffer.position_of(offset)
}

/// The span the selection covers, as whole lines when `lines`.
fn visual_span(state: &State, buffer: &Buffer, lines: bool) -> Span {
    let (start, end) = (state.anchor.min(state.head), state.anchor.max(state.head));
    match state.mode == Mode::VisualLine || lines {
        true => Span::lines(start.line, end.line),
        false => Span::chars(start, text::after(buffer, end)),
    }
}

/// Puts the selection visual mode stands for on the buffer.
///
/// A selection of whole lines is put on as its two ends where they are, so
/// the cursor stays in the column it was in; the window draws it as the
/// whole lines it stands for, since [`State::mode`] says it is linewise.
fn show_visual(state: &mut State, buffer: &mut Buffer) {
    if state.mode == Mode::VisualLine {
        buffer.set_selection(Selection {
            anchor: state.anchor,
            head: state.head,
        });
        state.shown = Some(buffer.selection());
        return;
    }
    let span = visual_span(state, buffer, false);
    let (start, end) = (span.start, span.end);
    let selection = match state.head < state.anchor {
        true => Selection {
            anchor: end,
            head: start,
        },
        false => Selection {
            anchor: start,
            head: end,
        },
    };
    buffer.set_selection(selection);
    state.shown = Some(buffer.selection());
}

/// Grows the selection to `object`.
fn select(target: &mut Stage, object: Object, around: bool) {
    let Some(span) = object.span(target.buffer, target.state.head, around) else {
        return;
    };
    let state = &mut *target.state;
    match span.linewise {
        true => {
            state.mode = Mode::VisualLine;
            state.anchor = span.start;
            state.head = span.end;
        }
        false => {
            state.anchor = span.start;
            state.head = before_end(
                target.buffer,
                Selection {
                    anchor: span.start,
                    head: span.end,
                },
            );
        }
    }
    show_visual(state, target.buffer);
}

/// Joins `times` lines from the cursor's, at least two, with a space
/// between them for `J`.
fn join(target: &mut Stage, times: usize, spaces: bool) {
    let buffer = &mut *target.buffer;
    let (first, count) = match target.state.mode.is_visual() {
        true => {
            let span = visual_span(target.state, buffer, true);
            target.state.mode = Mode::Normal;
            (span.start.line, span.end.line - span.start.line + 1)
        }
        false => (buffer.selection().head.line, times),
    };
    let joins = count.max(2) - 1;
    if first + 1 >= buffer.line_count() {
        return;
    }
    buffer.grouped(|buffer| {
        for _ in 0..joins {
            if first + 1 >= buffer.line_count() {
                break;
            }
            let end = Position::new(first, buffer.line_len(first));
            match spaces {
                true => {
                    buffer.set_selection(Selection::at(end));
                    buffer.join_lines();
                }
                false => {
                    buffer.replace(end..Position::new(first + 1, 0), "");
                    buffer.place(end, false);
                }
            }
        }
    });
}

/// `text` ending in a line break, as a line put back must.
fn with_break(text: &str) -> String {
    match text.ends_with('\n') {
        true => text.to_owned(),
        false => format!("{text}\n"),
    }
}

/// Runs `:s/old/new/`, on the cursor's line, on every line after `%`, or on
/// the lines of `selection`.
///
/// The pattern is literal text; `g` replaces every match on a line rather
/// than the first.
fn substitute(buffer: &mut Buffer, line: &str, selection: Option<Span>) {
    let (whole, rest) = match line.strip_prefix('%') {
        Some(rest) => (true, rest),
        None => (false, line.strip_prefix("'<,'>").unwrap_or(line)),
    };
    let Some(rest) = rest
        .strip_prefix("s")
        .or_else(|| rest.strip_prefix("substitute"))
    else {
        return;
    };
    let mut chars = rest.chars();
    let Some(separator) = chars.next().filter(|ch| !ch.is_alphanumeric()) else {
        return;
    };
    let parts = chars.as_str().split(separator).collect::<Vec<_>>();
    let (Some(old), Some(new)) = (parts.first(), parts.get(1)) else {
        return;
    };
    if old.is_empty() {
        return;
    }
    let every = parts.get(2).is_some_and(|flags| flags.contains('g'));
    let head = buffer.selection().head.line;
    let (first, last) = match (whole, selection) {
        (true, _) => (0, buffer.line_count().saturating_sub(1)),
        (false, Some(span)) => span.line_range(),
        (false, None) => (head, head),
    };
    let edits = (first..=last)
        .filter_map(|line| {
            let text = buffer.line_text(line);
            let replaced = match every {
                true => text.replace(old, new),
                false => text.replacen(old, new, 1),
            };
            (replaced != text).then(|| {
                (
                    Position::new(line, 0)..Position::new(line, buffer.line_len(line)),
                    replaced,
                )
            })
        })
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return;
    }
    let to = edits[edits.len() - 1].0.start.line;
    buffer.apply_edits(edits);
    buffer.set_selection(Selection::at(Position::new(
        to,
        first_non_blank(buffer, to),
    )));
}
