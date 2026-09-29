//! The places of a snippet still to be filled in, and moving between them.
//!
//! A snippet put into the buffer leaves places behind it, each a span or
//! several spans written at once. They are kept here in characters and moved
//! with every change the text takes, so that Tab lands on a place wherever
//! the typing since has pushed it; a change that cuts across one ends the
//! snippet, since there is no longer a place there to land on.

use std::ops::Range;

use crate::buffer::Buffer;
use crate::cursor::Selection;

/// A snippet's places, as the buffer keeps them while they are being filled.
#[derive(Clone, Debug, Default)]
pub(crate) struct Places {
    /// Every place in order, each as the spans it covers, in characters.
    stops: Vec<Vec<Range<usize>>>,
    /// Which of them the cursor is on.
    at: usize,
}

impl Places {
    /// Moves every place with a change at the character `start`, which took
    /// `removed` characters out and put `added` in, answering whether every
    /// place survived it.
    ///
    /// The place being filled in grows with typing at either of its ends;
    /// any other place is pushed along by typing before it and left alone by
    /// typing after it.
    fn shift(&mut self, start: usize, removed: usize, added: usize) -> bool {
        let end = start + removed;
        for (index, spans) in self.stops.iter_mut().enumerate() {
            let current = index == self.at;
            for span in spans {
                let within = start >= span.start && end <= span.end;
                let grows = match current {
                    true => within,
                    false => within && start > span.start && end < span.end,
                };
                if grows {
                    span.end = span.end + added - removed;
                } else if end <= span.start && !(current && start == span.start) {
                    span.start = span.start + added - removed;
                    span.end = span.end + added - removed;
                } else if start < span.end {
                    return false;
                }
            }
        }
        true
    }
}

impl Buffer {
    /// Begins filling in a snippet put in at the character `base`, whose
    /// places `stops` are counted from there, and selects the first place.
    ///
    /// A snippet whose only place is where the cursor ends up has nothing to
    /// fill in, and the cursor is only put there.
    pub fn begin_snippet(&mut self, base: usize, stops: Vec<Vec<Range<usize>>>) {
        let stops = stops
            .into_iter()
            .map(|spans| {
                spans
                    .into_iter()
                    .map(|span| base + span.start..base + span.end)
                    .collect()
            })
            .collect::<Vec<Vec<Range<usize>>>>();
        self.places = Some(Places { stops, at: 0 });
        self.select_place();
    }

    /// Whether a snippet is being filled in.
    pub fn in_snippet(&self) -> bool {
        self.places.is_some()
    }

    /// Moves to the snippet's next place, answering whether there was one.
    ///
    /// Reaching the last place, where the cursor ends up, ends the snippet.
    pub fn next_place(&mut self) -> bool {
        let Some(places) = self.places.as_mut() else {
            return false;
        };
        places.at += 1;
        self.select_place();
        true
    }

    /// Moves back to the snippet's previous place, answering whether there was one.
    pub fn previous_place(&mut self) -> bool {
        let Some(places) = self.places.as_mut() else {
            return false;
        };
        if places.at == 0 {
            return false;
        }
        places.at -= 1;
        self.select_place();
        true
    }

    /// Stops filling in the snippet, leaving the text and cursor as they are.
    pub fn end_snippet(&mut self) {
        self.places = None;
    }

    /// Selects every span of the place the snippet is on, ending the
    /// snippet once that is the last place.
    fn select_place(&mut self) {
        let Some(places) = self.places.as_ref() else {
            return;
        };
        let Some(spans) = places.stops.get(places.at).cloned() else {
            self.places = None;
            return;
        };
        if places.at + 1 >= places.stops.len() {
            self.places = None;
        }
        let selections = spans
            .into_iter()
            .map(|span| Selection {
                anchor: self.position_of(span.start),
                head: self.position_of(span.end),
            })
            .collect::<Vec<_>>();
        if !selections.is_empty() {
            self.set_selections(selections);
        }
    }

    /// Moves the snippet's places with a change to the text, ending the
    /// snippet when the change cut across one of them.
    pub(super) fn shift_places(&mut self, start: usize, removed: usize, added: usize) {
        if let Some(places) = self.places.as_mut()
            && !places.shift(start, removed, added)
        {
            self.places = None;
        }
    }
}
