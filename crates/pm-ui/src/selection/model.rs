//! Selection in content coordinates, independent of wrapping and scrolling.

use std::ops::Range;
use std::time::{Duration, Instant};

use pm_gfx::{Point, Rect};

use crate::{Placed, nearest};

/// A character boundary in a logical content row.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Spot {
    /// The logical row counted from the beginning of the content.
    pub row: usize,
    /// The character boundary within the row.
    pub column: usize,
}

/// The amount of content selected by each step of a pointer gesture.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Grain {
    /// Individual character boundaries.
    #[default]
    Character,
    /// Words, whitespace stretches or punctuation stretches.
    Word,
    /// Whole logical paragraphs.
    Paragraph,
}

/// One content row and how it joins the preceding row when copied.
#[derive(Clone)]
pub struct SelectionRow {
    /// Text drawn by placed runs, in reading order.
    pub text: String,
    /// Characters used only to indent a wrapped continuation.
    pub lead: usize,
    /// The boundary before this row: newline, space, tab or no separator.
    pub separator: &'static str,
}

/// Logical rows assembled from a surface's visual rows.
#[derive(Default)]
pub struct SelectionContent {
    /// Text and copy boundaries after visual continuations have been rejoined.
    rows: Vec<SelectionRow>,
}

impl SelectionContent {
    /// Discards the previous content before recording a new layout.
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    /// Appends a visual row and returns the logical start of its placed characters.
    pub fn push(&mut self, row: SelectionRow) -> Spot {
        if row.separator == " "
            && let Some(previous) = self.rows.last_mut()
        {
            previous.text.push(' ');
            let column = previous.text.chars().count().saturating_sub(row.lead);
            previous.text.extend(row.text.chars().skip(row.lead));
            return Spot {
                row: self.rows.len() - 1,
                column,
            };
        }
        let start = Spot {
            row: self.rows.len(),
            column: 0,
        };
        self.rows.push(row);
        start
    }

    /// Updates a live row without changing its logical identity.
    pub fn replace(&mut self, at: usize, row: SelectionRow) {
        self.rows[at] = row;
    }

    /// Returns the number of logical content rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no logical rows have been recorded.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Returns a logical row for selection extension and read-back.
    pub fn row(&self, at: usize) -> SelectionRow {
        self.rows[at].clone()
    }
}

/// The anchor and head of a selection in content coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection {
    /// The two boundaries, preserving the direction of the gesture.
    ends: Option<(Spot, Spot)>,
}

impl Selection {
    /// Returns the nonempty range with its first boundary first.
    pub fn range(&self) -> Option<(Spot, Spot)> {
        let (anchor, head) = self.ends?;
        (anchor != head).then_some((anchor.min(head), anchor.max(head)))
    }

    /// Selects the boundaries from `anchor` through `head`.
    pub fn select(&mut self, anchor: Spot, head: Spot) {
        self.ends = Some((anchor, head));
    }

    /// Clears the selected range.
    pub fn clear(&mut self) {
        self.ends = None;
    }

    /// Returns the selected characters of a run starting at `start`.
    pub fn picked(&self, start: Spot, length: usize) -> Option<Range<usize>> {
        let (first, last) = self.range()?;
        if start.row < first.row || start.row > last.row {
            return None;
        }
        let from = if start.row == first.row {
            first.column.saturating_sub(start.column)
        } else {
            0
        };
        let to = if start.row == last.row {
            last.column.saturating_sub(start.column).min(length)
        } else {
            length
        };
        (from < to).then_some(from..to)
    }

    /// Reads the selection, rejoining wrapped rows using their boundaries.
    pub fn text(&self, count: usize, row: impl Fn(usize) -> SelectionRow) -> Option<String> {
        let (first, last) = self.range()?;
        let mut copied = String::new();
        for at in first.row..last.row.saturating_add(1).min(count) {
            let line = row(at);
            let from = if at == first.row {
                first.column.max(line.lead)
            } else {
                line.lead
            };
            let to = if at == last.row {
                last.column
            } else {
                usize::MAX
            };
            if at > first.row {
                copied.push_str(line.separator);
            }
            copied.extend(line.text.chars().take(to).skip(from));
        }
        Some(copied)
    }

