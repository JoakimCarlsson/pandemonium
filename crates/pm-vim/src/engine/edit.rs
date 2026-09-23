//! Changing the text: operators at every cursor, the commands that stand
//! for an operator and a target, putting registers back, joining lines,
//! replacing characters, counting numbers up and down, and the surround
//! commands.

use pm_text::{Buffer, Position, Selection};

use crate::action::{Act, Shorthand, Waiting, find};
use crate::engine::cursors::{heads, place_all};
use crate::engine::visual;
use crate::engine::{Effect, Stage, Vim};
use crate::format;
use crate::mode::Mode;
use crate::motion::{Kind, Motion};
use crate::object::Object;
use crate::operator::{self, Operator, Span, Store};
use crate::register::Filling;
use crate::surround;
use crate::text::{self, Class, class, first_non_blank, last_column, on_char};

/// What an operator in normal mode is given to act on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    /// The span a motion covers.
    Motion(Motion),
    /// A text object, its delimiters too when `around`.
    Object(Object, bool),
    /// Whole lines from the cursor's, as the operator doubled says.
    Lines,
    /// A span found some other way, such as the match `gn` selects.
    Span(Span),
}

impl Vim {
    /// Moves by `motion`, or finishes the operator waiting with it,
    /// answering whether the command is done.
    pub(crate) fn motion(&mut self, stage: &mut Stage, motion: Motion) -> bool {
        let count = self.count(stage);
        if let Some(operator) = stage.state.pending.operator {
            return self.operate(stage, operator, Target::Motion(motion), count);
        }
        match stage.state.mode.is_visual() {
            true => self.visual_motion(stage, &motion, count),
            false => self.normal_motion(stage, &motion, count),
        }
        true
    }

    /// Finishes `i` or `a` with `object`: the operator waiting acts on it,
    /// or the selection grows to it.
    pub(crate) fn object(&mut self, stage: &mut Stage, object: Object) -> bool {
        let around = stage.state.pending.object.take().unwrap_or(false);
        let count = self.count(stage);
        if let Some(operator) = stage.state.pending.operator {
            return self.operate(stage, operator, Target::Object(object, around), count);
        }
        if stage.state.mode.is_visual() {
            self.select_object(stage, object, around, count);
        }
        true
    }

    /// The operator doubled: it acts on whole lines from the cursor's.
    pub(crate) fn operate_lines(&mut self, stage: &mut Stage) -> bool {
        let Some(operator) = stage.state.pending.operator else {
            return true;
        };
        let count = self.count(stage);
        self.operate(stage, operator, Target::Lines, count)
    }

    /// Applies `operator` to what `target` names at every cursor, answering
    /// whether the command is done or waits for a character.
    pub(crate) fn operate(
        &mut self,
        stage: &mut Stage,
        operator: Operator,
        target: Target,
        count: Option<usize>,
    ) -> bool {
        let (places, primary) = heads(stage.buffer);
        let spans = places
            .iter()
            .filter_map(|from| {
                self.span_for(stage, operator, &target, *from, count)
                    .map(|span| (span, *from))
            })
            .collect::<Vec<_>>();
        match operator {
            Operator::Exchange => {
                if let Some((span, _)) = spans.first() {
                    self.begin_change(stage);
                    self.exchange(stage, *span);
                }
                return true;
            }
            Operator::AddSurround => {
                if spans.is_empty() {
                    self.cancel(stage);
                    return true;
                }
                let pending = &mut stage.state.pending;
                pending.surrounding = spans.into_iter().map(|(span, _)| span).collect();
                pending.operator = None;
                pending.waiting = Some(Waiting::AddSurround);
                return false;
            }
            _ => {}
        }
        if operator.changes() {
            self.begin_change(stage);
        }
        let depth = stage.buffer.undo_depth();
        if spans.is_empty() {
            if operator == Operator::Change {
                self.begin_typing(stage, Mode::Insert, depth, 1, None);
            }
            return true;
        }
        let landed = self.apply(stage, operator, &spans, false);
        let to = landed.get(primary).or(landed.first()).copied();
        if let Some(to) = to {
            stage.state.marks.insert('.', to);
        }
        place_all(stage.buffer, landed, primary);
        if operator == Operator::Change {
            self.begin_typing(stage, Mode::Insert, depth, 1, None);
            return true;
        }
        stage.buffer.squash_since(depth);
        true
    }

