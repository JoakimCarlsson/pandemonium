//! Visual mode: selections of characters, of lines and of blocks, and what
//! an operator does to them.
//!
//! A selection is kept as the two ends vim means — the anchor and the
//! character the cursor is on — and put on the buffer in the shape the mode
//! draws: characters with the one under the cursor included, lines as they
//! are (the window widens them to whole lines), and a block as one span on
//! every line it crosses. Several selections at once are several regions,
//! each moved and operated on as one.

use pm_text::{Buffer, Position, Selection};

use crate::engine::cursors::{heads, place_all};
use crate::engine::repeat::VisualShape;
use crate::engine::{Region, Stage, State, Vim};
use crate::mode::Mode;
use crate::motion::Motion;
use crate::object::Object;
use crate::operator::{Operator, Span};
use crate::search::LastSearch;
use crate::text::{self, on_char};

/// The shape of the primary selection, for `.` to select as much again.
pub(crate) fn shape_of(state: &State, buffer: &Buffer) -> Option<VisualShape> {
    if !state.mode.is_visual() {
        return None;
    }
    let region = state.regions.last()?;
    let (start, end) = (
        region.anchor.min(region.head),
        region.anchor.max(region.head),
    );
    let width = match state.mode {
        Mode::VisualBlock => {
            let (left, right) = block_columns(state, buffer, region);
            right.saturating_sub(left)
        }
        _ if start.line == end.line => end.column - start.column,
        _ => end.column,
    };
    Some(VisualShape {
        mode: state.mode,
        lines: end.line - start.line,
        width,
        to_end: state.goal == Some(usize::MAX),
    })
}

/// Selects as much as `shape` from `at`, the way `.` repeats a change made
/// to a selection.
pub(crate) fn select_shape(
    state: &mut State,
    buffer: &mut Buffer,
    shape: VisualShape,
    at: Position,
) {
    let last = buffer.line_count().saturating_sub(1);
    let line = (at.line + shape.lines).min(last);
    let head = match (shape.mode, shape.lines) {
        (Mode::VisualLine, _) => Position::new(line, at.column),
        (Mode::VisualBlock, _) => {
            buffer.position_at_display(line, buffer.display_column(at) + shape.width)
        }
        (_, 0) => Position::new(at.line, at.column + shape.width),
        _ => Position::new(line, shape.width),
    };
    state.mode = shape.mode;
    state.goal = shape.to_end.then_some(usize::MAX);
    state.regions = vec![Region {
        anchor: at,
        head: buffer.clamped(head),
    }];
    show(state, buffer);
}

/// The display columns a block covers, its left edge and one past its
/// right, the right running to each line's end after `$`.
fn block_columns(state: &State, buffer: &Buffer, region: &Region) -> (usize, usize) {
    let anchor = buffer.display_column(region.anchor);
    let head = buffer.display_column(region.head);
    let right = match state.goal == Some(usize::MAX) {
        true => usize::MAX,
        false => anchor.max(head) + 1,
    };
    (anchor.min(head), right)
}

/// The spans the selections cover, as whole lines when `lines`: one per
/// region, or one per line of a block.
pub(crate) fn spans(state: &State, buffer: &Buffer, lines: bool) -> Vec<Span> {
    let mut spans = Vec::new();
    for region in &state.regions {
        let (start, end) = (
            region.anchor.min(region.head),
            region.anchor.max(region.head),
        );
        match state.mode {
            _ if lines || state.mode == Mode::VisualLine => {
                spans.push(Span::lines(start.line, end.line))
            }
            Mode::VisualBlock => {
                let (left, right) = block_columns(state, buffer, region);
                for line in start.line..=end.line {
                    if buffer.display_width(line) <= left && right != usize::MAX {
                        continue;
                    }
                    let from = buffer.position_at_display(line, left);
                    let to = match right {
                        usize::MAX => Position::new(line, buffer.line_len(line)),
                        right => buffer.position_at_display(line, right),
                    };
                    spans.push(Span::chars(from, to));
                }
            }
            _ => spans.push(Span::chars(start, text::after(buffer, end))),
        }
    }
    spans.sort_by_key(|span| span.start);
    spans
}

