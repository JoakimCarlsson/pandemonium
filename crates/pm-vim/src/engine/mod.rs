//! The engine: keys in, a buffer changed, and what the window must do.
//!
//! [`State`] is what one buffer is in the middle of — its mode, the keys
//! gathered towards a command, its selection and marks. [`Vim`] is what the
//! whole window shares: the bindings, the registers, the last change, the
//! last search, the macros. Every key goes through [`Vim::press`]: it is
//! looked up in the bindings for the situation editing is in, and the
//! action it names is carried out here or in the module for its kind.

mod cursors;
mod edit;
mod ex;
mod repeat;
mod typing;
mod visual;

use std::collections::HashMap;
use std::ops::Range;

use pm_text::{Buffer, Position, Selection};

use crate::action::{Action, Command, Placement, Waiting};
use crate::key::{Key, Keystroke};
use crate::keymap::{BadBinding, Keymap, Situation};
use crate::mode::{Mode, Shape};
use crate::motion::{Find, View};
use crate::operator::{Operator, Span};
use crate::register::{Clipboard, ClipboardUse, Registers};
use crate::search::{LastSearch, Pattern};

pub(crate) use repeat::Change;

/// Something the window has to do because of a key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Carry out one of the window's own commands.
    Command(Command),
    /// Scroll the cursor's line to a place in the view.
    Scroll(Placement),
    /// Scroll the view by this many lines, down when positive.
    ScrollBy(isize),
    /// Scroll the view by this many columns, right when positive.
    ScrollColumns(isize),
    /// The cursor jumped away from here, for the jump list to take down.
    Jumped(Position),
}

/// What became of a key.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Outcome {
    /// Whether the key was taken; one that was not is the window's to handle.
    pub handled: bool,
    /// What the window has to do.
    pub effects: Vec<Effect>,
}

/// A line being typed at the bottom of the window: a search or a command.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Line {
    /// What it is for.
    kind: LineKind,
    /// What has been typed.
    text: String,
}

/// What a typed line is for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum LineKind {
    /// A search, forward for `/` and back for `?`.
    Search { forward: bool },
    /// A command line, after `:`.
    #[default]
    Command,
}

impl Line {
    /// The character the line is shown after.
    fn prompt(&self) -> char {
        match self.kind {
            LineKind::Search { forward: true } => '/',
            LineKind::Search { forward: false } => '?',
            LineKind::Command => ':',
        }
    }
}

/// The keys gathered towards a command, and what they have said so far.
#[derive(Clone, Debug, Default)]
struct Pending {
    /// The keys of a binding not yet told apart from a longer one.
    keys: Vec<Keystroke>,
    /// Every key of the command except its counts, for `.` to play again.
    typed: Vec<Keystroke>,
    /// Every key of the command, for the status bar.
    shown: Vec<Keystroke>,
    /// The count typed before the command.
    pre: Option<usize>,
    /// The count typed after its operator.
    post: Option<usize>,
    /// The register named with `"`.
    register: Option<char>,
    /// The operator waiting for a motion or an object.
    operator: Option<Operator>,
    /// Whether `v` turned the motion's kind over.
    forced: bool,
    /// Whether `i` or `a` was typed, and which.
    object: Option<bool>,
    /// What the next character is for.
    waiting: Option<Waiting>,
    /// The line being typed, for `/`, `?` and `:`.
    line: Option<Line>,
    /// The spans `ys` found, waiting for the delimiters to put round them.
    surrounding: Vec<Span>,
}

impl Pending {
    /// The count the command was given, the two counts multiplied.
    fn count(&self) -> Option<usize> {
        match (self.pre, self.post) {
            (None, None) => None,
            (pre, post) => Some(pre.unwrap_or(1).saturating_mul(post.unwrap_or(1))),
        }
    }

    /// Whether nothing has been typed towards a command.
    fn is_empty(&self) -> bool {
        self.shown.is_empty() && self.keys.is_empty()
    }
}

/// One visual selection: where it began and where its cursor is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Region {
    /// The end that stays put.
    pub anchor: Position,
    /// The end the cursor is on.
    pub head: Position,
}

/// A stretch of typing begun by a command, and what ending it must do.
#[derive(Clone, Debug, Default)]
pub(crate) struct Typing {
    /// How deep the undo history was before the command, to join it into one step.
    pub depth: usize,
    /// How many times the typing is made in all.
    pub count: usize,
    /// Whether the typing opens a new line each time it is made again.
    pub opens: Option<bool>,
    /// The keys typed so far.
    pub keys: Vec<Keystroke>,
    /// What each character typed over in replace mode stood on, for Backspace.
    pub replaced: Vec<Option<char>>,
    /// Whether the cursors go back to one when the typing ends, as after
    /// typing into a block.
    pub collapse: bool,
}

