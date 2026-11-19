//! The bar row: a name with a figure beside it and a bar under them, as long as the row's share of the longest.

use crate::theme;
use crate::theme::sp;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use crate::theme::{Radius, Role, Tone};

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
    tone: Tone,
    /// How wide the row is, in logical pixels.
    width: f32,
    /// What the caller puts around its box, outside its own.
    modifier: Modifier,
}

impl BarRow {
    /// A row naming `label` with `value` beside it, `width` logical pixels across, its bar empty and in the accent colour.
    pub fn new(label: impl Into<String>, value: impl Into<String>, width: f32) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            detail: None,
            share: 0.0,
            tone: Role::Accent.into(),
            width,
            modifier: Modifier::new(),
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
    pub fn tone(mut self, tone: impl Into<Tone>) -> Self {
        self.tone = tone.into();
        self
    }

    /// How wide the bar is drawn, in logical pixels: its share of the row, and at least a sliver when it stands for anything.
    pub fn filled(&self) -> f32 {
        if self.share <= 0.0 {
            0.0
        } else {
            (self.share * self.width).clamp(LEAST_BAR.min(self.width), self.width)
        }
    }

    /// The row with `modifier` put around its box, outside its own: margins, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the row: the name and the figure on one line, the detail under them, and the bar on its track.
    #[track_caller]
    pub fn build(self) {
        let filled = self.filled();
        let Self {
            label,
            value,
            detail,
            tone,
            width,
            modifier,
            ..
        } = self;
        column_arranged(
            modifier.chain(Modifier::new().width(width)),
            Arrangement::SpacedBy(sp(1)),
            Alignment::Start,
            move || {
                row_arranged(
                    Modifier::new().fill_max_width(),
                    Arrangement::SpacedBy(sp(3)),
                    Alignment::Center,
                    || {
                        column_arranged(
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
                row_arranged(
                    Modifier::new()
                        .width(width)
                        .height(BAR_HEIGHT)
                        .background_rounded(theme::color(Role::SurfaceHover), Radius::Sm.px())
                        .clip(),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        if filled > 0.0 {
                            rustle::spacer(
                                Modifier::new()
                                    .width(filled)
                                    .height(BAR_HEIGHT)
                                    .background_rounded(theme::color(tone), Radius::Sm.px()),
                            );
                        }
                    },
                );
            },
        );
    }
}

#[cfg(test)]
/// Tests for how long a row's bar is drawn.
mod tests {
    use super::*;

    /// The bar is as long as its share of the row, empty for nothing, a sliver for very little, and never longer than the row.
    #[test]
    fn the_bar_is_its_share_of_the_row() {
        let row = BarRow::new("a", "$1.00", 200.0);
        assert_eq!(row.filled(), 0.0);
        assert_eq!(row.clone().share(0.5).filled(), 100.0);
        assert_eq!(row.clone().share(0.001).filled(), LEAST_BAR);
        assert_eq!(row.clone().share(7.0).filled(), 200.0);
        assert_eq!(row.clone().share(f32::NAN).filled(), 0.0);
        assert_eq!(row.share(-1.0).filled(), 0.0);
    }
}