    /// The span `target` names for `operator` from a cursor at `from`.
    fn span_for(
        &mut self,
        stage: &Stage,
        operator: Operator,
        target: &Target,
        from: Position,
        count: Option<usize>,
    ) -> Option<Span> {
        let times = count.unwrap_or(1).max(1);
        let last = stage.buffer.line_count().saturating_sub(1);
        let span = match target {
            Target::Lines => Some(Span::lines(from.line, (from.line + times - 1).min(last))),
            Target::Object(object, around) => object.span(stage.buffer, from, *around, times),
            Target::Motion(motion) => self.motion_span(stage, operator, motion, from, count),
            Target::Span(span) => Some(*span),
        }?;
        Some(match operator.is_linewise() {
            true => Span::lines(span.start.line, span.end.line),
            false => span,
        })
    }

    /// The span `motion` covers from `from` for `operator`.
    ///
    /// `cw` on a word changes to its end rather than to the next word's
    /// start, and `dw` on the last word of a line stops at the line's end
    /// rather than joining the next: vim's two exceptions to its own rule.
    /// A `v` typed after the operator turns the motion's kind over.
    fn motion_span(
        &mut self,
        stage: &Stage,
        operator: Operator,
        motion: &Motion,
        from: Position,
        count: Option<usize>,
    ) -> Option<Span> {
        let buffer = &*stage.buffer;
        let forced = stage.state.pending.forced;
        if let Motion::NextWordStart { big } = motion
            && operator == Operator::Change
            && !forced
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
        let goal = stage.state.goal;
        let mut moved = self.resolve(stage, motion, from, count, goal)?;
        let buffer = &*stage.buffer;
        if forced {
            moved.kind = match moved.kind {
                Kind::Linewise | Kind::Inclusive => Kind::Exclusive,
                Kind::Exclusive => Kind::Inclusive,
            };
            if matches!(motion, Motion::Up | Motion::Down) {
                moved.to = Position::new(
                    moved.to.line,
                    from.column.min(buffer.line_len(moved.to.line)),
                );
            }
            return Some(Span::of_motion(buffer, from, moved));
        }
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
        if motion.is_jump() {
            return Some(Span::of_motion(buffer, from, moved));
        }
        Some(Span::of_motion(buffer, from, moved))
    }

    /// Carries `operator` out on every one of `spans`, each with the cursor
    /// it was found from, answering where each cursor lands.
    ///
    /// The registers are filled once, with a piece from every span, so that
    /// yanking at three cursors and putting back at three puts each back
    /// where it belongs.
    pub(crate) fn apply(
        &mut self,
        stage: &mut Stage,
        operator: Operator,
        spans: &[(Span, Position)],
        block: bool,
    ) -> Vec<Position> {
        let register = stage.state.pending.register;
        let source = match operator {
            Operator::ReplaceWithRegister => self
                .registers
                .read(register, &mut *stage.clipboard)
                .map(|held| held.text),
            _ => None,
        };
        let mut store = Store {
            registers: &mut self.registers,
            register,
            clipboard: &mut *stage.clipboard,
            pieces: Vec::new(),
            filling: None,
            source,
            wrap: stage.view.wrap.max(1),
            block,
        };
        if operator == Operator::Yank {
            let mut landed = Vec::new();
            for (span, from) in spans.iter().rev() {
                landed.push(operator.apply(stage.buffer, *span, *from, &mut store));
            }
            store.finish();
            landed.reverse();
            return landed;
        }
        let spans = spans.iter().map(|(span, _)| *span).collect::<Vec<_>>();
        let landed = each_span(stage.buffer, &spans, |buffer, span| {
            operator.apply(buffer, span, span.start, &mut store)
        });
        store.finish();
        landed
    }

    /// Applies the operator and the target a one-key command stands for.
    fn shorthand(&mut self, stage: &mut Stage, shorthand: Shorthand) {
        let (operator, target) = match shorthand {
            Shorthand::DeleteRight => (Operator::Delete, Target::Motion(Motion::Right)),
            Shorthand::DeleteLeft => (Operator::Delete, Target::Motion(Motion::Left)),
            Shorthand::Substitute => (Operator::Change, Target::Motion(Motion::Right)),
            Shorthand::SubstituteLine => (Operator::Change, Target::Lines),
            Shorthand::ChangeToEndOfLine => (Operator::Change, Target::Motion(Motion::LineEnd)),
            Shorthand::DeleteToEndOfLine => (Operator::Delete, Target::Motion(Motion::LineEnd)),
            Shorthand::YankLine => (Operator::Yank, Target::Lines),
        };
        let count = self.count(stage);
        self.operate(stage, operator, target, count);
    }