/// Puts the selections visual mode stands for on the buffer.
pub(crate) fn show(state: &mut State, buffer: &mut Buffer) {
    let mut selections = Vec::new();
    match state.mode {
        Mode::VisualLine => {
            for region in &state.regions {
                selections.push(Selection {
                    anchor: buffer.clamped(region.anchor),
                    head: buffer.clamped(region.head),
                });
            }
        }
        Mode::VisualBlock => {
            let Some(region) = state.regions.last().copied() else {
                return;
            };
            let reversed =
                buffer.display_column(region.head) < buffer.display_column(region.anchor);
            let mut lines = spans(state, buffer, false)
                .into_iter()
                .map(|span| match reversed {
                    true => Selection {
                        anchor: span.end,
                        head: span.start,
                    },
                    false => span.as_selection(),
                })
                .collect::<Vec<_>>();
            if lines.is_empty() {
                lines.push(Selection::at(region.head));
            }
            let primary = lines
                .iter()
                .position(|selection| selection.head.line == region.head.line)
                .unwrap_or(lines.len() - 1);
            let chosen = lines.remove(primary);
            lines.push(chosen);
            selections = lines;
        }
        _ => {
            for region in &state.regions {
                let span = Span::chars(
                    region.anchor.min(region.head),
                    text::after(buffer, region.anchor.max(region.head)),
                );
                selections.push(match region.head < region.anchor {
                    true => Selection {
                        anchor: span.end,
                        head: span.start,
                    },
                    false => span.as_selection(),
                });
            }
        }
    }
    if selections.is_empty() {
        return;
    }
    buffer.set_selections(selections);
    state.shown = Some(buffer.selections());
}

impl Vim {
    /// Enters `mode` from normal mode, a selection at every cursor.
    pub(crate) fn enter_visual(&mut self, stage: &mut Stage, mode: Mode) {
        let (places, primary) = heads(stage.buffer);
        let mut regions = places
            .into_iter()
            .map(|place| Region {
                anchor: place,
                head: place,
            })
            .collect::<Vec<_>>();
        let chosen = regions.remove(primary.min(regions.len().saturating_sub(1)));
        regions.push(chosen);
        if mode == Mode::VisualBlock {
            regions = vec![chosen];
        }
        stage.state.regions = regions;
        stage.state.mode = mode;
        show(stage.state, stage.buffer);
    }

    /// Switches between the kinds of selection, or leaves visual mode when
    /// asked for the kind it is already in.
    pub(crate) fn toggle_visual(&mut self, stage: &mut Stage, mode: Mode) {
        match stage.state.mode {
            current if current == mode => self.leave_visual(stage),
            Mode::Normal => self.enter_visual(stage, mode),
            _ => {
                if mode == Mode::VisualBlock {
                    let primary = stage.state.regions.last().copied();
                    stage.state.regions = primary.into_iter().collect();
                }
                stage.state.mode = mode;
                show(stage.state, stage.buffer);
            }
        }
    }

    /// Leaves visual mode, a cursor where each selection's cursor was.
    pub(crate) fn leave_visual(&mut self, stage: &mut Stage) {
        let state = &mut *stage.state;
        state.last_visual = Some((state.mode, state.regions.clone()));
        self.mark_selection(stage);
        let state = &mut *stage.state;
        let places = state
            .regions
            .iter()
            .map(|region| on_char(stage.buffer, region.head))
            .collect::<Vec<_>>();
        let primary = places.len().saturating_sub(1);
        state.mode = Mode::Normal;
        state.regions.clear();
        state.shown = None;
        if !places.is_empty() {
            place_all(stage.buffer, places, primary);
        }
    }