    /// Expands two boundaries to the words or paragraphs they touch.
    pub fn extend(
        anchor: Spot,
        head: Spot,
        grain: Grain,
        count: usize,
        row: impl Fn(usize) -> SelectionRow,
    ) -> (Spot, Spot) {
        let (first, last) = (anchor.min(head), anchor.max(head));
        if count == 0 || grain == Grain::Character {
            return (anchor, head);
        }
        if grain == Grain::Word {
            let word = |spot: Spot| {
                let chars = row(spot.row.min(count - 1))
                    .text
                    .chars()
                    .collect::<Vec<_>>();
                let at = spot.column.min(chars.len().saturating_sub(1));
                let Some(kind) = chars.get(at).copied().map(kind_of) else {
                    return (spot, spot);
                };
                let from = chars[..at]
                    .iter()
                    .rposition(|ch| kind_of(*ch) != kind)
                    .map_or(0, |before| before + 1);
                let to = chars[at..]
                    .iter()
                    .position(|ch| kind_of(*ch) != kind)
                    .map_or(chars.len(), |after| at + after);
                (
                    Spot {
                        column: from,
                        ..spot
                    },
                    Spot { column: to, ..spot },
                )
            };
            return (word(first).0, word(last).1);
        }
        let mut start = first.row.min(count - 1);
        while start > 0 && row(start).separator == " " {
            start -= 1;
        }
        let mut end = last.row.min(count - 1);
        while end + 1 < count && row(end + 1).separator == " " {
            end += 1;
        }
        (
            Spot {
                row: start,
                column: 0,
            },
            Spot {
                row: end,
                column: row(end).text.chars().count(),
            },
        )
    }

    /// Returns the boundaries covering all content rows.
    pub fn everything(count: usize, row: impl Fn(usize) -> SelectionRow) -> Option<(Spot, Spot)> {
        let last = count.checked_sub(1)?;
        Some((
            Spot::default(),
            Spot {
                row: last,
                column: row(last).text.chars().count(),
            },
        ))
    }
}

/// Maps a window point to a content position through the last painted runs.
pub fn spot_at(placements: &[Placed], starts: &[Spot], point: Point) -> Option<Spot> {
    let placed = nearest(placements, point)?;
    let start = *starts.get(placed.key)?;
    Some(Spot {
        column: start.column + placed.caret_at(point.x),
        ..start
    })
}

/// A captured text gesture with a content anchor and an edge scroll clock.
#[derive(Clone, Copy)]
pub struct SelectionDrag {
    /// The content boundary at the original press.
    pub anchor: Spot,
    /// The latest window position of the held pointer.
    pub pointer: Point,
    /// The next edge scroll deadline.
    pub next_scroll: Instant,
}

impl SelectionDrag {
    /// Begins a gesture at a content boundary and pointer position.
    pub fn new(anchor: Spot, pointer: Point) -> Self {
        Self {
            anchor,
            pointer,
            next_scroll: Instant::now(),
        }
    }

    /// Keeps the head within the visible area for placement hit testing.
    pub fn head_point(&self, view: Rect) -> Point {
        Point::new(
            self.pointer.x,
            self.pointer
                .y
                .clamp(view.top(), view.bottom().max(view.top())),
        )
    }

    /// Returns the signed scroll step when the pointer is held past an edge.
    pub fn scroll_step(&self, view: Rect, offset: f32, end: f32) -> Option<f32> {
        let distance = if self.pointer.y < view.top() && offset > 0.0 {
            self.pointer.y - view.top()
        } else if self.pointer.y > view.bottom() && offset < end {
            self.pointer.y - view.bottom()
        } else {
            return None;
        };
        Some(distance.signum() * (distance.abs() * 0.25).clamp(4.0, 40.0))
    }

    /// Advances the sixteen millisecond clock when a scroll step is due.
    pub fn scroll_due(&mut self, now: Instant) -> bool {
        if now < self.next_scroll {
            return false;
        }
        self.next_scroll = now + Duration::from_millis(16);
        true
    }
}

/// Classifies letters, spaces and punctuation for word selection.
fn kind_of(character: char) -> u8 {
    match character {
        ch if ch.is_alphanumeric() || ch == '_' => 0,
        ch if ch.is_whitespace() => 1,
        _ => 2,
    }
}