    /// Carries out a command that is neither a motion nor an operator,
    /// answering whether the key it came from was taken.
    pub(crate) fn act(&mut self, stage: &mut Stage, act: Act) -> bool {
        let times = self.count(stage).unwrap_or(1).max(1);
        match act {
            Act::NormalMode => return self.normal_mode(stage),
            Act::InsertBefore
            | Act::InsertAfter
            | Act::InsertFirstNonBlank
            | Act::InsertEndOfLine
            | Act::InsertLineBelow
            | Act::InsertLineAbove => self.insert(stage, &act, times),
            Act::InsertAtPrevious => {
                let at = stage.state.marks.get(&'^').copied();
                if let Some(at) = at {
                    stage
                        .buffer
                        .set_selection(Selection::at(stage.buffer.clamped(at)));
                }
                self.insert(stage, &Act::InsertBefore, 1);
            }
            Act::InsertEmptyLineBelow | Act::InsertEmptyLineAbove => {
                self.begin_change(stage);
                let below = act == Act::InsertEmptyLineBelow;
                let breaks = "\n".repeat(times);
                let (places, primary) = heads(stage.buffer);
                stage.buffer.grouped(|buffer| {
                    buffer.at_each(|buffer| {
                        let head = buffer.selection().head;
                        let at = match below {
                            true => Position::new(head.line, buffer.line_len(head.line)),
                            false => Position::new(head.line, 0),
                        };
                        buffer.replace(at..at, &breaks);
                        buffer.set_selection(Selection::at(head));
                    });
                });
                let shifted = places
                    .iter()
                    .enumerate()
                    .map(|(index, place)| match below {
                        true => Position::new(place.line + index * times, place.column),
                        false => Position::new(place.line + (index + 1) * times, place.column),
                    })
                    .collect();
                place_all(stage.buffer, shifted, primary);
            }
            Act::ToggleVisual(mode) => self.toggle_visual(stage, mode),
            Act::RestoreVisual => self.restore_visual(stage),
            Act::OtherEnd => self.other_end(stage, false),
            Act::OtherEndRowAware => self.other_end(stage, true),
            Act::ToggleReplace => {
                self.begin_change(stage);
                let depth = stage.buffer.undo_depth();
                self.begin_typing(stage, Mode::Replace, depth, times, None);
            }
            Act::Shorthand(shorthand) => self.shorthand(stage, shorthand),
            Act::VisualLines(operator) => self.visual_operator(stage, operator, true),
            Act::JoinLines { spaces } => self.join(stage, times, spaces),
            Act::Paste { before, preserve } => self.paste(stage, before, preserve, times),
            Act::Undo | Act::Redo => {
                for _ in 0..times {
                    let done = match act == Act::Undo {
                        true => stage.buffer.undo(),
                        false => stage.buffer.redo(),
                    };
                    if !done {
                        break;
                    }
                }
                let start = stage.buffer.selection().start();
                stage.buffer.set_selection(Selection::at(start));
                stage.state.mode = Mode::Normal;
                stage.state.regions.clear();
            }
            Act::Repeat => self.repeat(stage),
            Act::ChangeCase if stage.state.mode.is_visual() => {
                self.visual_operator(stage, Operator::ToggleCase, false);
            }
            Act::ChangeCase => {
                self.begin_change(stage);
                stage.buffer.grouped(|buffer| {
                    buffer.at_each(|buffer| {
                        let head = buffer.selection().head;
                        let len = buffer.line_len(head.line);
                        if len == 0 {
                            return;
                        }
                        let end = Position::new(head.line, (head.column + times).min(len));
                        let converted =
                            operator::convert(&buffer.text_in(head..end), Operator::ToggleCase);
                        buffer.replace(head..end, &converted);
                        buffer.set_selection(Selection::at(on_char(buffer, end)));
                    });
                });
            }
            Act::ConvertCase(operator) => self.visual_operator(stage, operator, false),
            Act::Increment { delta, step } => self.increment(stage, delta * times as i64, step),
            Act::VisualInsert { end, text } => self.visual_insert(stage, end, text),
            Act::ToggleRecord => match self.recording.take() {
                Some((name, mut keys)) => {
                    keys.pop();
                    self.macros.insert(name, keys);
                }
                None => stage.state.pending.waiting = Some(Waiting::Record),
            },
            Act::ReplayLastRecording => {
                if let Some(name) = self.last_macro {
                    self.play(stage, name, times);
                }
            }
            Act::Scroll(placement) => stage.effects.push(Effect::Scroll(placement)),
            Act::ScrollLines(lines) => self.scroll_lines(stage, lines * times as isize),
            Act::ScrollColumns(columns) => stage
                .effects
                .push(Effect::ScrollColumns(columns * times as isize)),
            Act::SelectMatch { forward } => match stage.state.pending.operator {
                Some(operator) => {
                    if let Some(span) = self.next_match(stage, forward) {
                        self.operate(stage, operator, Target::Span(span), None);
                    }
                }
                None => self.select_match(stage, forward),
            },
            Act::GoToTab { forward } => stage.effects.push(Effect::Command(match forward {
                true => crate::action::Command::NextTab,
                false => crate::action::Command::PreviousTab,
            })),
            Act::TemporaryNormal => {
                self.finish_typing(stage, false);
                stage.state.temporary = true;
            }
            Act::DeleteWordBefore => stage.buffer.at_each(Buffer::delete_word_left),
            Act::DeleteToLineStart => stage.buffer.at_each(|buffer| {
                let head = buffer.selection().head;
                let indent = first_non_blank(buffer, head.line);
                let start = if head.column > indent { indent } else { 0 };
                buffer.replace(Position::new(head.line, start)..head, "");
            }),
            Act::ShiftLine { indent } => stage.buffer.on_each_line(match indent {
                true => Buffer::indent_lines,
                false => Buffer::outdent_lines,
            }),
            Act::CopyFromLine { above } => stage.buffer.at_each(|buffer| {
                let head = buffer.selection().head;
                let line = match above {
                    true => head.line.checked_sub(1),
                    false => (head.line + 1 < buffer.line_count()).then_some(head.line + 1),
                };
                if let Some(ch) =
                    line.and_then(|line| buffer.char_at(Position::new(line, head.column)))
                {
                    buffer.insert(&ch.to_string());
                }
            }),
            Act::UndoReplace => self.undo_replace(stage),
            Act::SwitchTyping => {
                stage.state.mode = match stage.state.mode {
                    Mode::Insert => Mode::Replace,
                    _ => Mode::Insert,
                };
            }
        }
        true
    }