    /// Sets the `'<` and `'>` marks to where the primary selection starts
    /// and ends.
    fn mark_selection(&mut self, stage: &mut Stage) {
        if let Some(region) = stage.state.regions.last().copied() {
            stage
                .state
                .marks
                .insert('<', region.anchor.min(region.head));
            stage
                .state
                .marks
                .insert('>', region.anchor.max(region.head));
        }
    }

    /// Moves every selection's cursor by `motion`.
    pub(crate) fn visual_motion(
        &mut self,
        stage: &mut Stage,
        motion: &Motion,
        count: Option<usize>,
    ) {
        let goal = stage.state.goal;
        let regions = stage.state.regions.clone();
        let primary = regions.len().saturating_sub(1);
        let mut moved = regions.clone();
        let mut primary_goal = goal;
        for (index, region) in regions.iter().enumerate() {
            let goal = if index == primary { goal } else { None };
            if let Some(step) = self.resolve(stage, motion, region.head, count, goal) {
                moved[index].head = step.to;
                if index == primary {
                    primary_goal = step.goal;
                    if motion.is_jump() {
                        stage.state.marks.insert('\'', region.head);
                    }
                }
            }
        }
        let half = (stage.view.rows / 2).max(1) as isize;
        match motion {
            Motion::HalfPageDown => stage.effects.push(crate::engine::Effect::ScrollBy(half)),
            Motion::HalfPageUp => stage.effects.push(crate::engine::Effect::ScrollBy(-half)),
            _ => {}
        }
        stage.state.goal = primary_goal;
        stage.state.regions = moved;
        show(stage.state, stage.buffer);
    }

    /// Grows every selection to `object` around its cursor.
    pub(crate) fn select_object(
        &mut self,
        stage: &mut Stage,
        object: Object,
        around: bool,
        count: Option<usize>,
    ) {
        let times = count.unwrap_or(1);
        let mut regions = stage.state.regions.clone();
        let mut linewise = false;
        for region in &mut regions {
            let Some(span) = object.span(stage.buffer, region.head, around, times) else {
                continue;
            };
            linewise |= span.linewise;
            *region = match span.linewise {
                true => Region {
                    anchor: span.start,
                    head: Position::new(span.end.line, 0),
                },
                false => Region {
                    anchor: span.start,
                    head: text::before(stage.buffer, span.end).max(span.start),
                },
            };
        }
        stage.state.regions = regions;
        if linewise && stage.state.mode == Mode::Visual {
            stage.state.mode = Mode::VisualLine;
        }
        if !linewise && stage.state.mode == Mode::VisualLine {
            stage.state.mode = Mode::Visual;
        }
        show(stage.state, stage.buffer);
    }

    /// Moves each selection's cursor to its other end; with `row_aware` in a
    /// block, to the other end of the same line.
    pub(crate) fn other_end(&mut self, stage: &mut Stage, row_aware: bool) {
        let block = stage.state.mode == Mode::VisualBlock;
        for region in &mut stage.state.regions {
            match row_aware && block {
                true => {
                    let anchor = region.anchor;
                    region.anchor = Position::new(anchor.line, region.head.column);
                    region.head = Position::new(region.head.line, anchor.column);
                }
                false => std::mem::swap(&mut region.anchor, &mut region.head),
            }
        }
        show(stage.state, stage.buffer);
    }

    /// Selects again what was selected last.
    pub(crate) fn restore_visual(&mut self, stage: &mut Stage) {
        let Some((mode, regions)) = stage.state.last_visual.clone() else {
            return;
        };
        let regions = regions
            .into_iter()
            .map(|region| Region {
                anchor: stage.buffer.clamped(region.anchor),
                head: stage.buffer.clamped(region.head),
            })
            .collect();
        stage.state.mode = mode;
        stage.state.regions = regions;
        show(stage.state, stage.buffer);
    }

