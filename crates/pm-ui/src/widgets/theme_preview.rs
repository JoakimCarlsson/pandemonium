//! The theme preview: a miniature editor painted in the theme it offers.

use pm_gfx::{Point, Quad, Rect, Rgba, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Length, Style, Styled};
use crate::theme::{Appearance, TextScale, Theme, ThemeFamily};

/// Height of the tile, before the label under it.
const TILE_HEIGHT: f32 = 104.0;

/// Space between the tile and its label.
const LABEL_GAP: f32 = 6.0;

/// Padding between a pane's edge and the mock content inside it.
const PANE_PADDING: f32 = 5.0;

/// Share of a pane's width the mock file tree takes.
const TREE_SHARE: f32 = 0.24;

/// Height of one mock line.
const LINE_HEIGHT: f32 = 2.5;

/// Space between mock lines.
const LINE_GAP: f32 = 3.5;

/// How wide each mock line of the file tree is, as a share of the tree.
const TREE_LINES: [f32; 9] = [0.9, 0.62, 0.74, 0.55, 0.82, 0.48, 0.7, 0.58, 0.66];

/// How each mock line of code is indented and how wide it is, in shares.
const CODE_LINES: [(f32, f32); 12] = [
    (0.0, 0.52),
    (0.0, 0.7),
    (0.12, 0.46),
    (0.12, 0.62),
    (0.24, 0.38),
    (0.12, 0.55),
    (0.0, 0.3),
    (0.0, 0.64),
    (0.12, 0.5),
    (0.24, 0.42),
    (0.12, 0.58),
    (0.0, 0.34),
];

/// A tile that offers a theme family by being painted in it.
///
/// With one appearance asked for, the tile is that variant. With none — the
/// theme mode that follows the desktop — the tile is split down the middle,
/// light on the left and dark on the right, so one tile shows both.
pub struct ThemePreview<M> {
    /// The family the tile offers.
    family: ThemeFamily,
    /// The appearance to show, or none to show both.
    appearance: Option<Appearance>,
    /// Whether this is the chosen family.
    selected: bool,
    /// What choosing this family sends.
    message: M,
    /// How the tile is sized; tiles usually share a row.
    style: Style,
}

/// A tile offering `family` that sends `message` when it is chosen.
pub fn theme_preview<M>(
    family: ThemeFamily,
    appearance: Option<Appearance>,
    selected: bool,
    message: M,
) -> ThemePreview<M> {
    ThemePreview {
        family,
        appearance,
        selected,
        message,
        style: Style::default(),
    }
}

impl<M> ThemePreview<M> {
    /// Paints one pane of the mock editor: a file tree and lines of code.
    fn paint_pane(
        &self,
        bounds: Rect,
        theme: &Theme,
        radii: [f32; 4],
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        cx.quad(Quad::filled(bounds, theme.colors.background).corner_radii(radii));

        let content = bounds.inset(PANE_PADDING);
        let tree_width = (content.size.width * TREE_SHARE).round();
        cx.quad(Quad::filled(
            Rect::from_xywh(
                content.left(),
                content.top(),
                tree_width,
                content.size.height,
            ),
            theme.colors.surface,
        ));

        let tree_content = tree_width - PANE_PADDING * 2.0;
        self.paint_lines(
            Point::new(content.left() + PANE_PADDING, content.top() + PANE_PADDING),
            tree_content,
            TREE_LINES
                .iter()
                .map(|share| (0.0, *share, theme.colors.text_subtle)),
            cx,
        );

        let code_left = content.left() + tree_width + PANE_PADDING;
        let code_width = (content.right() - code_left).max(0.0);
        let palette = self.code_palette(theme);
        self.paint_lines(
            Point::new(code_left, content.top() + PANE_PADDING),
            code_width,
            CODE_LINES
                .iter()
                .enumerate()
                .map(|(index, (indent, share))| (*indent, *share, palette[index % palette.len()])),
            cx,
        );
    }

    /// The colours the mock lines of code cycle through.
    fn code_palette(&self, theme: &Theme) -> [Rgba; 6] {
        [
            theme.syntax.keyword,
            theme.colors.text,
            theme.syntax.function,
            theme.syntax.string,
            theme.syntax.comment,
            theme.syntax.number,
        ]
    }

    /// Paints a column of mock lines, each indented and sized by its shares.
    fn paint_lines<I>(&self, origin: Point, width: f32, lines: I, cx: &mut PaintContext<'_, '_, M>)
    where
        I: IntoIterator<Item = (f32, f32, Rgba)>,
    {
        let mut top = origin.y;
        for (indent, share, color) in lines {
            cx.quad(
                Quad::filled(
                    Rect::from_xywh(
                        origin.x + width * indent,
                        top,
                        (width * share).max(1.0),
                        LINE_HEIGHT,
                    ),
                    color,
                )
                .corner_radius(LINE_HEIGHT / 2.0),
            );
            top += LINE_HEIGHT + LINE_GAP;
        }
    }
}

impl<M> Styled for ThemePreview<M> {
    /// How the tile is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: Clone> Element<M> for ThemePreview<M> {
    /// How the tile is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Sizes the tile, plus the label under it.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let label = cx.measure(self.family.name, TextScale::DEFAULT.xs);
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => available.width,
            Length::Auto => label.width.max(available.width),
        };

        Size::new(width, TILE_HEIGHT + LABEL_GAP + label.height)
    }

    /// Registers the press target, then paints the panes, the frame and the label.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let interaction = cx.interactive(bounds, self.message.clone());
        let ambient = *cx.theme();
        let radius = ambient.radius.lg;
        let tile = Rect::from_xywh(bounds.left(), bounds.top(), bounds.size.width, TILE_HEIGHT);

        match self.appearance {
            Some(appearance) => {
                let theme = self.family.variant(appearance);
                self.paint_pane(tile, &theme, [radius; 4], cx);
            }
            None => {
                let half = (tile.size.width / 2.0).round();
                let left = Rect::from_xywh(tile.left(), tile.top(), half, tile.size.height);
                let right = Rect::from_xywh(
                    tile.left() + half,
                    tile.top(),
                    tile.size.width - half,
                    tile.size.height,
                );
                self.paint_pane(left, &self.family.light, [radius, 0.0, 0.0, radius], cx);
                self.paint_pane(right, &self.family.dark, [0.0, radius, radius, 0.0], cx);
            }
        }

        let (border_width, border_color) = if interaction.focused {
            (2.0, ambient.colors.border_focused)
        } else if self.selected {
            (2.0, ambient.colors.border_selected)
        } else if interaction.hovered {
            (1.0, ambient.colors.text_subtle)
        } else {
            (1.0, ambient.colors.border)
        };
        cx.quad(
            Quad::filled(tile, Rgba::TRANSPARENT)
                .corner_radius(radius)
                .border(border_width, border_color),
        );

        let run = cx.shape(self.family.name, TextScale::DEFAULT.xs);
        let color = if self.selected {
            ambient.colors.text
        } else {
            ambient.colors.text_muted
        };
        let origin = Point::new(
            (bounds.left() + (bounds.size.width - run.width) / 2.0).round(),
            (tile.bottom() + LABEL_GAP).round(),
        );
        cx.text(origin, run, color);
    }
}