    /// Escape: out of visual mode, out of typing, out of what is half typed;
    /// in normal mode with nothing half typed, the window's own Escape.
    fn normal_mode(&mut self, stage: &mut Stage) -> bool {
        match stage.state.mode {
            Mode::Insert | Mode::Replace => self.finish_typing(stage, true),
            mode if mode.is_visual() => self.leave_visual(stage),
            _ if !stage.state.pending.shown.is_empty() => {}
            _ if stage.state.temporary => {}
            _ => {
                stage.buffer.collapse_cursors();
                return false;
            }
        }
        true
    }

    /// Begins typing at every cursor in the place `act` names, `times` over.
    fn insert(&mut self, stage: &mut Stage, act: &Act, times: usize) {
        self.begin_change(stage);
        let depth = stage.buffer.undo_depth();
        let opens = match act {
            Act::InsertLineBelow => Some(true),
            Act::InsertLineAbove => Some(false),
            _ => None,
        };
        let act = act.clone();
        stage.buffer.at_each(|buffer| {
            let head = buffer.selection().head;
            let line = head.line;
            match act {
                Act::InsertAfter => {
                    let column = (head.column + 1).min(buffer.line_len(line));
                    buffer.place(Position::new(line, column), false);
                }
                Act::InsertFirstNonBlank => {
                    buffer.place(Position::new(line, first_non_blank(buffer, line)), false)
                }
                Act::InsertEndOfLine => {
                    buffer.place(Position::new(line, buffer.line_len(line)), false)
                }
                Act::InsertLineBelow => buffer.insert_line_below(),
                Act::InsertLineAbove => buffer.insert_line_above(),
                _ => {}
            }
        });
        self.begin_typing(stage, Mode::Insert, depth, times, opens);
    }

