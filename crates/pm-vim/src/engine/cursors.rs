//! Every cursor at once: moving them all by a motion, and keeping each on a
//! character the way normal mode does.
//!
//! The buffer holds one primary cursor and any number of others; a motion
//! in normal mode moves every one of them, measured from where each is, and
//! the primary is the one the column memory, the jump list and the view
//! follow.

use pm_text::{Buffer, Position, Selection};

use crate::engine::{Effect, Stage, Vim};
use crate::motion::{Context, Motion, Moved};
use crate::text::on_char;

/// Every cursor's head in the order they appear, and which is the primary.
pub(crate) fn heads(buffer: &Buffer) -> (Vec<Position>, usize) {
    let primary = buffer.selection();
    let all = buffer.selections();
    let index = all
        .iter()
        .position(|selection| *selection == primary)
        .unwrap_or(0);
    (all.iter().map(|selection| selection.head).collect(), index)
}

/// Puts a cursor at each of `places`, the `primary`-th being the primary.
pub(crate) fn place_all(buffer: &mut Buffer, places: Vec<Position>, primary: usize) {
    let mut selections = places.into_iter().map(Selection::at).collect::<Vec<_>>();
    if primary < selections.len() {
        let chosen = selections.remove(primary);
        selections.push(chosen);
    }
    buffer.set_selections(selections);
}

impl Vim {
    /// Where `motion` takes a cursor at `from`, if it takes it anywhere.
    pub(crate) fn resolve(
        &mut self,
        stage: &Stage,
        motion: &Motion,
        from: Position,
        count: Option<usize>,
        goal: Option<usize>,
    ) -> Option<Moved> {
        let mut cx = Context {
            count,
            goal,
            view: stage.view,
            last_find: &mut self.last_find,
            last_search: &mut self.last_search,
            marks: &stage.state.marks,
        };
        let moved = motion.resolve(stage.buffer, from, &mut cx)?;
        (moved.to != from || motion.always_moves()).then_some(moved)
    }

    /// Moves every cursor by `motion`, as normal mode does.
    ///
    /// A jump takes down where the primary cursor left, both as the `''`
    /// mark and on the window's jump list; half a page scrolls the view
    /// with it.
    pub(crate) fn normal_motion(
        &mut self,
        stage: &mut Stage,
        motion: &Motion,
        count: Option<usize>,
    ) {
        let (places, primary) = heads(stage.buffer);
        let goal = stage.state.goal;
        let mut moved_primary = None;
        let mut moved = Vec::with_capacity(places.len());
        for (index, from) in places.iter().enumerate() {
            let goal = if index == primary { goal } else { None };
            match self.resolve(stage, motion, *from, count, goal) {
                Some(step) => {
                    if index == primary {
                        moved_primary = Some(step);
                    }
                    moved.push(step.to);
                }
                None => moved.push(*from),
            }
        }
        let Some(step) = moved_primary else {
            return;
        };
        stage.state.goal = step.goal;
        let from = places[primary];
        if motion.is_jump() {
            stage.state.marks.insert('\'', from);
            stage.state.marks.insert('`', from);
            stage.effects.push(Effect::Jumped(from));
        }
        let half = (stage.view.rows / 2).max(1) as isize;
        match motion {
            Motion::HalfPageDown => stage.effects.push(Effect::ScrollBy(half)),
            Motion::HalfPageUp => stage.effects.push(Effect::ScrollBy(-half)),
            _ => {}
        }
        place_all(stage.buffer, moved, primary);
    }

    /// Brings every cursor onto a character after a command, as normal
    /// mode keeps them, or shows the selection in visual mode.
    pub(crate) fn settle(&mut self, stage: &mut Stage) {
        match stage.state.mode {
            crate::mode::Mode::Normal => {
                let (places, primary) = heads(stage.buffer);
                let settled = places
                    .iter()
                    .map(|place| on_char(stage.buffer, *place))
                    .collect::<Vec<_>>();
                let collapsed = stage.buffer.selections().iter().all(Selection::is_empty);
                if settled != places || !collapsed {
                    place_all(stage.buffer, settled, primary);
                }
                stage.state.shown = None;
                stage.state.regions.clear();
            }
            mode if mode.is_visual() => crate::engine::visual::show(stage.state, stage.buffer),
            _ => {}
        }
    }
}
