//! The bar row: a name with a figure beside it and a bar under them, as long as the row's share of the longest.

use crate::theme;
use rustle::{Alignment, Arrangement};
use rustle::{Color, Modifier};
use std::cell::Cell;
use std::rc::Rc;

use crate::theme::Role;

use crate::parts::line;

/// How tall the bar is, in logical pixels.
const BAR_HEIGHT: f32 = 4.0;

/// How wide a bar that stands for something is at least, in logical pixels, so that a small share still shows.
const LEAST_BAR: f32 = 2.0;

/// A bar row being described: what it names, the figure beside the name, the line under it, and how long and in which colour its bar is.
///
/// Build it with [`BarRow::new`], change it with the methods that follow, and
/// emit it with [`BarRow::build`].
#[derive(Clone, Debug)]
pub struct BarRow {
    /// What the row names.
    label: String,
    /// The figure at the end of the row.
    value: String,
    /// What is said under the name, when something is.
    detail: Option<String>,
    /// How much of the track the bar fills, from nought to one.
    share: f32,
    /// The colour of the bar.
    tone: Color,
}

impl BarRow {
    /// A row naming `label` with `value` beside it, as wide as its parent lets it be, its bar empty and in the accent colour.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            detail: None,
            share: 0.0,
            tone: Role::Accent.into(),
        }
    }

    /// The row saying `detail` in a quiet line under its name.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The row with its bar filling `share` of the track, from nought to one.
    pub fn share(mut self, share: f32) -> Self {
        self.share = if share.is_finite() {
            share.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    /// The row with its bar in `tone`.
    pub fn tone(mut self, tone: impl Into<Color>) -> Self {
        self.tone = tone.into();
        self
    }

    /// How wide the bar is drawn on a track `width` logical pixels long: its share of the track, and at least a sliver when it stands for anything.
    pub fn filled(&self, width: f32) -> f32 {
        filled(self.share, width)
    }

    /// Emits the row across its parent: the name and the figure on one line, the detail under them, and the bar on its track as rustle's linear progress indicator, whose fill is worked out from the track's laid-out width when it is drawn.
    #[track_caller]
    pub fn build(self) {
        let Self {
            label,
            value,
            detail,
            share,
            tone,
        } = self;
        let track = rustle::remember(|| Rc::new(Cell::new(0.0_f32))).get();
        rustle::column_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::SpacedBy(theme::sp(1)),
            Alignment::Start,
            move || {
                rustle::row_arranged(
                    Modifier::new().fill_max_width(),
                    Arrangement::SpacedBy(theme::sp(3)),
                    Alignment::Center,
                    || {
                        rustle::column_arranged(
                            Modifier::new().weight(1.0).clip(),
                            Arrangement::Start,
                            Alignment::Start,
                            || {
                                line(label, 13, 1.4, Role::Text);
                            },
                        );
                        line(value, 13, 1.4, Role::Text);
                    },
                );
                if let Some(detail) = detail {
                    line(detail, 12, 1.4, Role::Muted);
                }
                let laid = Rc::clone(&track);
                rustle::linear_progress_indicator_with(
                    move || {
                        let width = track.get();
                        filled(share, width) / width.max(f32::EPSILON)
                    },
                    Modifier::new()
                        .fill_max_width()
                        .height(BAR_HEIGHT)
                        .on_size_changed(move |size| laid.set(size.width)),
                    theme::color(tone),
                    theme::color(Role::SurfaceHover),
                );
            },
        );
    }
}

/// How wide a bar filling `share` of a track `width` logical pixels long is drawn: its share of the track, and at least a sliver when it stands for anything.
fn filled(share: f32, width: f32) -> f32 {
    if share <= 0.0 {
        0.0
    } else {
        (share * width).clamp(LEAST_BAR.min(width), width)
    }
}

#[cfg(test)]
/// Tests for how long a row's bar is drawn.
mod tests {
    use super::*;

    /// The bar is as long as its share of the row, empty for nothing, a sliver for very little, and never longer than the row.
    #[test]
    fn the_bar_is_its_share_of_the_row() {
        let row = BarRow::new("a", "$1.00");
        assert_eq!(row.filled(200.0), 0.0);
        assert_eq!(row.clone().share(0.5).filled(200.0), 100.0);
        assert_eq!(row.clone().share(0.001).filled(200.0), LEAST_BAR);
        assert_eq!(row.clone().share(7.0).filled(200.0), 200.0);
        assert_eq!(row.clone().share(f32::NAN).filled(200.0), 0.0);
        assert_eq!(row.share(-1.0).filled(200.0), 0.0);
    }
}