    /// Scrolls the view by `lines`, keeping the cursor on it.
    fn scroll_lines(&mut self, stage: &mut Stage, lines: isize) {
        stage.effects.push(Effect::ScrollBy(lines));
        let view = stage.view;
        let last = stage.buffer.line_count().saturating_sub(1);
        let top = view.top.saturating_add_signed(lines).min(last);
        let margin = view.margin.min(view.rows.saturating_sub(1) / 2);
        let first = (top + margin).min(last);
        let bottom = (top + view.rows.saturating_sub(1))
            .saturating_sub(margin)
            .max(first);
        let head = stage.buffer.selection().head;
        let line = head.line.clamp(first, bottom.min(last));
        if line != head.line {
            let column = stage
                .state
                .goal
                .unwrap_or(head.column)
                .min(last_column(stage.buffer, line));
            stage
                .buffer
                .set_selection(Selection::at(Position::new(line, column)));
        }
    }

    /// Carries out what a waiting key was waiting for, now its character has
    /// come, answering whether the command is done.
    pub(crate) fn finish_waiting(&mut self, stage: &mut Stage, waiting: Waiting, ch: char) -> bool {
        let times = self.count(stage).unwrap_or(1).max(1);
        match waiting {
            Waiting::Find { forward, till } => return self.motion(stage, find(forward, till, ch)),
            Waiting::Replace => self.replace_chars(stage, ch, times),
            Waiting::Mark => {
                let head = match stage.state.regions.last() {
                    Some(region) => region.head,
                    None => stage.buffer.selection().head,
                };
                stage.state.marks.insert(ch, head);
            }
            Waiting::Jump { line } => {
                let name = match ch {
                    '`' | '\'' => '\'',
                    other => other,
                };
                return self.motion(stage, Motion::Mark { name, line });
            }
            Waiting::Register => {
                stage.state.pending.register = Some(ch);
                return false;
            }
            Waiting::Record if ch.is_ascii_alphanumeric() || ch == '"' => {
                self.recording = Some((ch.to_ascii_lowercase(), Vec::new()));
            }
            Waiting::Record => {}
            Waiting::Replay => {
                let name = match ch {
                    '@' => self.last_macro,
                    ':' => None,
                    name => Some(name.to_ascii_lowercase()),
                };
                if let Some(name) = name {
                    self.play(stage, name, times);
                }
            }
            Waiting::InsertRegister => {
                if let Some(held) = self.registers.read(Some(ch), &mut *stage.clipboard) {
                    stage.buffer.at_each(|buffer| buffer.insert(&held.text));
                }
            }
            Waiting::DeleteSurround => {
                self.begin_change(stage);
                self.at_cursors(stage, |buffer, at| surround::delete(buffer, at, ch));
            }
            Waiting::ChangeSurround => {
                stage.state.pending.waiting = Some(Waiting::ChangeSurroundTo(ch));
                return false;
            }
            Waiting::ChangeSurroundTo(from) => {
                self.begin_change(stage);
                self.at_cursors(stage, |buffer, at| surround::change(buffer, at, from, ch));
            }
            Waiting::AddSurround => {
                self.begin_change(stage);
                let spans = std::mem::take(&mut stage.state.pending.surrounding);
                let depth = stage.buffer.undo_depth();
                let mut landed = None;
                for span in spans.iter().rev() {
                    landed = surround::add(stage.buffer, *span, ch).or(landed);
                }
                stage.buffer.squash_since(depth);
                if let Some(at) = landed {
                    stage.buffer.set_selection(Selection::at(at));
                }
            }
        }
        true
    }

    /// Does `change` at every cursor, leaving each where it answers.
    fn at_cursors(
        &mut self,
        stage: &mut Stage,
        mut change: impl FnMut(&mut Buffer, Position) -> Option<Position>,
    ) {
        stage.buffer.grouped(|buffer| {
            buffer.at_each(|buffer| {
                let head = buffer.selection().head;
                if let Some(to) = change(buffer, head) {
                    buffer.set_selection(Selection::at(to));
                }
            });
        });
    }

    /// Replaces `times` characters from every cursor with `ch`, or every
    /// character of the selection.
    fn replace_chars(&mut self, stage: &mut Stage, ch: char, times: usize) {
        self.begin_change(stage);
        if stage.state.mode.is_visual() {
            let spans = visual::spans(stage.state, stage.buffer, false);
            stage.state.mode = Mode::Normal;
            stage.state.regions.clear();
            let first = spans.first().map(|span| span.start);
            stage.buffer.grouped(|buffer| {
                for span in spans.iter().rev() {
                    let range = span.body(buffer);
                    let replaced = buffer
                        .text_in(range.clone())
                        .chars()
                        .map(|old| if old == '\n' { old } else { ch })
                        .collect::<String>();
                    buffer.replace(range, &replaced);
                }
            });
            if let Some(first) = first {
                stage.buffer.set_selection(Selection::at(first));
            }
            return;
        }
        let short = heads(stage.buffer)
            .0
            .iter()
            .any(|head| head.column + times > stage.buffer.line_len(head.line));
        if short {
            self.change = None;
            return;
        }
        stage.buffer.grouped(|buffer| {
            buffer.at_each(|buffer| {
                let head = buffer.selection().head;
                let end = Position::new(head.line, head.column + times);
                match ch {
                    '\n' => buffer.replace(head..end, "\n"),
                    _ => {
                        buffer.replace(head..end, &ch.to_string().repeat(times));
                        buffer
                            .set_selection(Selection::at(Position::new(head.line, end.column - 1)));
                    }
                }
            });
        });
    }