    /// The match of the last search at or after the cursor, or at or before
    /// it: what `gn` and `gN` select, or give an operator.
    pub(crate) fn next_match(&mut self, stage: &Stage, forward: bool) -> Option<Span> {
        let LastSearch { pattern, .. } = self.last_search.clone()?;
        let head = stage.buffer.selection().head;
        let here = pattern
            .matches_on(stage.buffer, head.line..head.line + 1)
            .into_iter()
            .find(|found| found.start <= head && head < found.end);
        if let Some(found) = here {
            return Some(Span::chars(found.start, found.end));
        }
        let from = stage.buffer.char_of(head);
        let start = pattern.find(stage.buffer, from, forward)?;
        let start = stage.buffer.position_of(start);
        let found = pattern.matches_on(stage.buffer, start.line..start.line + 1);
        let matched = found.into_iter().find(|found| found.start == start)?;
        Some(Span::chars(matched.start, matched.end))
    }

    /// Selects the next match of the last search, or grows the selection to
    /// it: `gn` and `gN`.
    pub(crate) fn select_match(&mut self, stage: &mut Stage, forward: bool) {
        let Some(matched) = self.next_match(stage, forward) else {
            return;
        };
        let start = matched.start;
        let end = text::before(stage.buffer, matched.end);
        let region = match stage.state.mode.is_visual() {
            true => {
                let anchor = stage
                    .state
                    .regions
                    .last()
                    .map_or(start, |region| region.anchor);
                Region {
                    anchor,
                    head: if forward { end } else { start },
                }
            }
            false => Region {
                anchor: if forward { start } else { end },
                head: if forward { end } else { start },
            },
        };
        stage.state.mode = if stage.state.mode.is_visual() {
            stage.state.mode
        } else {
            Mode::Visual
        };
        stage.state.regions = vec![region];
        show(stage.state, stage.buffer);
    }

    /// Takes up a selection the pointer made, or a click that ended one.
    pub(crate) fn follow_pointer(&mut self, stage: &mut Stage) {
        let selections = stage.buffer.selections();
        let state = &mut *stage.state;
        let selected = selections.iter().any(|selection| !selection.is_empty());
        match state.mode {
            Mode::Normal if selected => {
                state.mode = Mode::Visual;
                state.regions = regions_of(stage.buffer, &selections);
                show(state, stage.buffer);
            }
            mode if mode.is_visual() && state.shown.as_ref() != Some(&selections) => {
                if !selected {
                    state.mode = Mode::Normal;
                    state.regions.clear();
                    state.shown = None;
                    return;
                }
                if state.mode == Mode::VisualBlock {
                    state.mode = Mode::Visual;
                }
                state.regions = regions_of(stage.buffer, &selections);
                show(state, stage.buffer);
            }
            _ => {}
        }
    }

    /// Applies `operator` to every selection, as whole lines when `lines`,
    /// and leaves visual mode.
    pub(crate) fn visual_operator(&mut self, stage: &mut Stage, operator: Operator, lines: bool) {
        let spans = spans(stage.state, stage.buffer, lines || operator.is_linewise());
        let block = stage.state.mode == Mode::VisualBlock;
        if operator.changes() {
            self.begin_change(stage);
        }
        stage.state.last_visual = Some((stage.state.mode, stage.state.regions.clone()));
        self.mark_selection(stage);
        stage.state.mode = Mode::Normal;
        stage.state.regions.clear();
        stage.state.shown = None;
        let first = spans
            .first()
            .map_or(stage.buffer.selection().head, |span| span.start);
        place_all(stage.buffer, vec![first], 0);
        if operator == Operator::AddSurround {
            stage.state.pending.surrounding = spans;
            stage.state.pending.waiting = Some(crate::action::Waiting::AddSurround);
            return;
        }
        let times = self.count(stage).unwrap_or(1);
        let depth = stage.buffer.undo_depth();
        match operator {
            Operator::Indent | Operator::Outdent => {
                let first = spans.first().map_or(0, |span| span.start.line);
                for span in &spans {
                    crate::operator::shift(
                        stage.buffer,
                        *span,
                        operator == Operator::Indent,
                        times,
                    );
                }
                let column = crate::text::first_non_blank(stage.buffer, first);
                place_all(stage.buffer, vec![Position::new(first, column)], 0);
            }
            _ => {
                let spans = spans
                    .iter()
                    .map(|span| (*span, span.start))
                    .collect::<Vec<_>>();
                let landed = self.apply(stage, operator, &spans, block);
                if operator == Operator::Change {
                    let collapse = block;
                    place_all(stage.buffer, landed, 0);
                    self.begin_typing(stage, Mode::Insert, depth, 1, None);
                    if let Some(typing) = stage.state.typing.as_mut() {
                        typing.collapse = collapse;
                    }
                    return;
                }
                let first = landed.first().copied().unwrap_or_default();
                place_all(stage.buffer, vec![first], 0);
            }
        }
        stage.buffer.squash_since(depth);
    }

