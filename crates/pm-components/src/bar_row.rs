//! The bar row: a name with a figure beside it and a bar under them, as long as the row's share of the longest.

use pm_style::{
    Color, Radius, Role, bg, flex, flex_1, flex_col, flex_none, gap, h_px, items_center,
    justify_between, min_w_px, overflow_hidden, rounded, w_px,
};
use pm_ui::{Node, node};

use crate::parts::line;

/// How tall the bar is, in logical pixels.
const BAR_HEIGHT: f32 = 4.0;

/// How wide a bar that stands for something is at least, in logical pixels, so that a small share still shows.
const LEAST_BAR: f32 = 2.0;

/// A bar row being described: what it names, the figure beside the name, the line under it, and how long and in which colour its bar is.
///
/// Build it with [`BarRow::new`], change it with the methods that follow, and
/// turn it into a node with [`BarRow::build`].
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
    /// How wide the row is, in logical pixels.
    width: f32,
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

    /// How wide the bar is drawn, in logical pixels: its share of the row, and at least a sliver when it stands for anything.
    pub fn filled(&self) -> f32 {
        if self.share <= 0.0 {
            0.0
        } else {
            (self.share * self.width).clamp(LEAST_BAR.min(self.width), self.width)
        }
    }

    /// The node that draws the row: the name and the figure on one line, the detail under them, and the bar on its track.
    pub fn build(self) -> Node {
        let filled = self.filled();
        let track = node()
            .styles([
                flex(),
                flex_none(),
                w_px(self.width),
                h_px(BAR_HEIGHT),
                rounded(Radius::Sm),
                bg(Role::SurfaceHover),
                overflow_hidden(),
            ])
            .children((filled > 0.0).then(|| {
                node().styles([
                    flex_none(),
                    w_px(filled),
                    h_px(BAR_HEIGHT),
                    rounded(Radius::Sm),
                    bg(self.tone),
                ])
            }));
        node()
            .styles([flex(), flex_col(), flex_none(), gap(1), w_px(self.width)])
            .child(
                node()
                    .styles([flex(), items_center(), justify_between(), gap(3)])
                    .child(
                        node()
                            .styles([flex_1(), min_w_px(0), overflow_hidden()])
                            .child(line(self.label, 13, 1.4, Role::Text)),
                    )
                    .child(line(self.value, 13, 1.4, Role::Text).styles([flex_none()])),
            )
            .children(self.detail.map(|detail| line(detail, 12, 1.4, Role::Muted)))
            .child(track)
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