    /// Puts a register's text in after every cursor, or before, or in
    /// place of every selection.
    fn paste(&mut self, stage: &mut Stage, before: bool, preserve: bool, times: usize) {
        let register = stage.state.pending.register;
        let Some(held) = self.registers.read(register, &mut *stage.clipboard) else {
            return;
        };
        self.begin_change(stage);
        let linewise = held.is_linewise();
        let depth = stage.buffer.undo_depth();

        if stage.state.mode.is_visual() {
            let spans = visual::spans(stage.state, stage.buffer, false);
            let replaced_lines = stage.state.mode == Mode::VisualLine;
            stage.state.last_visual = Some((stage.state.mode, stage.state.regions.clone()));
            stage.state.mode = Mode::Normal;
            stage.state.regions.clear();
            stage.state.shown = None;
            let pieces = pieces_for(&held, spans.len());
            let mut store = Store {
                registers: &mut self.registers,
                register: if preserve { Some('_') } else { None },
                clipboard: &mut *stage.clipboard,
                pieces: Vec::new(),
                filling: None,
                source: None,
                wrap: 1,
                block: false,
            };
            let selections = spans.iter().map(Span::as_selection).collect();
            stage.buffer.set_selections(selections);
            let mut index = spans.len();
            stage.buffer.grouped(|buffer| {
                buffer.at_each(|buffer| {
                    index -= 1;
                    let span = Span::from_selection(buffer.selection(), replaced_lines);
                    let text = pieces[index].repeat(times);
                    let at = operator::delete(buffer, span, &mut store);
                    let past_end = span.linewise && span.start.line >= buffer.line_count();
                    let (at, inserted, landing) = match (span.linewise, linewise) {
                        (true, _)
                            if past_end || (span.start.line > 0 && at.line < span.start.line) =>
                        {
                            let last = buffer.line_count().saturating_sub(1);
                            let end = Position::new(last, buffer.line_len(last));
                            (
                                end,
                                format!("\n{}", text.strip_suffix('\n').unwrap_or(&text)),
                                Some(last + 1),
                            )
                        }
                        (true, _) => (
                            Position::new(span.start.line, 0),
                            with_break(&text),
                            Some(span.start.line),
                        ),
                        (false, true) => (
                            at,
                            format!("\n{}", text.strip_suffix('\n').unwrap_or(&text)),
                            None,
                        ),
                        (false, false) => (at, text, None),
                    };
                    buffer.replace(at..at, &inserted);
                    let to = match landing {
                        Some(line) => Position::new(line, first_non_blank(buffer, line)),
                        None => at,
                    };
                    buffer.set_selection(Selection::at(to));
                });
            });
            store.finish();
            stage.buffer.squash_since(depth);
            return;
        }

        let (places, primary) = heads(stage.buffer);
        if held.block && held.pieces.len() != places.len() {
            let head = places[primary];
            put_block(stage.buffer, &held.pieces, head, before, times);
            stage.buffer.squash_since(depth);
            return;
        }
        let pieces = pieces_for(&held, places.len());
        let mut index = places.len();
        stage.buffer.grouped(|buffer| {
            buffer.at_each(|buffer| {
                index -= 1;
                let text = pieces[index].repeat(times);
                put(buffer, &text, linewise, before);
            });
        });
        let landed = heads(stage.buffer).0;
        place_all(stage.buffer, landed, primary);
        stage.buffer.squash_since(depth);
    }