/// What one buffer is in the middle of.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The mode it is in.
    mode: Mode,
    /// The keys gathered towards a command.
    pending: Pending,
    /// The selections, in visual mode; the last is the primary one.
    regions: Vec<Region>,
    /// The selections last put on the buffer, to notice the pointer
    /// changing them.
    shown: Option<Vec<Selection>>,
    /// The column vertical motions aim at.
    goal: Option<usize>,
    /// The marks set in the buffer.
    marks: HashMap<char, Position>,
    /// The last selection, for `gv`.
    last_visual: Option<(Mode, Vec<Region>)>,
    /// The typing under way, in insert or replace mode.
    typing: Option<Typing>,
    /// Whether normal mode is only for one command, after Ctrl-O.
    temporary: bool,
    /// The span a first `cx` marked, for the second to swap with.
    exchange: Option<Span>,
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
            _ if self.pending.operator.is_some() && self.pending.waiting.is_none() => {
                Shape::Underline
            }
            _ => Shape::Block,
        }
    }

    /// What has been typed towards a command, as the status bar shows it:
    /// the line being typed, or the keys so far.
    pub fn pending(&self) -> Option<String> {
        if let Some(line) = &self.pending.line {
            return Some(format!("{}{}", line.prompt(), line.text));
        }
        let keys = self
            .pending
            .shown
            .iter()
            .chain(&self.pending.keys)
            .map(|key| key.label())
            .collect::<String>();
        let temporary = if self.temporary { "(insert) " } else { "" };
        match (keys.is_empty(), self.temporary) {
            (true, false) => None,
            _ => Some(format!("{temporary}{keys}")),
        }
    }

    /// The situation the bindings are read in.
    fn situation(&self) -> Situation {
        Situation {
            mode: self.mode,
            operator: self.pending.operator,
            object: self.pending.object.is_some(),
            count: self.pending.pre.is_some() || self.pending.post.is_some(),
        }
    }
}

/// What the window shares between buffers.
#[derive(Debug)]
pub struct Vim {
    /// The bindings in force.
    keymap: Keymap,
    /// Every register.
    registers: Registers,
    /// The last character search, for `;` and `,`.
    last_find: Option<Find>,
    /// The last search, for `n` and `N`.
    last_search: Option<LastSearch>,
    /// Whether the last search's matches are lit, until `:noh`.
    highlight: bool,
    /// The last change, for `.`.
    last_change: Option<Change>,
    /// The change being made, until it is finished.
    change: Option<Change>,
    /// The register being recorded into and the keys recorded so far.
    recording: Option<(char, Vec<Keystroke>)>,
    /// The keys recorded into each register.
    macros: HashMap<char, Vec<Keystroke>>,
    /// The register last played, for `@@` and `Q`.
    last_macro: Option<char>,
    /// The last substitution, for `:&` and `&`.
    last_substitute: Option<String>,
    /// Whether `.` is replaying a change.
    repeating: bool,
    /// How many macros deep the keys being fed come from.
    playing: usize,
}

impl Default for Vim {
    /// Modal editing with Zed's bindings and nothing yet in any register.
    fn default() -> Self {
        Self {
            keymap: Keymap::default(),
            registers: Registers::default(),
            last_find: None,
            last_search: None,
            highlight: true,
            last_change: None,
            change: None,
            recording: None,
            macros: HashMap::new(),
            last_macro: None,
            last_substitute: None,
            repeating: false,
            playing: 0,
        }
    }
}

/// Everything a key is carried out against.
pub(crate) struct Stage<'a> {
    /// The buffer's state.
    pub state: &'a mut State,
    /// The buffer.
    pub buffer: &'a mut Buffer,
    /// The part of the buffer the pane shows.
    pub view: View,
    /// The system clipboard.
    pub clipboard: &'a mut dyn Clipboard,
    /// What the window has to do.
    pub effects: &'a mut Vec<Effect>,
}

impl Vim {
    /// The register being recorded into, if one is.
    pub fn recording(&self) -> Option<char> {
        self.recording.as_ref().map(|(name, _)| *name)
    }

    /// Puts the bindings back to Zed's, forgetting the reader's own.
    pub fn reset_bindings(&mut self) {
        self.keymap = Keymap::default();
    }

    /// Adds the reader's binding of `keys` to `action` when `when` holds, on
    /// top of those in force.
    pub fn bind(&mut self, keys: &str, action: &str, when: &str) -> Result<(), BadBinding> {
        self.keymap.bind(keys, action, when)
    }

    /// Shares the unnamed register with the system clipboard as `sharing` says.
    pub fn share_clipboard(&mut self, sharing: ClipboardUse) {
        self.registers.share(sharing);
    }

