//! The text element: one shaped, unwrapped run in one colour.

use std::ops::Range;
use std::sync::Arc;

use pm_gfx::{FontStyle, Point, Quad, Rect, Rgba, ShapedRun, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::placed::{Placed, Placements};
use crate::style::{Length, Style, Styled};
use crate::theme::{Font, TextSize};

/// A run of text, sized from the type scale and coloured from the theme.
pub struct Text {
    /// The characters to draw.
    content: String,
    /// The step of the scale to draw at, and the variations on it.
    font: Font,
    /// The colour to draw in, or the theme's body colour when unset.
    color: Option<Rgba>,
    /// How the run is sized and padded.
    style: Style,
    /// The characters washed in the selection colour behind the glyphs.
    selected: Option<Range<usize>>,
    /// Where the run writes down its carets as it paints, and under what key.
    placed: Option<(Placements, usize)>,
    /// The run measurement shaped, and the style it was shaped in, for
    /// painting to draw without shaping it again.
    shaped: Option<(FontStyle, Arc<ShapedRun>)>,
}

/// A run of `content` at the base size in the theme's body colour.
pub fn text(content: impl Into<String>) -> Text {
    Text {
        content: content.into(),
        font: Font::default(),
        color: None,
        style: Style::default(),
        selected: None,
        placed: None,
        shaped: None,
    }
}

impl Text {
    /// Returns this run drawn in `color`.
    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Returns this run in the monospaced family.
    pub fn font_mono(mut self) -> Self {
        self.font = self.font.mono();
        self
    }

    /// Returns this run at the smallest step, for badges and captions.
    pub fn text_xs(mut self) -> Self {
        self.font = self.font.size(TextSize::Xs);
        self
    }

    /// Returns this run at the secondary-label step.
    pub fn text_sm(mut self) -> Self {
        self.font = self.font.size(TextSize::Sm);
        self
    }

    /// Returns this run at the body step.
    pub fn text_base(mut self) -> Self {
        self.font = self.font.size(TextSize::Base);
        self
    }

    /// Returns this run at the section-title step.
    pub fn text_lg(mut self) -> Self {
        self.font = self.font.size(TextSize::Lg);
        self
    }

    /// Returns this run at the page-heading step.
    pub fn text_xl(mut self) -> Self {
        self.font = self.font.size(TextSize::Xl);
        self
    }

    /// Returns this run at the step the heading of a screen is drawn at.
    pub fn text_xxl(mut self) -> Self {
        self.font = self.font.size(TextSize::Xxl);
        self
    }

    /// Returns this run at weight 300.
    pub fn font_light(mut self) -> Self {
        self.font = self.font.weight(300);
        self
    }

    /// Returns this run at weight 500.
    pub fn font_medium(mut self) -> Self {
        self.font = self.font.weight(500);
        self
    }

    /// Returns this run at weight 600.
    pub fn font_semibold(mut self) -> Self {
        self.font = self.font.weight(600);
        self
    }

    /// Returns this run at weight 700.
    pub fn font_bold(mut self) -> Self {
        self.font = self.font.weight(700);
        self
    }

    /// Returns this run slanted.
    pub fn italic(mut self) -> Self {
        self.font = self.font.italic();
        self
    }

    /// Returns this run with the characters in `characters` shown selected.
    pub fn selected(mut self, characters: Range<usize>) -> Self {
        self.selected = Some(characters);
        self
    }

    /// Returns this run writing down where its characters land in
    /// `placements`, under `key`, as it paints.
    pub fn placed(mut self, placements: Placements, key: usize) -> Self {
        self.placed = Some((placements, key));
        self
    }

    /// Returns this run with `pixels` between baselines.
    pub fn leading(mut self, pixels: f32) -> Self {
        self.font = self.font.leading(pixels);
        self
    }
}

impl Styled for Text {
    /// How the run is sized and padded.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M> Element<M> for Text {
    /// How the run is sized and padded.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Shapes the run and reports the line box it occupies.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let font = self.font.resolve(&cx.theme.text);
        let shaped = cx.shape(&self.content, font);
        let run = shaped.size();
        self.shaped = Some((font, shaped));
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => available.width,
            Length::Auto => run.width + self.style.padding.horizontal(),
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            Length::Full => available.height,
            Length::Auto => run.height + self.style.padding.vertical(),
        };

        Size::new(width, height)
    }

    /// Paints the background if there is one, the selection, then the glyphs.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        cx.quad(
            Quad::filled(bounds, self.style.background)
                .corner_radii(self.style.corner_radii)
                .border(self.style.border_width, self.style.border_color),
        );

        let color = self.color.unwrap_or(cx.theme().colors.text);
        let font = self.font.resolve(&cx.theme().text);
        let run = match self.shaped.take() {
            Some((shaped_in, run)) if shaped_in == font => run,
            _ => cx.shape(&self.content, font),
        };
        let origin = Point::new(
            bounds.left() + self.style.padding.left,
            bounds.top() + self.style.padding.top,
        );
        if self.selected.is_some() || self.placed.is_some() {
            let carets = run
                .carets(&self.content)
                .into_iter()
                .map(|caret| origin.x + caret)
                .collect::<Vec<_>>();
            self.paint_selection(&carets, origin.y, run.height, cx);
            if let Some((placements, key)) = &self.placed {
                placements.borrow_mut().push(Placed {
                    key: *key,
                    bounds: Rect::from_xywh(origin.x, origin.y, run.width, run.height),
                    carets,
                });
            }
        }
        cx.text(origin, run, color);
    }
}

impl Text {
    /// Washes the selected characters, whose carets are `carets`, over a
    /// line box `height` tall from `top`.
    fn paint_selection<M>(
        &self,
        carets: &[f32],
        top: f32,
        height: f32,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let Some(selected) = &self.selected else {
            return;
        };
        let last = carets.len().saturating_sub(1);
        let (start, end) = (
            carets[selected.start.min(last)],
            carets[selected.end.min(last)],
        );
        if end <= start {
            return;
        }
        let theme = cx.theme();
        let color = theme.colors.selection.alpha(theme.emphasis.selection);
        cx.quad(Quad::filled(
            Rect::from_xywh(start, top, end - start, height),
            color,
        ));
    }
}
