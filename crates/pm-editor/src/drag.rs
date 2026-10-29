//! Moving or copying selected text by dragging it: where it is picked up, where it would be dropped, and the one change that drops it.

use std::ops::Range;

use pm_text::Edit;

use crate::document::Document;

/// Selected text being carried to another place.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Carried {
    /// The selected stretch that was picked up.
    pub(crate) range: Range<usize>,
    /// The byte it would be dropped before, while the pointer is over the text.
    pub(crate) over: Option<usize>,
}

impl Document {
    /// Whether pressing at the byte `at` picks up the selection: it is in the middle of the one selection of a text that can be changed.
    pub fn picks_up_at(&self, at: usize) -> bool {
        !self.field.is_read_only()
            && !self.field.has_others()
            && self
                .field
                .selection()
                .is_some_and(|range| range.start < at && at < range.end)
    }

    /// Picks up the selection pressed at the byte `at`, and says whether there was one to pick up.
    pub fn drag_started(&mut self, at: usize) -> bool {
        let picked = self.picks_up_at(at);
        self.drag = self
            .field
            .selection()
            .filter(|_| picked)
            .map(|range| Carried { range, over: None });
        picked
    }

    /// Notes that the selection being carried is over the byte `byte`.
    pub fn drag_over(&mut self, byte: usize) {
        let length = self.field.text().len();
        if let Some(carried) = &mut self.drag {
            carried.over = Some(byte.min(length));
        }
    }

    /// Where the selection being carried would be dropped, to mark it: nowhere while it is over itself.
    pub fn drop_mark(&self) -> Option<usize> {
        let carried = self.drag.as_ref()?;
        carried
            .over
            .filter(|over| !(carried.range.start..=carried.range.end).contains(over))
    }

    /// Whether a selection is being carried.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Lets go of the selection being carried where it was, changing nothing.
    pub fn drag_cancelled(&mut self) {
        self.drag = None;
    }

    /// Puts down the selection being carried: the one change that moves it to where it is over, or copies it there when `copy`, leaving what was dropped selected; nothing when it is not over a place outside itself.
    pub fn drag_dropped(&mut self, copy: bool) -> Option<Edit> {
        let range = self.drag.as_ref()?.range.clone();
        let target = self.drop_mark();
        self.drag = None;
        let target = target?;
        let text = self.field.text();
        let carried = &text[range.clone()];
        let length = carried.len();
        if copy {
            return Some(Edit::Replace(
                target..target,
                carried.to_owned(),
                (target, target + length),
            ));
        }
        Some(if target > range.end {
            let between = &text[range.end..target];
            let landing = target - length;
            Edit::Replace(
                range.start..target,
                format!("{between}{carried}"),
                (landing, landing + length),
            )
        } else {
            let between = &text[target..range.start];
            Edit::Replace(
                target..range.end,
                format!("{carried}{between}"),
                (target, target + length),
            )
        })
    }
}