    /// Joins `times` lines from every cursor's, at least two, or the lines
    /// of every selection, with a space between them for `J`.
    fn join(&mut self, stage: &mut Stage, times: usize, spaces: bool) {
        self.begin_change(stage);
        let runs = match stage.state.mode.is_visual() {
            true => {
                let spans = visual::spans(stage.state, stage.buffer, true);
                stage.state.mode = Mode::Normal;
                stage.state.regions.clear();
                spans
                    .iter()
                    .map(|span| (span.start.line, span.end.line - span.start.line + 1))
                    .collect::<Vec<_>>()
            }
            false => heads(stage.buffer)
                .0
                .iter()
                .map(|head| (head.line, times))
                .collect(),
        };
        let depth = stage.buffer.undo_depth();
        let mut landed = Vec::new();
        for (first, count) in runs.into_iter().rev() {
            if let Some(at) = join_from(stage.buffer, first, count.max(2) - 1, spaces) {
                landed.push(at);
            }
        }
        stage.buffer.squash_since(depth);
        if !landed.is_empty() {
            landed.reverse();
            place_all(stage.buffer, landed, 0);
        }
    }

    /// Counts the number at or after every cursor up by `delta`, or the
    /// numbers in every line of the selection, by a growing step when
    /// `step`.
    fn increment(&mut self, stage: &mut Stage, delta: i64, step: bool) {
        self.begin_change(stage);
        let depth = stage.buffer.undo_depth();
        if stage.state.mode.is_visual() {
            let spans = visual::spans(stage.state, stage.buffer, false);
            let linewise = stage.state.mode == Mode::VisualLine;
            stage.state.mode = Mode::Normal;
            stage.state.regions.clear();
            let mut lines = Vec::new();
            for span in &spans {
                for line in span.start.line..=span.end.line {
                    let from = if linewise || line != span.start.line {
                        0
                    } else {
                        span.start.column
                    };
                    lines.push((line, from));
                }
            }
            let first = lines.first().copied();
            for (index, (line, from)) in lines.into_iter().enumerate() {
                let delta = if step {
                    delta * (index as i64 + 1)
                } else {
                    delta
                };
                let text = stage.buffer.line_text(line);
                if let Some((start, end, written)) = format::increment(&text, from, delta) {
                    stage.buffer.replace(
                        Position::new(line, start)..Position::new(line, end),
                        &written,
                    );
                }
            }
            if let Some((line, from)) = first {
                stage
                    .buffer
                    .set_selection(Selection::at(Position::new(line, from)));
            }
            stage.buffer.squash_since(depth);
            return;
        }
        stage.buffer.grouped(|buffer| {
            buffer.at_each(|buffer| {
                let head = buffer.selection().head;
                let text = buffer.line_text(head.line);
                if let Some((start, end, written)) = format::increment(&text, head.column, delta) {
                    let range = Position::new(head.line, start)..Position::new(head.line, end);
                    let width = written.chars().count();
                    buffer.replace(range, &written);
                    buffer
                        .set_selection(Selection::at(Position::new(head.line, start + width - 1)));
                }
            });
        });
        stage.buffer.squash_since(depth);
    }

    /// Marks `span` for exchanging, or swaps it with the span marked before.
    fn exchange(&mut self, stage: &mut Stage, span: Span) {
        let Some(first) = stage.state.exchange.take() else {
            stage.state.exchange = Some(span);
            self.change = None;
            return;
        };
        let (earlier, later) = match first.start <= span.start {
            true => (first, span),
            false => (span, first),
        };
        let earlier_text = stage.buffer.text_in(earlier.body(stage.buffer));
        let later_text = stage.buffer.text_in(later.body(stage.buffer));
        let (earlier_range, later_range) = (earlier.body(stage.buffer), later.body(stage.buffer));
        stage.buffer.grouped(|buffer| {
            buffer.replace(later_range, &earlier_text);
            buffer.replace(earlier_range.clone(), &later_text);
        });
        stage
            .buffer
            .set_selection(Selection::at(earlier_range.start));
    }

    /// Keeps `text` typed in insert mode for `".`.
    pub(crate) fn keep_typed(&mut self, text: String) {
        self.registers.typed(text);
    }

    /// Fills a register from outside a command: a yank the command line
    /// asked for.
    pub(crate) fn keep(
        &mut self,
        stage: &mut Stage,
        register: Option<char>,
        text: String,
        filling: Filling,
    ) {
        self.registers
            .fill(register, vec![text], filling, false, &mut *stage.clipboard);
    }
}