    /// Begins typing at the start of every line of the selection, or at the
    /// end, or at the ends of the lines' text: `I`, `A`, `gI` and `gA`.
    pub(crate) fn visual_insert(&mut self, stage: &mut Stage, end: bool, text_ends: bool) {
        let mode = stage.state.mode;
        let depth = stage.buffer.undo_depth();
        let regions = stage.state.regions.clone();
        self.begin_change(stage);
        let places = match (mode, text_ends) {
            (Mode::VisualBlock, false) => {
                let region = regions.last().copied().unwrap_or_default();
                let (left, right) = block_columns(stage.state, stage.buffer, &region);
                let (top, bottom) = (
                    region.anchor.line.min(region.head.line),
                    region.anchor.line.max(region.head.line),
                );
                let mut places = Vec::new();
                for line in top..=bottom {
                    let width = stage.buffer.display_width(line);
                    match end {
                        true if right == usize::MAX => {
                            places.push(Position::new(line, stage.buffer.line_len(line)))
                        }
                        true => {
                            if width < right {
                                let at = Position::new(line, stage.buffer.line_len(line));
                                stage.buffer.replace(at..at, &" ".repeat(right - width));
                            }
                            places.push(stage.buffer.position_at_display(line, right));
                        }
                        false if width < left => {}
                        false => places.push(stage.buffer.position_at_display(line, left)),
                    }
                }
                places
            }
            (_, true) => {
                let lines = spans(stage.state, stage.buffer, true);
                lines
                    .iter()
                    .flat_map(|span| span.start.line..=span.end.line)
                    .map(|line| match end {
                        true => Position::new(line, stage.buffer.line_len(line)),
                        false => {
                            Position::new(line, crate::text::first_non_blank(stage.buffer, line))
                        }
                    })
                    .collect()
            }
            _ => spans(stage.state, stage.buffer, false)
                .iter()
                .map(|span| match (end, mode) {
                    (true, Mode::VisualLine) => {
                        Position::new(span.end.line, stage.buffer.line_len(span.end.line))
                    }
                    (true, _) => span.end,
                    (false, _) => span.start,
                })
                .collect(),
        };
        stage.state.last_visual = Some((mode, regions));
        stage.state.mode = Mode::Normal;
        stage.state.regions.clear();
        stage.state.shown = None;
        if places.is_empty() {
            return;
        }
        place_all(stage.buffer, places, 0);
        self.begin_typing(stage, Mode::Insert, depth, 1, None);
        if let Some(typing) = stage.state.typing.as_mut() {
            typing.collapse = mode == Mode::VisualBlock;
        }
    }
}

/// The regions `selections` stand for, each cursor on the last character
/// its selection covers.
fn regions_of(buffer: &Buffer, selections: &[Selection]) -> Vec<Region> {
    selections
        .iter()
        .map(|selection| Region {
            anchor: selection.anchor,
            head: match selection.head > selection.anchor {
                true => text::before(buffer, selection.head),
                false => selection.head,
            },
        })
        .collect()
}
