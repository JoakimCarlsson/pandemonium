//! The text element: one shaped, unwrapped run in one colour.

use pm_gfx::{Point, Quad, Rect, Rgba, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Length, Style, Styled};
use crate::theme::TextScale;

/// A run of text, sized from the type scale and coloured from the theme.
pub struct Text {
    /// The characters to draw.
    content: String,
    /// Size, leading, weight and slant.
    font: pm_gfx::FontStyle,
    /// The colour to draw in, or the theme's body colour when unset.
    color: Option<Rgba>,
    /// How the run is sized and padded.
    style: Style,
}

/// A run of `content` at the base size in the theme's body colour.
pub fn text(content: impl Into<String>) -> Text {
    Text {
        content: content.into(),
        font: TextScale::DEFAULT.base,
        color: None,
        style: Style::default(),
    }
}

impl Text {
    /// Returns this run drawn in `color`.
    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Returns this run at 11px, for badges and the smallest captions.
    pub fn text_xs(mut self) -> Self {
        self.font = TextScale::DEFAULT.xs;
        self
    }

    /// Returns this run at 12px, for secondary labels.
    pub fn text_sm(mut self) -> Self {
        self.font = TextScale::DEFAULT.sm;
        self
    }

    /// Returns this run at 14px, the body size.
    pub fn text_base(mut self) -> Self {
        self.font = TextScale::DEFAULT.base;
        self
    }

    /// Returns this run at 16px, for section titles.
    pub fn text_lg(mut self) -> Self {
        self.font = TextScale::DEFAULT.lg;
        self
    }

    /// Returns this run at 20px, for page headings.
    pub fn text_xl(mut self) -> Self {
        self.font = TextScale::DEFAULT.xl;
        self
    }

    /// Returns this run at 26px, for the heading at the top of a screen.
    pub fn text_xxl(mut self) -> Self {
        self.font = TextScale::DEFAULT.xxl;
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

    /// Returns this run with `pixels` between baselines.
    pub fn leading(mut self, pixels: f32) -> Self {
        self.font = self.font.line_height(pixels);
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
        let run = cx.measure(&self.content, self.font);
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

    /// Paints the background if there is one, then the glyphs.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        cx.quad(
            Quad::filled(bounds, self.style.background)
                .corner_radius(self.style.corner_radius)
                .border(self.style.border_width, self.style.border_color),
        );

        let color = self.color.unwrap_or(cx.theme().colors.text);
        let run = cx.shape(&self.content, self.font);
        let origin = Point::new(
            bounds.left() + self.style.padding.left,
            bounds.top() + self.style.padding.top,
        );
        cx.text(origin, run, color);
    }
}