    /// The matches to light on `lines`: of the search being typed, or of the
    /// last search until `:noh`.
    pub fn matches(
        &self,
        state: &State,
        buffer: &Buffer,
        lines: Range<usize>,
    ) -> Vec<Range<Position>> {
        let typing = state
            .pending
            .line
            .as_ref()
            .and_then(|line| match line.kind {
                LineKind::Search { .. } => Some(Pattern::typed(&line.text)),
                LineKind::Command => None,
            });
        let pattern = typing.or_else(|| {
            self.highlight
                .then(|| {
                    self.last_search
                        .as_ref()
                        .map(|search| search.pattern.clone())
                })
                .flatten()
        });
        pattern.map_or_else(Vec::new, |pattern| pattern.matches_on(buffer, lines))
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
        let mut stage = Stage {
            state,
            buffer,
            view,
            clipboard,
            effects: &mut effects,
        };
        let handled = self.feed(&mut stage, key);
        Outcome { handled, effects }
    }

    /// Whether `.` or a macro is feeding the keys, rather than the reader.
    fn replaying(&self) -> bool {
        self.repeating || self.playing > 0
    }

    /// Sends `key` to whatever the buffer's mode does with keys.
    pub(crate) fn feed(&mut self, stage: &mut Stage, key: Keystroke) -> bool {
        match stage.state.mode {
            Mode::Insert => self.insert_key(stage, key),
            Mode::Replace => self.replace_key(stage, key),
            _ => self.command_key(stage, key),
        }
    }

    /// A key typed in normal or visual mode, gathered towards a command.
    fn command_key(&mut self, stage: &mut Stage, key: Keystroke) -> bool {
        self.follow_pointer(stage);
        if stage.state.pending.line.is_some() {
            self.line_key(stage, key);
            return true;
        }
        if let Some(waiting) = stage.state.pending.waiting {
            self.waiting_key(stage, waiting, key);
            return true;
        }

        let fresh = stage.state.pending.is_empty();
        stage.state.pending.keys.push(key);
        let situation = stage.state.situation();
        let lookup = self.keymap.lookup(&stage.state.pending.keys, &situation);
        if lookup.longer {
            return true;
        }
        let keys = std::mem::take(&mut stage.state.pending.keys);
        let Some(action) = lookup.exact else {
            let unclaimed = key.ctrl || !matches!(key.key, Key::Char(_));
            if fresh && keys.len() == 1 && unclaimed {
                return false;
            }
            self.cancel(stage);
            return true;
        };
        let pending = &mut stage.state.pending;
        if !matches!(action, Action::Number(_)) {
            pending.typed.extend(&keys);
        }
        pending.shown.extend(&keys);
        self.dispatch(stage, action)
    }

    /// A key typed at the line being typed for `/`, `?` or `:`.
    ///
    /// Backspace takes a character back, and taking back from nothing
    /// cancels, the way vim's own command line does; Enter runs the line.
    fn line_key(&mut self, stage: &mut Stage, key: Keystroke) {
        let pending = &mut stage.state.pending;
        pending.typed.push(key);
        let Some(line) = pending.line.as_mut() else {
            return;
        };
        if key.is_escape() {
            return self.cancel(stage);
        }
        match key.key {
            Key::Enter => {
                let line = pending.line.take().unwrap_or_default();
                self.submit_line(stage, line);
            }
            Key::Backspace if line.text.pop().is_none() => self.cancel(stage),
            Key::Backspace => {}
            _ => line.text.extend(key.char()),
        }
    }

    /// Runs a line typed for `/`, `?` or `:`.
    fn submit_line(&mut self, stage: &mut Stage, line: Line) {
        match line.kind {
            LineKind::Search { forward } => {
                let text = match line.text.is_empty() {
                    true => self
                        .last_search
                        .as_ref()
                        .map(|search| search.pattern.source.clone())
                        .unwrap_or_default(),
                    false => line.text,
                };
                self.highlight = true;
                self.dispatch(
                    stage,
                    Action::Motion(crate::motion::Motion::Search { text, forward }),
                );
            }
            LineKind::Command => {
                self.ex(stage, &line.text);
                self.complete(stage);
            }
        }
    }