/// Does `change` at each of `spans`, the last in the text first so that no
/// change moves the text an earlier one was measured against, answering
/// where each span's cursor lands, carried past the changes made after it.
///
/// Spans that only touch are kept apart, which the buffer's own cursors
/// would merge: `x` at two cursors side by side deletes two characters.
pub(crate) fn each_span(
    buffer: &mut Buffer,
    spans: &[Span],
    mut change: impl FnMut(&mut Buffer, Span) -> Position,
) -> Vec<Position> {
    let mut spans = spans.to_vec();
    spans.sort_by_key(|span| span.start);
    spans.dedup();
    let mut landed: Vec<usize> = Vec::with_capacity(spans.len());
    buffer.grouped(|buffer| {
        for span in spans.iter().rev() {
            let before = buffer.len_chars();
            let to = change(buffer, *span);
            let moved = buffer.len_chars() as isize - before as isize;
            for later in &mut landed {
                *later = later.saturating_add_signed(moved);
            }
            landed.push(buffer.char_of(to));
        }
    });
    landed.reverse();
    landed
        .into_iter()
        .map(|offset| buffer.position_of(offset))
        .collect()
}

/// The piece of `held` each of `cursors` puts back: its own when there are
/// as many pieces as cursors, the whole text otherwise.
fn pieces_for(held: &crate::register::Register, cursors: usize) -> Vec<String> {
    match held.pieces.len() == cursors && cursors > 1 {
        true => held.pieces.clone(),
        false => vec![held.text.clone(); cursors],
    }
}

/// Puts `text` in at the buffer's cursor as `p` or `P` does: as lines below
/// or above when it holds whole lines, after or before the cursor otherwise.
fn put(buffer: &mut Buffer, text: &str, linewise: bool, before: bool) {
    let head = buffer.selection().head;
    if linewise {
        let (at, inserted, line) = match (before, head.line + 1 < buffer.line_count()) {
            (true, _) => (Position::new(head.line, 0), text.to_owned(), head.line),
            (false, true) => (
                Position::new(head.line + 1, 0),
                text.to_owned(),
                head.line + 1,
            ),
            (false, false) => (
                Position::new(head.line, buffer.line_len(head.line)),
                format!("\n{}", text.strip_suffix('\n').unwrap_or(text)),
                head.line + 1,
            ),
        };
        buffer.replace(at..at, &inserted);
        buffer.set_selection(Selection::at(Position::new(
            line,
            first_non_blank(buffer, line),
        )));
        return;
    }
    let at = match before || buffer.line_len(head.line) == 0 {
        true => head,
        false => Position::new(head.line, head.column + 1),
    };
    buffer.replace(at..at, text);
    let end = at.after(text);
    let to = match text.contains('\n') {
        true => at,
        false => Position::new(end.line, end.column.saturating_sub(1)),
    };
    buffer.set_selection(Selection::at(to));
}

/// Puts the lines of a block back as a block: each at the same column of
/// the lines from `head`'s down, after the cursor or before it, the lines
/// padded out to reach the column and added when the text runs out.
fn put_block(buffer: &mut Buffer, pieces: &[String], head: Position, before: bool, times: usize) {
    let offset = usize::from(!before && buffer.line_len(head.line) > 0);
    let column = buffer.display_column(head) + offset;
    buffer.grouped(|buffer| {
        for (index, piece) in pieces.iter().enumerate() {
            let line = head.line + index;
            if line >= buffer.line_count() {
                let last = buffer.line_count().saturating_sub(1);
                let end = Position::new(last, buffer.line_len(last));
                buffer.replace(end..end, "\n");
            }
            let width = buffer.display_width(line);
            if width < column {
                let end = Position::new(line, buffer.line_len(line));
                buffer.replace(end..end, &" ".repeat(column - width));
            }
            let at = buffer.position_at_display(line, column);
            buffer.replace(at..at, &piece.repeat(times));
        }
    });
    let at = buffer.position_at_display(head.line, column);
    buffer.set_selection(Selection::at(at));
}

/// Joins `joins` lines onto `first`, answering where the cursor goes: onto
/// the last place two lines were joined.
fn join_from(buffer: &mut Buffer, first: usize, joins: usize, spaces: bool) -> Option<Position> {
    if first + 1 >= buffer.line_count() {
        return None;
    }
    let mut landed = None;
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
                false => buffer.replace(end..Position::new(first + 1, 0), ""),
            }
            landed = Some(end);
        }
    });
    landed
}

/// `text` ending in a line break, as a line put back must.
fn with_break(text: &str) -> String {
    match text.ends_with('\n') {
        true => text.to_owned(),
        false => format!("{text}\n"),
    }
}