    /// Carries out what a binding named, answering whether the key it came
    /// from was taken.
    pub(crate) fn dispatch(&mut self, stage: &mut Stage, action: Action) -> bool {
        let pending = &mut stage.state.pending;
        match action {
            Action::Nothing => {}
            Action::Number(digit) => {
                let slot = match pending.operator {
                    Some(_) => &mut pending.post,
                    None => &mut pending.pre,
                };
                *slot = Some(slot.unwrap_or(0).saturating_mul(10).saturating_add(digit));
                return true;
            }
            Action::Push(operator) if stage.state.mode.is_visual() => {
                self.visual_operator(stage, operator, false);
                if stage.state.pending.waiting.is_some() || stage.state.mode.is_typing() {
                    return true;
                }
            }
            Action::Push(operator) => {
                pending.operator = Some(operator);
                return true;
            }
            Action::PushObject { around } => {
                pending.object = Some(around);
                return true;
            }
            Action::Wait(waiting) => {
                pending.waiting = Some(waiting);
                return true;
            }
            Action::ForcedMotion => {
                pending.forced = !pending.forced;
                return true;
            }
            Action::Search { backwards } => {
                pending.line = Some(Line {
                    kind: LineKind::Search {
                        forward: !backwards,
                    },
                    text: String::new(),
                });
                return true;
            }
            Action::CommandLine => {
                let text = match (stage.state.mode.is_visual(), pending.count()) {
                    (true, _) => "'<,'>".to_owned(),
                    (false, Some(count)) if count > 1 => format!(".,.+{}", count - 1),
                    _ => String::new(),
                };
                pending.line = Some(Line {
                    kind: LineKind::Command,
                    text,
                });
                return true;
            }
            Action::CurrentLine => {
                if !self.operate_lines(stage) {
                    return true;
                }
            }
            Action::Motion(motion) => {
                if !self.motion(stage, motion) {
                    return true;
                }
            }
            Action::Object(object) => {
                if !self.object(stage, object) {
                    return true;
                }
            }
            Action::Act(crate::action::Act::TemporaryNormal) => {
                self.act(stage, crate::action::Act::TemporaryNormal);
                stage.state.pending = Pending::default();
                return true;
            }
            Action::Act(act) => {
                let handled = self.act(stage, act);
                if stage.state.pending.waiting.is_some() || stage.state.pending.line.is_some() {
                    return true;
                }
                self.complete(stage);
                return handled;
            }
            Action::App(command) => {
                stage.effects.push(Effect::Command(command));
                self.cancel(stage);
                return true;
            }
            Action::SendKeystrokes(keys) => {
                self.cancel(stage);
                for key in keys {
                    self.feed(stage, key);
                }
                return true;
            }
        }
        self.complete(stage);
        true
    }

    /// The character a key after a waiting key stands for.
    fn waiting_key(&mut self, stage: &mut Stage, waiting: Waiting, key: Keystroke) {
        if key.is_escape() {
            return self.cancel(stage);
        }
        let ch = match key.key {
            Key::Enter => Some('\n'),
            Key::Tab => Some('\t'),
            _ => key.char(),
        };
        let Some(ch) = ch else {
            return self.cancel(stage);
        };
        let pending = &mut stage.state.pending;
        pending.typed.push(key);
        pending.shown.push(key);
        pending.waiting = None;
        if self.finish_waiting(stage, waiting, ch) {
            self.complete(stage);
        }
    }

    /// Forgets the command being typed.
    fn cancel(&mut self, stage: &mut Stage) {
        stage.state.pending = Pending::default();
        if !self.repeating {
            self.change = None;
        }
        stage.state.temporary = stage.state.temporary && stage.state.mode == Mode::Normal;
    }

    /// Ends the command just carried out: keeps it for `.` when it changed
    /// the text, goes back to typing after Ctrl-O, and brings the cursor
    /// onto a character.
    fn complete(&mut self, stage: &mut Stage) {
        stage.state.pending = Pending::default();
        if stage.state.mode.is_typing() {
            return;
        }
        if !self.repeating
            && let Some(change) = self.change.take()
        {
            self.last_change = Some(change);
        }
        if stage.state.temporary && stage.state.mode == Mode::Normal {
            stage.state.temporary = false;
            if stage.state.goal == Some(usize::MAX) {
                let head = stage.buffer.selection().head;
                let end = Position::new(head.line, stage.buffer.line_len(head.line));
                stage.buffer.set_selection(Selection::at(end));
            }
            let depth = stage.buffer.undo_depth();
            self.begin_typing(stage, Mode::Insert, depth, 1, None);
            return;
        }
        self.settle(stage);
    }

    /// Begins keeping the command being carried out as the change `.`
    /// repeats, with the keys typed for it so far.
    pub(crate) fn begin_change(&mut self, stage: &Stage) {
        if self.repeating {
            return;
        }
        self.change = Some(Change {
            keys: stage.state.pending.typed.clone(),
            count: stage.state.pending.count(),
            visual: visual::shape_of(stage.state, stage.buffer),
        });
    }

    /// The count the command was given.
    pub(crate) fn count(&self, stage: &Stage) -> Option<usize> {
        stage.state.pending.count()
    }
}
